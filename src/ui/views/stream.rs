use gtk4 as gtk;
use gtk::prelude::*;
use std::cell::RefCell;
use std::rc::Rc;

use crate::api::waveform::generate_synthetic_waveform;
use crate::models::track::Track;
use crate::ui::waveform_widget::WaveformWidget;

pub struct StreamView {
    container: gtk::Box,
    tracks_flow: gtk::Box,
    genre_box: gtk::Box,
    loading_spinner: gtk::Spinner,
    on_play_track: Rc<RefCell<Option<Box<dyn Fn(Track)>>>>,
    on_select_track: Rc<RefCell<Option<Box<dyn Fn(Track)>>>>,
    on_genre_change: Rc<RefCell<Option<Box<dyn Fn(String)>>>>,
    current_genre: Rc<RefCell<String>>,
}

impl StreamView {
    pub fn new() -> Self {
        let container = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(16)
            .margin_top(16)
            .margin_bottom(16)
            .margin_start(20)
            .margin_end(20)
            .hexpand(true)
            .vexpand(true)
            .build();

        // Header and Genre Filter Pills
        let header_row = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(16)
            .build();

        let title = gtk::Label::builder()
            .label("Discover & Charts")
            .css_classes(["brand-label"])
            .halign(gtk::Align::Start)
            .build();

        let loading_spinner = gtk::Spinner::builder()
            .spinning(false)
            .halign(gtk::Align::Start)
            .build();

        header_row.append(&title);
        header_row.append(&loading_spinner);
        container.append(&header_row);

        // Genre pills bar
        let genre_scroller = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Automatic)
            .vscrollbar_policy(gtk::PolicyType::Never)
            .hexpand(true)
            .min_content_height(40)
            .build();

        let genre_box = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(8)
            .build();

        genre_scroller.set_child(Some(&genre_box));
        container.append(&genre_scroller);

        // Tracks listing area inside a scrolled window
        let scrolled = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Never)
            .vscrollbar_policy(gtk::PolicyType::Automatic)
            .hexpand(true)
            .vexpand(true)
            .build();

        let tracks_flow = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(12)
            .hexpand(true)
            .build();

        scrolled.set_child(Some(&tracks_flow));
        container.append(&scrolled);

        let on_play_track = Rc::new(RefCell::new(None));
        let on_select_track = Rc::new(RefCell::new(None));
        let on_genre_change = Rc::new(RefCell::new(None));
        let current_genre = Rc::new(RefCell::new("my_stream".to_string()));

        let view = Self {
            container,
            tracks_flow,
            genre_box,
            loading_spinner,
            on_play_track,
            on_select_track,
            on_genre_change,
            current_genre,
        };

        let default_genres = vec![
            ("Personal Stream".to_string(), "my_stream".to_string()),
            ("Mixed For You".to_string(), "mixed_for_you".to_string()),
            ("Trending".to_string(), "trending".to_string()),
        ];
        view.set_genres(default_genres);
        view
    }

    pub fn widget(&self) -> &gtk::Box {
        &self.container
    }

    pub fn set_genres(&self, genres: Vec<(String, String)>) {
        while let Some(child) = self.genre_box.first_child() {
            self.genre_box.remove(&child);
        }

        let cur_key = self.current_genre.borrow().clone();
        let mut has_active = false;

        for (label, key) in genres {
            let is_match = key == cur_key;
            let chip = gtk::Button::builder()
                .label(&label)
                .css_classes(["filter-chip"])
                .build();

            if is_match || (!has_active && (key == "my_stream" || cur_key.is_empty())) {
                chip.add_css_class("active");
                has_active = true;
            }

            let key_str = key.clone();
            let genre_cb = Rc::clone(&self.on_genre_change);
            let current_g = Rc::clone(&self.current_genre);
            let chip_clone = chip.clone();
            let genre_box_clone = self.genre_box.clone();

            chip.connect_clicked(move |_| {
                let mut child = genre_box_clone.first_child();
                while let Some(w) = child {
                    w.remove_css_class("active");
                    child = w.next_sibling();
                }
                chip_clone.add_css_class("active");
                *current_g.borrow_mut() = key_str.clone();

                if let Some(ref cb) = *genre_cb.borrow() {
                    cb(key_str.clone());
                }
            });

            self.genre_box.append(&chip);
        }
    }

    pub fn set_loading(&self, loading: bool) {
        self.loading_spinner.set_spinning(loading);
    }

    pub fn set_tracks(&self, tracks: Vec<Track>) {
        // Clear existing tracks
        while let Some(child) = self.tracks_flow.first_child() {
            self.tracks_flow.remove(&child);
        }

        if tracks.is_empty() {
            let empty_label = gtk::Label::builder()
                .label("No tracks found in this category.")
                .css_classes(["player-artist"])
                .margin_top(40)
                .build();
            self.tracks_flow.append(&empty_label);
            return;
        }

        for track in tracks {
            let card = self.create_track_card(track);
            self.tracks_flow.append(&card);
        }
    }

    fn create_track_card(&self, track: Track) -> gtk::Box {
        let card = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(14)
            .css_classes(["track-card"])
            .hexpand(true)
            .build();

        // 1. Play button & Artwork Overlay
        let art_overlay = gtk::Overlay::builder()
            .valign(gtk::Align::Center)
            .halign(gtk::Align::Center)
            .build();

        let art_img = gtk::Image::builder()
            .pixel_size(56)
            .icon_name("audio-x-generic-symbolic")
            .css_classes(["track-card-artwork"])
            .build();

        let art_url = track.thumbnail_url();
        crate::ui::image_loader::load_into_image(
            &art_img,
            art_url.as_deref(),
            "audio-x-generic-symbolic",
        );

        let play_btn = gtk::Button::builder()
            .icon_name("media-playback-start-symbolic")
            .css_classes(["btn-transport-play"])
            .width_request(36)
            .height_request(36)
            .valign(gtk::Align::Center)
            .halign(gtk::Align::Center)
            .tooltip_text("Play track")
            .build();

        let track_for_play = track.clone();
        let play_cb = Rc::clone(&self.on_play_track);
        play_btn.connect_clicked(move |_| {
            if let Some(ref cb) = *play_cb.borrow() {
                cb(track_for_play.clone());
            }
        });

        art_overlay.set_child(Some(&art_img));
        art_overlay.add_overlay(&play_btn);
        card.append(&art_overlay);

        // 2. Track Info & Waveform preview
        let content_box = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(6)
            .hexpand(true)
            .build();

        let title_row = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(8)
            .build();

        let title_lbl = gtk::Label::builder()
            .label(&track.title)
            .css_classes(["track-card-title"])
            .halign(gtk::Align::Start)
            .hexpand(true)
            .ellipsize(gtk::pango::EllipsizeMode::End)
            .build();

        let duration_lbl = gtk::Label::builder()
            .label(&track.duration_formatted())
            .css_classes(["time-label"])
            .halign(gtk::Align::End)
            .build();

        title_row.append(&title_lbl);
        title_row.append(&duration_lbl);

        let artist_lbl = gtk::Label::builder()
            .label(track.artist_name())
            .css_classes(["track-card-artist"])
            .halign(gtk::Align::Start)
            .build();

        // Waveform preview
        let wave = WaveformWidget::new(32);
        let samples = generate_synthetic_waveform(track.id, 90);
        wave.set_samples(samples);

        // Engagement stats
        let stats_row = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(16)
            .build();

        let plays_text = format!("{} plays", format_count(track.playback_count.unwrap_or(0)));
        let likes_text = format!("{} likes", format_count(track.likes_count.unwrap_or(0)));
        let reposts_text = format!("{} reposts", format_count(track.reposts_count.unwrap_or(0)));
        let comments_text = format!("{} comments", format_count(track.comment_count.unwrap_or(0)));

        for text in [plays_text, likes_text, reposts_text, comments_text] {
            let stat_lbl = gtk::Label::builder()
                .label(&text)
                .css_classes(["track-card-stats"])
                .build();
            stats_row.append(&stat_lbl);
        }

        content_box.append(&title_row);
        content_box.append(&artist_lbl);
        content_box.append(wave.widget());
        content_box.append(&stats_row);

        card.append(&content_box);

        // Make card clickable to play track immediately (consistent with Library)
        let click_gesture = gtk::GestureClick::new();
        let track_for_play_card = track.clone();
        let play_cb_card = Rc::clone(&self.on_play_track);
        click_gesture.connect_pressed(move |_g, _n, _x, _y| {
            if let Some(ref cb) = *play_cb_card.borrow() {
                cb(track_for_play_card.clone());
            }
        });
        card.add_controller(click_gesture);

        // Click title to view comments & track details
        let title_gesture = gtk::GestureClick::new();
        let track_for_detail = track.clone();
        let detail_cb = Rc::clone(&self.on_select_track);
        title_gesture.connect_pressed(move |_g, _n, _x, _y| {
            if let Some(ref cb) = *detail_cb.borrow() {
                cb(track_for_detail.clone());
            }
        });
        title_lbl.add_controller(title_gesture);
        title_lbl.set_tooltip_text(Some("Click title to view comments & details"));

        card
    }

    pub fn connect_play_track<F: Fn(Track) + 'static>(&self, f: F) {
        *self.on_play_track.borrow_mut() = Some(Box::new(f));
    }

    pub fn connect_select_track<F: Fn(Track) + 'static>(&self, f: F) {
        *self.on_select_track.borrow_mut() = Some(Box::new(f));
    }

    pub fn connect_genre_change<F: Fn(String) + 'static>(&self, f: F) {
        *self.on_genre_change.borrow_mut() = Some(Box::new(f));
    }
}

pub fn format_count(count: u64) -> String {
    if count >= 1_000_000 {
        format!("{:.1}M", count as f64 / 1_000_000.0)
    } else if count >= 1_000 {
        format!("{:.1}K", count as f64 / 1_000.0)
    } else {
        count.to_string()
    }
}
