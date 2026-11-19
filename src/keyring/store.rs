use anyhow::{Context, Result};
use keyring::Entry;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use tracing::{debug, warn};

const SERVICE_NAME: &str = "cloudpulse_soundcloud";
const KEY_OAUTH_TOKEN: &str = "oauth_token";
const KEY_CLIENT_ID: &str = "client_id";

#[derive(Serialize, Deserialize, Default, Debug)]
struct FallbackStore {
    client_id: Option<String>,
    oauth_token: Option<String>,
}

#[derive(Clone, Debug)]
pub struct AuthStore {
    service: String,
    fallback_path: PathBuf,
}

impl AuthStore {
    pub fn new() -> Self {
        let xdg_data = std::env::var("XDG_DATA_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|_| {
                let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
                PathBuf::from(home).join(".local").join("share")
            });
        let app_dir = xdg_data.join("cloudpulse");
        if let Err(e) = fs::create_dir_all(&app_dir) {
            warn!("Could not create app data dir: {}", e);
        }

        Self {
            service: SERVICE_NAME.to_string(),
            fallback_path: app_dir.join("auth_session.json"),
        }
    }

    pub fn with_custom_path(service: &str, path: PathBuf) -> Self {
        Self {
            service: service.to_string(),
            fallback_path: path,
        }
    }

    pub fn save_oauth_token(&self, token: &str) -> Result<()> {
        let clean = token.trim();
        if clean.is_empty() {
            return self.clear_oauth_token();
        }

        // Always persist to local fallback file for instant reboot survival
        let _ = self.mutate_fallback(|f| f.oauth_token = Some(clean.to_string()));

        // Also persist to system keyring
        if let Ok(entry) = Entry::new(&self.service, KEY_OAUTH_TOKEN) {
            if let Err(e) = entry.set_password(clean) {
                warn!("Keyring set_password failed (file fallback saved): {}", e);
            } else {
                debug!("Saved oauth_token to system keyring and fallback store");
            }
        }
        Ok(())
    }

    pub fn get_oauth_token(&self) -> Result<Option<String>> {
        if let Ok(entry) = Entry::new(&self.service, KEY_OAUTH_TOKEN) {
            match entry.get_password() {
                Ok(token) if !token.trim().is_empty() => {
                    return Ok(Some(token));
                }
                _ => {}
            }
        }
        let fallback = self.read_fallback()?;
        Ok(fallback.oauth_token)
    }

    pub fn clear_oauth_token(&self) -> Result<()> {
        if let Ok(entry) = Entry::new(&self.service, KEY_OAUTH_TOKEN) {
            let _ = entry.delete_credential();
        }
        self.mutate_fallback(|f| f.oauth_token = None)?;
        Ok(())
    }

    pub fn save_client_id(&self, client_id: &str) -> Result<()> {
        let clean = client_id.trim();
        let _ = self.mutate_fallback(|f| f.client_id = Some(clean.to_string()));

        if let Ok(entry) = Entry::new(&self.service, KEY_CLIENT_ID) {
            if let Err(e) = entry.set_password(clean) {
                warn!("Keyring client_id set failed (file fallback saved): {}", e);
            }
        }
        Ok(())
    }

    pub fn get_client_id(&self) -> Result<Option<String>> {
        if let Ok(entry) = Entry::new(&self.service, KEY_CLIENT_ID) {
            match entry.get_password() {
                Ok(cid) if !cid.trim().is_empty() => {
                    return Ok(Some(cid));
                }
                _ => {}
            }
        }
        let fallback = self.read_fallback()?;
        Ok(fallback.client_id)
    }

    pub fn clear_all(&self) -> Result<()> {
        self.clear_oauth_token()?;
        if let Ok(entry) = Entry::new(&self.service, KEY_CLIENT_ID) {
            let _ = entry.delete_credential();
        }
        if self.fallback_path.exists() {
            let _ = fs::remove_file(&self.fallback_path);
        }
        Ok(())
    }

    // Backward-compat aliases
    pub fn save_token(&self, token: &str) -> Result<()> {
        self.save_oauth_token(token)
    }

    pub fn get_token(&self) -> Result<Option<String>> {
        self.get_oauth_token()
    }

    pub fn clear_token(&self) -> Result<()> {
        self.clear_oauth_token()
    }

    fn read_fallback(&self) -> Result<FallbackStore> {
        if !self.fallback_path.exists() {
            return Ok(FallbackStore::default());
        }
        let content = fs::read_to_string(&self.fallback_path)
            .with_context(|| format!("Reading fallback file {:?}", self.fallback_path))?;
        let store: FallbackStore = serde_json::from_str(&content).unwrap_or_default();
        Ok(store)
    }

    fn mutate_fallback<F: FnOnce(&mut FallbackStore)>(&self, f: F) -> Result<()> {
        let mut store = self.read_fallback().unwrap_or_default();
        f(&mut store);
        let content = serde_json::to_string_pretty(&store)?;
        fs::write(&self.fallback_path, content.as_bytes())?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if let Ok(meta) = fs::metadata(&self.fallback_path) {
                let mut perms = meta.permissions();
                perms.set_mode(0o600);
                let _ = fs::set_permissions(&self.fallback_path, perms);
            }
        }
        Ok(())
    }
}
