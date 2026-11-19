use serde::{Deserialize, Serialize};
use super::track::Track;
use super::user::UserProfile;

fn default_playlist_title() -> String {
    "SoundCloud Playlist".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Playlist {
    pub id: u64,
    #[serde(default = "default_playlist_title")]
    pub title: String,
    pub description: Option<String>,
    pub duration: Option<u64>,
    pub track_count: Option<u64>,
    pub artwork_url: Option<String>,
    pub permalink_url: Option<String>,
    #[serde(default)]
    pub is_public: Option<bool>,
    pub user: Option<UserProfile>,
    #[serde(default)]
    pub tracks: Vec<Track>,
    pub created_at: Option<String>,
    pub release_date: Option<String>,
    #[serde(default)]
    pub likes_count: Option<u64>,
}

impl Playlist {
    pub fn cover_url(&self) -> Option<String> {
        if let Some(ref a) = self.artwork_url {
            let s = a.trim();
            if !s.is_empty() && s != "null" {
                return Some(s.to_string());
            }
        }
        for track in &self.tracks {
            if let Some(ref a) = track.artwork_url {
                let s = a.trim();
                if !s.is_empty() && s != "null" {
                    return Some(s.to_string());
                }
            }
            if let Some(ref u) = track.user {
                if let Some(ref a) = u.avatar_url {
                    let s = a.trim();
                    if !s.is_empty() && s != "null" {
                        return Some(s.to_string());
                    }
                }
            }
        }
        if let Some(ref u) = self.user {
            if let Some(ref a) = u.avatar_url {
                let s = a.trim();
                if !s.is_empty() && s != "null" {
                    return Some(s.to_string());
                }
            }
        }
        None
    }

    pub fn author_name(&self) -> &str {
        self.user
            .as_ref()
            .map(|u| u.username.as_str())
            .unwrap_or("SoundCloud Creator")
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreatePlaylistPayload {
    pub title: String,
    pub description: Option<String>,
    pub sharing: String, // "public" or "private"
    #[serde(default)]
    pub tracks: Vec<u64>,
}
