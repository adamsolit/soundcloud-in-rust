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
    pub sleep_timer_end: Rc<RefCell<Option<std::time::Instant>>>,
    pub sleep_timer_track_end: Rc<RefCell<bool>>,
    pub original_volume: Rc<RefCell<f64>>,
    pub in_fade_out: Rc<RefCell<bool>>,
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
            sleep_timer_end: Rc::new(RefCell::new(None)),
            sleep_timer_track_end: Rc::new(RefCell::new(false)),
            original_volume: Rc::new(RefCell::new(1.0)),
            in_fade_out: Rc::new(RefCell::new(false)),
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
        app.set_accels_for_action("app.quit", &["<Control>q"]);

        // Register Ctrl+W to dismiss active popovers or close subviews (go back)
        let action_close_view = gtk4::gio::SimpleAction::new("close-view", None);
        let win_close = Rc::clone(&application.window);
        action_close_view.connect_activate(move |_, _| {
            win_close.go_back();
        });
        app.add_action(&action_close_view);
        app.set_accels_for_action("app.close-view", &["<Control>w"]);

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

        // Register Ctrl+M to toggle mini-player mode
        let action_mini = gtk4::gio::SimpleAction::new("toggle-mini", None);
        let win_mini_act = Rc::clone(&application.window);
        action_mini.connect_activate(move |_, _| {
            win_mini_act.toggle_mini_player();
        });
        app.add_action(&action_mini);
        app.set_accels_for_action("app.toggle-mini", &["<Control>m"]);

        // Register Ctrl+Shift+T to toggle aesthetic theme on the fly
        let action_toggle_theme = gtk4::gio::SimpleAction::new("toggle-theme", None);
        let win_theme_act = Rc::clone(&application.window);
        let state_theme_act = Rc::clone(&application.state);
        action_toggle_theme.connect_activate(move |_, _| {
            let next = match crate::ui::theme::current_theme() {
                crate::ui::theme::AppTheme::AnthropicClassical => crate::ui::theme::AppTheme::ObsidianDark,
                crate::ui::theme::AppTheme::ObsidianDark => crate::ui::theme::AppTheme::AnthropicClassical,
            };
            crate::ui::theme::apply_theme(next);
            let _ = state_theme_act.db.set_setting("app_theme", next.as_str());
            win_theme_act.settings_view.set_active_theme(next);
            win_theme_act.queue_redraw_waveforms();
        });
        app.add_action(&action_toggle_theme);
        app.set_accels_for_action("app.toggle-theme", &["<Control><Shift>t"]);

        // Register F1 and Ctrl+? to show shortcuts modal cheat sheet
        let action_shortcuts = gtk4::gio::SimpleAction::new("shortcuts", None);
        let win_shortcuts = Rc::clone(&application.window);
        action_shortcuts.connect_activate(move |_, _| {
            crate::ui::shortcuts_dialog::show_shortcuts_dialog(win_shortcuts.window());
        });
        app.add_action(&action_shortcuts);
        app.set_accels_for_action("app.shortcuts", &["F1", "<Control>question"]);

        // Register Ctrl+F to focus search bar
        let action_search = gtk4::gio::SimpleAction::new("focus-search", None);
        let win_search = Rc::clone(&application.window);
        action_search.connect_activate(move |_, _| {
            win_search.show_view("search");
            win_search.sidebar().set_active(crate::ui::sidebar::NavTarget::Search);
            win_search.search_view.focus_search();
        });
        app.add_action(&action_search);
        app.set_accels_for_action("app.focus-search", &["<Control>f"]);

        // Register Shift+N (Next Track) and Shift+P (Prev Track)
        let action_next = gtk4::gio::SimpleAction::new("next-track", None);
        let audio_next_sc = Arc::clone(&application.state.audio);
        let state_next_sc = Rc::clone(&application.state);
        let win_next_sc = Rc::clone(&application.window);
        action_next.connect_activate(move |_, _| {
            let next = audio_next_sc.get_queue().lock().unwrap().skip_next_track();
            if let Some(t) = next {
                let s = Rc::clone(&state_next_sc);
                let w = Rc::clone(&win_next_sc);
                glib::spawn_future_local(async move {
                    let _ = play_track_internal(&s, &w, t).await;
                });
            }
        });
        app.add_action(&action_next);
        app.set_accels_for_action("app.next-track", &["<Shift>n"]);

        let action_prev = gtk4::gio::SimpleAction::new("prev-track", None);
        let audio_prev_sc = Arc::clone(&application.state.audio);
        let state_prev_sc = Rc::clone(&application.state);
        let win_prev_sc = Rc::clone(&application.window);
        action_prev.connect_activate(move |_, _| {
            let prev = audio_prev_sc.get_queue().lock().unwrap().previous_track();
            if let Some(t) = prev {
                let s = Rc::clone(&state_prev_sc);
                let w = Rc::clone(&win_prev_sc);
                glib::spawn_future_local(async move {
                    let _ = play_track_internal(&s, &w, t).await;
                });
            }
        });
        app.add_action(&action_prev);
        app.set_accels_for_action("app.prev-track", &["<Shift>p"]);

        // Window-level key event controller for Space and Left/Right (preserving text inputs)
        let key_controller = gtk4::EventControllerKey::new();
        let audio_key = Arc::clone(&application.state.audio);
        let win_key = Rc::clone(&application.window);
        let state_key = Rc::clone(&application.state);
        let mpris_key = Rc::clone(&application.state.mpris);

        key_controller.connect_key_pressed(move |_controller, key, _keycode, _modifier| {
            if let Some(focus) = gtk4::prelude::GtkWindowExt::focus(win_key.window()) {
                if focus.is::<gtk4::Editable>() || focus.is::<gtk4::TextView>() {
                    return glib::Propagation::Proceed;
                }
            }

            match key {
                gdk4::Key::space => {
                    let st = audio_key.get_state();
                    let needs_stream = !audio_key.is_loaded()
                        || st == crate::audio::PlaybackState::Stopped
                        || state_key.pending_resume_pos.borrow().is_some();

                    if needs_stream {
                        if let Some(ref track) = *state_key.current_track.borrow() {
                            let s = Rc::clone(&state_key);
                            let w = Rc::clone(&win_key);
                            let t = track.clone();
                            let resume_pos = state_key.pending_resume_pos.borrow_mut().take().unwrap_or(0);
                            glib::spawn_future_local(async move {
                                let _ = play_track_internal_with_seek(&s, &w, t, resume_pos).await;
                            });
                            return glib::Propagation::Stop;
                        }
                    }

                    audio_key.toggle_play_pause();
                    let playing = audio_key.get_state() == crate::audio::PlaybackState::Playing;
                    win_key.player_bar().update_playback_state(playing);

                    if !playing {
                        if let Some(ref track) = *state_key.current_track.borrow() {
                            let pos = audio_key.get_position_ms();
                            let _ = state_key.db.save_playback_state(track, pos);
                        }
                    }

                    if let Some(ref m) = *mpris_key.borrow() {
                        m.handler.set_playback_status(if playing {
                            crate::mpris::PlaybackStatus::Playing
                        } else {
                            crate::mpris::PlaybackStatus::Paused
                        });
                    }
                    glib::Propagation::Stop
                }
                gdk4::Key::Left => {
                    let cur = audio_key.get_position_ms();
                    let target = cur.saturating_sub(5000);
                    let _ = audio_key.seek(target);
                    glib::Propagation::Stop
                }
                gdk4::Key::Right => {
                    let cur = audio_key.get_position_ms();
                    let dur = audio_key.get_duration_ms();
                    let target = (cur + 5000).min(dur);
                    let _ = audio_key.seek(target);
                    glib::Propagation::Stop
                }
                gdk4::Key::b | gdk4::Key::B => {
                    if let Some(ref track) = *state_key.current_track.borrow() {
                        let pos = state_key.audio.get_position_ms();
                        let dur = track.duration;
                        win_key.player_bar().prompt_add_bookmark(pos, dur);
                    }
                    glib::Propagation::Stop
                }
                _ => glib::Propagation::Proceed,
            }
        });
        application.window.window().add_controller(key_controller);

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
        let state_mpris_init = Rc::clone(&self.state);
        glib::spawn_future_local(async move {
            match MprisService::start(cmd_tx).await {
                Ok(service) => {
                    if let Some(ref track) = *state_mpris_init.current_track.borrow() {
                        let pos = state_mpris_init.pending_resume_pos.borrow().unwrap_or(0);
                        service.handler.set_position_micros(pos * 1000);
                        service.update_metadata(Some(track)).await;
                        service.update_status(crate::mpris::PlaybackStatus::Paused).await;
                        service.emit_seeked(pos * 1000).await;
                    }

                    let q_arc = state_mpris_init.audio.get_queue();
                    let q = q_arc.lock().unwrap();
                    let shuf = q.shuffle;
                    let loop_st = match q.repeat_mode {
                        crate::audio::types::RepeatMode::Off => crate::mpris::LoopStatus::None,
                        crate::audio::types::RepeatMode::One => crate::mpris::LoopStatus::Track,
                        crate::audio::types::RepeatMode::All => crate::mpris::LoopStatus::Playlist,
                    };
                    drop(q);
                    service.update_shuffle(shuf).await;
                    service.update_loop_status(loop_st).await;

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
                    MprisCommand::Play | MprisCommand::PlayPause => {
                        let st = audio_clone.get_state();
                        let needs_stream = !audio_clone.is_loaded()
                            || st == crate::audio::PlaybackState::Stopped
                            || state_clone.pending_resume_pos.borrow().is_some();

                        if needs_stream {
                            if let Some(ref track) = *state_clone.current_track.borrow() {
                                let s = Rc::clone(&state_clone);
                                let w = Rc::clone(&app_win);
                                let t = track.clone();
                                let resume_pos = state_clone.pending_resume_pos.borrow_mut().take().unwrap_or(0);
                                glib::spawn_future_local(async move {
                                    let _ = play_track_internal_with_seek(&s, &w, t, resume_pos).await;
                                });
                                continue;
                            }
                        }

                        if matches!(cmd, MprisCommand::Play) {
                            audio_clone.resume();
                        } else {
                            audio_clone.toggle_play_pause();
                        }
                        let playing = audio_clone.get_state() == crate::audio::PlaybackState::Playing;
                        app_win.player_bar().update_playback_state(playing);
                        app_win.mini_player().update_playback_state(playing);
                        let pos = audio_clone.get_position_ms();
                        let mpris_holder = Rc::clone(&state_clone.mpris);
                        glib::spawn_future_local(async move {
                            if let Some(ref m) = *mpris_holder.borrow() {
                                let status = if playing {
                                    crate::mpris::PlaybackStatus::Playing
                                } else {
                                    crate::mpris::PlaybackStatus::Paused
                                };
                                m.update_status(status).await;
                                m.emit_seeked(pos * 1000).await;
                            }
                        });
                    }
                    MprisCommand::Pause => {
                        audio_clone.pause();
                        app_win.player_bar().update_playback_state(false);
                        app_win.mini_player().update_playback_state(false);
                        let pos = audio_clone.get_position_ms();
                        let mpris_holder = Rc::clone(&state_clone.mpris);
                        glib::spawn_future_local(async move {
                            if let Some(ref m) = *mpris_holder.borrow() {
                                m.update_status(crate::mpris::PlaybackStatus::Paused).await;
                                m.emit_seeked(pos * 1000).await;
                            }
                        });
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
                        app_win.mini_player().update_playback_state(false);
                        let mpris_holder = Rc::clone(&state_clone.mpris);
                        glib::spawn_future_local(async move {
                            if let Some(ref m) = *mpris_holder.borrow() {
                                m.update_status(crate::mpris::PlaybackStatus::Stopped).await;
                            }
                        });
                    }
                    MprisCommand::Seek(offset_ms) => {
                        let current_pos = audio_clone.get_position_ms();
                        let new_pos = (current_pos as i64 + offset_ms).max(0) as u64;
                        let _ = audio_clone.seek(new_pos);
                        let dur = audio_clone.get_duration_ms();
                        app_win.player_bar().update_position(new_pos, dur);
                        app_win.mini_player().update_position(new_pos, dur);
                        if app_win.track_detail_view.widget().is_visible() {
                            app_win.track_detail_view.update_position(new_pos, dur);
                        }
                        let mpris_holder = Rc::clone(&state_clone.mpris);
                        glib::spawn_future_local(async move {
                            if let Some(ref m) = *mpris_holder.borrow() {
                                m.emit_seeked(new_pos * 1000).await;
                            }
                        });
                    }
                    MprisCommand::SetPosition(target_ms) => {
                        let _ = audio_clone.seek(target_ms);
                        let dur = audio_clone.get_duration_ms();
                        app_win.player_bar().update_position(target_ms, dur);
                        app_win.mini_player().update_position(target_ms, dur);
                        if app_win.track_detail_view.widget().is_visible() {
                            app_win.track_detail_view.update_position(target_ms, dur);
                        }
                        let mpris_holder = Rc::clone(&state_clone.mpris);
                        glib::spawn_future_local(async move {
                            if let Some(ref m) = *mpris_holder.borrow() {
                                m.emit_seeked(target_ms * 1000).await;
                            }
                        });
                    }
                    MprisCommand::SetShuffle(shuffle) => {
                        let q_arc = audio_clone.get_queue();
                        let mut q = q_arc.lock().unwrap();
                        q.set_shuffle(shuffle);
                        let curr = q.current.clone();
                        let items = q.items.clone();
                        drop(q);
                        app_win.library_view.playlist_view.set_shuffle_active(shuffle);
                        app_win.player_bar().set_shuffle(shuffle);
                        app_win.player_bar().update_queue(curr.as_ref(), &items);
                        let mpris_holder = Rc::clone(&state_clone.mpris);
                        glib::spawn_future_local(async move {
                            if let Some(ref m) = *mpris_holder.borrow() {
                                m.update_shuffle(shuffle).await;
                            }
                        });
                    }
                    MprisCommand::SetLoopStatus(loop_status) => {
                        let mode = match loop_status {
                            crate::mpris::LoopStatus::None => crate::audio::types::RepeatMode::Off,
                            crate::mpris::LoopStatus::Track => crate::audio::types::RepeatMode::One,
                            crate::mpris::LoopStatus::Playlist => crate::audio::types::RepeatMode::All,
                        };
                        audio_clone.get_queue().lock().unwrap().repeat_mode = mode;
                        app_win.player_bar().set_repeat_mode(mode);
                        let mpris_holder = Rc::clone(&state_clone.mpris);
                        glib::spawn_future_local(async move {
                            if let Some(ref m) = *mpris_holder.borrow() {
                                m.update_loop_status(loop_status).await;
                            }
                        });
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
                win_c.mini_player().update_position(pos, dur);
                if win_c.track_detail_view.widget().is_visible() {
                    win_c.track_detail_view.update_position(pos, dur);
                }

                if let Some(ref m) = *state_sync.mpris.borrow() {
                    m.handler.set_position_micros(pos * 1000);
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

            // Smart Sleep Timer processing
            let now = std::time::Instant::now();
            if let Some(target) = *state_sync.sleep_timer_end.borrow() {
                if now >= target {
                    // Timer expired: pause playback, restore original volume, reset timer UI
                    audio_c.pause();
                    let orig_vol = *state_sync.original_volume.borrow();
                    audio_c.set_volume(orig_vol);
                    *state_sync.sleep_timer_end.borrow_mut() = None;
                    *state_sync.in_fade_out.borrow_mut() = false;
                    win_c.player_bar().update_sleep_timer_status(None, false);
                    win_c.player_bar().update_playback_state(false);
                    win_c.mini_player().update_playback_state(false);
                } else {
                    let rem_sec = target.duration_since(now).as_secs();
                    win_c.player_bar().update_sleep_timer_status(Some(rem_sec), false);

                    // Final 20 seconds: smooth logarithmic/quadratic fade-out down to 0
                    if rem_sec <= 20 {
                        let rem_millis = target.duration_since(now).as_millis() as f64;
                        let ratio = (rem_millis / 20_000.0).clamp(0.0, 1.0);
                        let fade = ratio * ratio; // quadratic fade
                        let orig_vol = *state_sync.original_volume.borrow();
                        audio_c.set_volume(orig_vol * fade);
                        *state_sync.in_fade_out.borrow_mut() = true;
                    }
                }
            } else if *state_sync.sleep_timer_track_end.borrow() {
                win_c.player_bar().update_sleep_timer_status(None, true);
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
                        if *state_c.sleep_timer_track_end.borrow() {
                            *state_c.sleep_timer_track_end.borrow_mut() = false;
                            state_c.audio.pause();
                            win_c.player_bar().update_sleep_timer_status(None, false);
                            win_c.player_bar().update_playback_state(false);
                            win_c.mini_player().update_playback_state(false);
                            if let Some(ref m) = *state_c.mpris.borrow() {
                                m.update_status(crate::mpris::PlaybackStatus::Paused).await;
                            }
                            continue;
                        }

                        let next = state_c.audio.get_queue().lock().unwrap().next_track();
                        if let Some(track) = next {
                            let _ = play_track_internal(&state_c, &win_c, track).await;
                        } else {
                            win_c.player_bar().update_playback_state(false);
                            win_c.mini_player().update_playback_state(false);
                            if let Some(ref m) = *state_c.mpris.borrow() {
                                m.update_status(crate::mpris::PlaybackStatus::Stopped).await;
                            }
                        }
                    }
                    crate::audio::AudioEvent::StateChanged(st) => {
                        let is_playing = st == crate::audio::PlaybackState::Playing;
                        win_c.player_bar().update_playback_state(is_playing);
                        win_c.mini_player().update_playback_state(is_playing);
                        let pos = state_c.audio.get_position_ms();
                        if let Some(ref m) = *state_c.mpris.borrow() {
                            let status = if is_playing {
                                crate::mpris::PlaybackStatus::Playing
                            } else if st == crate::audio::PlaybackState::Paused {
                                crate::mpris::PlaybackStatus::Paused
                            } else {
                                crate::mpris::PlaybackStatus::Stopped
                            };
                            m.update_status(status).await;
                            m.emit_seeked(pos * 1000).await;
                        }
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
            win_pp.mini_player().update_playback_state(playing);

            if !playing {
                if let Some(ref track) = *state_pp.current_track.borrow() {
                    let pos = audio_pp.get_position_ms();
                    let _ = state_pp.db.save_playback_state(track, pos);
                }
            }

            let mpris_clone = Rc::clone(&mpris_pp);
            let pos = audio_pp.get_position_ms();
            glib::spawn_future_local(async move {
                if let Some(ref m) = *mpris_clone.borrow() {
                    let st = if playing {
                        crate::mpris::PlaybackStatus::Playing
                    } else {
                        crate::mpris::PlaybackStatus::Paused
                    };
                    m.update_status(st).await;
                    m.emit_seeked(pos * 1000).await;
                }
            });
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
                    win_seek.mini_player().update_position(target_ms, track.duration);
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
                win_seek.player_bar().update_position(target_ms, dur);
                win_seek.mini_player().update_position(target_ms, dur);
                if win_seek.track_detail_view.widget().is_visible() {
                    win_seek.track_detail_view.update_position(target_ms, dur);
                }
                let m_c = Rc::clone(&state_seek.mpris);
                glib::spawn_future_local(async move {
                    if let Some(ref m) = *m_c.borrow() {
                        m.emit_seeked(target_ms * 1000).await;
                    }
                });
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
        let state_shuf = Rc::clone(&state_c);
        player_bar.connect_shuffle(move || {
            let q_arc = audio_shuf.get_queue();
            let mut q = q_arc.lock().unwrap();
            let active = q.toggle_shuffle();
            let curr = q.current.clone();
            let items = q.items.clone();
            drop(q);
            win_shuf.library_view.playlist_view.set_shuffle_active(active);
            win_shuf.player_bar().set_shuffle(active);
            win_shuf.player_bar().update_queue(curr.as_ref(), &items);
            let m_c = Rc::clone(&state_shuf.mpris);
            glib::spawn_future_local(async move {
                if let Some(ref m) = *m_c.borrow() {
                    m.update_shuffle(active).await;
                }
            });
            active
        });

        // Repeat
        let audio_rep = Arc::clone(&audio_c);
        let win_rep = Rc::clone(&win_c);
        let state_rep = Rc::clone(&state_c);
        player_bar.connect_repeat(move || {
            let new_mode = audio_rep.get_queue().lock().unwrap().cycle_repeat();
            win_rep.player_bar().set_repeat_mode(new_mode);
            let m_c = Rc::clone(&state_rep.mpris);
            glib::spawn_future_local(async move {
                if let Some(ref m) = *m_c.borrow() {
                    let loop_st = match new_mode {
                        crate::audio::types::RepeatMode::Off => crate::mpris::LoopStatus::None,
                        crate::audio::types::RepeatMode::One => crate::mpris::LoopStatus::Track,
                        crate::audio::types::RepeatMode::All => crate::mpris::LoopStatus::Playlist,
                    };
                    m.update_loop_status(loop_st).await;
                }
            });
            new_mode
        });

        // Like / Unlike from player bar
        let state_like = Rc::clone(&state_c);
        let win_like = Rc::clone(&win_c);
        player_bar.connect_like(move |liked| {
            if let Some(ref track) = *state_like.current_track.borrow() {
                handle_track_like_internal(&state_like, &win_like, track.clone(), liked);
            }
        });

        // Mini player toggle
        let win_mini = Rc::clone(&win_c);
        player_bar.connect_toggle_mini(move || {
            win_mini.toggle_mini_player();
        });

        // Queue jump
        let audio_q_jump = Arc::clone(&audio_c);
        let state_q_jump = Rc::clone(&state_c);
        let win_q_jump = Rc::clone(&win_c);
        player_bar.connect_queue_jump(move |idx| {
            let track_opt = audio_q_jump.get_queue().lock().unwrap().jump_to(idx);
            if let Some(t) = track_opt {
                let s = Rc::clone(&state_q_jump);
                let w = Rc::clone(&win_q_jump);
                glib::spawn_future_local(async move {
                    let _ = play_track_internal(&s, &w, t).await;
                });
            }
        });

        // Queue remove
        let audio_q_rem = Arc::clone(&audio_c);
        let win_q_rem = Rc::clone(&win_c);
        player_bar.connect_queue_remove(move |idx| {
            let q_arc = audio_q_rem.get_queue();
            let mut q = q_arc.lock().unwrap();
            q.remove_at(idx);
            let curr = q.current.clone();
            let items = q.items.clone();
            drop(q);
            win_q_rem.player_bar().update_queue(curr.as_ref(), &items);
        });

        // Queue clear
        let audio_q_clr = Arc::clone(&audio_c);
        let win_q_clr = Rc::clone(&win_c);
        player_bar.connect_queue_clear(move || {
            let q_arc = audio_q_clr.get_queue();
            let mut q = q_arc.lock().unwrap();
            q.clear();
            let curr = q.current.clone();
            drop(q);
            win_q_clr.player_bar().update_queue(curr.as_ref(), &[]);
        });

        // Pitch-Preserving Speed Control (0.5x - 2.5x)
        let audio_spd = Arc::clone(&audio_c);
        player_bar.connect_speed(move |spd| {
            audio_spd.set_speed(spd);
        });

        // Smart Sleep Timer
        let state_sl = Rc::clone(&state_c);
        let win_sl = Rc::clone(&win_c);
        player_bar.connect_sleep_timer(move |secs_opt, end_of_track| {
            if end_of_track {
                *state_sl.sleep_timer_track_end.borrow_mut() = true;
                *state_sl.sleep_timer_end.borrow_mut() = None;
                win_sl.player_bar().update_sleep_timer_status(None, true);
            } else if let Some(secs) = secs_opt {
                let target = std::time::Instant::now() + std::time::Duration::from_secs(secs as u64);
                *state_sl.sleep_timer_end.borrow_mut() = Some(target);
                *state_sl.sleep_timer_track_end.borrow_mut() = false;
                *state_sl.original_volume.borrow_mut() = state_sl.audio.get_volume();
                win_sl.player_bar().update_sleep_timer_status(Some(secs as u64), false);
            } else {
                *state_sl.sleep_timer_end.borrow_mut() = None;
                *state_sl.sleep_timer_track_end.borrow_mut() = false;
                if *state_sl.in_fade_out.borrow() {
                    state_sl.audio.set_volume(*state_sl.original_volume.borrow());
                    *state_sl.in_fade_out.borrow_mut() = false;
                }
                win_sl.player_bar().update_sleep_timer_status(None, false);
            }
        });

        // Quick Bookmark from Player Bar
        let state_bm_pb = Rc::clone(&state_c);
        let win_bm_pb = Rc::clone(&win_c);
        player_bar.connect_add_bookmark(move || {
            if let Some(ref track) = *state_bm_pb.current_track.borrow() {
                let pos = state_bm_pb.audio.get_position_ms();
                let dur = track.duration;
                win_bm_pb.player_bar().prompt_add_bookmark(pos, dur);
            }
        });

        // Save Bookmark from Naming Prompt
        let state_bm_save = Rc::clone(&state_c);
        let win_bm_save = Rc::clone(&win_c);
        player_bar.connect_save_bookmark(move |pos_ms, note| {
            if let Some(ref track) = *state_bm_save.current_track.borrow() {
                let clean_note = if note.trim().is_empty() { "Bookmark".to_string() } else { note.trim().to_string() };
                let _ = state_bm_save.db.add_bookmark(track.id, pos_ms, &clean_note);
                refresh_bookmarks_ui(&state_bm_save, &win_bm_save, track.id, track.duration);
            }
        });

        // Stash Toggle from Player Bar
        let state_st_pb = Rc::clone(&state_c);
        let win_st_pb = Rc::clone(&win_c);
        player_bar.connect_stash(move || {
            if let Some(ref track) = *state_st_pb.current_track.borrow() {
                handle_toggle_stash_internal(&state_st_pb, &win_st_pb, track.clone());
            }
        });

        // Wire Mini Player controls
        let mini = self.window.mini_player();

        // Mini Play/Pause
        let audio_mini_pp = Arc::clone(&audio_c);
        let win_mini_pp = Rc::clone(&win_c);
        let state_mini_pp = Rc::clone(&state_c);
        let mpris_mini_pp = Rc::clone(&self.state.mpris);
        mini.connect_play_pause(move || {
            let st = audio_mini_pp.get_state();
            let needs_stream = !audio_mini_pp.is_loaded()
                || st == crate::audio::PlaybackState::Stopped
                || state_mini_pp.pending_resume_pos.borrow().is_some();

            if needs_stream {
                if let Some(ref track) = *state_mini_pp.current_track.borrow() {
                    let s = Rc::clone(&state_mini_pp);
                    let w = Rc::clone(&win_mini_pp);
                    let t = track.clone();
                    let resume_pos = state_mini_pp.pending_resume_pos.borrow_mut().take().unwrap_or(0);
                    glib::spawn_future_local(async move {
                        let _ = play_track_internal_with_seek(&s, &w, t, resume_pos).await;
                    });
                    return;
                }
            }

            audio_mini_pp.toggle_play_pause();
            let playing = audio_mini_pp.get_state() == crate::audio::PlaybackState::Playing;
            win_mini_pp.player_bar().update_playback_state(playing);
            win_mini_pp.mini_player().update_playback_state(playing);

            if !playing {
                if let Some(ref track) = *state_mini_pp.current_track.borrow() {
                    let pos = audio_mini_pp.get_position_ms();
                    let _ = state_mini_pp.db.save_playback_state(track, pos);
                }
            }

            let mpris_clone = Rc::clone(&mpris_mini_pp);
            let pos = audio_mini_pp.get_position_ms();
            glib::spawn_future_local(async move {
                if let Some(ref m) = *mpris_clone.borrow() {
                    let st = if playing {
                        crate::mpris::PlaybackStatus::Playing
                    } else {
                        crate::mpris::PlaybackStatus::Paused
                    };
                    m.update_status(st).await;
                    m.emit_seeked(pos * 1000).await;
                }
            });
        });

        // Mini Next
        let audio_mini_next = Arc::clone(&audio_c);
        let state_mini_next = Rc::clone(&state_c);
        let win_mini_next = Rc::clone(&win_c);
        mini.connect_next(move || {
            let next = audio_mini_next.get_queue().lock().unwrap().skip_next_track();
            if let Some(t) = next {
                let s_c = Rc::clone(&state_mini_next);
                let w_c = Rc::clone(&win_mini_next);
                glib::spawn_future_local(async move {
                    let _ = play_track_internal(&s_c, &w_c, t).await;
                });
            }
        });

        // Mini Prev
        let audio_mini_prev = Arc::clone(&audio_c);
        let state_mini_prev = Rc::clone(&state_c);
        let win_mini_prev = Rc::clone(&win_c);
        mini.connect_prev(move || {
            let prev = audio_mini_prev.get_queue().lock().unwrap().previous_track();
            if let Some(t) = prev {
                let s_c = Rc::clone(&state_mini_prev);
                let w_c = Rc::clone(&win_mini_prev);
                glib::spawn_future_local(async move {
                    let _ = play_track_internal(&s_c, &w_c, t).await;
                });
            }
        });

        // Mini Seek
        let audio_mini_seek = Arc::clone(&audio_c);
        let state_mini_seek = Rc::clone(&state_c);
        let win_mini_seek = Rc::clone(&win_c);
        mini.connect_seek(move |pct| {
            if !audio_mini_seek.is_loaded() || state_mini_seek.pending_resume_pos.borrow().is_some() {
                if let Some(ref track) = *state_mini_seek.current_track.borrow() {
                    let target_ms = (pct * track.duration as f64) as u64;
                    win_mini_seek.player_bar().update_position(target_ms, track.duration);
                    win_mini_seek.mini_player().update_position(target_ms, track.duration);
                    let s = Rc::clone(&state_mini_seek);
                    let w = Rc::clone(&win_mini_seek);
                    let t = track.clone();
                    state_mini_seek.pending_resume_pos.borrow_mut().take();
                    glib::spawn_future_local(async move {
                        let _ = play_track_internal_with_seek(&s, &w, t, target_ms).await;
                    });
                    return;
                }
            }

            let dur = audio_mini_seek.get_duration_ms();
            if dur > 0 {
                let target_ms = (pct * dur as f64) as u64;
                let _ = audio_mini_seek.seek(target_ms);
                win_mini_seek.player_bar().update_position(target_ms, dur);
                win_mini_seek.mini_player().update_position(target_ms, dur);
                if win_mini_seek.track_detail_view.widget().is_visible() {
                    win_mini_seek.track_detail_view.update_position(target_ms, dur);
                }
                let m_c = Rc::clone(&state_mini_seek.mpris);
                glib::spawn_future_local(async move {
                    if let Some(ref m) = *m_c.borrow() {
                        m.emit_seeked(target_ms * 1000).await;
                    }
                });
            }
        });

        // Mini Like
        let state_mini_like = Rc::clone(&state_c);
        let win_mini_like = Rc::clone(&win_c);
        mini.connect_like(move |liked| {
            if let Some(ref track) = *state_mini_like.current_track.borrow() {
                handle_track_like_internal(&state_mini_like, &win_mini_like, track.clone(), liked);
            }
        });

        // Mini Restore
        let win_mini_restore = Rc::clone(&win_c);
        mini.connect_restore(move || {
            win_mini_restore.toggle_mini_player();
        });
    }

    fn wire_views(&self) {
        let state_c = Rc::clone(&self.state);
        let win_c = Rc::clone(&self.window);

        // 1. Stream / Discover View
        let s_stm_q_play = Rc::clone(&state_c);
        let w_stm_q_play = Rc::clone(&win_c);
        self.window.stream_view.connect_play_queue(move |tracks, idx| {
            handle_play_queue_internal(&s_stm_q_play, &w_stm_q_play, tracks, idx);
        });

        let s1 = Rc::clone(&state_c);
        let w1 = Rc::clone(&win_c);
        self.window.stream_view.connect_play_track(move |track| {
            let s = Rc::clone(&s1);
            let w = Rc::clone(&w1);
            glib::spawn_future_local(async move {
                let _ = play_track_internal(&s, &w, track).await;
            });
        });

        let s_stm_like = Rc::clone(&state_c);
        let w_stm_like = Rc::clone(&win_c);
        self.window.stream_view.connect_like_track(move |track, liked| {
            handle_track_like_internal(&s_stm_like, &w_stm_like, track, liked);
        });

        let s_sel = Rc::clone(&state_c);
        let w_sel = Rc::clone(&win_c);
        self.window.stream_view.connect_select_track(move |track| {
            show_track_detail_internal(&s_sel, &w_sel, track);
        });

        let s_stm_up_next = Rc::clone(&state_c);
        let w_stm_up_next = Rc::clone(&win_c);
        self.window.stream_view.connect_queue_track(move |track| {
            handle_add_to_up_next(&s_stm_up_next, &w_stm_up_next, track);
        });

        let s_stm_art = Rc::clone(&state_c);
        let w_stm_art = Rc::clone(&win_c);
        self.window.stream_view.connect_artist_click(move |user_id| {
            show_artist_profile_internal(&s_stm_art, &w_stm_art, user_id);
        });

        let s_stm_stash = Rc::clone(&state_c);
        let w_stm_stash = Rc::clone(&win_c);
        self.window.stream_view.connect_stash_track(move |track| {
            handle_toggle_stash_internal(&s_stm_stash, &w_stm_stash, track);
        });
        let s_stm_chk = Rc::clone(&state_c);
        self.window.stream_view.set_stash_checker(move |track_id| {
            s_stm_chk.db.is_track_stashed(track_id)
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

        let s_srch_q = Rc::clone(&state_c);
        let w_srch_q = Rc::clone(&win_c);
        self.window.search_view.connect_play_queue(move |tracks, idx| {
            handle_play_queue_internal(&s_srch_q, &w_srch_q, tracks, idx);
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

        let s_srch_like = Rc::clone(&state_c);
        let w_srch_like = Rc::clone(&win_c);
        self.window.search_view.connect_like_track(move |track, liked| {
            handle_track_like_internal(&s_srch_like, &w_srch_like, track, liked);
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

        let s_srch_up_next = Rc::clone(&state_c);
        let w_srch_up_next = Rc::clone(&win_c);
        self.window.search_view.connect_queue_track(move |track| {
            handle_add_to_up_next(&s_srch_up_next, &w_srch_up_next, track);
        });

        let s_srch_stash = Rc::clone(&state_c);
        let w_srch_stash = Rc::clone(&win_c);
        self.window.search_view.connect_stash_track(move |track| {
            handle_toggle_stash_internal(&s_srch_stash, &w_srch_stash, track);
        });
        let s_srch_chk = Rc::clone(&state_c);
        self.window.search_view.set_stash_checker(move |track_id| {
            s_srch_chk.db.is_track_stashed(track_id)
        });

        // 2b. Artist Profile View
        let s_art_q = Rc::clone(&state_c);
        let w_art_q = Rc::clone(&win_c);
        self.window.artist_profile_view.connect_play_queue(move |tracks, idx| {
            handle_play_queue_internal(&s_art_q, &w_art_q, tracks, idx);
        });

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
            handle_play_queue_internal(&s_art_all, &w_art_all, tracks, 0);
        });

        let s_art_like = Rc::clone(&state_c);
        let w_art_like = Rc::clone(&win_c);
        self.window.artist_profile_view.connect_like_track(move |track, liked| {
            handle_track_like_internal(&s_art_like, &w_art_like, track, liked);
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

        let s_art_up_next = Rc::clone(&state_c);
        let w_art_up_next = Rc::clone(&win_c);
        self.window.artist_profile_view.connect_queue_track(move |track| {
            handle_add_to_up_next(&s_art_up_next, &w_art_up_next, track);
        });

        let s_art_stash = Rc::clone(&state_c);
        let w_art_stash = Rc::clone(&win_c);
        self.window.artist_profile_view.connect_stash_track(move |track| {
            handle_toggle_stash_internal(&s_art_stash, &w_art_stash, track);
        });
        let s_art_chk = Rc::clone(&state_c);
        self.window.artist_profile_view.set_stash_checker(move |track_id| {
            s_art_chk.db.is_track_stashed(track_id)
        });

        // 3. Library View
        let s_lib_q = Rc::clone(&state_c);
        let w_lib_q = Rc::clone(&win_c);
        self.window.library_view.connect_play_queue(move |tracks, idx| {
            handle_play_queue_internal(&s_lib_q, &w_lib_q, tracks, idx);
        });

        let s_lib_play = Rc::clone(&state_c);
        let w_lib_play = Rc::clone(&win_c);
        self.window.library_view.connect_play_track(move |track| {
            let s = Rc::clone(&s_lib_play);
            let w = Rc::clone(&w_lib_play);
            glib::spawn_future_local(async move {
                let _ = play_track_internal(&s, &w, track).await;
            });
        });

        let s_lib_like = Rc::clone(&state_c);
        let w_lib_like = Rc::clone(&win_c);
        self.window.library_view.connect_like_track(move |track, liked| {
            handle_track_like_internal(&s_lib_like, &w_lib_like, track, liked);
        });

        let s_lib_sel = Rc::clone(&state_c);
        let w_lib_sel = Rc::clone(&win_c);
        self.window.library_view.connect_select_track(move |track| {
            show_track_detail_internal(&s_lib_sel, &w_lib_sel, track);
        });

        let s_lib_art = Rc::clone(&state_c);
        let w_lib_art = Rc::clone(&win_c);
        self.window.library_view.connect_artist_click(move |user_id| {
            show_artist_profile_internal(&s_lib_art, &w_lib_art, user_id);
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
                crate::ui::views::LibraryTab::Stash => {
                    if let Ok(stashed) = s_lib_tab.db.get_stashed_tracks() {
                        w_lib_tab.library_view.set_stashed_tracks(stashed);
                    }
                }
            }
        });

        let s_lib_rem_st = Rc::clone(&state_c);
        let w_lib_rem_st = Rc::clone(&win_c);
        self.window.library_view.connect_remove_stash(move |track_id| {
            if let Some(path) = s_lib_rem_st.db.get_stashed_file_path(track_id) {
                let _ = std::fs::remove_file(path);
            }
            let _ = s_lib_rem_st.db.remove_stashed_track(track_id);
            if let Ok(stashed) = s_lib_rem_st.db.get_stashed_tracks() {
                w_lib_rem_st.library_view.set_stashed_tracks(stashed);
            }
            if let Some(ref cur) = *s_lib_rem_st.current_track.borrow() {
                if cur.id == track_id {
                    w_lib_rem_st.player_bar().set_stashed(false);
                    w_lib_rem_st.track_detail_view.set_stashed(false);
                }
            }
        });

        let s_lib_up_next = Rc::clone(&state_c);
        let w_lib_up_next = Rc::clone(&win_c);
        self.window.library_view.connect_queue_track(move |track| {
            handle_add_to_up_next(&s_lib_up_next, &w_lib_up_next, track);
        });

        let s_lib_stash = Rc::clone(&state_c);
        let w_lib_stash = Rc::clone(&win_c);
        self.window.library_view.connect_stash_track(move |track| {
            handle_toggle_stash_internal(&s_lib_stash, &w_lib_stash, track);
        });
        let s_lib_chk = Rc::clone(&state_c);
        self.window.library_view.set_stash_checker(move |track_id| {
            s_lib_chk.db.is_track_stashed(track_id)
        });

        // 4. Playlist View (inside Library)
        let s_pl_q = Rc::clone(&state_c);
        let w_pl_q = Rc::clone(&win_c);
        self.window.library_view.playlist_view.connect_play_queue(move |tracks, start_idx| {
            handle_play_queue_internal(&s_pl_q, &w_pl_q, tracks, start_idx);
        });

        let s_pl_play = Rc::clone(&state_c);
        let w_pl_play = Rc::clone(&win_c);
        self.window.library_view.playlist_view.connect_play_track(move |track| {
            let s = Rc::clone(&s_pl_play);
            let w = Rc::clone(&w_pl_play);
            glib::spawn_future_local(async move {
                let _ = play_track_internal(&s, &w, track).await;
            });
        });

        let s_pl_trk_like = Rc::clone(&state_c);
        let w_pl_trk_like = Rc::clone(&win_c);
        self.window.library_view.playlist_view.connect_like_track(move |track, liked| {
            handle_track_like_internal(&s_pl_trk_like, &w_pl_trk_like, track, liked);
        });

        let s_pl_det = Rc::clone(&state_c);
        let w_pl_det = Rc::clone(&win_c);
        self.window.library_view.playlist_view.connect_select_track(move |track| {
            show_track_detail_internal(&s_pl_det, &w_pl_det, track);
        });

        let s_pl_art = Rc::clone(&state_c);
        let w_pl_art = Rc::clone(&win_c);
        self.window.library_view.playlist_view.connect_artist_click(move |user_id| {
            show_artist_profile_internal(&s_pl_art, &w_pl_art, user_id);
        });

        let s_pl_up_next = Rc::clone(&state_c);
        let w_pl_up_next = Rc::clone(&win_c);
        self.window.library_view.playlist_view.connect_queue_track(move |track| {
            handle_add_to_up_next(&s_pl_up_next, &w_pl_up_next, track);
        });

        let s_pl_stash = Rc::clone(&state_c);
        let w_pl_stash = Rc::clone(&win_c);
        self.window.library_view.playlist_view.connect_stash_track(move |track| {
            handle_toggle_stash_internal(&s_pl_stash, &w_pl_stash, track);
        });
        let s_pl_chk = Rc::clone(&state_c);
        self.window.library_view.playlist_view.set_stash_checker(move |track_id| {
            s_pl_chk.db.is_track_stashed(track_id)
        });

        let s_pl_unlike = Rc::clone(&state_c);
        let w_pl_unlike = Rc::clone(&win_c);
        self.window.library_view.playlist_view.connect_unlike_playlist(move |playlist_id| {
            let s = Rc::clone(&s_pl_unlike);
            let w = Rc::clone(&w_pl_unlike);
            glib::spawn_future_local(async move {
                if let Err(e) = s.client.toggle_like_playlist(playlist_id, false).await {
                    warn!("Failed to unlike playlist {}: {}", playlist_id, e);
                }
                if let Ok(pls) = s.client.get_my_playlists(30).await {
                    w.library_view.set_playlists(pls);
                }
            });
        });

        let audio_pl_shuf = Arc::clone(&self.state.audio);
        let win_pl_shuf = Rc::clone(&self.window);
        let state_pl_shuf = Rc::clone(&self.state);
        self.window.library_view.playlist_view.connect_shuffle_toggled(move |active| {
            let q_arc = audio_pl_shuf.get_queue();
            let mut q = q_arc.lock().unwrap();
            q.set_shuffle(active);
            let curr = q.current.clone();
            let items = q.items.clone();
            drop(q);
            win_pl_shuf.player_bar().set_shuffle(active);
            win_pl_shuf.player_bar().update_queue(curr.as_ref(), &items);
            let m_c = Rc::clone(&state_pl_shuf.mpris);
            glib::spawn_future_local(async move {
                if let Some(ref m) = *m_c.borrow() {
                    m.update_shuffle(active).await;
                }
            });
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

        let audio_q_pl = Arc::clone(&state_c.audio);
        let win_q_pl = Rc::clone(&win_c);
        self.window.library_view.playlist_view.connect_queue_track(move |track| {
            let q_arc = audio_q_pl.get_queue();
            let mut q = q_arc.lock().unwrap();
            q.add_tail(track);
            let curr = q.current.clone();
            let items = q.items.clone();
            drop(q);
            win_q_pl.player_bar().update_queue(curr.as_ref(), &items);
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

        let win_td_seek = Rc::clone(&self.window);
        let state_td_seek = Rc::clone(&self.state);
        self.window.track_detail_view.connect_seek(move |pct| {
            if let Some(detail_track) = win_td_seek.track_detail_view.current_track() {
                let is_loaded = state_td_seek.current_track.borrow().as_ref().map(|t| t.id) == Some(detail_track.id)
                    && state_td_seek.audio.is_loaded();
                let dur = if is_loaded {
                    state_td_seek.audio.get_duration_ms().max(detail_track.duration)
                } else {
                    detail_track.duration
                };
                if dur > 0 {
                    let target_ms = (pct.clamp(0.0, 1.0) * dur as f64) as u64;
                    jump_to_track_position(&state_td_seek, &win_td_seek, detail_track, target_ms, false);
                }
            }
        });

        let win_td_jump = Rc::clone(&self.window);
        let state_td_jump = Rc::clone(&self.state);
        self.window.track_detail_view.connect_jump_timestamp(move |track, target_ms| {
            jump_to_track_position(&state_td_jump, &win_td_jump, track, target_ms, true);
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

        // Track Detail Personal Bookmarks & Stash
        let s_td_bm = Rc::clone(&state_c);
        let w_td_bm = Rc::clone(&win_c);
        self.window.track_detail_view.connect_add_bookmark(move |track_id, timestamp_ms, note| {
            let _ = s_td_bm.db.add_bookmark(track_id, timestamp_ms, &note);
            let fallback_dur = w_td_bm
                .track_detail_view
                .current_track()
                .map(|t| t.duration)
                .unwrap_or(0);
            refresh_bookmarks_ui(&s_td_bm, &w_td_bm, track_id, fallback_dur);
        });

        let s_td_del_bm = Rc::clone(&state_c);
        let w_td_del_bm = Rc::clone(&win_c);
        self.window.track_detail_view.connect_delete_bookmark(move |bookmark_id| {
            let _ = s_td_del_bm.db.delete_bookmark(bookmark_id);
            if let Some(track) = w_td_del_bm
                .track_detail_view
                .current_track()
                .or_else(|| s_td_del_bm.current_track.borrow().clone())
            {
                refresh_bookmarks_ui(&s_td_del_bm, &w_td_del_bm, track.id, track.duration);
            }
        });

        let s_td_stash = Rc::clone(&state_c);
        let w_td_stash = Rc::clone(&win_c);
        self.window.track_detail_view.connect_stash(move || {
            if let Some(track) = w_td_stash
                .track_detail_view
                .current_track()
                .or_else(|| s_td_stash.current_track.borrow().clone())
            {
                handle_toggle_stash_internal(&s_td_stash, &w_td_stash, track);
            }
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
                let token_opt = tokio::task::spawn_blocking(crate::api::browser_cookies::scan_for_soundcloud_cookie)
                    .await
                    .ok()
                    .flatten();
                if let Some(token) = token_opt {
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

        // Audio dynamic normalization toggle
        let audio_norm = Arc::clone(&state_c.audio);
        let db_norm = state_c.db.clone();
        self.window.settings_view.connect_audio_normalization(move |enabled| {
            audio_norm.set_audio_normalization(enabled);
            let _ = db_norm.set_bool_setting("audio_normalization", enabled);
        });

        // Desktop notifications toggle
        let db_notif = state_c.db.clone();
        self.window.settings_view.connect_desktop_notifications(move |enabled| {
            let _ = db_notif.set_bool_setting("desktop_notifications", enabled);
        });

        // Theme change handler
        let db_theme = state_c.db.clone();
        let win_theme_h = Rc::clone(&win_c);
        self.window.settings_view.connect_theme_changed(move |theme| {
            crate::ui::theme::apply_theme(theme);
            let _ = db_theme.set_setting("app_theme", theme.as_str());
            win_theme_h.queue_redraw_waveforms();
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

        // Restore audio normalization setting
        let norm_enabled = self.state.db.get_bool_setting("audio_normalization", false);
        self.state.audio.set_audio_normalization(norm_enabled);
        self.window.settings_view.set_audio_normalization(norm_enabled);

        // Restore desktop notifications setting
        let notif_enabled = self.state.db.get_bool_setting("desktop_notifications", true);
        self.window.settings_view.set_desktop_notifications(notif_enabled);

        // Restore active theme setting (default to Anthropic Classical)
        let theme_str = self.state.db.get_setting("app_theme").ok().flatten().unwrap_or_else(|| "anthropic".to_string());
        let initial_theme = crate::ui::theme::AppTheme::from_str(&theme_str);
        crate::ui::theme::apply_theme(initial_theme);
        self.window.settings_view.set_active_theme(initial_theme);

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
                let token_opt = tokio::task::spawn_blocking(crate::api::browser_cookies::scan_for_soundcloud_cookie)
                    .await
                    .ok()
                    .flatten();
                if let Some(cookie_token) = token_opt {
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

            self.window.player_bar().set_track(&saved.track, samples.clone());
            self.window.player_bar().update_position(saved.position_ms, saved.track.duration);
            self.window.player_bar().update_playback_state(false);

            self.window.mini_player().set_track(&saved.track, samples);
            self.window.mini_player().update_position(saved.position_ms, saved.track.duration);
            self.window.mini_player().update_playback_state(false);

            *self.state.current_track.borrow_mut() = Some(saved.track.clone());
            *self.state.pending_resume_pos.borrow_mut() = Some(saved.position_ms);
            self.state.audio.get_queue().lock().unwrap().current = Some(saved.track.clone());

            let mpris_holder = Rc::clone(&self.state.mpris);
            let track_c = saved.track;
            let resume_pos = saved.position_ms;
            glib::spawn_future_local(async move {
                if let Some(ref m) = *mpris_holder.borrow() {
                    m.handler.set_position_micros(resume_pos * 1000);
                    m.update_metadata(Some(&track_c)).await;
                    m.update_status(crate::mpris::PlaybackStatus::Paused).await;
                    m.emit_seeked(resume_pos * 1000).await;
                }
            });
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

fn handle_play_queue_internal(
    state: &Rc<AppState>,
    window: &Rc<MainWindow>,
    tracks: Vec<Track>,
    start_idx: usize,
) {
    if tracks.is_empty() {
        return;
    }
    let first_track = {
        let q_arc = state.audio.get_queue();
        let mut q = q_arc.lock().unwrap();
        let track = q.set_queue(tracks, start_idx);
        let curr = q.current.clone();
        let items = q.items.clone();
        drop(q);
        window.player_bar().update_queue(curr.as_ref(), &items);
        track
    };

    if let Some(track) = first_track {
        let s = Rc::clone(state);
        let w = Rc::clone(window);
        glib::spawn_future_local(async move {
            let _ = play_track_internal(&s, &w, track).await;
        });
    }
}

fn handle_add_to_up_next(
    state: &Rc<AppState>,
    window: &Rc<MainWindow>,
    track: Track,
) {
    let is_idle = {
        let q_arc = state.audio.get_queue();
        let mut q = q_arc.lock().unwrap();
        let idle = q.current.is_none() || state.audio.get_state() != crate::audio::PlaybackState::Playing;
        if !idle {
            q.add_next(track.clone());
            let curr = q.current.clone();
            let items = q.items.clone();
            drop(q);
            window.player_bar().update_queue(curr.as_ref(), &items);
        }
        idle
    };

    if is_idle {
        let s = Rc::clone(state);
        let w = Rc::clone(window);
        glib::spawn_future_local(async move {
            let _ = play_track_internal(&s, &w, track).await;
        });
    }
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

    // 2. Update UI PlayerBar & MiniPlayer immediately
    window.player_bar().set_track(&track, samples.clone());
    window.mini_player().set_track(&track, samples.clone());
    if seek_to_ms > 0 {
        window.player_bar().update_position(seek_to_ms, track.duration);
        window.mini_player().update_position(seek_to_ms, track.duration);
        if window.track_detail_view.current_track_id() == Some(track.id) {
            window.track_detail_view.update_position(seek_to_ms, track.duration);
        }
    }
    window.player_bar().update_playback_state(true);
    window.mini_player().update_playback_state(true);

    *state.current_track.borrow_mut() = Some(track.clone());

    // 3. Resolve playable audio stream: Check if stashed locally for zero-latency offline playback
    let local_file = if let Some(path) = state.db.get_stashed_file_path(track.id) {
        if std::path::Path::new(&path).exists() {
            Some(format!("file://{}", path))
        } else {
            None
        }
    } else {
        None
    };

    let stream_res = if let Some(local_uri) = local_file {
        info!("Instant offline playback from stash: {}", local_uri);
        Ok(local_uri)
    } else {
        state.client.get_stream_url(&track).await
    };

    // Update stash status on player bar and track detail
    let is_stashed = state.db.is_track_stashed(track.id);
    window.player_bar().set_stashed(is_stashed);
    if window.track_detail_view.widget().is_visible() {
        window.track_detail_view.set_stashed(is_stashed);
    }

    // Load and render personal bookmarks for this track on waveform
    if let Ok(bms) = state.db.get_bookmarks(track.id) {
        let dur = track.duration;
        let markers: Vec<(f64, String, i64)> = bms.iter().map(|b| {
            let pct = if dur > 0 { (b.timestamp_ms as f64 / dur as f64).clamp(0.0, 1.0) } else { 0.0 };
            (pct, b.note.clone(), b.id)
        }).collect();
        window.player_bar().waveform().set_bookmarks(markers);
    }

    match stream_res {
        Ok(stream_url) => {
            if state.play_request_counter.load(std::sync::atomic::Ordering::SeqCst) != req_id {
                return Ok(());
            }

            if let Err(e) = state.audio.play_stream(&stream_url, track.clone(), seek_to_ms) {
                error!("Audio engine error playing stream: {}", e);
                state.audio.stop();
                window.player_bar().update_playback_state(false);
                window.mini_player().update_playback_state(false);
            } else {
                // Record history and cache track in database
                let _ = state.db.cache_track(&track, false);
                let _ = state.db.record_playback(track.id);
                let _ = state.db.save_playback_state(&track, seek_to_ms);

                // Update MPRIS metadata
                if let Some(ref m) = *state.mpris.borrow() {
                    m.handler.set_position_micros(seek_to_ms * 1000);
                    m.update_metadata(Some(&track)).await;
                    m.update_status(crate::mpris::PlaybackStatus::Playing).await;
                    m.emit_seeked(seek_to_ms * 1000).await;
                }

                // Sync Queue in PlayerBar popover
                let (curr, items) = {
                    let q_arc = state.audio.get_queue();
                    let q = q_arc.lock().unwrap();
                    (q.current.clone(), q.items.clone())
                };
                window.player_bar().update_queue(curr.as_ref(), &items);
            }
        }
        Err(e) => {
            error!("Could not resolve stream URL for track {}: {}", track.id, e);
            state.audio.stop();
            window.player_bar().update_playback_state(false);
            window.mini_player().update_playback_state(false);
        }
    }

    Ok(())
}

fn refresh_bookmarks_ui(state: &Rc<AppState>, window: &Rc<MainWindow>, track_id: u64, fallback_dur: u64) {
    let audio_dur = if state.current_track.borrow().as_ref().map(|t| t.id) == Some(track_id) {
        state.audio.get_duration_ms()
    } else {
        0
    };
    let dur = audio_dur.max(fallback_dur);
    if let Ok(bms) = state.db.get_bookmarks(track_id) {
        if window.track_detail_view.current_track_id() == Some(track_id) {
            window.track_detail_view.set_bookmarks(bms.clone(), dur);
        }
        if state.current_track.borrow().as_ref().map(|t| t.id) == Some(track_id) {
            let markers: Vec<(f64, String, i64)> = bms
                .into_iter()
                .map(|b| {
                    let pct = if dur > 0 {
                        (b.timestamp_ms as f64 / dur as f64).clamp(0.0, 1.0)
                    } else {
                        0.0
                    };
                    (pct, b.note, b.id)
                })
                .collect();
            window.player_bar().waveform().set_bookmarks(markers);
        }
    }
}

fn jump_to_track_position(
    state: &Rc<AppState>,
    window: &Rc<MainWindow>,
    track: Track,
    target_ms: u64,
    resume_if_paused: bool,
) {
    let same_loaded_track = state.current_track.borrow().as_ref().map(|t| t.id) == Some(track.id)
        && state.audio.is_loaded()
        && state.pending_resume_pos.borrow().is_none();

    if same_loaded_track {
        let dur = state.audio.get_duration_ms().max(track.duration);
        if dur > 0 {
            let clamped_ms = target_ms.min(dur);
            let _ = state.audio.seek(clamped_ms);
            if resume_if_paused && state.audio.get_state() == crate::audio::types::PlaybackState::Paused {
                state.audio.resume();
                window.player_bar().update_playback_state(true);
                window.mini_player().update_playback_state(true);
            }
            window.player_bar().update_position(clamped_ms, dur);
            window.mini_player().update_position(clamped_ms, dur);
            if window.track_detail_view.current_track_id() == Some(track.id) {
                window.track_detail_view.update_position(clamped_ms, dur);
            }
            let m_c = Rc::clone(&state.mpris);
            glib::spawn_future_local(async move {
                if let Some(ref m) = *m_c.borrow() {
                    m.emit_seeked(clamped_ms * 1000).await;
                }
            });
            return;
        }
    }

    state.pending_resume_pos.borrow_mut().take();
    window.player_bar().update_position(target_ms, track.duration);
    window.mini_player().update_position(target_ms, track.duration);
    if window.track_detail_view.current_track_id() == Some(track.id) {
        window.track_detail_view.update_position(target_ms, track.duration);
    }
    let s = Rc::clone(state);
    let w = Rc::clone(window);
    glib::spawn_future_local(async move {
        let _ = play_track_internal_with_seek(&s, &w, track, target_ms).await;
    });
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

        if s_c.current_track.borrow().as_ref().map(|t| t.id) == Some(track_id) {
            let pos = if s_c.audio.is_loaded() {
                s_c.audio.get_position_ms()
            } else {
                s_c.pending_resume_pos.borrow().unwrap_or(0)
            };
            let active_dur = s_c.audio.get_duration_ms().max(dur);
            if active_dur > 0 {
                win_c.track_detail_view.update_position(pos, active_dur);
            }
        }

        // Load personal bookmarks and stash state for track
        if let Ok(bms) = s_c.db.get_bookmarks(track_id) {
            win_c.track_detail_view.set_bookmarks(bms, dur);
        }
        let is_stashed = s_c.db.is_track_stashed(track_id);
        win_c.track_detail_view.set_stashed(is_stashed);

        // Fetch comments asynchronously
        let client = s_c.client.clone();
        let w_c = Rc::clone(&win_c);
        match client.get_comments(track_id, 60).await {
            Ok(comments) => {
                info!("Fetched {} comments for track {}", comments.len(), track_id);
                w_c.track_detail_view.set_comments(comments, dur);
            }
            Err(e) => {
                warn!("Could not fetch comments for track {}: {}", track_id, e);
            }
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

fn handle_track_like_internal(
    state: &Rc<AppState>,
    window: &Rc<MainWindow>,
    track: Track,
    liked: bool,
) {
    let client = state.client.clone();
    let db = state.db.clone();
    let track_id = track.id;
    let mut t = track;
    t.user_favorite = Some(liked);
    let _ = db.cache_track(&t, liked);
    let _ = db.set_track_liked(track_id, liked);

    if let Some(ref cur) = *state.current_track.borrow() {
        if cur.id == track_id {
            window.player_bar().update_like_ui(liked);
        }
    }

    glib::spawn_future_local(async move {
        if let Err(e) = client.toggle_like(track_id, liked).await {
            warn!("Failed to toggle like on track {}: {}", track_id, e);
        }
    });
}

fn handle_add_bookmark_quick(state: &Rc<AppState>, window: &Rc<MainWindow>, note: &str) {
    if let Some(ref track) = *state.current_track.borrow() {
        let pos = state.audio.get_position_ms();
        let _ = state.db.add_bookmark(track.id, pos, note);
        refresh_bookmarks_ui(state, window, track.id, track.duration);
    }
}

fn handle_toggle_stash_internal(state: &Rc<AppState>, window: &Rc<MainWindow>, track: Track) {
    let s = Rc::clone(state);
    let w = Rc::clone(window);
    glib::spawn_future_local(async move {
        let is_stashed = s.db.is_track_stashed(track.id);
        if is_stashed {
            // Remove from stash
            if let Some(path) = s.db.get_stashed_file_path(track.id) {
                let _ = std::fs::remove_file(path);
            }
            let _ = s.db.remove_stashed_track(track.id);
            let is_current = s.current_track.borrow().as_ref().map(|t| t.id == track.id).unwrap_or(false);
            if is_current {
                w.player_bar().set_stashed(false);
            }
            if w.track_detail_view.current_track_id() == Some(track.id) {
                w.track_detail_view.set_stashed(false);
            }
            if let Ok(stashed) = s.db.get_stashed_tracks() {
                w.library_view.set_stashed_tracks(stashed);
            }
            info!("Track {} removed from offline stash", track.id);
        } else {
            // Download track stream to <data_dir>/stash/<track_id>.m4a
            let stash_dir = crate::paths::data_dir().join("stash");
            let _ = std::fs::create_dir_all(&stash_dir);
            let dest_file = stash_dir.join(format!("{}.m4a", track.id));

            match s.client.get_stream_url(&track).await {
                Ok(stream_url) => {
                    info!("Stashing track {} via mpv stream dump from {}", track.id, stream_url);

                    let mut mpv_path = std::path::PathBuf::from("mpv");
                    if let Ok(exe) = std::env::current_exe() {
                        if let Some(dir) = exe.parent() {
                            let local = dir.join(if cfg!(windows) { "mpv.exe" } else { "mpv" });
                            if local.exists() {
                                mpv_path = local;
                            }
                        }
                    }

                    let mut cmd = tokio::process::Command::new(mpv_path);
                    cmd.arg(&stream_url)
                        .arg(format!("--stream-dump={}", dest_file.to_string_lossy()))
                        .arg("--terminal=no")
                        .arg("--no-video");

                    #[cfg(windows)]
                    {
                        cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW
                    }

                    let status = cmd.status().await;

                    match status {
                        Ok(exit_code) if exit_code.success() => {
                            let file_size = std::fs::metadata(&dest_file).map(|m| m.len()).unwrap_or(0);
                            let _ = s.db.save_stashed_track(&track, dest_file.to_str().unwrap_or(""), file_size);
                            let is_current = s.current_track.borrow().as_ref().map(|t| t.id == track.id).unwrap_or(false);
                            if is_current {
                                w.player_bar().set_stashed(true);
                            }
                            if w.track_detail_view.current_track_id() == Some(track.id) {
                                w.track_detail_view.set_stashed(true);
                            }
                            if let Ok(stashed) = s.db.get_stashed_tracks() {
                                w.library_view.set_stashed_tracks(stashed);
                            }
                            info!("Successfully stashed track {} ({:.1} MB)", track.id, file_size as f64 / 1_048_576.0);
                        }
                        _ => {
                            warn!("mpv stream dump failed for track {}, falling back to direct stream fetch", track.id);
                            if !stream_url.contains(".m3u8") {
                                if let Ok(resp) = reqwest::get(&stream_url).await {
                                    if let Ok(bytes) = resp.bytes().await {
                                        if std::fs::write(&dest_file, &bytes).is_ok() {
                                            let file_size = bytes.len() as u64;
                                            let _ = s.db.save_stashed_track(&track, dest_file.to_str().unwrap_or(""), file_size);
                                            let is_current = s.current_track.borrow().as_ref().map(|t| t.id == track.id).unwrap_or(false);
                                            if is_current {
                                                w.player_bar().set_stashed(true);
                                            }
                                            if w.track_detail_view.current_track_id() == Some(track.id) {
                                                w.track_detail_view.set_stashed(true);
                                            }
                                            if let Ok(stashed) = s.db.get_stashed_tracks() {
                                                w.library_view.set_stashed_tracks(stashed);
                                            }
                                            info!("Direct stash succeeded for track {} ({:.1} MB)", track.id, file_size as f64 / 1_048_576.0);
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                Err(e) => {
                    warn!("Could not get stream URL to stash track {}: {}", track.id, e);
                }
            }
        }
    });
}
