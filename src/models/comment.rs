use serde::{Deserialize, Serialize};
use super::track::format_milliseconds;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Comment {
    pub id: u64,
    pub body: String,
    pub timestamp: Option<u64>, // Milliseconds into track where comment was posted
    pub created_at: Option<String>,
    pub user: Option<CommentUser>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommentUser {
    pub id: u64,
    pub username: String,
    pub avatar_url: Option<String>,
    pub permalink_url: Option<String>,
}

impl Comment {
    pub fn timestamp_formatted(&self) -> String {
        match self.timestamp {
            Some(ts) => format_milliseconds(ts),
            None => "0:00".to_string(),
        }
    }
}
