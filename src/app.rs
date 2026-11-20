use anyhow::Result;
use libadwaita as adw;
use libadwaita::prelude::*;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver};
use tracing::{error, info, warn};

use crate::api::SoundCloudClient;
use crate::audio::AudioEngine;
use crate::db::Database;
use crate::models::track::Track;
use crate::mpris::{MprisCommand, MprisService};
use crate::ui::window::MainWindow;

pub struct AppState {
    pub client: SoundCloudClient,
    pub audio: Arc<AudioEngine>,
    pub db: Database,
    pub mpris: Rc<RefCell<Option<MprisService>>>,
    pub current_track: Rc<RefCell<Option<Track>>>,
    pub pending_resume_pos: Rc<RefCell<Option<u64>>>,
    pub play_request_counter: std::sync::atomic::AtomicU64,
}

pub struct Application {
    pub state: Rc<AppState>,
    pub window: Rc<MainWindow>,
}

impl Application {
    pub fn new(app: &adw::Application) -> Result<Self> {
        let client = SoundCloudClient::new();
        let audio = Arc::new(AudioEngine::new()?);
        let db = Database::new()?;

        let state = Rc::new(AppState {
            client,
            audio,
            db,
            mpris: Rc::new(RefCell::new(None)),
            current_track: Rc::new(RefCell::new(None)),
            pending_resume_pos: Rc::new(RefCell::new(None)),
            play_request_counter: std::sync::atomic::AtomicU64::new(0),
        });

        let window = Rc::new(MainWindow::new(app));
        let application = Self { state, window };

        application.setup_mpris();
        application.setup_playback_sync();
        application.setup_audio_events();
        application.wire_player_bar();
        application.wire_views();
        application.load_initial_data();

        let audio_close = Arc::clone(&application.state.audio);
        let state_close = Rc::clone(&application.state);
        application.window.window().connect_close_request(move |_| {
            info!("Window close requested. Terminating CloudPulse completely...");
            if let Some(ref track) = *state_close.current_track.borrow() {
                let pos = audio_close.get_position_ms();
                let _ = state_close.db.save_playback_state(track, pos);
            }
            audio_close.terminate();
            std::process::exit(0);
        });

        // Register Ctrl+Q and Ctrl+W global accelerators to terminate completely
        use gtk4::gio::prelude::ActionMapExt;
        let action_quit = gtk4::gio::SimpleAction::new("quit", None);
        let audio_quit = Arc::clone(&application.state.audio);
        let state_quit = Rc::clone(&application.state);
        action_quit.connect_activate(move |_, _| {
            info!("Quit action triggered via accelerator. Terminating completely...");
            if let Some(ref track) = *state_quit.current_track.borrow() {
                let pos = audio_quit.get_position_ms();
                let _ = state_quit.db.save_playback_state(track, pos);
            }
            audio_quit.terminate();
            std::process::exit(0);
        });
        app.add_action(&action_quit);
        app.set_accels_for_action("app.quit", &["<Control>q", "<Control>w"]);

        // Register Ctrl+B and F9 global accelerators to toggle sidebar
        let action_toggle_sidebar = gtk4::gio::SimpleAction::new("toggle-sidebar", None);
        let sidebar_toggle_act = Rc::clone(&application.window.sidebar());
        let db_toggle = application.state.db.clone();
        action_toggle_sidebar.connect_activate(move |_, _| {
            sidebar_toggle_act.toggle();
            let state_str = if sidebar_toggle_act.is_collapsed() { "true" } else { "false" };
            let _ = db_toggle.set_setting("sidebar_collapsed", state_str);
        });
        app.add_action(&action_toggle_sidebar);
        app.set_accels_for_action("app.toggle-sidebar", &["<Control>b", "F9"]);

        // Persist collapse state on mouse click
        let db_col_persist = application.state.db.clone();
        application.window.sidebar().connect_collapse_changed(move |collapsed| {
            let state_str = if collapsed { "true" } else { "false" };
            let _ = db_col_persist.set_setting("sidebar_collapsed", state_str);
        });

        Ok(application)
    }

    fn setup_mpris(&self) {
        let (cmd_tx, mut cmd_rx): (_, UnboundedReceiver<MprisCommand>) = unbounded_channel();
        let mpris_holder = Rc::clone(&self.state.mpris);
        let audio_clone = Arc::clone(&self.state.audio);
        let state_clone = Rc::clone(&self.state);
        let app_win = Rc::clone(&self.window);

        // Spawn MPRIS listener on local async context
        glib::spawn_future_local(async move {
            match MprisService::start(cmd_tx).await {
                Ok(service) => {
                    *mpris_holder.borrow_mut() = Some(service);
                }
                Err(e) => {
                    warn!("MPRIS server start failed: {}", e);
                }
            }
        });

        // Handle MPRIS incoming commands from D-Bus / desktop media keys
        glib::spawn_future_local(async move {
            while let Some(cmd) = cmd_rx.recv().await {
                match cmd {
                    MprisCommand::Play => {
                        audio_clone.resume();
                        app_win.player_bar().update_playback_state(true);
                    }
                    MprisCommand::Pause => {
                        audio_clone.pause();
                        app_win.player_bar().update_playback_state(false);
                    }
                    MprisCommand::PlayPause => {
                        audio_clone.toggle_play_pause();
                        let playing = audio_clone.get_state() == crate::audio::PlaybackState::Playing;
                        app_win.player_bar().update_playback_state(playing);
                    }
                    MprisCommand::Next => {
                        let next = audio_clone.get_queue().lock().unwrap().next_track();
                        if let Some(t) = next {
                            let state_c = Rc::clone(&state_clone);
                            let win_c = Rc::clone(&app_win);
                            glib::spawn_future_local(async move {
                                let _ = play_track_internal(&state_c, &win_c, t).await;
                            });
                        }
                    }
                    MprisCommand::Previous => {
                        let prev = audio_clone.get_queue().lock().unwrap().previous_track();
                        if let Some(t) = prev {
                            let state_c = Rc::clone(&state_clone);
                            let win_c = Rc::clone(&app_win);
                            glib::spawn_future_local(async move {
                                let _ = play_track_internal(&state_c, &win_c, t).await;
                            });
                        }
                    }
                    MprisCommand::Stop => {
                        audio_clone.stop();
                        app_win.player_bar().update_playback_state(false);
                    }
                    MprisCommand::Seek(offset_ms) => {
                        let current_pos = audio_clone.get_position_ms();
                        let new_pos = (current_pos as i64 + offset_ms).max(0) as u64;
                        let _ = audio_clone.seek(new_pos);
                    }
                    MprisCommand::SetVolume(vol) => {
                        audio_clone.set_volume(vol);
                    }
                }
            }
        });
    }

    fn setup_playback_sync(&self) {
        let audio_c = Arc::clone(&self.state.audio);
        let win_c = Rc::clone(&self.window);
        let state_sync = Rc::clone(&self.state);
        let _mpris_holder = Rc::clone(&self.state.mpris);
        let sync_tick = Rc::new(RefCell::new(0u64));

        // Smooth UI timer for waveform progress and time display (80ms interval, avoids GPU bus overload)
        glib::timeout_add_local(Duration::from_millis(80), move || {
            let state = audio_c.get_state();
            let is_playing = state == crate::audio::PlaybackState::Playing;

            if is_playing {
                let pos = audio_c.get_position_ms();
                let dur = audio_c.get_duration_ms();
                win_c.player_bar().update_position(pos, dur);
                if win_c.track_detail_view.widget().is_visible() {
                    win_c.track_detail_view.update_position(pos, dur);
                }

                // Auto-save playback state every ~5 seconds (60 ticks * 80ms = 4.8s)
                let mut tick = sync_tick.borrow_mut();
                *tick += 1;
                if *tick % 60 == 0 && pos > 0 {
                    if let Some(ref track) = *state_sync.current_track.borrow() {
                        let _ = state_sync.db.save_playback_state(track, pos);
                    }
                }
            }

            glib::ControlFlow::Continue
        });
    }

    fn setup_audio_events(&self) {
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<crate::audio::AudioEvent>();
        self.state.audio.set_callback(Arc::new(move |evt| {
            let _ = tx.send(evt);
        }));

        let state_c = Rc::clone(&self.state);
        let win_c = Rc::clone(&self.window);

        glib::spawn_future_local(async move {
            while let Some(evt) = rx.recv().await {
                match evt {
                    crate::audio::AudioEvent::TrackEnded => {
                        let next = state_c.audio.get_queue().lock().unwrap().next_track();
                        if let Some(track) = next {
                            let _ = play_track_internal(&state_c, &win_c, track).await;
                        } else {
                            win_c.player_bar().update_playback_state(false);
                        }
                    }
                    crate::audio::AudioEvent::StateChanged(st) => {
                        let is_playing = st == crate::audio::PlaybackState::Playing;
                        win_c.player_bar().update_playback_state(is_playing);
                    }
                    _ => {}
                }
            }
        });
    }

    fn wire_player_bar(&self) {
        let player_bar = self.window.player_bar();
        let audio_c = Arc::clone(&self.state.audio);
        let win_c = Rc::clone(&self.window);
        let state_c = Rc::clone(&self.state);

        // Play / Pause toggle
        let audio_pp = Arc::clone(&audio_c);
        let win_pp = Rc::clone(&win_c);
        let state_pp = Rc::clone(&state_c);
        let mpris_pp = Rc::clone(&self.state.mpris);
        player_bar.connect_play_pause(move || {
            let st = audio_pp.get_state();
            let needs_stream = !audio_pp.is_loaded()
                || st == crate::audio::PlaybackState::Stopped
                || state_pp.pending_resume_pos.borrow().is_some();

            if needs_stream {
                if let Some(ref track) = *state_pp.current_track.borrow() {
                    let s = Rc::clone(&state_pp);
                    let w = Rc::clone(&win_pp);
                    let t = track.clone();
                    let resume_pos = state_pp.pending_resume_pos.borrow_mut().take().unwrap_or(0);
                    glib::spawn_future_local(async move {
                        let _ = play_track_internal_with_seek(&s, &w, t, resume_pos).await;
                    });
                    return;
                }
            }

            audio_pp.toggle_play_pause();
            let playing = audio_pp.get_state() == crate::audio::PlaybackState::Playing;
            win_pp.player_bar().update_playback_state(playing);

            if !playing {
                if let Some(ref track) = *state_pp.current_track.borrow() {
                    let pos = audio_pp.get_position_ms();
                    let _ = state_pp.db.save_playback_state(track, pos);
                }
            }

            if let Some(ref m) = *mpris_pp.borrow() {
                m.handler.set_playback_status(if playing {
                    mpris_server::PlaybackStatus::Playing
                } else {
                    mpris_server::PlaybackStatus::Paused
                });
            }
        });

        // Seek from full-width waveform
        let audio_seek = Arc::clone(&audio_c);
        let state_seek = Rc::clone(&state_c);
        let win_seek = Rc::clone(&win_c);
        player_bar.connect_seek(move |pct| {
            if !audio_seek.is_loaded() || state_seek.pending_resume_pos.borrow().is_some() {
                if let Some(ref track) = *state_seek.current_track.borrow() {
                    let target_ms = (pct * track.duration as f64) as u64;
                    win_seek.player_bar().update_position(target_ms, track.duration);
                    let s = Rc::clone(&state_seek);
                    let w = Rc::clone(&win_seek);
                    let t = track.clone();
                    state_seek.pending_resume_pos.borrow_mut().take();
                    glib::spawn_future_local(async move {
                        let _ = play_track_internal_with_seek(&s, &w, t, target_ms).await;
                    });
                    return;
                }
            }

            let dur = audio_seek.get_duration_ms();
            if dur > 0 {
                let target_ms = (pct * dur as f64) as u64;
                let _ = audio_seek.seek(target_ms);
            }
        });

        // Volume control
        let audio_vol = Arc::clone(&audio_c);
        let mpris_vol = Rc::clone(&self.state.mpris);
        player_bar.connect_volume(move |vol| {
            audio_vol.set_volume(vol);
            if let Some(ref m) = *mpris_vol.borrow() {
                m.handler.set_volume(vol);
            }
        });

        // Next Track (skips past RepeatMode::One when explicitly triggered by user)
        let audio_next = Arc::clone(&audio_c);
        let state_next = Rc::clone(&state_c);
        let win_next = Rc::clone(&win_c);
        player_bar.connect_next(move || {
            let next = audio_next.get_queue().lock().unwrap().skip_next_track();
            if let Some(t) = next {
                let s_c = Rc::clone(&state_next);
                let w_c = Rc::clone(&win_next);
                glib::spawn_future_local(async move {
                    let _ = play_track_internal(&s_c, &w_c, t).await;
                });
            }
        });

        // Previous Track
        let audio_prev = Arc::clone(&audio_c);
        let state_prev = Rc::clone(&state_c);
        let win_prev = Rc::clone(&win_c);
        player_bar.connect_prev(move || {
            let prev = audio_prev.get_queue().lock().unwrap().previous_track();
            if let Some(t) = prev {
                let s_c = Rc::clone(&state_prev);
                let w_c = Rc::clone(&win_prev);
                glib::spawn_future_local(async move {
                    let _ = play_track_internal(&s_c, &w_c, t).await;
                });
            }
        });

        // Shuffle
        let audio_shuf = Arc::clone(&audio_c);
        let win_shuf = Rc::clone(&win_c);
        player_bar.connect_shuffle(move || {
            let active = audio_shuf.get_queue().lock().unwrap().toggle_shuffle();
            win_shuf.library_view.playlist_view.set_shuffle_active(active);
            active
        });

        // Repeat
        let audio_rep = Arc::clone(&audio_c);
        player_bar.connect_repeat(move || {
            audio_rep.get_queue().lock().unwrap().cycle_repeat()
        });

        // Like / Unlike from player bar
        let state_like = Rc::clone(&state_c);
        player_bar.connect_like(move |liked| {
            if let Some(ref track) = *state_like.current_track.borrow() {
                let track_id = track.id;
                let client = state_like.client.clone();
                let db = state_like.db.clone();
                glib::spawn_future_local(async move {
                    let _ = client.toggle_like(track_id, liked).await;
                    let _ = db.set_track_liked(track_id, liked);
                });
            }
        });
    }

    fn wire_views(&self) {
        let state_c = Rc::clone(&self.state);
        let win_c = Rc::clone(&self.window);

        // 1. Stream / Discover View
        let s1 = Rc::clone(&state_c);
        let w1 = Rc::clone(&win_c);
        self.window.stream_view.connect_play_track(move |track| {
            let s = Rc::clone(&s1);
            let w = Rc::clone(&w1);
            glib::spawn_future_local(async move {
                let _ = play_track_internal(&s, &w, track).await;
            });
        });

        let s_sel = Rc::clone(&state_c);
        let w_sel = Rc::clone(&win_c);
        self.window.stream_view.connect_select_track(move |track| {
            show_track_detail_internal(&s_sel, &w_sel, track);
        });

        let s_genre = Rc::clone(&state_c);
        let w_genre = Rc::clone(&win_c);
        self.window.stream_view.connect_genre_change(move |genre| {
            let s = Rc::clone(&s_genre);
            let w = Rc::clone(&w_genre);
            glib::spawn_future_local(async move {
                w.stream_view.set_loading(true);
                match s.client.get_charts(&genre, 25, 0).await {
                    Ok(tracks) => {
                        w.stream_view.set_tracks(tracks);
                    }
                    Err(e) => {
                        warn!("Failed to fetch genre {}: {}", genre, e);
                    }
                }
                w.stream_view.set_loading(false);
            });
        });

        // 2. Search View
        let s_search = Rc::clone(&state_c);
        let w_search = Rc::clone(&win_c);
        self.window.search_view.connect_search(move |query, filter| {
            let s = Rc::clone(&s_search);
            let w = Rc::clone(&w_search);
            glib::spawn_future_local(async move {
                w.search_view.set_loading(true);
                match s.client.search(&query, filter, 30).await {
                    Ok(results) => {
                        w.search_view.set_results(results);
                    }
                    Err(e) => {
                        warn!("Search failed: {}", e);
                    }
                }
                w.search_view.set_loading(false);
            });
        });

        let s_srch_play = Rc::clone(&state_c);
        let w_srch_play = Rc::clone(&win_c);
        self.window.search_view.connect_play_track(move |track| {
            let s = Rc::clone(&s_srch_play);
            let w = Rc::clone(&w_srch_play);
            glib::spawn_future_local(async move {
                let _ = play_track_internal(&s, &w, track).await;
            });
        });

        let s_srch_sel = Rc::clone(&state_c);
        let w_srch_sel = Rc::clone(&win_c);
        self.window.search_view.connect_select_track(move |track| {
            show_track_detail_internal(&s_srch_sel, &w_srch_sel, track);
        });

        let s_srch_usr = Rc::clone(&state_c);
        let w_srch_usr = Rc::clone(&win_c);
        self.window.search_view.connect_select_user(move |user_id| {
            show_artist_profile_internal(&s_srch_usr, &w_srch_usr, user_id);
        });

        // 2b. Artist Profile View
        let s_art_play = Rc::clone(&state_c);
        let w_art_play = Rc::clone(&win_c);
        self.window.artist_profile_view.connect_play_track(move |track| {
            let s = Rc::clone(&s_art_play);
            let w = Rc::clone(&w_art_play);
            glib::spawn_future_local(async move {
                let _ = play_track_internal(&s, &w, track).await;
            });
        });

        let s_art_all = Rc::clone(&state_c);
        let w_art_all = Rc::clone(&win_c);
        self.window.artist_profile_view.connect_play_all(move |tracks| {
            if tracks.is_empty() {
                return;
            }
            let s = Rc::clone(&s_art_all);
            let w = Rc::clone(&w_art_all);
            let first = tracks[0].clone();
            let remaining = tracks[1..].to_vec();
            {
                let q_arc = s.audio.get_queue();
                let mut q = q_arc.lock().unwrap();
                q.items = remaining;
            }
            glib::spawn_future_local(async move {
                let _ = play_track_internal(&s, &w, first).await;
            });
        });

        let s_art_sel = Rc::clone(&state_c);
        let w_art_sel = Rc::clone(&win_c);
        self.window.artist_profile_view.connect_select_track(move |track| {
            show_track_detail_internal(&s_art_sel, &w_art_sel, track);
        });

        let s_art_fol = Rc::clone(&state_c);
        self.window.artist_profile_view.connect_toggle_follow(move |uid, follow| {
            let s = Rc::clone(&s_art_fol);
            glib::spawn_future_local(async move {
                let _ = s.client.follow_user(uid, follow).await;
            });
        });

        let w_art_back = Rc::clone(&win_c);
        self.window.artist_profile_view.connect_back(move || {
            w_art_back.go_back();
        });

        // 3. Library View
        let s_lib_play = Rc::clone(&state_c);
        let w_lib_play = Rc::clone(&win_c);
        self.window.library_view.connect_play_track(move |track| {
            let s = Rc::clone(&s_lib_play);
            let w = Rc::clone(&w_lib_play);
            glib::spawn_future_local(async move {
                let _ = play_track_internal(&s, &w, track).await;
            });
        });

        let s_lib_tab = Rc::clone(&state_c);
        let w_lib_tab = Rc::clone(&win_c);
        self.window.library_view.connect_tab_change(move |tab| {
            match tab {
                crate::ui::views::LibraryTab::Likes => {
                    if let Ok(tracks) = s_lib_tab.db.get_liked_tracks() {
                        w_lib_tab.library_view.set_tracks(tracks);
                    }
                }
                crate::ui::views::LibraryTab::Playlists => {
                    let s = Rc::clone(&s_lib_tab);
                    let w = Rc::clone(&w_lib_tab);
                    glib::spawn_future_local(async move {
                        if let Ok(pls) = s.client.get_my_playlists(30).await {
                            w.library_view.set_playlists(pls);
                        }
                    });
                }
                crate::ui::views::LibraryTab::History => {
                    if let Ok(tracks) = s_lib_tab.db.get_playback_history(50) {
                        w_lib_tab.library_view.set_tracks(tracks);
                    }
                }
            }
        });

        // 4. Playlist View (inside Library)
        let s_pl_play = Rc::clone(&state_c);
        let w_pl_play = Rc::clone(&win_c);
        self.window.library_view.playlist_view.connect_play_track(move |track| {
            let s = Rc::clone(&s_pl_play);
            let w = Rc::clone(&w_pl_play);
            glib::spawn_future_local(async move {
                let _ = play_track_internal(&s, &w, track).await;
            });
        });

        let s_pl_q = Rc::clone(&state_c);
        let w_pl_q = Rc::clone(&win_c);
        self.window.library_view.playlist_view.connect_play_queue(move |tracks, start_idx| {
            if tracks.is_empty() {
                return;
            }
            let s = Rc::clone(&s_pl_q);
            let w = Rc::clone(&w_pl_q);

            // Set up queue using set_queue so original order is preserved and shuffle state is respected
            let first_track = s.audio.get_queue().lock().unwrap().set_queue(tracks, start_idx);

            if let Some(track) = first_track {
                glib::spawn_future_local(async move {
                    let _ = play_track_internal(&s, &w, track).await;
                });
            }
        });

        let audio_pl_shuf = Arc::clone(&self.state.audio);
        let win_pl_shuf = Rc::clone(&self.window);
        self.window.library_view.playlist_view.connect_shuffle_toggled(move |active| {
            audio_pl_shuf.get_queue().lock().unwrap().set_shuffle(active);
            win_pl_shuf.player_bar().set_shuffle(active);
        });

        let s_pl_sel = Rc::clone(&state_c);
        let w_pl_sel = Rc::clone(&win_c);
        self.window.library_view.playlist_view.connect_select_playlist(move |pl_id| {
            let s = Rc::clone(&s_pl_sel);
            let w = Rc::clone(&w_pl_sel);
            glib::spawn_future_local(async move {
                w.library_view.playlist_view.set_loading(true);
                match s.client.get_playlist_details(pl_id).await {
                    Ok(detail) => {
                        w.library_view.playlist_view.open_playlist_detail(detail);
                    }
                    Err(e) => {
                        warn!("Failed to load playlist {}: {}", pl_id, e);
                    }
                }
                w.library_view.playlist_view.set_loading(false);
            });
        });

        let s_pl_create = Rc::clone(&state_c);
        let w_pl_create = Rc::clone(&win_c);
        self.window.library_view.playlist_view.connect_create_playlist(move |payload| {
            let s = Rc::clone(&s_pl_create);
            let w = Rc::clone(&w_pl_create);
            glib::spawn_future_local(async move {
                match s.client.create_playlist(payload).await {
                    Ok(pl) => {
                        info!("Created playlist: {}", pl.title);
                        if let Ok(pls) = s.client.get_my_playlists(30).await {
                            w.library_view.set_playlists(pls);
                        }
                    }
                    Err(e) => {
                        warn!("Could not create playlist: {}", e);
                    }
                }
            });
        });

        // 5. Track Detail View
        let s_td_play = Rc::clone(&state_c);
        let w_td_play = Rc::clone(&win_c);
        self.window.track_detail_view.connect_play_track(move |track| {
            let s = Rc::clone(&s_td_play);
            let w = Rc::clone(&w_td_play);
            glib::spawn_future_local(async move {
                let _ = play_track_internal(&s, &w, track).await;
            });
        });

        let audio_td_seek = Arc::clone(&self.state.audio);
        self.window.track_detail_view.connect_seek(move |pct| {
            let dur = audio_td_seek.get_duration_ms();
            if dur > 0 {
                let target_ms = (pct * dur as f64) as u64;
                let _ = audio_td_seek.seek(target_ms);
            }
        });

        let s_td_post = Rc::clone(&state_c);
        let w_td_post = Rc::clone(&win_c);
        self.window
            .track_detail_view
            .connect_post_comment(move |track_id, body, timestamp_ms| {
                let s = Rc::clone(&s_td_post);
                let w = Rc::clone(&w_td_post);
                glib::spawn_future_local(async move {
                    match s.client.post_comment(track_id, &body, timestamp_ms).await {
                        Ok(_) => {
                            // Refresh comments
                            if let Ok(comments) = s.client.get_comments(track_id, 50).await {
                                let dur = s.audio.get_duration_ms();
                                w.track_detail_view.set_comments(comments, dur);
                            }
                        }
                        Err(e) => {
                            warn!("Failed to post comment: {}", e);
                        }
                    }
                });
            });

        let s_td_usr = Rc::clone(&state_c);
        let w_td_usr = Rc::clone(&win_c);
        self.window.track_detail_view.connect_select_user(move |user_id| {
            show_artist_profile_internal(&s_td_usr, &w_td_usr, user_id);
        });

        // 6. Upload View
        let s_upload = Rc::clone(&state_c);
        let w_upload = Rc::clone(&win_c);
        self.window.upload_view.connect_upload(move |metadata| {
            let s = Rc::clone(&s_upload);
            let w = Rc::clone(&w_upload);
            glib::spawn_future_local(async move {
                let (prog_tx, mut prog_rx) = tokio::sync::mpsc::unbounded_channel();
                let w_p = Rc::clone(&w);
                glib::spawn_future_local(async move {
                    while let Some(prog) = prog_rx.recv().await {
                        w_p.upload_view.set_progress(prog);
                    }
                });

                let res = s
                    .client
                    .upload_track(metadata, move |prog| {
                        let _ = prog_tx.send(prog);
                    })
                    .await;

                match res {
                    Ok(id) => {
                        info!("Upload finished with track ID {}", id);
                    }
                    Err(e) => {
                        error!("Track upload failed: {}", e);
                        w.upload_view.set_error(&e.to_string());
                    }
                }
            });
        });

        // 7. Settings View
        let s_tok = Rc::clone(&state_c);
        let w_tok = Rc::clone(&win_c);
        self.window.settings_view.connect_token(move |tok| {
            let s = Rc::clone(&s_tok);
            let w = Rc::clone(&w_tok);
            glib::spawn_future_local(async move {
                w.settings_view.set_loading(true);
                w.settings_view.set_status("Verifying token with SoundCloud /me...", false);

                let _ = s.client.session.set_credentials(None, Some(tok)).await;
                match s.client.get_current_user().await {
                    Ok(user) => {
                        w.settings_view.set_user_profile(Some(&user));
                        w.update_user_profile(Some(&user));
                        w.settings_view.set_status(&format!("Connected successfully as @{}!", user.username), false);
                        let _ = sync_user_data_internal(&s, &w).await;
                    }
                    Err(e) => {
                        let _ = s.client.session.clear_credentials().await;
                        w.settings_view.set_user_profile(None);
                        w.update_user_profile(None);
                        w.settings_view.set_status(&format!("Authentication failed: {}. Please verify that your oauth_token is valid.", e), true);
                    }
                }
                w.settings_view.set_loading(false);
            });
        });

        let s_auto = Rc::clone(&state_c);
        let w_auto = Rc::clone(&win_c);
        self.window.settings_view.connect_auto_detect(move || {
            let s = Rc::clone(&s_auto);
            let w = Rc::clone(&w_auto);
            glib::spawn_future_local(async move {
                w.settings_view.set_status("Scanning browser profiles for soundcloud.com cookies...", false);
                if let Some(token) = crate::api::browser_cookies::scan_for_soundcloud_cookie() {
                    w.settings_view.set_token_input(&token);
                    w.settings_view.set_status("Found session cookie! Verifying with SoundCloud...", false);

                    let _ = s.client.session.set_credentials(None, Some(token)).await;
                    match s.client.get_current_user().await {
                        Ok(user) => {
                            w.settings_view.set_user_profile(Some(&user));
                            w.update_user_profile(Some(&user));
                            w.settings_view.set_status(&format!("Auto-detected and connected as @{}!", user.username), false);
                            let _ = sync_user_data_internal(&s, &w).await;
                        }
                        Err(e) => {
                            w.settings_view.set_status(&format!("Cookie expired or invalid: {}", e), true);
                        }
                    }
                } else {
                    w.settings_view.set_status(
                        "No active soundcloud.com cookie found in local browser profiles. Please sign in to soundcloud.com in your browser first, or paste the oauth_token manually.",
                        true,
                    );
                }
            });
        });

        let s_signout = Rc::clone(&state_c);
        let w_signout = Rc::clone(&win_c);
        self.window.settings_view.connect_sign_out(move || {
            let s = Rc::clone(&s_signout);
            let w = Rc::clone(&w_signout);
            glib::spawn_future_local(async move {
                let _ = s.client.session.clear_credentials().await;
                w.settings_view.set_user_profile(None);
                w.update_user_profile(None);
                w.settings_view.set_status("Signed out. Session credentials cleared from keyring.", false);
            });
        });

        let s_sync = Rc::clone(&state_c);
        let w_sync = Rc::clone(&win_c);
        self.window.settings_view.connect_sync_library(move || {
            let s = Rc::clone(&s_sync);
            let w = Rc::clone(&w_sync);
            glib::spawn_future_local(async move {
                let _ = sync_user_data_internal(&s, &w).await;
            });
        });

        let s_cid = Rc::clone(&state_c);
        let w_cid = Rc::clone(&win_c);
        self.window.settings_view.connect_save_cid(move |cid| {
            let s = Rc::clone(&s_cid);
            let w = Rc::clone(&w_cid);
            glib::spawn_future_local(async move {
                let _ = s.client.session.set_credentials(Some(cid), None).await;
                w.settings_view.set_status("Client ID saved to system keyring.", false);
            });
        });

        let s_disc = Rc::clone(&state_c);
        let w_disc = Rc::clone(&win_c);
        self.window.settings_view.connect_discover_cid(move || {
            let s = Rc::clone(&s_disc);
            let w = Rc::clone(&w_disc);
            glib::spawn_future_local(async move {
                w.settings_view.set_loading(true);
                match s.client.session.discover_client_id(&reqwest::Client::new()).await {
                    Ok(cid) => {
                        w.settings_view.set_client_id(&cid);
                        w.settings_view.set_status(&format!("Discovered valid Client ID: {}", cid), false);
                    }
                    Err(e) => {
                        w.settings_view.set_status(&format!("Discovery failed: {}", e), true);
                    }
                }
                w.settings_view.set_loading(false);
            });
        });

        let w_clear = Rc::clone(&win_c);
        self.window.settings_view.connect_clear_cache(move || {
            let cache_dir = crate::ui::image_loader::get_cache_dir();
            let _ = std::fs::remove_dir_all(&cache_dir);
            let _ = std::fs::create_dir_all(&cache_dir);
            w_clear.settings_view.set_status("Artwork cache and temporary files cleared.", false);
        });

        // 8. Notifications View
        let s_notif = Rc::clone(&state_c);
        let w_notif = Rc::clone(&win_c);
        self.window.notifications_view.connect_refresh(move || {
            let s = Rc::clone(&s_notif);
            let w = Rc::clone(&w_notif);
            glib::spawn_future_local(async move {
                w.notifications_view.set_loading(true);
                if let Ok(acts) = s.client.get_activities(30).await {
                    w.notifications_view.set_activities(acts);
                }
                w.notifications_view.set_loading(false);
            });
        });

        let s_notif_play = Rc::clone(&state_c);
        let w_notif_play = Rc::clone(&win_c);
        self.window.notifications_view.connect_play_track(move |track| {
            let s = Rc::clone(&s_notif_play);
            let w = Rc::clone(&w_notif_play);
            glib::spawn_future_local(async move {
                let _ = play_track_internal(&s, &w, track).await;
            });
        });

        // 9. Messages View
        let s_msg_ref = Rc::clone(&state_c);
        let w_msg_ref = Rc::clone(&win_c);
        self.window.messages_view.connect_refresh(move || {
            let s = Rc::clone(&s_msg_ref);
            let w = Rc::clone(&w_msg_ref);
            glib::spawn_future_local(async move {
                w.messages_view.set_loading(true);
                if let Ok(convs) = s.client.get_conversations(30).await {
                    w.messages_view.set_conversations(convs);
                }
                w.messages_view.set_loading(false);
            });
        });

        let s_msg_sel = Rc::clone(&state_c);
        let w_msg_sel = Rc::clone(&win_c);
        self.window.messages_view.connect_select_conversation(move |other_user_id| {
            let s = Rc::clone(&s_msg_sel);
            let w = Rc::clone(&w_msg_sel);
            glib::spawn_future_local(async move {
                w.messages_view.set_loading(true);
                if let Ok(msgs) = s.client.get_conversation_messages(other_user_id, 40).await {
                    w.messages_view.set_messages(msgs);
                }
                w.messages_view.set_loading(false);
            });
        });

        let s_msg_send = Rc::clone(&state_c);
        let w_msg_send = Rc::clone(&win_c);
        self.window.messages_view.connect_send_message(move |other_user_id, text: String| {
            let s = Rc::clone(&s_msg_send);
            let w = Rc::clone(&w_msg_send);
            glib::spawn_future_local(async move {
                w.messages_view.set_loading(true);
                if let Ok(_) = s.client.send_conversation_message(other_user_id, &text).await {
                    if let Ok(msgs) = s.client.get_conversation_messages(other_user_id, 40).await {
                        w.messages_view.set_messages(msgs);
                    }
                }
                w.messages_view.set_loading(false);
            });
        });

        // 10. Profile Update
        let s_prof = Rc::clone(&state_c);
        let w_prof = Rc::clone(&win_c);
        self.window.settings_view.connect_update_profile(move |username, bio, city, country, avatar_path| {
            let s = Rc::clone(&s_prof);
            let w = Rc::clone(&w_prof);
            glib::spawn_future_local(async move {
                w.settings_view.set_loading(true);
                w.settings_view.set_status("Updating profile info on SoundCloud...", false);

                if let Some(ref av_p) = avatar_path {
                    if let Err(e) = s.client.upload_avatar(av_p).await {
                        warn!("Could not upload avatar: {}", e);
                    }
                }

                match s.client.update_profile(&username, bio.as_deref(), city.as_deref(), country.as_deref()).await {
                    Ok(updated_user) => {
                        let _ = s.db.save_user_profile(&updated_user);
                        w.settings_view.set_user_profile(Some(&updated_user));
                        w.update_user_profile(Some(&updated_user));
                        w.settings_view.set_status("Profile updated successfully on SoundCloud!", false);
                    }
                    Err(e) => {
                        w.settings_view.set_status(&format!("Failed to update profile: {}", e), true);
                    }
                }
                w.settings_view.set_loading(false);
            });
        });
    }

    fn load_initial_data(&self) {
        // Restore sidebar collapse state if previously saved
        if let Ok(Some(col_str)) = self.state.db.get_setting("sidebar_collapsed") {
            if col_str == "true" || col_str == "1" {
                self.window.sidebar().set_collapsed(true);
            }
        }

        let state_c = Rc::clone(&self.state);
        let win_c = Rc::clone(&self.window);

        // Load credentials and verify active user session
        glib::spawn_future_local(async move {
            let cid = state_c.client.session.get_client_id().await;
            win_c.settings_view.set_client_id(&cid);

            // 1. Show cached profile immediately if available from previous sessions
            if let Ok(Some(cached_user)) = state_c.db.get_user_profile() {
                win_c.settings_view.set_user_profile(Some(&cached_user));
                win_c.update_user_profile(Some(&cached_user));
            }

            // 2. If not authenticated, check browser session cookies
            if !state_c.client.session.is_authenticated().await {
                if let Some(cookie_token) = crate::api::browser_cookies::scan_for_soundcloud_cookie() {
                    let _ = state_c.client.session.set_oauth_token(&cookie_token).await;
                    info!("Auto-detected active SoundCloud session from browser");
                }
            }

            // 3. Verify live session and sync data
            if state_c.client.session.is_authenticated().await {
                match state_c.client.get_current_user().await {
                    Ok(user) => {
                        let _ = state_c.db.save_user_profile(&user);
                        win_c.settings_view.set_user_profile(Some(&user));
                        win_c.update_user_profile(Some(&user));
                        win_c.messages_view.set_current_user_id(user.id);
                        info!("Session active for user @{}", user.username);
                        let _ = sync_user_data_internal(&state_c, &win_c).await;

                        // Load initial notifications
                        let s_notif_init = Rc::clone(&state_c);
                        let w_notif_init = Rc::clone(&win_c);
                        glib::spawn_future_local(async move {
                            w_notif_init.notifications_view.set_loading(true);
                            if let Ok(acts) = s_notif_init.client.get_activities(30).await {
                                w_notif_init.notifications_view.set_activities(acts);
                            }
                            w_notif_init.notifications_view.set_loading(false);
                        });

                        // Load initial messages
                        let s_msg_init = Rc::clone(&state_c);
                        let w_msg_init = Rc::clone(&win_c);
                        glib::spawn_future_local(async move {
                            w_msg_init.messages_view.set_loading(true);
                            if let Ok(convs) = s_msg_init.client.get_conversations(30).await {
                                w_msg_init.messages_view.set_conversations(convs);
                            }
                            w_msg_init.messages_view.set_loading(false);
                        });
                    }
                    Err(e) => {
                        warn!("Saved session verification failed (offline or network error): {}", e);
                        // Keep cached profile if network is down
                        if state_c.db.get_user_profile().ok().flatten().is_none() {
                            win_c.settings_view.set_user_profile(None);
                            win_c.update_user_profile(None);
                        }
                    }
                }
            } else if state_c.db.get_user_profile().ok().flatten().is_none() {
                win_c.settings_view.set_user_profile(None);
                win_c.update_user_profile(None);
            }
        });

        // Load initial discover charts
        let s_charts = Rc::clone(&self.state);
        let w_charts = Rc::clone(&self.window);
        glib::spawn_future_local(async move {
            w_charts.stream_view.set_loading(true);
            match s_charts.client.get_charts("my_stream", 25, 0).await {
                Ok(tracks) => {
                    w_charts.stream_view.set_tracks(tracks);
                }
                Err(e) => {
                    warn!("Initial charts fetch: {}", e);
                }
            }
            w_charts.stream_view.set_loading(false);
        });

        // Load liked tracks into library
        let s_lib = Rc::clone(&self.state);
        let w_lib = Rc::clone(&self.window);
        if let Ok(liked) = s_lib.db.get_liked_tracks() {
            w_lib.library_view.set_tracks(liked);
        }

        // Restore last played track & exact playback position from database
        if let Ok(Some(saved)) = self.state.db.get_playback_state() {
            info!(
                "Restoring last played track on startup: '{}' by {} at position {}ms",
                saved.track.title,
                saved.track.user.as_ref().map(|u| u.username.as_str()).unwrap_or("Unknown Artist"),
                saved.position_ms
            );
            let samples = self
                .state
                .db
                .get_cached_waveform(saved.track.id)
                .unwrap_or(None)
                .unwrap_or_else(|| crate::api::waveform::generate_synthetic_waveform(saved.track.id, 100));

            self.window.player_bar().set_track(&saved.track, samples);
            self.window.player_bar().update_position(saved.position_ms, saved.track.duration);
            self.window.player_bar().update_playback_state(false);
            *self.state.current_track.borrow_mut() = Some(saved.track.clone());
            *self.state.pending_resume_pos.borrow_mut() = Some(saved.position_ms);
            self.state.audio.get_queue().lock().unwrap().current = Some(saved.track);
        }

        // Initialize dynamic personalized genre tabs based on the user's algorithm and listening history
        refresh_personalized_genres(&self.state, &self.window);
    }
}

fn refresh_personalized_genres(state: &AppState, window: &MainWindow) {
    let mut genres = vec![
        ("Personal Stream".to_string(), "my_stream".to_string()),
        ("Mixed For You".to_string(), "mixed_for_you".to_string()),
    ];

    if let Ok(user_genres) = state.db.get_user_top_genres(6) {
        for g in user_genres {
            if !genres.iter().any(|(_, k)| k.eq_ignore_ascii_case(&g)) {
                genres.push((g.clone(), g));
            }
        }
    }

    if genres.len() < 5 {
        genres.push(("Trending".to_string(), "trending".to_string()));
    }

    window.stream_view.set_genres(genres);
}

async fn sync_user_data_internal(
    state: &Rc<AppState>,
    window: &Rc<MainWindow>,
) -> Result<()> {
    info!("Starting user library & playlist sync...");
    window.settings_view.set_loading(true);
    window.settings_view.set_status("Syncing personal likes and playlists from SoundCloud...", false);

    // 1. Liked tracks
    match state.client.get_my_liked_tracks(50).await {
        Ok(tracks) => {
            let count = tracks.len();
            for t in &tracks {
                let _ = state.db.cache_track(t, true);
            }
            window.library_view.set_tracks(tracks);
            info!("Synced {} personal liked tracks from SoundCloud account", count);
        }
        Err(e) => {
            warn!("Could not sync liked tracks: {}", e);
        }
    }

    // 2. Playlists
    match state.client.get_my_playlists(30).await {
        Ok(playlists) => {
            let count = playlists.len();
            window.library_view.set_playlists(playlists);
            info!("Synced {} personal playlists from SoundCloud account", count);
        }
        Err(e) => {
            warn!("Could not sync user playlists: {}", e);
        }
    }

    // 3. Refresh personalized genre tabs based on updated library and listening tastes
    refresh_personalized_genres(state, window);

    window.settings_view.set_loading(false);
    window.settings_view.set_status("Account data synchronized successfully.", false);
    Ok(())
}

async fn play_track_internal(
    state: &Rc<AppState>,
    window: &Rc<MainWindow>,
    track: Track,
) -> Result<()> {
    play_track_internal_with_seek(state, window, track, 0).await
}

async fn play_track_internal_with_seek(
    state: &Rc<AppState>,
    window: &Rc<MainWindow>,
    mut track: Track,
    seek_to_ms: u64,
) -> Result<()> {
    let req_id = state.play_request_counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1;
    info!("Preparing to stream track: {} (ID: {}) [req: {}] (seek: {}ms)", track.title, track.id, req_id, seek_to_ms);

    // If track is a stub or missing transcodings, fetch full track first
    if track.media.as_ref().map(|m| m.transcodings.is_empty()).unwrap_or(true) || track.title == "SoundCloud Track" {
        if let Ok(full) = state.client.get_track(track.id).await {
            track = full;
        }
    }

    if state.play_request_counter.load(std::sync::atomic::Ordering::SeqCst) != req_id {
        return Ok(());
    }

    // 1. Get waveform samples (from SQLite cache or network/procedural)
    let samples = if let Ok(Some(cached_samples)) = state.db.get_cached_waveform(track.id) {
        cached_samples
    } else {
        let fresh_samples = state.client.get_waveform_samples(&track).await;
        let _ = state.db.save_cached_waveform(track.id, &fresh_samples);
        fresh_samples
    };

    if state.play_request_counter.load(std::sync::atomic::Ordering::SeqCst) != req_id {
        return Ok(());
    }

    // 2. Update UI PlayerBar immediately
    window.player_bar().set_track(&track, samples.clone());
    if seek_to_ms > 0 {
        window.player_bar().update_position(seek_to_ms, track.duration);
    }
    window.player_bar().update_playback_state(true);

    *state.current_track.borrow_mut() = Some(track.clone());

    // 3. Resolve playable audio stream URL (HLS / progressive)
    match state.client.get_stream_url(&track).await {
        Ok(stream_url) => {
            if state.play_request_counter.load(std::sync::atomic::Ordering::SeqCst) != req_id {
                return Ok(());
            }

            if let Err(e) = state.audio.play_stream(&stream_url, track.clone(), seek_to_ms) {
                error!("Audio engine error playing stream: {}", e);
                state.audio.stop();
                window.player_bar().update_playback_state(false);
            } else {
                // Record history and cache track in database
                let _ = state.db.cache_track(&track, false);
                let _ = state.db.record_playback(track.id);
                let _ = state.db.save_playback_state(&track, seek_to_ms);

                // Update MPRIS metadata
                if let Some(ref m) = *state.mpris.borrow() {
                    m.update_metadata(Some(&track)).await;
                    m.update_status(mpris_server::PlaybackStatus::Playing).await;
                }
            }
        }
        Err(e) => {
            error!("Could not resolve stream URL for track {}: {}", track.id, e);
            state.audio.stop();
            window.player_bar().update_playback_state(false);
        }
    }

    Ok(())
}

fn show_track_detail_internal(
    state: &Rc<AppState>,
    window: &Rc<MainWindow>,
    mut track: Track,
) {
    let s_c = Rc::clone(state);
    let win_c = Rc::clone(window);
    glib::spawn_future_local(async move {
        if track.media.as_ref().map(|m| m.transcodings.is_empty()).unwrap_or(true) || track.title == "SoundCloud Track" {
            if let Ok(full) = s_c.client.get_track(track.id).await {
                track = full;
            }
        }
        let track_id = track.id;
        let dur = track.duration;
        let samples = crate::api::waveform::generate_synthetic_waveform(track.id, 140);

        win_c.track_detail_view.set_track(track, samples);
        win_c.show_view("track_detail");

        // Fetch comments asynchronously
        let client = s_c.client.clone();
        let w_c = Rc::clone(&win_c);
        if let Ok(comments) = client.get_comments(track_id, 60).await {
            w_c.track_detail_view.set_comments(comments, dur);
        }
    });
}

fn show_artist_profile_internal(
    state: &Rc<AppState>,
    window: &Rc<MainWindow>,
    user_id: u64,
) {
    let s = Rc::clone(state);
    let w = Rc::clone(window);
    w.show_view("artist_profile");
    w.artist_profile_view.set_loading(true);

    glib::spawn_future_local(async move {
        // 1. Fetch user profile
        if let Ok(user) = s.client.get_user_profile(user_id).await {
            w.artist_profile_view.set_artist(&user);
        }

        // 2. Check if currently following
        let following = s.client.is_following_user(user_id).await;
        w.artist_profile_view.set_following(following);

        // 3. Fetch top tracks
        if let Ok(tracks) = s.client.get_user_top_tracks(user_id, 35).await {
            w.artist_profile_view.set_tracks(tracks);
        }

        w.artist_profile_view.set_loading(false);
    });
}
