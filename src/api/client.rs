use anyhow::{bail, Context, Result};
use reqwest::Client;
use tracing::{debug, info, warn};

use crate::api::auth::SoundCloudSession;
use crate::api::stream::resolve_stream_url;
use crate::api::upload::{upload_track, TrackUploadMetadata, UploadProgress};
use crate::api::waveform::fetch_waveform_samples;
use crate::models::{
    Comment, CreatePlaylistPayload, Playlist, SearchFilter, SearchResultItem, SearchResults, Track,
    UserProfile,
};

const API_V2_BASE: &str = "https://api-v2.soundcloud.com";

#[derive(Clone)]
pub struct SoundCloudClient {
    http: Client,
    pub session: SoundCloudSession,
}

impl SoundCloudClient {
    pub fn new() -> Self {
        let http = Client::builder()
            .user_agent("Mozilla/5.0 (X11; Linux x86_64; rv:130.0) Gecko/20100101 Firefox/130.0")
            .tcp_nodelay(true)
            .build()
            .unwrap_or_default();

        Self {
            http,
            session: SoundCloudSession::new(),
        }
    }

    pub async fn ensure_valid_client_id(&self) -> Result<String> {
        let current = self.session.get_client_id().await;
        if current.is_empty() {
            self.session.discover_client_id(&self.http).await
        } else {
            Ok(current)
        }
    }

    /// Fetches trending / discovery tracks, personalized when authenticated
    pub async fn get_charts(&self, genre: &str, limit: usize, _offset: usize) -> Result<Vec<Track>> {
        if genre == "my_stream" {
            if self.session.is_authenticated().await {
                if let Ok(stream) = self.get_my_stream(limit).await {
                    if !stream.is_empty() {
                        return Ok(stream);
                    }
                }
            }
        }

        if genre == "mixed_for_you" {
            if let Ok(mixed) = self.get_mixed_selections().await {
                if !mixed.is_empty() {
                    return Ok(mixed);
                }
            }
        }

        let query = match genre {
            "my_stream" | "all" | "trending" | "" => "trending",
            other => other,
        };
        self.search_tracks(query, limit).await
    }

    /// Fetches algorithmic mixed selections & personalized discovery from SoundCloud
    pub async fn get_mixed_selections(&self) -> Result<Vec<Track>> {
        let client_id = self.ensure_valid_client_id().await?;
        let oauth_token = self.session.get_oauth_token().await;

        let url = format!("{}/mixed-selections?client_id={}", API_V2_BASE, client_id);
        let mut req = self
            .http
            .get(&url)
            .header("Accept-Language", "ar,en-US;q=0.8,en;q=0.5");

        if let Some(ref token) = oauth_token {
            req = req.header("Authorization", format!("OAuth {}", token));
        }

        let resp = req.send().await.context("Fetching mixed selections")?;
        if !resp.status().is_success() {
            bail!("Failed to fetch mixed selections: HTTP {}", resp.status());
        }

        let body: serde_json::Value = resp.json().await.context("Parsing mixed selections JSON")?;
        let mut track_ids = Vec::new();

        if let Some(sections) = body.get("collection").and_then(|c| c.as_array()) {
            for section in sections {
                if let Some(items) = section.get("items").and_then(|i| i.get("collection")).and_then(|c| c.as_array()) {
                    for item in items {
                        if item.get("kind").and_then(|k| k.as_str()) == Some("track") {
                            if let Some(id) = item.get("id").and_then(|id| id.as_u64()) {
                                if !track_ids.contains(&id) {
                                    track_ids.push(id);
                                }
                            }
                        } else if let Some(tracks) = item.get("tracks").and_then(|t| t.as_array()) {
                            for t in tracks.iter().take(6) {
                                if let Some(id) = t.get("id").and_then(|id| id.as_u64()) {
                                    if !track_ids.contains(&id) {
                                        track_ids.push(id);
                                    }
                                }
                            }
                        }
                    }
                }
                if track_ids.len() >= 35 {
                    break;
                }
            }
        }

        if track_ids.is_empty() {
            return Ok(Vec::new());
        }

        self.get_tracks_bulk(&track_ids).await
    }

    /// Search across SoundCloud with filters and personalized relevance
    pub async fn search(&self, query: &str, filter: SearchFilter, limit: usize) -> Result<SearchResults> {
        let mut client_id = self.ensure_valid_client_id().await?;
        let endpoint = match filter {
            SearchFilter::All => "search",
            SearchFilter::Tracks => "search/tracks",
            SearchFilter::Playlists => "search/playlists_without_albums",
            SearchFilter::Users => "search/users",
        };

        let encoded_query = urlencoding::encode(query);
        let url = format!(
            "{}/{}?q={}&client_id={}&limit={}",
            API_V2_BASE, endpoint, encoded_query, client_id, limit
        );

        debug!("Searching endpoint: {}", url);
        let oauth_token = self.session.get_oauth_token().await;
        let mut req_builder = self
            .http
            .get(&url)
            .header("Accept-Language", "ar,en-US;q=0.8,en;q=0.5");

        if let Some(ref token) = oauth_token {
            req_builder = req_builder.header("Authorization", format!("OAuth {}", token));
        }

        let mut resp = req_builder.send().await.context("Executing search")?;
        
        // Auto-heal if client ID expired or restricted
        if resp.status().as_u16() == 401 {
            info!("Client ID returned 401; auto-healing dynamically from soundcloud.com assets...");
            client_id = self.session.discover_client_id(&self.http).await?;
            let healed_url = format!(
                "{}/{}?q={}&client_id={}&limit={}",
                API_V2_BASE, endpoint, encoded_query, client_id, limit
            );
            let mut healed_builder = self
                .http
                .get(&healed_url)
                .header("Accept-Language", "ar,en-US;q=0.8,en;q=0.5");
            if let Some(ref token) = oauth_token {
                healed_builder = healed_builder.header("Authorization", format!("OAuth {}", token));
            }
            resp = healed_builder.send().await.context("Executing search with healed client ID")?;
        }

        if !resp.status().is_success() {
            bail!("Search failed with status: {}", resp.status());
        }

        match filter {
            SearchFilter::Tracks => {
                #[derive(serde::Deserialize)]
                struct TrackSearchResp {
                    collection: Vec<Track>,
                }
                let body: TrackSearchResp = resp.json().await.context("Parsing track search JSON")?;
                let collection = body.collection.into_iter().map(SearchResultItem::Track).collect();
                Ok(SearchResults {
                    collection,
                    next_href: None,
                    total_results: None,
                })
            }
            SearchFilter::Playlists => {
                #[derive(serde::Deserialize)]
                struct PlaylistSearchResp {
                    collection: Vec<Playlist>,
                }
                let body: PlaylistSearchResp = resp.json().await.context("Parsing playlist search JSON")?;
                let collection = body.collection.into_iter().map(SearchResultItem::Playlist).collect();
                Ok(SearchResults {
                    collection,
                    next_href: None,
                    total_results: None,
                })
            }
            SearchFilter::Users => {
                #[derive(serde::Deserialize)]
                struct UserSearchResp {
                    collection: Vec<UserProfile>,
                }
                let body: UserSearchResp = resp.json().await.context("Parsing user search JSON")?;
                let collection = body.collection.into_iter().map(SearchResultItem::User).collect();
                Ok(SearchResults {
                    collection,
                    next_href: None,
                    total_results: None,
                })
            }
            SearchFilter::All => {
                let results: SearchResults = resp.json().await.context("Parsing mixed search JSON")?;
                Ok(results)
            }
        }
    }

    pub async fn search_tracks(&self, query: &str, limit: usize) -> Result<Vec<Track>> {
        let res = self.search(query, SearchFilter::Tracks, limit).await?;
        let mut tracks = Vec::new();
        for item in res.collection {
            if let SearchResultItem::Track(t) = item {
                tracks.push(t);
            }
        }
        Ok(tracks)
    }

    /// Fetches a single track's full metadata
    pub async fn get_track(&self, track_id: u64) -> Result<Track> {
        let client_id = self.ensure_valid_client_id().await?;
        let oauth_token = self.session.get_oauth_token().await;
        let url = format!("{}/tracks/{}?client_id={}", API_V2_BASE, track_id, client_id);
        let mut req = self
            .http
            .get(&url)
            .header("Accept-Language", "ar,en-US;q=0.8,en;q=0.5");
        if let Some(token) = oauth_token {
            req = req.header("Authorization", format!("OAuth {}", token));
        }
        let resp = req.send().await.context("Fetching track metadata")?;
        if !resp.status().is_success() {
            bail!("Failed to get track {}: HTTP {}", track_id, resp.status());
        }
        let track = resp.json::<Track>().await.context("Parsing track JSON")?;
        Ok(track)
    }

    /// Fetches multiple tracks in bulk by IDs
    pub async fn get_tracks_bulk(&self, track_ids: &[u64]) -> Result<Vec<Track>> {
        if track_ids.is_empty() {
            return Ok(Vec::new());
        }
        let client_id = self.ensure_valid_client_id().await?;
        let oauth_token = self.session.get_oauth_token().await;

        let mut all_tracks = Vec::new();
        for chunk in track_ids.chunks(45) {
            let ids_param = chunk
                .iter()
                .map(|id| id.to_string())
                .collect::<Vec<_>>()
                .join(",");
            let url = format!(
                "{}/tracks?ids={}&client_id={}",
                API_V2_BASE, ids_param, client_id
            );
            let mut req = self
                .http
                .get(&url)
                .header("Accept-Language", "ar,en-US;q=0.8,en;q=0.5");
            if let Some(ref token) = oauth_token {
                req = req.header("Authorization", format!("OAuth {}", token));
            }
            if let Ok(resp) = req.send().await {
                if resp.status().is_success() {
                    if let Ok(tracks) = resp.json::<Vec<Track>>().await {
                        all_tracks.extend(tracks);
                    }
                }
            }
        }
        Ok(all_tracks)
    }

    /// Fetches complete playlist detail and hydrates all stub tracks
    pub async fn get_playlist_details(&self, playlist_id: u64) -> Result<Playlist> {
        let client_id = self.ensure_valid_client_id().await?;
        let oauth_token = self.session.get_oauth_token().await;
        let url = format!("{}/playlists/{}?client_id={}", API_V2_BASE, playlist_id, client_id);
        let mut req = self
            .http
            .get(&url)
            .header("Accept-Language", "ar,en-US;q=0.8,en;q=0.5");
        if let Some(ref token) = oauth_token {
            req = req.header("Authorization", format!("OAuth {}", token));
        }
        let resp = req.send().await.context("Fetching playlist detail")?;
        if !resp.status().is_success() {
            bail!("Failed to get playlist {}: HTTP {}", playlist_id, resp.status());
        }
        let mut playlist: Playlist = resp.json().await.context("Parsing playlist JSON")?;

        // Identify stub tracks that lack media transcodings
        let stub_ids: Vec<u64> = playlist
            .tracks
            .iter()
            .filter(|t| t.media.as_ref().map(|m| m.transcodings.is_empty()).unwrap_or(true))
            .map(|t| t.id)
            .collect();

        if !stub_ids.is_empty() {
            if let Ok(hydrated_tracks) = self.get_tracks_bulk(&stub_ids).await {
                use std::collections::HashMap;
                let map: HashMap<u64, Track> = hydrated_tracks.into_iter().map(|t| (t.id, t)).collect();
                for t in &mut playlist.tracks {
                    if let Some(hydrated) = map.get(&t.id) {
                        *t = hydrated.clone();
                    }
                }
            }
        }

        Ok(playlist)
    }

    /// Resolves playable stream URL for a track, auto-hydrating if track was a stub
    pub async fn get_stream_url(&self, track: &Track) -> Result<String> {
        let client_id = self.ensure_valid_client_id().await?;
        let oauth_token = self.session.get_oauth_token().await;

        let full_track = if track.media.as_ref().map(|m| m.transcodings.is_empty()).unwrap_or(true) {
            info!("Track {} is a stub or missing media transcodings, fetching full metadata...", track.id);
            self.get_track(track.id).await.unwrap_or_else(|_| track.clone())
        } else {
            track.clone()
        };

        resolve_stream_url(&self.http, &full_track, &client_id, oauth_token.as_deref()).await
    }

    /// Fetches waveform point samples
    pub async fn get_waveform_samples(&self, track: &Track) -> Vec<f32> {
        fetch_waveform_samples(&self.http, track.waveform_url.as_deref(), track.id).await
    }

    /// Fetches comments for a track
    pub async fn get_comments(&self, track_id: u64, limit: usize) -> Result<Vec<Comment>> {
        let client_id = self.ensure_valid_client_id().await?;
        let url = format!(
            "{}/tracks/{}/comments?client_id={}&limit={}",
            API_V2_BASE, track_id, client_id, limit
        );

        let resp = self.http.get(&url).send().await.context("Fetching track comments")?;
        if !resp.status().is_success() {
            bail!("Failed to get comments: HTTP {}", resp.status());
        }

        #[derive(serde::Deserialize)]
        struct CommentResp {
            collection: Vec<Comment>,
        }

        let body: CommentResp = resp.json().await.context("Parsing comments JSON")?;
        let mut comments = body.collection;
        comments.sort_by_key(|c| c.timestamp.unwrap_or(0));
        Ok(comments)
    }

    /// Submits a timestamped comment to a track
    pub async fn post_comment(&self, track_id: u64, body: &str, timestamp_ms: u64) -> Result<Comment> {
        let client_id = self.ensure_valid_client_id().await?;
        let oauth_token = self.session.get_oauth_token().await;

        let url = format!("{}/tracks/{}/comments?client_id={}", API_V2_BASE, track_id, client_id);
        
        #[derive(serde::Serialize)]
        struct PostCommentPayload<'a> {
            body: &'a str,
            timestamp: u64,
        }

        let mut req = self.http.post(&url).json(&PostCommentPayload {
            body,
            timestamp: timestamp_ms,
        });

        if let Some(token) = oauth_token {
            req = req.header("Authorization", format!("OAuth {}", token));
        }

        let resp = req.send().await.context("Posting comment")?;
        if !resp.status().is_success() {
            bail!("Post comment failed with HTTP status: {}", resp.status());
        }

        let comment = resp.json::<Comment>().await.context("Parsing posted comment")?;
        info!("Posted timestamped comment to track {}", track_id);
        Ok(comment)
    }

    /// Toggles like status for a track
    pub async fn toggle_like(&self, track_id: u64, is_liked: bool) -> Result<()> {
        let client_id = self.ensure_valid_client_id().await?;
        let oauth_token = self.session.get_oauth_token().await;

        let url = format!("{}/likes/tracks/{}?client_id={}", API_V2_BASE, track_id, client_id);
        let mut req = if is_liked {
            self.http.post(&url)
        } else {
            self.http.delete(&url)
        };

        if let Some(token) = oauth_token {
            req = req.header("Authorization", format!("OAuth {}", token));
        }

        let resp = req.send().await.context("Toggling like status")?;
        if !resp.status().is_success() {
            debug!("SoundCloud remote like endpoint returned {}, updated locally", resp.status());
        }
        Ok(())
    }

    /// Toggles like/save status for a playlist
    pub async fn toggle_like_playlist(&self, playlist_id: u64, is_liked: bool) -> Result<()> {
        let client_id = self.ensure_valid_client_id().await?;
        let oauth_token = self.session.get_oauth_token().await;

        let user_id = match *self.session.user_id.read().await {
            Some(id) => id,
            None => {
                let me = self.get_current_user().await?;
                me.id
            }
        };

        let url = format!(
            "{}/users/{}/playlist_likes/{}?client_id={}",
            API_V2_BASE, user_id, playlist_id, client_id
        );
        let mut req = if is_liked {
            self.http.put(&url)
        } else {
            self.http.delete(&url)
        };

        if let Some(token) = oauth_token {
            req = req.header("Authorization", format!("OAuth {}", token));
        }

        let resp = req.send().await.context("Toggling playlist like status")?;
        if !resp.status().is_success() {
            warn!("SoundCloud remote playlist like endpoint returned {}: {}", resp.status(), resp.text().await.unwrap_or_default());
        } else {
            info!("Successfully updated playlist {} like status to {}", playlist_id, is_liked);
        }
        Ok(())
    }

    /// Creates a new playlist
    pub async fn create_playlist(&self, payload: CreatePlaylistPayload) -> Result<Playlist> {
        let client_id = self.ensure_valid_client_id().await?;
        let oauth_token = self.session.get_oauth_token().await;

        let url = format!("{}/playlists?client_id={}", API_V2_BASE, client_id);
        let mut req = self.http.post(&url).json(&payload);

        if let Some(token) = oauth_token {
            req = req.header("Authorization", format!("OAuth {}", token));
        }

        let resp = req.send().await.context("Creating playlist")?;
        if !resp.status().is_success() {
            bail!("Failed to create playlist, HTTP status: {}", resp.status());
        }

        let playlist = resp.json::<Playlist>().await?;
        Ok(playlist)
    }

    /// Uploads a new track with progress callback
    pub async fn upload_track(
        &self,
        metadata: TrackUploadMetadata,
        on_progress: impl Fn(UploadProgress) + Send + 'static,
    ) -> Result<u64> {
        let client_id = self.ensure_valid_client_id().await?;
        let oauth_token = self.session.get_oauth_token().await;
        upload_track(&self.http, &client_id, oauth_token.as_deref(), metadata, on_progress).await
    }

    /// Fetches the authenticated user's profile (`GET /me`)
    pub async fn get_current_user(&self) -> Result<UserProfile> {
        let client_id = self.ensure_valid_client_id().await?;
        let oauth_token = self
            .session
            .get_oauth_token()
            .await
            .context("No active OAuth session token found. Please log in first.")?;

        let url = format!("{}/me?client_id={}", API_V2_BASE, client_id);
        let resp = self
            .http
            .get(&url)
            .header("Authorization", format!("OAuth {}", oauth_token))
            .send()
            .await
            .context("Querying SoundCloud /me endpoint")?;

        if !resp.status().is_success() {
            bail!("Failed to authenticate session: HTTP {}", resp.status());
        }

        let user: UserProfile = resp.json().await.context("Parsing user profile JSON")?;
        *self.session.user_id.write().await = Some(user.id);
        info!("Authenticated SoundCloud session for user: {} (ID: {})", user.username, user.id);
        Ok(user)
    }

    /// Fetches the authenticated user's liked tracks
    pub async fn get_my_liked_tracks(&self, limit: usize) -> Result<Vec<Track>> {
        let client_id = self.ensure_valid_client_id().await?;
        let oauth_token = self
            .session
            .get_oauth_token()
            .await
            .context("No active OAuth session token found")?;

        let user_id = match *self.session.user_id.read().await {
            Some(id) => id,
            None => {
                let me = self.get_current_user().await?;
                me.id
            }
        };

        let url = format!(
            "{}/users/{}/track_likes?client_id={}&limit={}",
            API_V2_BASE, user_id, client_id, limit
        );

        let resp = self
            .http
            .get(&url)
            .header("Authorization", format!("OAuth {}", oauth_token))
            .send()
            .await
            .context("Fetching user liked tracks")?;

        if !resp.status().is_success() {
            bail!("Failed to fetch liked tracks: HTTP {}", resp.status());
        }

        #[derive(serde::Deserialize)]
        struct LikeItem {
            track: Track,
        }

        #[derive(serde::Deserialize)]
        struct LikesResp {
            collection: Vec<LikeItem>,
        }

        let body: LikesResp = resp.json().await.context("Parsing liked tracks JSON")?;
        let mut tracks = Vec::with_capacity(body.collection.len());
        for mut item in body.collection {
            item.track.user_favorite = Some(true);
            tracks.push(item.track);
        }
        Ok(tracks)
    }

    /// Fetches the authenticated user's real personalized activity stream (`GET /stream`)
    pub async fn get_my_stream(&self, limit: usize) -> Result<Vec<Track>> {
        let client_id = self.ensure_valid_client_id().await?;
        let oauth_token = self
            .session
            .get_oauth_token()
            .await
            .context("No active OAuth session token found")?;

        let url = format!("{}/stream?client_id={}&limit={}", API_V2_BASE, client_id, limit);
        let resp = self
            .http
            .get(&url)
            .header("Authorization", format!("OAuth {}", oauth_token))
            .header("Accept-Language", "ar,en-US;q=0.8,en;q=0.5")
            .send()
            .await
            .context("Fetching personalized stream")?;

        if !resp.status().is_success() {
            bail!("Failed to fetch stream: HTTP {}", resp.status());
        }

        #[derive(serde::Deserialize)]
        struct StreamItem {
            track: Option<Track>,
        }

        #[derive(serde::Deserialize)]
        struct StreamResp {
            collection: Vec<StreamItem>,
        }

        let body: StreamResp = resp.json().await.context("Parsing stream JSON")?;
        let mut tracks = Vec::new();
        for item in body.collection {
            if let Some(t) = item.track {
                tracks.push(t);
            }
        }
        Ok(tracks)
    }

    /// Fetches both created and liked/saved playlists for the authenticated user
    pub async fn get_my_playlists(&self, limit: usize) -> Result<Vec<Playlist>> {
        let client_id = self.ensure_valid_client_id().await?;
        let oauth_token = self
            .session
            .get_oauth_token()
            .await
            .context("No active OAuth session token found")?;

        let user_id = match *self.session.user_id.read().await {
            Some(id) => id,
            None => {
                let me = self.get_current_user().await?;
                me.id
            }
        };

        let mut all_playlists = Vec::new();
        let mut seen_ids = std::collections::HashSet::new();

        // 1. Fetch created playlists (/users/{id}/playlists)
        let created_url = format!(
            "{}/users/{}/playlists?client_id={}&limit={}",
            API_V2_BASE, user_id, client_id, limit
        );

        if let Ok(resp) = self
            .http
            .get(&created_url)
            .header("Authorization", format!("OAuth {}", oauth_token))
            .header("Accept-Language", "ar,en-US;q=0.8,en;q=0.5")
            .send()
            .await
        {
            if resp.status().is_success() {
                #[derive(serde::Deserialize)]
                struct CreatedResp {
                    collection: Vec<Playlist>,
                }
                if let Ok(body) = resp.json::<CreatedResp>().await {
                    for pl in body.collection {
                        if seen_ids.insert(pl.id) {
                            all_playlists.push(pl);
                        }
                    }
                }
            }
        }

        // 2. Fetch liked/saved playlists (/users/{id}/playlist_likes)
        let liked_url = format!(
            "{}/users/{}/playlist_likes?client_id={}&limit={}",
            API_V2_BASE, user_id, client_id, limit
        );

        if let Ok(resp) = self
            .http
            .get(&liked_url)
            .header("Authorization", format!("OAuth {}", oauth_token))
            .header("Accept-Language", "ar,en-US;q=0.8,en;q=0.5")
            .send()
            .await
        {
            if resp.status().is_success() {
                #[derive(serde::Deserialize)]
                struct LikedPlaylistItem {
                    playlist: Playlist,
                }
                #[derive(serde::Deserialize)]
                struct LikedPlaylistsResp {
                    collection: Vec<LikedPlaylistItem>,
                }
                if let Ok(body) = resp.json::<LikedPlaylistsResp>().await {
                    for item in body.collection {
                        if seen_ids.insert(item.playlist.id) {
                            all_playlists.push(item.playlist);
                        }
                    }
                }
            }
        }

        info!("Fetched {} total playlists (created + saved)", all_playlists.len());
        Ok(all_playlists)
    }

    /// Fetches recent user activities / notifications
    pub async fn get_activities(&self, limit: usize) -> Result<Vec<crate::models::activity::ActivityItem>> {
        let client_id = self.ensure_valid_client_id().await?;
        let oauth_token = self
            .session
            .get_oauth_token()
            .await
            .context("Authentication required to view notifications")?;

        let url = format!("{}/activities?client_id={}&limit={}", API_V2_BASE, client_id, limit);
        let resp = self
            .http
            .get(&url)
            .header("Authorization", format!("OAuth {}", oauth_token))
            .header("Accept-Language", "ar,en-US;q=0.8,en;q=0.5")
            .send()
            .await
            .context("Fetching activities")?;

        if !resp.status().is_success() {
            bail!("Activities request returned HTTP {}", resp.status());
        }

        let body: crate::models::activity::ActivitiesResponse = resp.json().await.context("Parsing activities JSON")?;
        Ok(body.collection)
    }

    /// Fetches user conversation threads
    pub async fn get_conversations(&self, limit: usize) -> Result<Vec<crate::models::conversation::Conversation>> {
        let client_id = self.ensure_valid_client_id().await?;
        let oauth_token = self
            .session
            .get_oauth_token()
            .await
            .context("Authentication required to view conversations")?;

        let user_id = match *self.session.user_id.read().await {
            Some(id) => id,
            None => {
                let user = self.get_current_user().await?;
                user.id
            }
        };

        let url = format!("{}/users/{}/conversations?client_id={}&limit={}", API_V2_BASE, user_id, client_id, limit);
        let resp = self
            .http
            .get(&url)
            .header("Authorization", format!("OAuth {}", oauth_token))
            .header("Accept-Language", "ar,en-US;q=0.8,en;q=0.5")
            .send()
            .await
            .context("Fetching conversations")?;

        if !resp.status().is_success() {
            bail!("Conversations request returned HTTP {}", resp.status());
        }

        let body: crate::models::conversation::ConversationsResponse = resp.json().await.context("Parsing conversations JSON")?;
        Ok(body.collection)
    }

    /// Fetches messages in a conversation thread with another user
    pub async fn get_conversation_messages(&self, other_user_id: u64, limit: usize) -> Result<Vec<crate::models::conversation::ConversationMessage>> {
        let client_id = self.ensure_valid_client_id().await?;
        let oauth_token = self
            .session
            .get_oauth_token()
            .await
            .context("Authentication required to view messages")?;

        let user_id = match *self.session.user_id.read().await {
            Some(id) => id,
            None => {
                let user = self.get_current_user().await?;
                user.id
            }
        };

        let url = format!(
            "{}/users/{}/conversations/{}/messages?client_id={}&limit={}",
            API_V2_BASE, user_id, other_user_id, client_id, limit
        );
        let resp = self
            .http
            .get(&url)
            .header("Authorization", format!("OAuth {}", oauth_token))
            .header("Accept-Language", "ar,en-US;q=0.8,en;q=0.5")
            .send()
            .await
            .context("Fetching conversation messages")?;

        if !resp.status().is_success() {
            bail!("Messages request returned HTTP {}", resp.status());
        }

        let body: crate::models::conversation::ConversationMessagesResponse = resp.json().await.context("Parsing messages JSON")?;
        Ok(body.collection)
    }

    /// Sends a direct message to another user in a conversation thread
    pub async fn send_conversation_message(&self, other_user_id: u64, content: &str) -> Result<crate::models::conversation::ConversationMessage> {
        let client_id = self.ensure_valid_client_id().await?;
        let oauth_token = self
            .session
            .get_oauth_token()
            .await
            .context("Authentication required to send message")?;

        let user_id = match *self.session.user_id.read().await {
            Some(id) => id,
            None => {
                let user = self.get_current_user().await?;
                user.id
            }
        };

        let url = format!(
            "{}/users/{}/conversations/{}/messages?client_id={}",
            API_V2_BASE, user_id, other_user_id, client_id
        );
        let resp = self
            .http
            .post(&url)
            .header("Authorization", format!("OAuth {}", oauth_token))
            .header("Accept-Language", "ar,en-US;q=0.8,en;q=0.5")
            .json(&serde_json::json!({ "content": content }))
            .send()
            .await
            .context("Sending message")?;

        if !resp.status().is_success() {
            let status = resp.status();
            let err = resp.text().await.unwrap_or_default();
            bail!("Failed to send message (HTTP {}): {}", status, err);
        }

        let msg: crate::models::conversation::ConversationMessage = resp.json().await.context("Parsing sent message JSON")?;
        Ok(msg)
    }

    /// Updates authenticated user profile information
    pub async fn update_profile(
        &self,
        username: &str,
        bio: Option<&str>,
        city: Option<&str>,
        country_code: Option<&str>,
    ) -> Result<UserProfile> {
        let client_id = self.ensure_valid_client_id().await?;
        let oauth_token = self
            .session
            .get_oauth_token()
            .await
            .context("Authentication required to update profile")?;

        let url = format!("{}/me?client_id={}", API_V2_BASE, client_id);
        let mut user_payload = serde_json::json!({
            "username": username,
        });

        if let Some(b) = bio {
            user_payload["description"] = serde_json::Value::String(b.to_string());
        }
        if let Some(c) = city {
            user_payload["city"] = serde_json::Value::String(c.to_string());
        }
        if let Some(cc) = country_code {
            user_payload["country_code"] = serde_json::Value::String(cc.to_string());
        }

        let resp = self
            .http
            .put(&url)
            .header("Authorization", format!("OAuth {}", oauth_token))
            .header("User-Agent", "Mozilla/5.0 (X11; Linux x86_64; rv:130.0) Gecko/20100101 Firefox/130.0")
            .json(&serde_json::json!({ "user": user_payload }))
            .send()
            .await
            .context("Sending profile update")?;

        if !resp.status().is_success() {
            let status = resp.status();
            let err = resp.text().await.unwrap_or_default();
            bail!("Profile update failed (HTTP {}): {}", status, err);
        }

        // Return updated user profile
        self.get_current_user().await
    }

    /// Uploads an avatar image for the authenticated user
    pub async fn upload_avatar(&self, image_path: &str) -> Result<()> {
        let client_id = self.ensure_valid_client_id().await?;
        let oauth_token = self
            .session
            .get_oauth_token()
            .await
            .context("Authentication required to upload avatar")?;

        let url = format!("{}/me/profile/avatar?client_id={}", API_V2_BASE, client_id);

        let file = tokio::fs::File::open(image_path).await?;
        let filename = std::path::Path::new(image_path)
            .file_name()
            .map(|f| f.to_string_lossy().to_string())
            .unwrap_or_else(|| "avatar.jpg".to_string());

        let stream = tokio_util::codec::FramedRead::new(file, tokio_util::codec::BytesCodec::new());
        let part = reqwest::multipart::Part::stream(reqwest::Body::wrap_stream(stream))
            .file_name(filename)
            .mime_str("image/jpeg")?;

        let form = reqwest::multipart::Form::new().part("image_data", part);

        let resp = self
            .http
            .post(&url)
            .header("Authorization", format!("OAuth {}", oauth_token))
            .header("User-Agent", "Mozilla/5.0 (X11; Linux x86_64; rv:130.0) Gecko/20100101 Firefox/130.0")
            .multipart(form)
            .send()
            .await
            .context("Uploading avatar image")?;

        if !resp.status().is_success() {
            let status = resp.status();
            let err = resp.text().await.unwrap_or_default();
            bail!("Avatar upload failed (HTTP {}): {}", status, err);
        }

        Ok(())
    }

    /// Fetches public profile details for any artist / user by ID
    pub async fn get_user_profile(&self, user_id: u64) -> Result<UserProfile> {
        let client_id = self.ensure_valid_client_id().await?;
        let oauth_token = self.session.get_oauth_token().await;
        let url = format!("{}/users/{}?client_id={}", API_V2_BASE, user_id, client_id);
        let mut req = self
            .http
            .get(&url)
            .header("Accept-Language", "ar,en-US;q=0.8,en;q=0.5");
        if let Some(token) = oauth_token {
            req = req.header("Authorization", format!("OAuth {}", token));
        }
        let resp = req.send().await.context("Fetching user profile")?;
        if !resp.status().is_success() {
            bail!("Failed to get user profile {}: HTTP {}", user_id, resp.status());
        }
        let user = resp.json::<UserProfile>().await.context("Parsing user JSON")?;
        Ok(user)
    }

    /// Fetches an artist's tracks
    pub async fn get_user_tracks(&self, user_id: u64, limit: usize) -> Result<Vec<Track>> {
        let client_id = self.ensure_valid_client_id().await?;
        let oauth_token = self.session.get_oauth_token().await;
        let url = format!(
            "{}/users/{}/tracks?client_id={}&limit={}",
            API_V2_BASE, user_id, client_id, limit
        );
        let mut req = self
            .http
            .get(&url)
            .header("Accept-Language", "ar,en-US;q=0.8,en;q=0.5");
        if let Some(token) = oauth_token {
            req = req.header("Authorization", format!("OAuth {}", token));
        }
        let resp = req.send().await.context("Fetching user tracks")?;
        if !resp.status().is_success() {
            bail!("Failed to get user tracks: HTTP {}", resp.status());
        }
        #[derive(serde::Deserialize)]
        struct UserTracksResp {
            collection: Vec<Track>,
        }
        let body: UserTracksResp = resp.json().await.context("Parsing user tracks JSON")?;
        Ok(body.collection)
    }

    /// Fetches top tracks or falls back to recent tracks
    pub async fn get_user_top_tracks(&self, user_id: u64, limit: usize) -> Result<Vec<Track>> {
        let client_id = self.ensure_valid_client_id().await?;
        let oauth_token = self.session.get_oauth_token().await;
        let url = format!(
            "{}/users/{}/toptracks?client_id={}&limit={}",
            API_V2_BASE, user_id, client_id, limit
        );
        let mut req = self
            .http
            .get(&url)
            .header("Accept-Language", "ar,en-US;q=0.8,en;q=0.5");
        if let Some(token) = oauth_token {
            req = req.header("Authorization", format!("OAuth {}", token));
        }
        if let Ok(resp) = req.send().await {
            if resp.status().is_success() {
                #[derive(serde::Deserialize)]
                struct UserTracksResp {
                    collection: Vec<Track>,
                }
                if let Ok(body) = resp.json::<UserTracksResp>().await {
                    if !body.collection.is_empty() {
                        return Ok(body.collection);
                    }
                }
            }
        }
        // Fallback to general tracks
        self.get_user_tracks(user_id, limit).await
    }

    /// Checks if current authenticated user follows this artist
    pub async fn is_following_user(&self, user_id: u64) -> bool {
        if let Ok(client_id) = self.ensure_valid_client_id().await {
            if let Some(token) = self.session.get_oauth_token().await {
                let url = format!("{}/me/followings/{}?client_id={}", API_V2_BASE, user_id, client_id);
                if let Ok(resp) = self
                    .http
                    .get(&url)
                    .header("Authorization", format!("OAuth {}", token))
                    .send()
                    .await
                {
                    return resp.status().is_success();
                }
            }
        }
        false
    }

    /// Follow or unfollow an artist
    pub async fn follow_user(&self, user_id: u64, follow: bool) -> Result<()> {
        let client_id = self.ensure_valid_client_id().await?;
        let token = self
            .session
            .get_oauth_token()
            .await
            .context("Authentication required to follow")?;
        let url = format!("{}/me/followings/{}?client_id={}", API_V2_BASE, user_id, client_id);
        let req = if follow {
            self.http.put(&url)
        } else {
            self.http.delete(&url)
        };
        let resp = req
            .header("Authorization", format!("OAuth {}", token))
            .send()
            .await?;
        if !resp.status().is_success() {
            debug!("Follow endpoint returned status {}", resp.status());
        }
        Ok(())
    }
}
