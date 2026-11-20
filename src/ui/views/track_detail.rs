use gtk4 as gtk;
use gtk::prelude::*;
use std::cell::RefCell;
use std::rc::Rc;

use crate::models::comment::Comment;
use crate::models::track::{format_milliseconds, Track};
use crate::ui::waveform_widget::WaveformWidget;

pub struct TrackDetailView {
    container: gtk::Box,
    artwork: gtk::Image,
    title_label: gtk::Label,
    artist_label: gtk::Label,
    genre_label: gtk::Label,
    btn_play: gtk::Button,
    btn_like: gtk::Button,
    waveform: Rc<WaveformWidget>,
    comment_input: gtk::Entry,
    btn_post_comment: gtk::Button,
    comments_list: gtk::Box,
    current_track: Rc<RefCell<Option<Track>>>,
    current_position_ms: Rc<RefCell<u64>>,
    on_play_track: Rc<RefCell<Option<Box<dyn Fn(Track)>>>>,
    on_seek: Rc<RefCell<Option<Box<dyn Fn(f64)>>>>,
    on_post_comment: Rc<RefCell<Option<Box<dyn Fn(u64, String, u64)>>>>,
    on_select_user: Rc<RefCell<Option<Box<dyn Fn(u64)>>>>,
}

impl TrackDetailView {
    pub fn new() -> Self {
        let container = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(20)
            .margin_top(20)
            .margin_bottom(20)
            .margin_start(24)
            .margin_end(24)
            .hexpand(true)
            .vexpand(true)
            .build();

        // 1. Hero Header
        let hero_box = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(20)
            .build();

        let artwork = gtk::Image::builder()
            .pixel_size(120)
            .icon_name("audio-x-generic-symbolic")
            .css_classes(["player-artwork"])
            .build();

        let meta_box = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(8)
            .valign(gtk::Align::Center)
            .hexpand(true)
            .build();

        let title_label = gtk::Label::builder()
            .label("Track Title")
            .css_classes(["brand-label"])
            .halign(gtk::Align::Start)
            .ellipsize(gtk::pango::EllipsizeMode::End)
            .build();

        let artist_label = gtk::Label::builder()
            .label("Artist Name")
            .css_classes(["track-card-artist"])
            .halign(gtk::Align::Start)
            .build();

        let genre_label = gtk::Label::builder()
            .label("Audio")
            .css_classes(["filter-chip"])
            .halign(gtk::Align::Start)
            .build();

        let actions_row = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(12)
            .build();

        let btn_play = gtk::Button::builder()
            .label("Play Track")
            .icon_name("media-playback-start-symbolic")
            .css_classes(["btn-pill-play"])
            .build();

        let btn_like = gtk::Button::builder()
            .icon_name("starred-symbolic")
            .css_classes(["btn-transport"])
            .tooltip_text("Like track")
            .build();

        actions_row.append(&btn_play);
        actions_row.append(&btn_like);

        meta_box.append(&title_label);
        meta_box.append(&artist_label);
        meta_box.append(&genre_label);
        meta_box.append(&actions_row);

        hero_box.append(&artwork);
        hero_box.append(&meta_box);
        container.append(&hero_box);

        // 2. Large Interactive Hero Waveform
        let wave_frame = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(4)
            .css_classes(["track-card"])
            .margin_top(8)
            .margin_bottom(8)
            .margin_start(8)
            .margin_end(8)
            .build();

        let waveform = Rc::new(WaveformWidget::new(80));
        wave_frame.append(waveform.widget());
        container.append(&wave_frame);

        // 3. Comment Input Box ("Add a comment at 1:42...")
        let comment_box = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(12)
            .build();

        let comment_input = gtk::Entry::builder()
            .placeholder_text("Write a comment at 0:00...")
            .css_classes(["sc-search-entry"])
            .hexpand(true)
            .build();

        let btn_post_comment = gtk::Button::builder()
            .label("Post Comment")
            .css_classes(["filter-chip", "active"])
            .build();

        comment_box.append(&comment_input);
        comment_box.append(&btn_post_comment);
        container.append(&comment_box);

        // 4. Community Comments Section
        let comments_title = gtk::Label::builder()
            .label("Community Comments")
            .css_classes(["player-title"])
            .halign(gtk::Align::Start)
            .build();
        container.append(&comments_title);

        let scrolled = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Never)
            .vscrollbar_policy(gtk::PolicyType::Automatic)
            .hexpand(true)
            .vexpand(true)
            .build();

        let comments_list = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(8)
            .hexpand(true)
            .build();

        scrolled.set_child(Some(&comments_list));
        container.append(&scrolled);

        let current_track: Rc<RefCell<Option<Track>>> = Rc::new(RefCell::new(None));
        let current_position_ms = Rc::new(RefCell::new(0));
        let on_play_track: Rc<RefCell<Option<Box<dyn Fn(Track)>>>> = Rc::new(RefCell::new(None));
        let on_seek: Rc<RefCell<Option<Box<dyn Fn(f64)>>>> = Rc::new(RefCell::new(None));
        let on_post_comment: Rc<RefCell<Option<Box<dyn Fn(u64, String, u64)>>>> = Rc::new(RefCell::new(None));
        let on_select_user: Rc<RefCell<Option<Box<dyn Fn(u64)>>>> = Rc::new(RefCell::new(None));

        // Click artist label to open their artist profile
        let user_sel_cb = Rc::clone(&on_select_user);
        let track_user_c = Rc::clone(&current_track);
        let artist_click = gtk::GestureClick::new();
        artist_click.connect_pressed(move |_g, _n, _x, _y| {
            if let Some(ref t) = *track_user_c.borrow() {
                if let Some(ref u) = t.user {
                    if let Some(ref cb) = *user_sel_cb.borrow() {
                        cb(u.id);
                    }
                }
            }
        });
        artist_label.add_controller(artist_click);
        artist_label.set_tooltip_text(Some("Click to view artist profile"));

        // Connect play button
        let track_play = Rc::clone(&current_track);
        let play_cb = Rc::clone(&on_play_track);
        btn_play.connect_clicked(move |_| {
            if let Some(ref t) = *track_play.borrow() {
                if let Some(ref cb) = *play_cb.borrow() {
                    cb(t.clone());
                }
            }
        });

        // Connect waveform seeking
        let seek_cb = Rc::clone(&on_seek);
        waveform.connect_seek(move |pct| {
            if let Some(ref cb) = *seek_cb.borrow() {
                cb(pct);
            }
        });

        // Connect post comment button
        let post_cb = Rc::clone(&on_post_comment);
        let track_post = Rc::clone(&current_track);
        let pos_post = Rc::clone(&current_position_ms);
        let input_c = comment_input.clone();

        btn_post_comment.connect_clicked(move |_| {
            let text = input_c.text().trim().to_string();
            if !text.is_empty() {
                if let Some(ref t) = *track_post.borrow() {
                    let pos = *pos_post.borrow();
                    if let Some(ref cb) = *post_cb.borrow() {
                        cb(t.id, text, pos);
                    }
                    input_c.set_text("");
                }
            }
        });

        Self {
            container,
            artwork,
            title_label,
            artist_label,
            genre_label,
            btn_play,
            btn_like,
            waveform,
            comment_input,
            btn_post_comment,
            comments_list,
            current_track,
            current_position_ms,
            on_play_track,
            on_seek,
            on_post_comment,
            on_select_user,
        }
    }

    pub fn widget(&self) -> &gtk::Box {
        &self.container
    }

    pub fn set_track(&self, track: Track, samples: Vec<f32>) {
        *self.current_track.borrow_mut() = Some(track.clone());
        self.title_label.set_label(&track.title);
        self.artist_label.set_label(track.artist_name());
        self.genre_label
            .set_label(track.genre.as_deref().unwrap_or("SoundCloud"));

        let art_url = track.best_artwork_url().or_else(|| track.thumbnail_url());
        crate::ui::image_loader::load_into_image(
            &self.artwork,
            art_url.as_deref(),
            "audio-x-generic-symbolic",
        );

        self.waveform.set_samples(samples);
        self.waveform.set_progress(0.0);
    }

    pub fn update_position(&self, position_ms: u64, duration_ms: u64) {
        *self.current_position_ms.borrow_mut() = position_ms;
        if duration_ms > 0 {
            let pct = (position_ms as f64 / duration_ms as f64).clamp(0.0, 1.0);
            self.waveform.set_progress(pct);
        }
        self.comment_input.set_placeholder_text(Some(&format!(
            "Write a comment at {}...",
            format_milliseconds(position_ms)
        )));
    }

    pub fn set_comments(&self, comments: Vec<Comment>, duration_ms: u64) {
        while let Some(child) = self.comments_list.first_child() {
            self.comments_list.remove(&child);
        }

        // Update markers on waveform
        let mut markers = Vec::new();
        for c in &comments {
            if let Some(ts) = c.timestamp {
                if duration_ms > 0 {
                    let pct = (ts as f64 / duration_ms as f64).clamp(0.0, 1.0);
                    markers.push((pct, c.body.clone()));
                }
            }
        }
        self.waveform.set_comments(markers);

        if comments.is_empty() {
            let empty_lbl = gtk::Label::builder()
                .label("No comments yet. Be the first to comment!")
                .css_classes(["player-artist"])
                .margin_top(20)
                .build();
            self.comments_list.append(&empty_lbl);
            return;
        }

        for comment in comments {
            let row = self.create_comment_row(comment, duration_ms);
            self.comments_list.append(&row);
        }
    }

    fn create_comment_row(&self, comment: Comment, duration_ms: u64) -> gtk::Box {
        let row = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(12)
            .css_classes(["track-card"])
            .margin_top(4)
            .margin_bottom(4)
            .margin_start(6)
            .margin_end(6)
            .build();

        let avatar = gtk::Image::builder()
            .icon_name("avatar-default-symbolic")
            .pixel_size(32)
            .css_classes(["track-card-artwork"])
            .valign(gtk::Align::Center)
            .build();

        let avatar_url = comment.user.as_ref().and_then(|u| u.avatar_url.as_deref());
        crate::ui::image_loader::load_into_image(
            &avatar,
            avatar_url,
            "avatar-default-symbolic",
        );

        let content_box = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(2)
            .hexpand(true)
            .build();

        let header_row = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(8)
            .build();

        let username = comment
            .user
            .as_ref()
            .map(|u| u.username.as_str())
            .unwrap_or("Anonymous");
        let user_lbl = gtk::Label::builder()
            .label(username)
            .css_classes(["player-title"])
            .halign(gtk::Align::Start)
            .build();

        let ts_lbl = gtk::Label::builder()
            .label(&format!("at {}", comment.timestamp_formatted()))
            .css_classes(["filter-chip"])
            .halign(gtk::Align::Start)
            .build();

        header_row.append(&user_lbl);
        header_row.append(&ts_lbl);

        let body_lbl = gtk::Label::builder()
            .label(&comment.body)
            .css_classes(["player-artist"])
            .halign(gtk::Align::Start)
            .wrap(true)
            .build();

        content_box.append(&header_row);
        content_box.append(&body_lbl);

        row.append(&avatar);
        row.append(&content_box);

        // Click comment to seek to its timestamp
        if let Some(ts) = comment.timestamp {
            let click_g = gtk::GestureClick::new();
            let seek_cb = Rc::clone(&self.on_seek);
            click_g.connect_pressed(move |_g, _n, _x, _y| {
                if duration_ms > 0 {
                    let pct = (ts as f64 / duration_ms as f64).clamp(0.0, 1.0);
                    if let Some(ref cb) = *seek_cb.borrow() {
                        cb(pct);
                    }
                }
            });
            row.add_controller(click_g);
        }

        row
    }

    pub fn connect_play_track<F: Fn(Track) + 'static>(&self, f: F) {
        *self.on_play_track.borrow_mut() = Some(Box::new(f));
    }

    pub fn connect_seek<F: Fn(f64) + 'static>(&self, f: F) {
        *self.on_seek.borrow_mut() = Some(Box::new(f));
    }

    pub fn connect_post_comment<F: Fn(u64, String, u64) + 'static>(&self, f: F) {
        *self.on_post_comment.borrow_mut() = Some(Box::new(f));
    }

    pub fn connect_select_user<F: Fn(u64) + 'static>(&self, f: F) {
        *self.on_select_user.borrow_mut() = Some(Box::new(f));
    }
}
