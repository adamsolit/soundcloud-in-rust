use std::path::{Path, PathBuf};
use tracing::{debug, info};

/// Scans standard Linux & mounted profile locations for a SoundCloud `oauth_token` cookie.
pub fn scan_for_soundcloud_cookie() -> Option<String> {
    let mut candidate_dbs = Vec::new();

    let home_path = std::env::var_os("HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("USERPROFILE").map(PathBuf::from))
        .unwrap_or_else(|| PathBuf::from("."));

    // 1. Firefox profiles (standard ~/.mozilla/firefox and XDG ~/.config/mozilla/firefox)
    collect_sqlite_files(&home_path.join(".config").join("mozilla").join("firefox"), &mut candidate_dbs);
    collect_sqlite_files(&home_path.join(".mozilla").join("firefox"), &mut candidate_dbs);

    // 2. Zen Browser profiles
    collect_sqlite_files(&home_path.join(".config").join("zen"), &mut candidate_dbs);
    collect_sqlite_files(&home_path.join(".zen"), &mut candidate_dbs);

    // 3. Chromium / Chrome / Brave / Edge profiles
    collect_sqlite_files(&home_path.join(".config").join("chromium"), &mut candidate_dbs);
    collect_sqlite_files(&home_path.join(".config").join("google-chrome"), &mut candidate_dbs);
    collect_sqlite_files(&home_path.join(".config").join("BraveSoftware").join("Brave-Browser"), &mut candidate_dbs);

    // 4. Dual-boot Windows profiles if mounted
    for mount_root in &["/mnt/windows/Users", "/mnt/c/Users", "/media/windows/Users"] {
        let users_dir = Path::new(mount_root);
        if users_dir.is_dir() {
            if let Ok(entries) = std::fs::read_dir(users_dir) {
                for entry in entries.flatten() {
                    let roaming = entry.path().join("AppData").join("Roaming");
                    if roaming.is_dir() {
                        collect_sqlite_files(&roaming.join("zen").join("Profiles"), &mut candidate_dbs);
                        collect_sqlite_files(&roaming.join("Mozilla").join("Firefox").join("Profiles"), &mut candidate_dbs);
                    }
                }
            }
        }
    }

    debug!("Scanning {} cookie databases for soundcloud session...", candidate_dbs.len());

    for db_path in candidate_dbs {
        if let Some(token) = extract_token_from_cookies_db(&db_path) {
            info!("Successfully auto-detected SoundCloud session token from {:?}", db_path);
            return Some(token);
        }
    }

    None
}

fn collect_sqlite_files(base_dir: &Path, results: &mut Vec<PathBuf>) {
    if !base_dir.exists() {
        return;
    }
    if let Ok(entries) = std::fs::read_dir(base_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                let cookie_file = path.join("cookies.sqlite");
                if cookie_file.exists() {
                    results.push(cookie_file);
                }
                let chrome_cookies = path.join("Cookies");
                if chrome_cookies.exists() {
                    results.push(chrome_cookies);
                }
                // Recursively check subdirectories (e.g. Profile 1, Default)
                collect_sqlite_files(&path, results);
            } else if path.file_name().and_then(|n| n.to_str()) == Some("cookies.sqlite")
                || path.file_name().and_then(|n| n.to_str()) == Some("Cookies")
            {
                results.push(path);
            }
        }
    }
}

fn extract_token_from_cookies_db(path: &Path) -> Option<String> {
    let temp_copy = std::env::temp_dir().join(format!("cp_cookie_check_{}.sqlite", fastrand::u64(..)));
    if std::fs::copy(path, &temp_copy).is_err() {
        return None;
    }

    let result = (|| -> Option<String> {
        let conn = rusqlite::Connection::open_with_flags(
            &temp_copy,
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
        ).ok()?;

        // Try Firefox/Zen schema (moz_cookies)
        if let Ok(mut stmt) = conn.prepare(
            "SELECT value FROM moz_cookies 
             WHERE host LIKE '%soundcloud.com%' 
             AND name = 'oauth_token' 
             ORDER BY lastAccessed DESC LIMIT 1;",
        ) {
            if let Ok(token) = stmt.query_row([], |row| row.get::<_, String>(0)) {
                let clean = token.trim().to_string();
                if !clean.is_empty() {
                    return Some(clean);
                }
            }
        }

        // Try Chromium schema (cookies table)
        if let Ok(mut stmt) = conn.prepare(
            "SELECT value FROM cookies 
             WHERE host_key LIKE '%soundcloud.com%' 
             AND name = 'oauth_token' 
             ORDER BY creation_utc DESC LIMIT 1;",
        ) {
            if let Ok(token) = stmt.query_row([], |row| row.get::<_, String>(0)) {
                let clean = token.trim().to_string();
                if !clean.is_empty() {
                    return Some(clean);
                }
            }
        }

        None
    })();

    let _ = std::fs::remove_file(&temp_copy);
    result
}
