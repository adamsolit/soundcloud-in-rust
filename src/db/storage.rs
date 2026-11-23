use anyhow::{Context, Result};
use rusqlite::{params, Connection};
use std::fs;
use std::sync::{Arc, Mutex};
use tracing::{debug, info};

use crate::models::track::Track;

#[derive(Clone)]
pub struct Database {
    conn: Arc<Mutex<Connection>>,
}

impl Database {
    pub fn new() -> Result<Self> {
        let app_dir = crate::paths::data_dir();
        fs::create_dir_all(&app_dir)?;
        let db_path = app_dir.join("library.sqlite3");

        let conn = Connection::open(&db_path)
            .with_context(|| format!("Opening SQLite database at {:?}", db_path))?;

        // Enable WAL mode for high concurrency & low write latency
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "synchronous", "NORMAL")?;

        let db = Self {
            conn: Arc::new(Mutex::new(conn)),
        };

        db.init_schema()?;
        info!("SQLite database initialized at {:?}", db_path);
        Ok(db)
    }

    fn init_schema(&self) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS cached_tracks (
                id INTEGER PRIMARY KEY,
                title TEXT NOT NULL,
                artist TEXT NOT NULL,
                duration INTEGER NOT NULL,
                artwork_url TEXT,
                waveform_url TEXT,
                permalink_url TEXT,
                is_liked INTEGER DEFAULT 0,
                json_payload TEXT NOT NULL,
                cached_at DATETIME DEFAULT CURRENT_TIMESTAMP
            );

            CREATE TABLE IF NOT EXISTS playback_history (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                track_id INTEGER NOT NULL,
                played_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                FOREIGN KEY(track_id) REFERENCES cached_tracks(id)
            );

            CREATE TABLE IF NOT EXISTS offline_playlists (
                id INTEGER PRIMARY KEY,
                title TEXT NOT NULL,
                description TEXT,
                tracks_json TEXT NOT NULL,
                synced_at DATETIME DEFAULT CURRENT_TIMESTAMP
            );

            CREATE TABLE IF NOT EXISTS waveform_cache (
                track_id INTEGER PRIMARY KEY,
                samples_blob TEXT NOT NULL,
                updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
            );

            CREATE TABLE IF NOT EXISTS app_settings (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS bookmarks (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                track_id INTEGER NOT NULL,
                timestamp_ms INTEGER NOT NULL,
                note TEXT NOT NULL,
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP
            );

            CREATE TABLE IF NOT EXISTS stashed_tracks (
                track_id INTEGER PRIMARY KEY,
                file_path TEXT NOT NULL,
                file_size INTEGER NOT NULL,
                json_payload TEXT NOT NULL,
                stashed_at DATETIME DEFAULT CURRENT_TIMESTAMP
            );

            CREATE INDEX IF NOT EXISTS idx_history_played_at ON playback_history(played_at DESC);
            CREATE INDEX IF NOT EXISTS idx_cached_tracks_liked ON cached_tracks(is_liked);
            CREATE INDEX IF NOT EXISTS idx_bookmarks_track ON bookmarks(track_id);
            CREATE INDEX IF NOT EXISTS idx_stashed_tracks_at ON stashed_tracks(stashed_at DESC);
            "#,
        )?;
        Ok(())
    }

    pub fn cache_track(&self, track: &Track, is_liked: bool) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let payload = serde_json::to_string(track)?;
        conn.execute(
            r#"
            INSERT INTO cached_tracks (id, title, artist, duration, artwork_url, waveform_url, permalink_url, is_liked, json_payload, cached_at)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, CURRENT_TIMESTAMP)
            ON CONFLICT(id) DO UPDATE SET
                title = excluded.title,
                artist = excluded.artist,
                duration = excluded.duration,
                artwork_url = excluded.artwork_url,
                waveform_url = excluded.waveform_url,
                permalink_url = excluded.permalink_url,
                is_liked = excluded.is_liked,
                json_payload = excluded.json_payload,
                cached_at = CURRENT_TIMESTAMP
            "#,
            params![
                track.id as i64,
                track.title,
                track.artist_name(),
                track.duration as i64,
                track.artwork_url,
                track.waveform_url,
                track.permalink_url,
                if is_liked { 1 } else { 0 },
                payload
            ],
        )?;
        debug!("Cached track {}: {}", track.id, track.title);
        Ok(())
    }

    pub fn get_liked_tracks(&self) -> Result<Vec<Track>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT json_payload FROM cached_tracks WHERE is_liked = 1 ORDER BY cached_at DESC LIMIT 100",
        )?;
        let rows = stmt.query_map([], |row| {
            let json_str: String = row.get(0)?;
            Ok(json_str)
        })?;

        let mut tracks = Vec::new();
        for row in rows {
            let json_str = row?;
            if let Ok(track) = serde_json::from_str::<Track>(&json_str) {
                tracks.push(track);
            }
        }
        Ok(tracks)
    }

    pub fn set_track_liked(&self, track_id: u64, liked: bool) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE cached_tracks SET is_liked = ?1 WHERE id = ?2",
            params![if liked { 1 } else { 0 }, track_id as i64],
        )?;
        Ok(())
    }

    pub fn is_track_liked(&self, track_id: u64) -> bool {
        let conn = self.conn.lock().unwrap();
        conn.query_row(
            "SELECT is_liked FROM cached_tracks WHERE id = ?1",
            params![track_id as i64],
            |row| row.get::<_, i64>(0),
        )
        .map(|v| v == 1)
        .unwrap_or(false)
    }

    pub fn record_playback(&self, track_id: u64) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO playback_history (track_id, played_at) VALUES (?1, CURRENT_TIMESTAMP)",
            params![track_id as i64],
        )?;
        Ok(())
    }

    pub fn get_playback_history(&self, limit: usize) -> Result<Vec<Track>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            r#"
            SELECT c.json_payload
            FROM playback_history h
            JOIN cached_tracks c ON h.track_id = c.id
            ORDER BY h.played_at DESC
            LIMIT ?1
            "#,
        )?;
        let rows = stmt.query_map([limit as i64], |row| {
            let json_str: String = row.get(0)?;
            Ok(json_str)
        })?;

        let mut tracks = Vec::new();
        for row in rows {
            let json_str = row?;
            if let Ok(track) = serde_json::from_str::<Track>(&json_str) {
                tracks.push(track);
            }
        }
        Ok(tracks)
    }

    pub fn get_cached_waveform(&self, track_id: u64) -> Result<Option<Vec<f32>>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT samples_blob FROM waveform_cache WHERE track_id = ?1")?;
        let mut rows = stmt.query([track_id as i64])?;
        if let Some(row) = rows.next()? {
            let json_str: String = row.get(0)?;
            let samples: Vec<f32> = serde_json::from_str(&json_str)?;
            Ok(Some(samples))
        } else {
            Ok(None)
        }
    }

    pub fn save_cached_waveform(&self, track_id: u64, samples: &[f32]) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let json_str = serde_json::to_string(samples)?;
        conn.execute(
            r#"
            INSERT INTO waveform_cache (track_id, samples_blob, updated_at)
            VALUES (?1, ?2, CURRENT_TIMESTAMP)
            ON CONFLICT(track_id) DO UPDATE SET
                samples_blob = excluded.samples_blob,
                updated_at = CURRENT_TIMESTAMP
            "#,
            params![track_id as i64, json_str],
        )?;
        Ok(())
    }

    pub fn get_setting(&self, key: &str) -> Result<Option<String>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT value FROM app_settings WHERE key = ?1")?;
        let mut rows = stmt.query([key])?;
        if let Some(row) = rows.next()? {
            let val: String = row.get(0)?;
            Ok(Some(val))
        } else {
            Ok(None)
        }
    }

    pub fn set_setting(&self, key: &str, value: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            r#"
            INSERT INTO app_settings (key, value) VALUES (?1, ?2)
            ON CONFLICT(key) DO UPDATE SET value = excluded.value
            "#,
            params![key, value],
        )?;
        Ok(())
    }

    pub fn get_bool_setting(&self, key: &str, default: bool) -> bool {
        self.get_setting(key).ok().flatten().map(|s| s == "true" || s == "1").unwrap_or(default)
    }

    pub fn set_bool_setting(&self, key: &str, val: bool) -> Result<()> {
        self.set_setting(key, if val { "true" } else { "false" })
    }

    pub fn save_user_profile(&self, user: &crate::models::user::UserProfile) -> Result<()> {
        let json = serde_json::to_string(user)?;
        self.set_setting("current_user_profile", &json)
    }

    pub fn get_user_profile(&self) -> Result<Option<crate::models::user::UserProfile>> {
        if let Some(json) = self.get_setting("current_user_profile")? {
            if let Ok(profile) = serde_json::from_str(&json) {
                return Ok(Some(profile));
            }
        }
        Ok(None)
    }

    pub fn save_playback_state(&self, track: &Track, position_ms: u64) -> Result<()> {
        let state = SavedPlaybackState {
            track: track.clone(),
            position_ms,
        };
        let json = serde_json::to_string(&state)?;
        self.set_setting("last_playback_state", &json)
    }

    pub fn get_playback_state(&self) -> Result<Option<SavedPlaybackState>> {
        if let Some(json) = self.get_setting("last_playback_state")? {
            if let Ok(state) = serde_json::from_str(&json) {
                return Ok(Some(state));
            }
        }
        Ok(None)
    }

    pub fn get_user_top_genres(&self, limit: usize) -> Result<Vec<String>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            r#"
            SELECT json_payload FROM cached_tracks WHERE is_liked = 1
            UNION ALL
            SELECT c.json_payload
            FROM playback_history h
            JOIN cached_tracks c ON h.track_id = c.id
            LIMIT 100
            "#,
        )?;
        let rows = stmt.query_map([], |row| {
            let json_str: String = row.get(0)?;
            Ok(json_str)
        })?;

        let mut genre_counts: std::collections::HashMap<String, usize> = std::collections::HashMap::new();

        for row in rows {
            if let Ok(json_str) = row {
                if let Ok(val) = serde_json::from_str::<serde_json::Value>(&json_str) {
                    if let Some(g) = val.get("genre").and_then(|v| v.as_str()) {
                        let trimmed = g.trim();
                        if !trimmed.is_empty() && trimmed.len() > 2 && trimmed.to_lowercase() != "none" {
                            *genre_counts.entry(trimmed.to_string()).or_insert(0) += 3;
                        }
                    }
                    if let Some(tags) = val.get("tag_list").and_then(|v| v.as_str()) {
                        for tag in tags.split_whitespace() {
                            let clean = tag.trim_matches('"').trim_matches('\'').trim();
                            if clean.len() >= 3 && !clean.chars().all(|c| c.is_numeric()) {
                                *genre_counts.entry(clean.to_string()).or_insert(0) += 1;
                            }
                        }
                    }
                }
            }
        }

        let mut sorted_genres: Vec<(String, usize)> = genre_counts.into_iter().collect();
        sorted_genres.sort_by(|a, b| b.1.cmp(&a.1));

        let top: Vec<String> = sorted_genres
            .into_iter()
            .take(limit)
            .map(|(g, _)| g)
            .collect();

        Ok(top)
    }

    // -------------------------------------------------------------------------
    // Personal Timestamped Bookmarks
    // -------------------------------------------------------------------------

    pub fn add_bookmark(&self, track_id: u64, timestamp_ms: u64, note: &str) -> Result<i64> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO bookmarks (track_id, timestamp_ms, note, created_at) VALUES (?1, ?2, ?3, CURRENT_TIMESTAMP)",
            params![track_id as i64, timestamp_ms as i64, note],
        )?;
        Ok(conn.last_insert_rowid())
    }

    pub fn get_bookmarks(&self, track_id: u64) -> Result<Vec<Bookmark>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, track_id, timestamp_ms, note, created_at FROM bookmarks WHERE track_id = ?1 ORDER BY timestamp_ms ASC",
        )?;
        let rows = stmt.query_map([track_id as i64], |row| {
            Ok(Bookmark {
                id: row.get(0)?,
                track_id: row.get::<_, i64>(1)? as u64,
                timestamp_ms: row.get::<_, i64>(2)? as u64,
                note: row.get(3)?,
                created_at: row.get(4)?,
            })
        })?;

        let mut list = Vec::new();
        for r in rows {
            list.push(r?);
        }
        Ok(list)
    }

    pub fn delete_bookmark(&self, bookmark_id: i64) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM bookmarks WHERE id = ?1", params![bookmark_id])?;
        Ok(())
    }

    // -------------------------------------------------------------------------
    // Offline Stash (Local Caching)
    // -------------------------------------------------------------------------

    pub fn save_stashed_track(&self, track: &Track, file_path: &str, file_size: u64) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let payload = serde_json::to_string(track)?;
        conn.execute(
            r#"
            INSERT INTO stashed_tracks (track_id, file_path, file_size, json_payload, stashed_at)
            VALUES (?1, ?2, ?3, ?4, CURRENT_TIMESTAMP)
            ON CONFLICT(track_id) DO UPDATE SET
                file_path = excluded.file_path,
                file_size = excluded.file_size,
                json_payload = excluded.json_payload,
                stashed_at = CURRENT_TIMESTAMP
            "#,
            params![track.id as i64, file_path, file_size as i64, payload],
        )?;
        Ok(())
    }

    pub fn get_stashed_tracks(&self) -> Result<Vec<(Track, String, u64)>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT json_payload, file_path, file_size FROM stashed_tracks ORDER BY stashed_at DESC",
        )?;
        let rows = stmt.query_map([], |row| {
            let json_str: String = row.get(0)?;
            let file_path: String = row.get(1)?;
            let file_size: i64 = row.get(2)?;
            Ok((json_str, file_path, file_size as u64))
        })?;

        let mut list = Vec::new();
        for r in rows {
            let (json_str, file_path, file_size) = r?;
            if let Ok(track) = serde_json::from_str::<Track>(&json_str) {
                list.push((track, file_path, file_size));
            }
        }
        Ok(list)
    }

    pub fn is_track_stashed(&self, track_id: u64) -> bool {
        let conn = self.conn.lock().unwrap();
        conn.query_row(
            "SELECT file_path FROM stashed_tracks WHERE track_id = ?1",
            params![track_id as i64],
            |row| row.get::<_, String>(0),
        )
        .map(|path| std::path::Path::new(&path).exists())
        .unwrap_or(false)
    }

    pub fn get_stashed_file_path(&self, track_id: u64) -> Option<String> {
        let conn = self.conn.lock().unwrap();
        conn.query_row(
            "SELECT file_path FROM stashed_tracks WHERE track_id = ?1",
            params![track_id as i64],
            |row| row.get::<_, String>(0),
        )
        .ok()
        .filter(|p| std::path::Path::new(p).exists())
    }

    pub fn remove_stashed_track(&self, track_id: u64) -> Result<Option<String>> {
        let conn = self.conn.lock().unwrap();
        let file_path: Option<String> = conn
            .query_row(
                "SELECT file_path FROM stashed_tracks WHERE track_id = ?1",
                params![track_id as i64],
                |row| row.get(0),
            )
            .ok();

        conn.execute("DELETE FROM stashed_tracks WHERE track_id = ?1", params![track_id as i64])?;
        Ok(file_path)
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Bookmark {
    pub id: i64,
    pub track_id: u64,
    pub timestamp_ms: u64,
    pub note: String,
    pub created_at: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SavedPlaybackState {
    pub track: Track,
    pub position_ms: u64,
}
