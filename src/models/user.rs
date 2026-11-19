use serde::{Deserialize, Serialize};

fn default_username() -> String {
    "SoundCloud User".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserProfile {
    pub id: u64,
    #[serde(default = "default_username")]
    pub username: String,
    pub permalink: Option<String>,
    pub permalink_url: Option<String>,
    pub avatar_url: Option<String>,
    pub country: Option<String>,
    pub city: Option<String>,
    pub description: Option<String>,
    #[serde(default)]
    pub track_count: Option<u64>,
    #[serde(default)]
    pub playlist_count: Option<u64>,
    #[serde(default)]
    pub followers_count: Option<u64>,
    #[serde(default)]
    pub followings_count: Option<u64>,
    #[serde(default)]
    pub likes_count: Option<u64>,
    #[serde(default)]
    pub verified: Option<bool>,
}

impl UserProfile {
    pub fn display_location(&self) -> Option<String> {
        match (&self.city, &self.country) {
            (Some(city), Some(country)) => Some(format!("{}, {}", city, country)),
            (Some(city), None) => Some(city.clone()),
            (None, Some(country)) => Some(country.clone()),
            (None, None) => None,
        }
    }
}
