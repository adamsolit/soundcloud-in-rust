use anyhow::{Context, Result};
use reqwest::Client;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

use crate::keyring::AuthStore;

// Standard public SoundCloud Web App Client ID
// Auto-discovered dynamically, with active tested default fallback
pub const DEFAULT_CLIENT_ID: &str = "LhtSg5xg6rKfbiBgb6ua4SevlglR3sJZ";

pub fn sanitize_token(token: &str) -> String {
    let t = token.trim().trim_matches('"').trim_matches('\'');
    if let Some(rest) = t.strip_prefix("OAuth ") {
        rest.trim().to_string()
    } else {
        t.to_string()
    }
}

#[derive(Clone, Debug)]
pub struct SoundCloudSession {
    pub client_id: Arc<RwLock<String>>,
    pub oauth_token: Arc<RwLock<Option<String>>>,
    pub user_id: Arc<RwLock<Option<u64>>>,
    auth_store: AuthStore,
}

impl SoundCloudSession {
    pub fn new() -> Self {
        let auth_store = AuthStore::new();
        let client_id = auth_store
            .get_client_id()
            .unwrap_or(None)
            .unwrap_or_else(|| DEFAULT_CLIENT_ID.to_string());
        let oauth_token = auth_store.get_oauth_token().unwrap_or(None);

        Self {
            client_id: Arc::new(RwLock::new(client_id)),
            oauth_token: Arc::new(RwLock::new(oauth_token)),
            user_id: Arc::new(RwLock::new(None)),
            auth_store,
        }
    }

    pub async fn get_client_id(&self) -> String {
        self.client_id.read().await.clone()
    }

    pub async fn get_oauth_token(&self) -> Option<String> {
        self.oauth_token.read().await.clone()
    }

    pub async fn set_oauth_token(&self, token: &str) -> Result<()> {
        self.set_credentials(None, Some(token.to_string())).await
    }

    pub async fn set_credentials(&self, client_id: Option<String>, oauth_token: Option<String>) -> Result<()> {
        if let Some(ref cid) = client_id {
            let clean_cid = cid.trim();
            if !clean_cid.is_empty() {
                *self.client_id.write().await = clean_cid.to_string();
                self.auth_store.save_client_id(clean_cid)?;
            }
        }
        if let Some(ref tok) = oauth_token {
            let clean_tok = sanitize_token(tok);
            if clean_tok.is_empty() {
                *self.oauth_token.write().await = None;
                self.auth_store.clear_oauth_token()?;
            } else {
                *self.oauth_token.write().await = Some(clean_tok.clone());
                self.auth_store.save_oauth_token(&clean_tok)?;
            }
        }
        Ok(())
    }

    pub async fn clear_credentials(&self) -> Result<()> {
        *self.oauth_token.write().await = None;
        *self.user_id.write().await = None;
        self.auth_store.clear_oauth_token()?;
        Ok(())
    }

    pub async fn is_authenticated(&self) -> bool {
        self.oauth_token.read().await.is_some()
    }

    /// Automatically discovers the latest valid client_id by inspecting soundcloud.com web scripts
    pub async fn discover_client_id(&self, http: &Client) -> Result<String> {
        info!("Attempting dynamic SoundCloud Client-ID auto-discovery...");
        let resp = http
            .get("https://soundcloud.com")
            .header("User-Agent", "Mozilla/5.0 (X11; Linux x86_64; rv:130.0) Gecko/20100101 Firefox/130.0")
            .send()
            .await
            .context("Connecting to soundcloud.com for discovery")?;

        let html = resp.text().await?;
        
        // Find script tags: src="https://a-v2.sndcdn.com/assets/..."
        let mut script_urls = Vec::new();
        for line in html.split('\n') {
            if let Some(start) = line.find("https://a-v2.sndcdn.com/assets/") {
                let rest = &line[start..];
                if let Some(end) = rest.find(".js") {
                    let script_url = &rest[..end + 3];
                    script_urls.push(script_url.to_string());
                }
            }
        }

        debug!("Discovered {} asset script URLs", script_urls.len());

        // Probe scripts from the end (app chunks usually hold client_id)
        for url in script_urls.into_iter().rev().take(8) {
            if let Ok(script_resp) = http.get(&url).send().await {
                if let Ok(script_body) = script_resp.text().await {
                    if let Some(cid) = extract_client_id(&script_body) {
                        info!("Discovered valid active SoundCloud Client-ID: {}", cid);
                        *self.client_id.write().await = cid.clone();
                        let _ = self.auth_store.save_client_id(&cid);
                        return Ok(cid);
                    }
                }
            }
        }

        warn!("Dynamic discovery found no client_id; falling back to default");
        Ok(DEFAULT_CLIENT_ID.to_string())
    }
}

fn extract_client_id(text: &str) -> Option<String> {
    let patterns = ["client_id:\"", "client_id=\"", "client_id:'"];
    for pat in patterns {
        if let Some(idx) = text.find(pat) {
            let rest = &text[idx + pat.len()..];
            let end_quote = if pat.ends_with('\'') { '\'' } else { '"' };
            if let Some(end_idx) = rest.find(end_quote) {
                let candidate = &rest[..end_idx];
                if candidate.len() == 32 && candidate.chars().all(|c| c.is_ascii_alphanumeric()) {
                    return Some(candidate.to_string());
                }
            }
        }
    }
    None
}
