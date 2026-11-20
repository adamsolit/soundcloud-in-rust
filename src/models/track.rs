use serde::{Deserialize, Serialize};

fn default_track_title() -> String {
    "SoundCloud Track".to_string()
}

fn default_username() -> String {
    "SoundCloud Artist".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Track {
    pub id: u64,
    #[serde(default = "default_track_title")]
    pub title: String,
    pub user: Option<TrackUser>,
    #[serde(default)]
    pub duration: u64, // Duration in milliseconds
    pub artwork_url: Option<String>,
    pub waveform_url: Option<String>,
    pub permalink_url: Option<String>,
    #[serde(default)]
    pub playback_count: Option<u64>,
    #[serde(default)]
    pub likes_count: Option<u64>,
    #[serde(default)]
    pub reposts_count: Option<u64>,
    #[serde(default)]
    pub comment_count: Option<u64>,
    pub description: Option<String>,
    pub genre: Option<String>,
    pub tag_list: Option<String>,
    pub created_at: Option<String>,
    #[serde(default)]
    pub user_favorite: Option<bool>,
    pub media: Option<TrackMedia>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrackUser {
    pub id: u64,
    #[serde(default = "default_username")]
    pub username: String,
    pub avatar_url: Option<String>,
    pub permalink_url: Option<String>,
    #[serde(default)]
    pub followers_count: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrackMedia {
    #[serde(default)]
    pub transcodings: Vec<Transcoding>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Transcoding {
    pub url: String,
    pub preset: Option<String>,
    pub duration: Option<u64>,
    pub snipped: Option<bool>,
    pub format: TranscodingFormat,
    pub quality: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranscodingFormat {
    pub protocol: String, // "hls" or "progressive"
    pub mime_type: String, // "audio/mpeg" or "audio/ogg; codecs=\"opus\""
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamResolutionResponse {
    pub url: String,
}

impl Track {
    pub fn duration_formatted(&self) -> String {
        format_milliseconds(self.duration)
    }

    pub fn artist_name(&self) -> &str {
        self.user
            .as_ref()
            .map(|u| u.username.as_str())
            .unwrap_or("Unknown Artist")
    }

    pub fn user_id(&self) -> Option<u64> {
        self.user.as_ref().map(|u| u.id)
    }

    pub fn best_artwork_url(&self) -> Option<String> {
        self.artwork_url
            .as_ref()
            .map(|url| {
                // SoundCloud default thumbnail is -large.jpg (100x100).
                // Replace with -t500x500.jpg for high-res artwork.
                if url.contains("-large.") {
                    url.replace("-large.", "-t500x500.")
                } else {
                    url.clone()
                }
            })
            .or_else(|| {
                self.user
                    .as_ref()
                    .and_then(|u| u.avatar_url.as_ref())
                    .map(|url| {
                        if url.contains("-large.") {
                            url.replace("-large.", "-t500x500.")
                        } else {
                            url.clone()
                        }
                    })
            })
    }

    pub fn thumbnail_url(&self) -> Option<String> {
        self.artwork_url
            .clone()
            .or_else(|| self.user.as_ref().and_then(|u| u.avatar_url.clone()))
    }

    pub fn best_transcoding(&self) -> Option<&Transcoding> {
        let trans = self.media.as_ref()?.transcodings.as_slice();
        if trans.is_empty() {
            return None;
        }

        // Prioritize:
        // 1. Progressive MP3 (direct static audio file)
        // 2. HLS AAC / MP4 (best modern streaming format on SoundCloud)
        // 3. HLS Opus / OGG
        // 4. Any other non-mpegurl transcoding
        trans
            .iter()
            .find(|t| t.format.protocol == "progressive" && t.format.mime_type.contains("mpeg"))
            .or_else(|| {
                trans
                    .iter()
                    .find(|t| t.format.protocol == "hls" && (t.format.mime_type.contains("mp4") || t.format.mime_type.contains("aac")))
            })
            .or_else(|| {
                trans
                    .iter()
                    .find(|t| t.format.protocol == "hls" && t.format.mime_type.contains("ogg"))
            })
            .or_else(|| {
                trans
                    .iter()
                    .find(|t| !t.format.mime_type.contains("mpegurl"))
            })
            .or_else(|| trans.first())
    }
}

pub fn format_milliseconds(ms: u64) -> String {
    let total_seconds = ms / 1000;
    let minutes = total_seconds / 60;
    let seconds = total_seconds % 60;
    let hours = minutes / 60;

    if hours > 0 {
        format!("{}:{:02}:{:02}", hours, minutes % 60, seconds)
    } else {
        format!("{}:{:02}", minutes, seconds)
    }
}
