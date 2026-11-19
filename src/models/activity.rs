use serde::{Deserialize, Serialize};
use crate::models::track::Track;
use crate::models::user::UserProfile;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActivityItem {
    #[serde(default)]
    pub r#type: String, // "affiliation", "track-like", "track-repost", "comment"
    #[serde(default)]
    pub created_at: String,
    #[serde(default)]
    pub user: Option<UserProfile>,
    #[serde(default)]
    pub origin: Option<serde_json::Value>,
}

impl ActivityItem {
    pub fn action_summary(&self) -> String {
        let name = self.user.as_ref().map(|u| u.username.as_str()).unwrap_or("Someone");
        match self.r#type.as_str() {
            "affiliation" => format!("{} started following you", name),
            "track-like" => {
                let track_title = self.origin.as_ref()
                    .and_then(|o| o.get("title"))
                    .and_then(|t| t.as_str())
                    .unwrap_or("your track");
                format!("{} liked {}", name, track_title)
            }
            "track-repost" => {
                let track_title = self.origin.as_ref()
                    .and_then(|o| o.get("title"))
                    .and_then(|t| t.as_str())
                    .unwrap_or("your track");
                format!("{} reposted {}", name, track_title)
            }
            "comment" => {
                let track_title = self.origin.as_ref()
                    .and_then(|o| o.get("title"))
                    .and_then(|t| t.as_str())
                    .unwrap_or("your track");
                format!("{} commented on {}", name, track_title)
            }
            other => format!("{} performed action: {}", name, other),
        }
    }

    pub fn target_track(&self) -> Option<Track> {
        self.origin.as_ref().and_then(|o| serde_json::from_value::<Track>(o.clone()).ok())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActivitiesResponse {
    pub collection: Vec<ActivityItem>,
    pub next_href: Option<String>,
}
