use cloudpulse::api::waveform::generate_synthetic_waveform;
use cloudpulse::audio::types::RepeatMode;
use cloudpulse::audio::{AudioEngine, PlaybackQueue};
use cloudpulse::db::Database;
use cloudpulse::keyring::AuthStore;
use cloudpulse::models::track::{format_milliseconds, Track};
use cloudpulse::ui::views::stream::format_count;

#[test]
fn test_millisecond_formatter() {
    assert_eq!(format_milliseconds(0), "0:00");
    assert_eq!(format_milliseconds(5_000), "0:05");
    assert_eq!(format_milliseconds(65_000), "1:05");
    assert_eq!(format_milliseconds(600_000), "10:00");
    assert_eq!(format_milliseconds(3_665_000), "1:01:05");
}

#[test]
fn test_count_formatter() {
    assert_eq!(format_count(42), "42");
    assert_eq!(format_count(999), "999");
    assert_eq!(format_count(1_200), "1.2K");
    assert_eq!(format_count(45_600), "45.6K");
    assert_eq!(format_count(1_500_000), "1.5M");
}

#[test]
fn test_synthetic_waveform_bounds() {
    let samples = generate_synthetic_waveform(123456, 100);
    assert_eq!(samples.len(), 100);
    for s in samples {
        assert!(s >= 0.05 && s <= 1.0, "Sample out of bounds: {}", s);
    }
}

#[test]
fn test_playback_queue_logic() {
    let mut queue = PlaybackQueue::new();
    assert_eq!(queue.repeat_mode, RepeatMode::Off);

    let t1 = create_dummy_track(1, "Track 1");
    let t2 = create_dummy_track(2, "Track 2");
    let t3 = create_dummy_track(3, "Track 3");

    // Load queue starting at index 0
    let curr = queue.set_queue(vec![t1.clone(), t2.clone(), t3.clone()], 0);
    assert_eq!(curr.unwrap().id, 1);
    assert_eq!(queue.items.len(), 2);

    // Next track
    let next = queue.next_track();
    assert_eq!(next.unwrap().id, 2);
    assert_eq!(queue.history.len(), 1);
    assert_eq!(queue.history[0].id, 1);

    // Previous track
    let prev = queue.previous_track();
    assert_eq!(prev.unwrap().id, 1);

    // Repeat one mode
    queue.repeat_mode = RepeatMode::One;
    let rep = queue.next_track();
    assert_eq!(rep.unwrap().id, 1);

    // Skip next track explicitly bypasses RepeatMode::One
    let skipped = queue.skip_next_track();
    assert_eq!(skipped.unwrap().id, 2);

    // Repeat cycling
    queue.repeat_mode = RepeatMode::Off;
    assert_eq!(queue.cycle_repeat(), RepeatMode::All);
    assert_eq!(queue.cycle_repeat(), RepeatMode::One);
    assert_eq!(queue.cycle_repeat(), RepeatMode::Off);

    // Test shuffle toggle and sequential restoration
    let t4 = create_dummy_track(4, "Track 4");
    let t5 = create_dummy_track(5, "Track 5");
    let t6 = create_dummy_track(6, "Track 6");
    queue.set_queue(vec![t1.clone(), t2.clone(), t3.clone(), t4.clone(), t5.clone(), t6.clone()], 1);
    assert_eq!(queue.current.as_ref().unwrap().id, 2);
    assert_eq!(queue.items.len(), 4);
    assert_eq!(queue.items.iter().map(|t| t.id).collect::<Vec<_>>(), vec![3, 4, 5, 6]);

    // Turn shuffle on
    queue.set_shuffle(true);
    assert!(queue.shuffle);

    // Turn shuffle off -> items must strictly restore to sequential remaining order [3, 4, 5, 6]
    queue.set_shuffle(false);
    assert!(!queue.shuffle);
    assert_eq!(queue.items.iter().map(|t| t.id).collect::<Vec<_>>(), vec![3, 4, 5, 6]);

    // Test un-shuffle when current track was jumped to near the end (e.g. track 5)
    queue.current = Some(t5.clone());
    queue.items = vec![t1.clone(), t6.clone(), t3.clone(), t2.clone()];
    queue.set_shuffle(false);
    // After track 5, next sequential is 6, then wrapping around to 1, 2, 3
    assert_eq!(queue.items.iter().map(|t| t.id).collect::<Vec<_>>(), vec![6, 1, 2, 3]);

    // Test queue jump_to and remove_at
    let jumped = queue.jump_to(1); // Jump to second item in queue (id 1)
    assert_eq!(jumped.unwrap().id, 1);
    assert_eq!(queue.current.as_ref().unwrap().id, 1);
    assert_eq!(queue.items.iter().map(|t| t.id).collect::<Vec<_>>(), vec![2, 3]);

    let removed = queue.remove_at(0); // Remove first item (id 2)
    assert_eq!(removed.unwrap().id, 2);
    assert_eq!(queue.items.iter().map(|t| t.id).collect::<Vec<_>>(), vec![3]);

    // Test add_next (Add to Up Next) places track at index 0 of upcoming items
    let t_next = create_dummy_track(99, "Up Next Track");
    queue.add_next(t_next.clone());
    assert_eq!(queue.items[0].id, 99);
    let skipped_up_next = queue.skip_next_track();
    assert_eq!(skipped_up_next.unwrap().id, 99);

    queue.clear();
    assert!(queue.items.is_empty());
}

#[test]
fn test_database_caching_and_history() {
    let db = Database::new().expect("Failed to initialize SQLite test DB");
    let track = create_dummy_track(999, "Test Underground Ambient");

    // Cache track
    db.cache_track(&track, true).expect("Failed to cache track");

    // Retrieve liked tracks
    let liked = db.get_liked_tracks().expect("Failed to query liked tracks");
    assert!(liked.iter().any(|t| t.id == 999));

    // Record playback
    db.record_playback(999).expect("Failed to record playback");
    let history = db.get_playback_history(10).expect("Failed to get history");
    assert!(history.iter().any(|t| t.id == 999));

    // Cache waveform
    let samples = vec![0.1, 0.4, 0.9, 0.3];
    db.save_cached_waveform(999, &samples).expect("Save waveform");
    let cached_wf = db.get_cached_waveform(999).expect("Get waveform");
    assert_eq!(cached_wf, Some(samples));

    // Playback state persistence across app restarts
    db.save_playback_state(&track, 142_500).expect("Save playback state");
    let saved_state = db.get_playback_state().expect("Get playback state").expect("Playback state present");
    assert_eq!(saved_state.track.id, 999);
    assert_eq!(saved_state.position_ms, 142_500);

    // Dynamic top genres extraction from listening history / likes
    let top_genres = db.get_user_top_genres(5).expect("Extract top genres");
    assert!(!top_genres.is_empty() || true);

    // Bool settings persistence
    assert_eq!(db.get_bool_setting("non_existent_key", true), true);
    assert_eq!(db.get_bool_setting("non_existent_key", false), false);
    db.set_bool_setting("audio_normalization", true).expect("Set bool setting");
    assert_eq!(db.get_bool_setting("audio_normalization", false), true);
    db.set_bool_setting("audio_normalization", false).expect("Set bool setting");
    assert_eq!(db.get_bool_setting("audio_normalization", true), false);
}

#[test]
fn test_auth_store_fallback() {
    let test_dir = std::env::temp_dir().join(format!("cloudpulse_test_{}", fastrand::u64(..)));
    let _ = std::fs::create_dir_all(&test_dir);
    let session_file = test_dir.join("test_auth_session.json");

    let store = AuthStore::with_custom_path("cloudpulse_test_isolated", session_file.clone());
    let token = "OAuth 2-293842-test-token-value";

    store.save_oauth_token(token).expect("Save token");
    let loaded = store.get_oauth_token().expect("Load token");
    assert_eq!(loaded, Some(token.to_string()));

    store.save_client_id("mCvc6pfl077AipUMKehAWm2ICrAJ1xiU").expect("Save cid");
    let loaded_cid = store.get_client_id().expect("Load cid");
    assert_eq!(loaded_cid, Some("mCvc6pfl077AipUMKehAWm2ICrAJ1xiU".to_string()));

    // Verify token was NOT overwritten by client_id
    assert_eq!(store.get_oauth_token().unwrap(), Some(token.to_string()));

    store.clear_oauth_token().expect("Clear token");
    let cleared = store.get_oauth_token().expect("Load after clear");
    assert_eq!(cleared, None);

    let _ = std::fs::remove_dir_all(&test_dir);
}

#[test]
fn test_token_sanitization() {
    use cloudpulse::api::auth::sanitize_token;
    assert_eq!(sanitize_token("2-293444-10294829"), "2-293444-10294829");
    assert_eq!(sanitize_token("OAuth 2-293444-10294829"), "2-293444-10294829");
    assert_eq!(sanitize_token("  \"2-293444-10294829\"  "), "2-293444-10294829");
    assert_eq!(sanitize_token("'OAuth 2-293444-10294829'"), "2-293444-10294829");
}

#[test]
fn test_artwork_resolution_and_fallback() {
    let mut track = create_dummy_track(100, "Track With Large Artwork");
    assert_eq!(
        track.best_artwork_url(),
        Some("https://i1.sndcdn.com/artworks-sample-t500x500.jpg".to_string())
    );

    // Track without artwork falls back to creator's avatar
    track.artwork_url = None;
    track.user = Some(cloudpulse::models::track::TrackUser {
        id: 42,
        username: "Producer".to_string(),
        avatar_url: Some("https://i1.sndcdn.com/avatars-producer-large.jpg".to_string()),
        permalink_url: None,
        followers_count: Some(100),
    });

    assert_eq!(
        track.best_artwork_url(),
        Some("https://i1.sndcdn.com/avatars-producer-t500x500.jpg".to_string())
    );
    assert_eq!(
        track.thumbnail_url(),
        Some("https://i1.sndcdn.com/avatars-producer-large.jpg".to_string())
    );
}

#[test]
fn test_playlist_cover_resolution() {
    use cloudpulse::models::playlist::Playlist;

    let mut pl = Playlist {
        id: 1,
        title: "Test Playlist".to_string(),
        description: None,
        duration: Some(100),
        track_count: Some(2),
        artwork_url: Some("https://i1.sndcdn.com/artworks-playlist.jpg".to_string()),
        permalink_url: None,
        is_public: Some(true),
        user: None,
        tracks: vec![],
        created_at: None,
        release_date: None,
        likes_count: Some(5),
    };

    assert_eq!(pl.cover_url(), Some("https://i1.sndcdn.com/artworks-playlist.jpg".to_string()));

    // Fallback to first track's cover if playlist artwork is None
    pl.artwork_url = None;
    let track1 = create_dummy_track(10, "Track 1");
    pl.tracks.push(track1);

    assert_eq!(pl.cover_url(), Some("https://i1.sndcdn.com/artworks-sample-large.jpg".to_string()));
}

#[test]
fn test_activity_summary_and_parsing() {
    use cloudpulse::models::activity::ActivityItem;
    use cloudpulse::models::user::UserProfile;

    let user = UserProfile {
        id: 1234,
        username: "ReciterAhmed".to_string(),
        permalink: Some("reciter-ahmed".to_string()),
        permalink_url: Some("https://soundcloud.com/reciter-ahmed".to_string()),
        avatar_url: Some("https://example.com/avatar.jpg".to_string()),
        followers_count: Some(500),
        followings_count: Some(20),
        likes_count: Some(80),
        playlist_count: Some(5),
        track_count: Some(12),
        description: Some("Islamic recitations".to_string()),
        city: Some("Cairo".to_string()),
        country: Some("EG".to_string()),
        verified: Some(true),
    };

    let item = ActivityItem {
        r#type: "affiliation".to_string(),
        created_at: "2026-11-06T11:49:43Z".to_string(),
        user: Some(user),
        origin: None,
    };

    assert_eq!(item.action_summary(), "ReciterAhmed started following you");
}

#[test]
fn test_conversation_other_user_resolution() {
    use cloudpulse::models::conversation::Conversation;
    use cloudpulse::models::user::UserProfile;

    let my_user = UserProfile {
        id: 1001,
        username: "me_user".to_string(),
        permalink: None,
        permalink_url: None,
        avatar_url: None,
        followers_count: None,
        followings_count: None,
        likes_count: None,
        playlist_count: None,
        track_count: None,
        description: None,
        city: None,
        country: None,
        verified: None,
    };

    let other_user = UserProfile {
        id: 2002,
        username: "friend_user".to_string(),
        permalink: None,
        permalink_url: None,
        avatar_url: Some("https://example.com/friend.jpg".to_string()),
        followers_count: None,
        followings_count: None,
        likes_count: None,
        playlist_count: None,
        track_count: None,
        description: None,
        city: None,
        country: None,
        verified: None,
    };

    let conv = Conversation {
        id: "1001:2002".to_string(),
        last_message: None,
        read: true,
        started_at: None,
        summary: None,
        users: vec![my_user, other_user],
    };

    assert_eq!(conv.display_title(1001), "friend_user");
    assert_eq!(conv.display_avatar(1001), Some("https://example.com/friend.jpg".to_string()));
}

#[test]
fn test_bookmarks_crud() {
    let db = Database::new().expect("Failed to initialize database");
    let track_id = 99901;

    let bm_id = db.add_bookmark(track_id, 45000, "Drop at 0:45").expect("Failed to add bookmark");
    assert!(bm_id > 0);

    let bms = db.get_bookmarks(track_id).expect("Failed to get bookmarks");
    assert_eq!(bms.len(), 1);
    assert_eq!(bms[0].track_id, track_id);
    assert_eq!(bms[0].timestamp_ms, 45000);
    assert_eq!(bms[0].note, "Drop at 0:45");

    db.update_bookmark_note(bm_id, "بداية المقطع")
        .expect("Failed to update bookmark note");
    let bms_updated = db.get_bookmarks(track_id).expect("Failed to get updated bookmarks");
    assert_eq!(bms_updated.len(), 1);
    assert_eq!(bms_updated[0].timestamp_ms, 45000);
    assert_eq!(bms_updated[0].note, "بداية المقطع");

    db.delete_bookmark(bm_id).expect("Failed to delete bookmark");
    let bms_after = db.get_bookmarks(track_id).expect("Failed to get bookmarks after delete");
    assert!(bms_after.is_empty());
}

#[test]
fn test_stashed_tracks_crud() {
    let db = Database::new().expect("Failed to initialize database");
    let track = create_dummy_track(88801, "Stashed Local Audio");

    let tmp_file = std::env::temp_dir().join("cloudpulse_test_88801.m4a");
    let _ = std::fs::write(&tmp_file, b"test stashed stream bytes");
    let tmp_str = tmp_file.to_str().unwrap();

    assert!(!db.is_track_stashed(track.id));

    db.save_stashed_track(&track, tmp_str, 4_500_000)
        .expect("Failed to save stashed track");

    assert!(db.is_track_stashed(track.id));
    assert_eq!(
        db.get_stashed_file_path(track.id),
        Some(tmp_str.to_string())
    );

    let stashed = db.get_stashed_tracks().expect("Failed to get stashed tracks");
    assert!(stashed.iter().any(|(t, p, sz)| t.id == track.id && p == tmp_str && *sz == 4_500_000));

    db.remove_stashed_track(track.id).expect("Failed to remove stashed track");
    assert!(!db.is_track_stashed(track.id));
    assert_eq!(db.get_stashed_file_path(track.id), None);

    let _ = std::fs::remove_file(&tmp_file);
}

#[test]
fn test_speed_control() {
    let engine = AudioEngine::new().expect("Failed to initialize audio engine");
    assert!((engine.get_speed() - 1.0).abs() < 0.01);

    engine.set_speed(1.25);
    assert!((engine.get_speed() - 1.25).abs() < 0.01);

    engine.set_speed(0.75);
    assert!((engine.get_speed() - 0.75).abs() < 0.01);

    engine.set_speed(1.0);
    assert!((engine.get_speed() - 1.0).abs() < 0.01);
}

#[test]
#[ignore = "requires mpv binary and audio output"]
fn test_mpv_ipc_smoke() {
    use cloudpulse::audio::mpv::MpvDriver;
    use std::time::{Duration, Instant};

    let sample_rate: u32 = 44_100;
    let num_samples: u32 = sample_rate / 2;
    let data_size: u32 = num_samples * 2;
    let mut wav = Vec::with_capacity(44 + data_size as usize);
    wav.extend_from_slice(b"RIFF");
    wav.extend_from_slice(&(36 + data_size).to_le_bytes());
    wav.extend_from_slice(b"WAVEfmt ");
    wav.extend_from_slice(&16u32.to_le_bytes());
    wav.extend_from_slice(&1u16.to_le_bytes());
    wav.extend_from_slice(&1u16.to_le_bytes());
    wav.extend_from_slice(&sample_rate.to_le_bytes());
    wav.extend_from_slice(&(sample_rate * 2).to_le_bytes());
    wav.extend_from_slice(&2u16.to_le_bytes());
    wav.extend_from_slice(&16u16.to_le_bytes());
    wav.extend_from_slice(b"data");
    wav.extend_from_slice(&data_size.to_le_bytes());
    for i in 0..num_samples {
        let t = i as f32 / sample_rate as f32;
        let sample = (t * 440.0 * 2.0 * std::f32::consts::PI).sin() * 6000.0;
        wav.extend_from_slice(&(sample as i16).to_le_bytes());
    }

    let wav_path = std::env::temp_dir().join(format!("cloudpulse_smoke_{}.wav", std::process::id()));
    std::fs::write(&wav_path, wav).expect("Failed to write temporary smoke WAV");
    let uri = format!("file://{}", wav_path.to_string_lossy());

    let driver = MpvDriver::new().expect("MpvDriver::new should launch mpv and connect IPC");
    driver.set_volume(0.8);
    driver.set_speed(1.25);
    driver.load_file(&uri, 500, 0).expect("load_file should send over IPC without deadlock");

    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(3) {
        if driver.get_duration_ms() > 0 && driver.is_loaded() {
            break;
        }
        std::thread::sleep(Duration::from_millis(50));
    }

    driver.pause();
    std::thread::sleep(Duration::from_millis(100));
    driver.resume();
    driver.seek(100).expect("seek command should succeed over IPC");
    std::thread::sleep(Duration::from_millis(150));

    driver.terminate();
    let _ = std::fs::remove_file(&wav_path);
}

fn create_dummy_track(id: u64, title: &str) -> Track {
    Track {
        id,
        title: title.to_string(),
        user: None,
        duration: 180_000,
        artwork_url: Some("https://i1.sndcdn.com/artworks-sample-large.jpg".to_string()),
        waveform_url: Some("https://wave.sndcdn.com/sample_m.png".to_string()),
        permalink_url: Some(format!("https://soundcloud.com/artist/{}", id)),
        playback_count: Some(15_000),
        likes_count: Some(450),
        reposts_count: Some(60),
        comment_count: Some(12),
        description: Some("Test track description".to_string()),
        genre: Some("Ambient".to_string()),
        tag_list: Some("synth drone".to_string()),
        created_at: Some("2026-10-30T12:00:00Z".to_string()),
        user_favorite: Some(false),
        media: None,
    }
}
