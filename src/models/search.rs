use serde::{Deserialize, Serialize};
use super::track::Track;
use super::user::UserProfile;
use super::playlist::Playlist;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SearchFilter {
    #[default]
    All,
    Tracks,
    Playlists,
    Users,
}

impl SearchFilter {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::All => "all",
            Self::Tracks => "tracks",
            Self::Playlists => "playlists",
            Self::Users => "users",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SearchResults {
    #[serde(default)]
    pub collection: Vec<SearchResultItem>,
    pub next_href: Option<String>,
    #[serde(default)]
    pub total_results: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum SearchResultItem {
    #[serde(rename = "track")]
    Track(Track),
    #[serde(rename = "playlist")]
    Playlist(Playlist),
    #[serde(rename = "user")]
    User(UserProfile),
    #[serde(other)]
    Unknown,
}
