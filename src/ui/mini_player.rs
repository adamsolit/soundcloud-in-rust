use gtk4 as gtk;
use gtk::prelude::*;
use libadwaita as adw;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

use crate::models::track::{format_milliseconds, Track};
use crate::ui::waveform_widget::WaveformWidget;

pub struct MiniPlayerWindow {
    window: gtk::Window,
    artwork: gtk::Image,
    title_lbl: gtk::Label,
    artist_lbl: gtk::Label,
    time_lbl: gtk::Label,
    btn_play_pause: gtk::Button,
    btn_prev: gtk::Button,
    btn_next: gtk::Button,
    btn_like: gtk::Button,
    btn_restore: gtk::Button,
    waveform: Rc<WaveformWidget>,
    is_playing: Rc<Cell<bool>>,
    is_liked: Rc<Cell<bool>>,
    current_track: Rc<RefCell<Option<Track>>>,
}

impl MiniPlayerWindow {
    pub fn new(app: &adw::Application) -> Self {
        let window = gtk::Window::builder()
            .application(app)
            .title("SoundCloud Mini Player")
            .icon_name("soundcloud")
            .default_width(440)
            .default_height(115)
            .resizable(false)
            .css_classes(["mini-player-window"])
            .build();

        let root_box = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(6)
            .margin_top(8)
            .margin_bottom(8)
            .margin_start(12)
            .margin_end(12)
            .css_classes(["mini-player-card"])
            .build();

        // Top Row: Artwork + Track Details + Controls
        let top_row = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(10)
            .hexpand(true)
            .valign(gtk::Align::Center)
            .build();

        let artwork = gtk::Image::builder()
            .pixel_size(44)
            .icon_name("audio-x-generic-symbolic")
            .css_classes(["mini-player-artwork"])
            .valign(gtk::Align::Center)
            .build();

        let meta_vbox = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(2)
            .hexpand(true)
            .valign(gtk::Align::Center)
            .build();

        let title_row = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(6)
            .hexpand(true)
            .build();

        let title_lbl = gtk::Label::builder()
            .label("No track selected")
            .css_classes(["mini-player-title"])
            .halign(gtk::Align::Start)
            .hexpand(true)
            .ellipsize(gtk::pango::EllipsizeMode::End)
            .build();

        let time_lbl = gtk::Label::builder()
            .label("0:00 / 0:00")
            .css_classes(["mini-player-time"])
            .halign(gtk::Align::End)
            .valign(gtk::Align::Center)
            .build();

        title_row.append(&title_lbl);
        title_row.append(&time_lbl);

        let artist_lbl = gtk::Label::builder()
            .label("SoundCloud")
            .css_classes(["mini-player-artist"])
            .halign(gtk::Align::Start)
            .ellipsize(gtk::pango::EllipsizeMode::End)
            .build();

        meta_vbox.append(&title_row);
        meta_vbox.append(&artist_lbl);

        let controls_box = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(4)
            .valign(gtk::Align::Center)
            .build();

        let btn_like = gtk::Button::builder()
            .icon_name("non-starred-symbolic")
            .css_classes(["btn-card-like"])
            .valign(gtk::Align::Center)
            .tooltip_text("Like track")
            .build();

        let btn_prev = gtk::Button::builder()
            .icon_name("media-skip-backward-symbolic")
            .css_classes(["btn-transport"])
            .valign(gtk::Align::Center)
            .tooltip_text("Previous track")
            .build();

        let btn_play_pause = gtk::Button::builder()
            .icon_name("media-playback-start-symbolic")
            .css_classes(["btn-transport-play"])
            .width_request(34)
            .height_request(34)
            .valign(gtk::Align::Center)
            .tooltip_text("Play / Pause")
            .build();

        let btn_next = gtk::Button::builder()
            .icon_name("media-skip-forward-symbolic")
            .css_classes(["btn-transport"])
            .valign(gtk::Align::Center)
            .tooltip_text("Next track")
            .build();

        let btn_restore = gtk::Button::builder()
            .icon_name("view-restore-symbolic")
            .css_classes(["btn-transport"])
            .valign(gtk::Align::Center)
            .tooltip_text("Restore Full Window")
            .build();

        controls_box.append(&btn_like);
        controls_box.append(&btn_prev);
        controls_box.append(&btn_play_pause);
        controls_box.append(&btn_next);
        controls_box.append(&btn_restore);

        top_row.append(&artwork);
        top_row.append(&meta_vbox);
        top_row.append(&controls_box);

        // Bottom Row: Seekable Waveform
        let waveform = Rc::new(WaveformWidget::new(26));

        root_box.append(&top_row);
        root_box.append(waveform.widget());

        window.set_child(Some(&root_box));

        Self {
            window,
            artwork,
            title_lbl,
            artist_lbl,
            time_lbl,
            btn_play_pause,
            btn_prev,
            btn_next,
            btn_like,
            btn_restore,
            waveform,
            is_playing: Rc::new(Cell::new(false)),
            is_liked: Rc::new(Cell::new(false)),
            current_track: Rc::new(RefCell::new(None)),
        }
    }

    pub fn window(&self) -> &gtk::Window {
        &self.window
    }

    pub fn set_track(&self, track: &Track, samples: Vec<f32>) {
        *self.current_track.borrow_mut() = Some(track.clone());
        self.title_lbl.set_label(&track.title);
        self.artist_lbl.set_label(track.artist_name());
        self.time_lbl.set_label(&format!("0:00 / {}", format_milliseconds(track.duration)));

        let art_url = track.thumbnail_url();
        crate::ui::image_loader::load_into_image(
            &self.artwork,
            art_url.as_deref(),
            "audio-x-generic-symbolic",
        );

        self.waveform.set_samples(samples);
        self.waveform.set_duration_ms(track.duration);
        self.set_liked(track.user_favorite.unwrap_or(false));
    }

    pub fn update_position(&self, position_ms: u64, duration_ms: u64) {
        if duration_ms > 0 {
            let prog = (position_ms as f64 / duration_ms as f64).clamp(0.0, 1.0);
            self.waveform.set_progress(prog);
        }
        self.time_lbl.set_label(&format!(
            "{} / {}",
            format_milliseconds(position_ms),
            format_milliseconds(duration_ms)
        ));
    }

    pub fn update_playback_state(&self, is_playing: bool) {
        self.is_playing.set(is_playing);
        if is_playing {
            self.btn_play_pause.set_icon_name("media-playback-pause-symbolic");
            self.btn_play_pause.set_tooltip_text(Some("Pause"));
        } else {
            self.btn_play_pause.set_icon_name("media-playback-start-symbolic");
            self.btn_play_pause.set_tooltip_text(Some("Play"));
        }
    }

    pub fn set_liked(&self, liked: bool) {
        self.is_liked.set(liked);
        if liked {
            self.btn_like.set_icon_name("starred-symbolic");
            self.btn_like.add_css_class("btn-heart-liked");
            self.btn_like.set_tooltip_text(Some("Unlike track"));
        } else {
            self.btn_like.set_icon_name("non-starred-symbolic");
            self.btn_like.remove_css_class("btn-heart-liked");
            self.btn_like.set_tooltip_text(Some("Like track"));
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

    pub fn connect_like<F: Fn(bool) + 'static>(&self, f: F) {
        let is_liked = Rc::clone(&self.is_liked);
        self.btn_like.connect_clicked(move |_| {
            let next = !is_liked.get();
            f(next);
        });
    }

    pub fn connect_seek<F: Fn(f64) + 'static>(&self, f: F) {
        self.waveform.connect_seek(f);
    }

    pub fn connect_restore<F: Fn() + 'static>(&self, f: F) {
        self.btn_restore.connect_clicked(move |_| f());
    }
}
