use gtk4 as gtk;
use gtk::prelude::*;
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use tracing::info;

use crate::models::track::{format_milliseconds, Track};
use crate::ui::waveform_widget::WaveformWidget;

pub struct PlayerBar {
    container: gtk::Box,
    artwork: gtk::Image,
    title_label: gtk::Label,
    artist_label: gtk::Label,
    like_btn: gtk::Button,
    btn_play_pause: gtk::Button,
    btn_prev: gtk::Button,
    btn_next: gtk::Button,
    btn_shuffle: gtk::Button,
    btn_repeat: gtk::Button,
    waveform: Rc<WaveformWidget>,
    time_current: gtk::Label,
    time_total: gtk::Label,
    volume_scale: gtk::Scale,
    vol_btn: gtk::Button,
    share_btn: gtk::Button,
    current_track: Rc<RefCell<Option<Track>>>,
    is_playing: Rc<RefCell<bool>>,
    is_liked: Rc<RefCell<bool>>,
    saved_volume: Rc<Cell<f64>>,
    is_muted: Rc<Cell<bool>>,
}

impl PlayerBar {
    pub fn new() -> Self {
        let container = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(4)
            .css_classes(["player-bar"])
            .hexpand(true)
            .build();

        // Top row: waveform and time labels
        let wave_row = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(12)
            .hexpand(true)
            .valign(gtk::Align::Center)
            .build();

        let time_current = gtk::Label::builder()
            .label("0:00")
            .css_classes(["time-label"])
            .width_chars(5)
            .xalign(1.0)
            .build();

        let waveform = Rc::new(WaveformWidget::new(38));

        let time_total = gtk::Label::builder()
            .label("0:00")
            .css_classes(["time-label"])
            .width_chars(5)
            .xalign(0.0)
            .build();

        wave_row.append(&time_current);
        wave_row.append(waveform.widget());
        wave_row.append(&time_total);

        // Bottom row: Info, Controls, Volume/Extras
        let controls_row = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(16)
            .hexpand(true)
            .valign(gtk::Align::Center)
            .build();

        // 1. Left section: Track info
        let info_box = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(12)
            .hexpand(false)
            .halign(gtk::Align::Start)
            .build();

        let artwork = gtk::Image::builder()
            .pixel_size(48)
            .icon_name("audio-x-generic-symbolic")
            .css_classes(["player-artwork"])
            .build();

        let titles_vbox = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(2)
            .valign(gtk::Align::Center)
            .build();

        let title_label = gtk::Label::builder()
            .label("No track selected")
            .css_classes(["player-title"])
            .halign(gtk::Align::Start)
            .ellipsize(gtk::pango::EllipsizeMode::End)
            .max_width_chars(25)
            .build();

        let artist_label = gtk::Label::builder()
            .label("SoundCloud")
            .css_classes(["player-artist"])
            .halign(gtk::Align::Start)
            .ellipsize(gtk::pango::EllipsizeMode::End)
            .max_width_chars(25)
            .build();

        titles_vbox.append(&title_label);
        titles_vbox.append(&artist_label);

        let like_btn = gtk::Button::builder()
            .icon_name("starred-symbolic")
            .css_classes(["btn-transport"])
            .tooltip_text("Like track")
            .valign(gtk::Align::Center)
            .build();

        info_box.append(&artwork);
        info_box.append(&titles_vbox);
        info_box.append(&like_btn);

        // 2. Center section: Transport Buttons
        let center_box = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(8)
            .hexpand(true)
            .halign(gtk::Align::Center)
            .valign(gtk::Align::Center)
            .build();

        let btn_shuffle = gtk::Button::builder()
            .icon_name("media-playlist-shuffle-symbolic")
            .css_classes(["btn-transport"])
            .tooltip_text("Shuffle")
            .build();

        let btn_prev = gtk::Button::builder()
            .icon_name("media-skip-backward-symbolic")
            .css_classes(["btn-transport"])
            .tooltip_text("Previous Track")
            .build();

        let btn_play_pause = gtk::Button::builder()
            .icon_name("media-playback-start-symbolic")
            .css_classes(["btn-transport-play"])
            .tooltip_text("Play")
            .build();

        let btn_next = gtk::Button::builder()
            .icon_name("media-skip-forward-symbolic")
            .css_classes(["btn-transport"])
            .tooltip_text("Next Track")
            .build();

        let btn_repeat = gtk::Button::builder()
            .icon_name("media-playlist-repeat-symbolic")
            .css_classes(["btn-transport"])
            .tooltip_text("Repeat")
            .build();

        center_box.append(&btn_shuffle);
        center_box.append(&btn_prev);
        center_box.append(&btn_play_pause);
        center_box.append(&btn_next);
        center_box.append(&btn_repeat);

        // 3. Right section: Volume & Share
        let right_box = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(12)
            .hexpand(false)
            .halign(gtk::Align::End)
            .valign(gtk::Align::Center)
            .build();

        let share_btn = gtk::Button::builder()
            .icon_name("emblem-shared-symbolic")
            .css_classes(["btn-transport"])
            .tooltip_text("Copy track link to clipboard")
            .build();

        let vol_btn = gtk::Button::builder()
            .icon_name("audio-volume-high-symbolic")
            .css_classes(["btn-transport"])
            .tooltip_text("Mute")
            .valign(gtk::Align::Center)
            .build();

        let volume_scale = gtk::Scale::with_range(gtk::Orientation::Horizontal, 0.0, 1.0, 0.05);
        volume_scale.set_value(1.0);
        volume_scale.set_width_request(100);
        volume_scale.add_css_class("volume-scale");
        volume_scale.set_tooltip_text(Some("Volume"));

        let saved_volume = Rc::new(Cell::new(1.0));
        let is_muted = Rc::new(Cell::new(false));

        // Click on audio icon toggles mute / unmute
        let vol_scale_click = volume_scale.clone();
        let vol_btn_click = vol_btn.clone();
        let saved_vol_click = Rc::clone(&saved_volume);
        let is_muted_click = Rc::clone(&is_muted);

        vol_btn.connect_clicked(move |_| {
            if is_muted_click.get() {
                // Restore volume
                is_muted_click.set(false);
                let prev = saved_vol_click.get();
                let target = if prev <= 0.05 { 0.75 } else { prev };
                vol_scale_click.set_value(target);
                if target > 0.66 {
                    vol_btn_click.set_icon_name("audio-volume-high-symbolic");
                } else if target > 0.33 {
                    vol_btn_click.set_icon_name("audio-volume-medium-symbolic");
                } else {
                    vol_btn_click.set_icon_name("audio-volume-low-symbolic");
                }
                vol_btn_click.set_tooltip_text(Some("Mute"));
            } else {
                // Mute
                let cur = vol_scale_click.value();
                if cur > 0.05 {
                    saved_vol_click.set(cur);
                }
                is_muted_click.set(true);
                vol_scale_click.set_value(0.0);
                vol_btn_click.set_icon_name("audio-volume-muted-symbolic");
                vol_btn_click.set_tooltip_text(Some("Unmute"));
            }
        });

        // When user drags the volume slider, update mute state and icon dynamically
        let vol_btn_change = vol_btn.clone();
        let is_muted_change = Rc::clone(&is_muted);
        let saved_vol_change = Rc::clone(&saved_volume);

        volume_scale.connect_value_changed(move |scale| {
            let val = scale.value();
            if val <= 0.001 {
                is_muted_change.set(true);
                vol_btn_change.set_icon_name("audio-volume-muted-symbolic");
                vol_btn_change.set_tooltip_text(Some("Unmute"));
            } else {
                is_muted_change.set(false);
                saved_vol_change.set(val);
                if val > 0.66 {
                    vol_btn_change.set_icon_name("audio-volume-high-symbolic");
                } else if val > 0.33 {
                    vol_btn_change.set_icon_name("audio-volume-medium-symbolic");
                } else {
                    vol_btn_change.set_icon_name("audio-volume-low-symbolic");
                }
                vol_btn_change.set_tooltip_text(Some("Mute"));
            }
        });

        right_box.append(&share_btn);
        right_box.append(&vol_btn);
        right_box.append(&volume_scale);

        // Assemble row
        controls_row.append(&info_box);
        controls_row.append(&center_box);
        controls_row.append(&right_box);

        container.append(&wave_row);
        container.append(&controls_row);

        let current_track: Rc<RefCell<Option<Track>>> = Rc::new(RefCell::new(None));
        let is_playing = Rc::new(RefCell::new(false));
        let is_liked = Rc::new(RefCell::new(false));

        // Connect share button to Wayland clipboard
        let track_share_clone = Rc::clone(&current_track);
        share_btn.connect_clicked(move |_btn| {
            if let Some(ref track) = *track_share_clone.borrow() {
                if let Some(ref url) = track.permalink_url.as_deref() {
                    if let Some(display) = gdk4::Display::default() {
                        display.clipboard().set_text(url);
                        info!("Copied track link to clipboard: {}", url);
                    }
                }
            }
        });

        Self {
            container,
            artwork,
            title_label,
            artist_label,
            like_btn,
            btn_play_pause,
            btn_prev,
            btn_next,
            btn_shuffle,
            btn_repeat,
            waveform,
            time_current,
            time_total,
            volume_scale,
            vol_btn,
            share_btn,
            current_track,
            is_playing,
            is_liked,
            saved_volume,
            is_muted,
        }
    }

    pub fn widget(&self) -> &gtk::Box {
        &self.container
    }

    pub fn set_track(&self, track: &Track, samples: Vec<f32>) {
        *self.current_track.borrow_mut() = Some(track.clone());
        self.title_label.set_label(&track.title);
        self.artist_label.set_label(track.artist_name());
        self.time_total.set_label(&track.duration_formatted());
        self.time_current.set_label("0:00");

        let art_url = track.best_artwork_url().or_else(|| track.thumbnail_url());
        crate::ui::image_loader::load_into_image(
            &self.artwork,
            art_url.as_deref(),
            "audio-x-generic-symbolic",
        );

        self.waveform.set_samples(samples);
        self.waveform.set_progress(0.0);

        let liked = track.user_favorite.unwrap_or(false);
        *self.is_liked.borrow_mut() = liked;
        self.update_like_ui(liked);
    }

    pub fn update_playback_state(&self, playing: bool) {
        *self.is_playing.borrow_mut() = playing;
        if playing {
            self.btn_play_pause
                .set_icon_name("media-playback-pause-symbolic");
            self.btn_play_pause.set_tooltip_text(Some("Pause"));
        } else {
            self.btn_play_pause
                .set_icon_name("media-playback-start-symbolic");
            self.btn_play_pause.set_tooltip_text(Some("Play"));
        }
    }

    pub fn update_position(&self, position_ms: u64, duration_ms: u64) {
        self.time_current.set_label(&format_milliseconds(position_ms));
        if duration_ms > 0 {
            let pct = (position_ms as f64 / duration_ms as f64).clamp(0.0, 1.0);
            self.waveform.set_progress(pct);
        }
    }

    pub fn update_like_ui(&self, liked: bool) {
        if liked {
            self.like_btn.add_css_class("btn-heart-liked");
            self.like_btn.set_icon_name("starred-symbolic");
            self.like_btn.set_tooltip_text(Some("Unlike track"));
        } else {
            self.like_btn.remove_css_class("btn-heart-liked");
            self.like_btn.set_icon_name("non-starred-symbolic");
            self.like_btn.set_tooltip_text(Some("Like track"));
        }
    }

    pub fn connect_play_pause<F: Fn() + 'static>(&self, f: F) {
        self.btn_play_pause.connect_clicked(move |_| f());
    }

    pub fn connect_prev<F: Fn() + 'static>(&self, f: F) {
        self.btn_prev.connect_clicked(move |_| f());
    }

    pub fn connect_next<F: Fn() + 'static>(&self, f: F) {
        self.btn_next.connect_clicked(move |_| f());
    }

    pub fn connect_shuffle<F: Fn() + 'static>(&self, f: F) {
        let btn_clone = self.btn_shuffle.clone();
        self.btn_shuffle.connect_clicked(move |_| {
            if btn_clone.has_css_class("active") {
                btn_clone.remove_css_class("active");
            } else {
                btn_clone.add_css_class("active");
            }
            f();
        });
    }

    pub fn connect_repeat<F: Fn() + 'static>(&self, f: F) {
        self.btn_repeat.connect_clicked(move |_| f());
    }

    pub fn connect_seek<F: Fn(f64) + 'static>(&self, f: F) {
        self.waveform.connect_seek(f);
    }

    pub fn connect_volume<F: Fn(f64) + 'static>(&self, f: F) {
        self.volume_scale.connect_value_changed(move |scale| {
            f(scale.value());
        });
    }

    pub fn connect_like<F: Fn(bool) + 'static>(&self, f: F) {
        let is_liked = Rc::clone(&self.is_liked);
        let like_btn = self.like_btn.clone();
        self.like_btn.connect_clicked(move |_| {
            let next = !*is_liked.borrow();
            *is_liked.borrow_mut() = next;
            if next {
                like_btn.add_css_class("btn-heart-liked");
                like_btn.set_icon_name("starred-symbolic");
            } else {
                like_btn.remove_css_class("btn-heart-liked");
                like_btn.set_icon_name("non-starred-symbolic");
            }
            f(next);
        });
    }

    pub fn waveform(&self) -> &WaveformWidget {
        &self.waveform
    }
}
