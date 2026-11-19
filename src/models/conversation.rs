use serde::{Deserialize, Serialize};
use crate::models::user::UserProfile;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversationMessage {
    #[serde(default)]
    pub content: String,
    #[serde(default)]
    pub conversation_id: Option<String>,
    #[serde(default)]
    pub sender: Option<UserProfile>,
    #[serde(default)]
    pub sent_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Conversation {
    pub id: String,
    pub last_message: Option<ConversationMessage>,
    #[serde(default)]
    pub read: bool,
    #[serde(default)]
    pub started_at: Option<String>,
    #[serde(default)]
    pub summary: Option<String>,
    #[serde(default)]
    pub users: Vec<UserProfile>,
}

impl Conversation {
    pub fn other_user(&self, current_user_id: u64) -> Option<&UserProfile> {
        self.users.iter().find(|u| u.id != current_user_id).or_else(|| self.users.first())
    }

    pub fn display_title(&self, current_user_id: u64) -> String {
        self.other_user(current_user_id)
            .map(|u| u.username.clone())
            .unwrap_or_else(|| "SoundCloud User".to_string())
    }

    pub fn display_avatar(&self, current_user_id: u64) -> Option<String> {
        self.other_user(current_user_id)
            .and_then(|u| u.avatar_url.clone())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversationsResponse {
    pub collection: Vec<Conversation>,
    pub next_href: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversationMessagesResponse {
    pub collection: Vec<ConversationMessage>,
    pub next_href: Option<String>,
}
