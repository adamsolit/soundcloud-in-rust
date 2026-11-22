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

        // 1. Try Firefox / Zen schema (moz_cookies table)
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

        // 2. Try Chromium schema (cookies table) - supporting both plaintext & AES encrypted_value
        if let Ok(mut stmt) = conn.prepare(
            "SELECT value, encrypted_value FROM cookies 
             WHERE host_key LIKE '%soundcloud.com%' 
             AND name = 'oauth_token' 
             ORDER BY creation_utc DESC LIMIT 1;",
        ) {
            if let Ok(row) = stmt.query_row([], |row| {
                let val: String = row.get(0).unwrap_or_default();
                let enc: Vec<u8> = row.get(1).unwrap_or_default();
                Ok((val, enc))
            }) {
                let (val, enc) = row;
                let clean = val.trim().to_string();
                if !clean.is_empty() {
                    return Some(clean);
                }
                if !enc.is_empty() {
                    if let Some(dec) = decrypt_chromium_cookie(&enc) {
                        return Some(dec);
                    }
                }
            }
        }

        None
    })();

    let _ = std::fs::remove_file(&temp_copy);
    result
}

fn decrypt_chromium_cookie(raw: &[u8]) -> Option<String> {
    if raw.len() <= 3 {
        return None;
    }

    // Modern Linux Chromium cookies start with b"v10" or b"v11"
    let ciphertext = if raw.starts_with(b"v10") || raw.starts_with(b"v11") {
        &raw[3..]
    } else {
        raw
    };

    let mut passwords: Vec<String> = Vec::new();
    for (service, user) in &[
        ("Chromium Safe Storage", "Chromium"),
        ("Chrome Safe Storage", "Chrome"),
        ("Brave Safe Storage", "Brave"),
    ] {
        if let Ok(entry) = keyring::Entry::new(service, user) {
            if let Ok(pwd) = entry.get_password() {
                if !pwd.is_empty() {
                    passwords.push(pwd);
                }
            }
        }
    }
    passwords.push("peanuts".to_string());

    let iv_hex = "20202020202020202020202020202020";

    for pwd in passwords {
        let key = derive_pbkdf2_sha1_16(pwd.as_bytes(), b"saltysalt");
        let key_hex = hex_encode(&key);

        let mut child = match std::process::Command::new("openssl")
            .args(["enc", "-d", "-aes-128-cbc", "-K", &key_hex, "-iv", iv_hex])
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .spawn()
        {
            Ok(c) => c,
            Err(_) => continue,
        };

        if let Some(mut stdin) = child.stdin.take() {
            use std::io::Write;
            let _ = stdin.write_all(ciphertext);
        }

        if let Ok(output) = child.wait_with_output() {
            if output.status.success() && !output.stdout.is_empty() {
                if let Ok(dec_str) = String::from_utf8(output.stdout) {
                    let clean = dec_str.trim().to_string();
                    if !clean.is_empty() && (clean.starts_with("2-") || clean.starts_with("0-") || clean.len() >= 10) {
                        return Some(clean);
                    }
                }
            }
        }
    }

    None
}

fn sha1(data: &[u8]) -> [u8; 20] {
    let mut h0 = 0x67452301u32;
    let mut h1 = 0xefcdab89u32;
    let mut h2 = 0x98badcfeu32;
    let mut h3 = 0x10325476u32;
    let mut h4 = 0xc3d2e1f0u32;

    let len = data.len();
    let mut msg = data.to_vec();
    msg.push(0x80);
    while (msg.len() % 64) != 56 {
        msg.push(0x00);
    }
    let bit_len = (len as u64) * 8;
    msg.extend_from_slice(&bit_len.to_be_bytes());

    for chunk in msg.chunks_exact(64) {
        let mut w = [0u32; 80];
        for i in 0..16 {
            w[i] = u32::from_be_bytes([chunk[i * 4], chunk[i * 4 + 1], chunk[i * 4 + 2], chunk[i * 4 + 3]]);
        }
        for i in 16..80 {
            w[i] = (w[i - 3] ^ w[i - 8] ^ w[i - 14] ^ w[i - 16]).rotate_left(1);
        }

        let mut a = h0;
        let mut b = h1;
        let mut c = h2;
        let mut d = h3;
        let mut e = h4;

        for i in 0..80 {
            let (f, k) = match i {
                0..=19 => ((b & c) | ((!b) & d), 0x5a827999u32),
                20..=39 => (b ^ c ^ d, 0x6ed9eba1u32),
                40..=59 => ((b & c) | (b & d) | (c & d), 0x8f1bbcdcu32),
                _ => (b ^ c ^ d, 0xca62c1d6u32),
            };
            let temp = a.rotate_left(5).wrapping_add(f).wrapping_add(e).wrapping_add(k).wrapping_add(w[i]);
            e = d;
            d = c;
            c = b.rotate_left(30);
            b = a;
            a = temp;
        }

        h0 = h0.wrapping_add(a);
        h1 = h1.wrapping_add(b);
        h2 = h2.wrapping_add(c);
        h3 = h3.wrapping_add(d);
        h4 = h4.wrapping_add(e);
    }

    let mut out = [0u8; 20];
    out[0..4].copy_from_slice(&h0.to_be_bytes());
    out[4..8].copy_from_slice(&h1.to_be_bytes());
    out[8..12].copy_from_slice(&h2.to_be_bytes());
    out[12..16].copy_from_slice(&h3.to_be_bytes());
    out[16..20].copy_from_slice(&h4.to_be_bytes());
    out
}

fn hmac_sha1(key: &[u8], msg: &[u8]) -> [u8; 20] {
    let mut k = [0u8; 64];
    if key.len() > 64 {
        let h = sha1(key);
        k[..20].copy_from_slice(&h);
    } else {
        k[..key.len()].copy_from_slice(key);
    }

    let mut i_key_pad = [0x36u8; 64];
    let mut o_key_pad = [0x5cu8; 64];
    for idx in 0..64 {
        i_key_pad[idx] ^= k[idx];
        o_key_pad[idx] ^= k[idx];
    }

    let mut inner = i_key_pad.to_vec();
    inner.extend_from_slice(msg);
    let inner_hash = sha1(&inner);

    let mut outer = o_key_pad.to_vec();
    outer.extend_from_slice(&inner_hash);
    sha1(&outer)
}

fn derive_pbkdf2_sha1_16(pwd: &[u8], salt: &[u8]) -> [u8; 16] {
    let mut salt_block = salt.to_vec();
    salt_block.extend_from_slice(&[0, 0, 0, 1]);
    let digest = hmac_sha1(pwd, &salt_block);
    let mut out = [0u8; 16];
    out.copy_from_slice(&digest[..16]);
    out
}

fn hex_encode(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        use std::fmt::Write;
        let _ = write!(s, "{:02x}", b);
    }
    s
}
