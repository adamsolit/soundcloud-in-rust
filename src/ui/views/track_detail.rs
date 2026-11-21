use gtk4 as gtk;
use gtk::prelude::*;
use std::cell::RefCell;
use std::rc::Rc;

use crate::models::comment::Comment;
use crate::models::track::{format_milliseconds, Track};
use crate::ui::waveform_widget::WaveformWidget;

pub struct TrackDetailView {
    scrolled: gtk::ScrolledWindow,
    container: gtk::Box,
    artwork: gtk::Image,
    title_label: gtk::Label,
    artist_label: gtk::Label,
    genre_label: gtk::Label,
    btn_play: gtk::Button,
    btn_like: gtk::Button,
    btn_stash: gtk::Button,
    waveform: Rc<WaveformWidget>,
    comment_input: gtk::Entry,
    btn_post_comment: gtk::Button,
    comments_list: gtk::Box,
    bookmark_input: gtk::Entry,
    btn_add_bookmark: gtk::Button,
    bookmarks_list: gtk::Box,
    comments_section: gtk::Box,
    bookmarks_section: gtk::Box,
    tab_comments_btn: gtk::Button,
    tab_bookmarks_btn: gtk::Button,
    current_track: Rc<RefCell<Option<Track>>>,
    current_position_ms: Rc<RefCell<u64>>,
    on_play_track: Rc<RefCell<Option<Box<dyn Fn(Track)>>>>,
    on_seek: Rc<RefCell<Option<Box<dyn Fn(f64)>>>>,
    on_post_comment: Rc<RefCell<Option<Box<dyn Fn(u64, String, u64)>>>>,
    on_select_user: Rc<RefCell<Option<Box<dyn Fn(u64)>>>>,
    on_add_bookmark: Rc<RefCell<Option<Box<dyn Fn(u64, u64, String)>>>>,
    on_delete_bookmark: Rc<RefCell<Option<Box<dyn Fn(i64)>>>>,
    on_stash_toggle: Rc<RefCell<Option<Box<dyn Fn()>>>>,
}

impl TrackDetailView {
    pub fn new() -> Self {
        let scrolled = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Never)
            .vscrollbar_policy(gtk::PolicyType::Automatic)
            .hexpand(true)
            .vexpand(true)
            .build();

        let container = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(20)
            .margin_top(20)
            .margin_bottom(24)
            .margin_start(24)
            .margin_end(16)
            .hexpand(true)
            .build();

        scrolled.set_child(Some(&container));

        // 1. Hero Header
        let hero_box = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(20)
            .margin_end(16)
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

        let btn_stash = gtk::Button::builder()
            .icon_name("folder-download-symbolic")
            .css_classes(["btn-transport"])
            .tooltip_text("Stash track offline")
            .build();

        actions_row.append(&btn_play);
        actions_row.append(&btn_like);
        actions_row.append(&btn_stash);

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
            .margin_end(24)
            .build();

        let waveform = Rc::new(WaveformWidget::new(80));
        wave_frame.append(waveform.widget());
        container.append(&wave_frame);

        // Tab switcher row (Community Comments vs Personal Bookmarks)
        let tabs_switcher = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(8)
            .margin_top(12)
            .margin_bottom(8)
            .build();

        let tab_comments_btn = gtk::Button::builder()
            .label("Community Comments")
            .css_classes(["filter-chip", "active"])
            .build();

        let tab_bookmarks_btn = gtk::Button::builder()
            .label("Personal Bookmarks & Notes")
            .css_classes(["filter-chip"])
            .build();

        tabs_switcher.append(&tab_comments_btn);
        tabs_switcher.append(&tab_bookmarks_btn);
        container.append(&tabs_switcher);

        // 3. Comments Section
        let comments_section = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(12)
            .margin_end(16)
            .hexpand(true)
            .build();

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
        comments_section.append(&comment_box);

        let comments_list = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(8)
            .hexpand(true)
            .build();
        comments_section.append(&comments_list);
        container.append(&comments_section);

        // 4. Personal Bookmarks Section
        let bookmarks_section = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(12)
            .margin_end(16)
            .hexpand(true)
            .visible(false)
            .build();

        let bookmark_input_box = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(12)
            .build();

        let bookmark_input = gtk::Entry::builder()
            .placeholder_text("Add personal bookmark note at 0:00...")
            .css_classes(["sc-search-entry"])
            .hexpand(true)
            .build();

        let btn_add_bookmark = gtk::Button::builder()
            .label("Bookmark (B)")
            .css_classes(["filter-chip", "active"])
            .build();

        bookmark_input_box.append(&bookmark_input);
        bookmark_input_box.append(&btn_add_bookmark);
        bookmarks_section.append(&bookmark_input_box);

        let bookmarks_list = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(8)
            .hexpand(true)
            .build();
        bookmarks_section.append(&bookmarks_list);
        container.append(&bookmarks_section);

        // Connect tab toggling
        let c_sec_c = comments_section.clone();
        let b_sec_c = bookmarks_section.clone();
        let c_btn_c = tab_comments_btn.clone();
        let b_btn_c = tab_bookmarks_btn.clone();

        tab_comments_btn.connect_clicked({
            let c_sec_c = c_sec_c.clone();
            let b_sec_c = b_sec_c.clone();
            let c_btn_c = c_btn_c.clone();
            let b_btn_c = b_btn_c.clone();
            move |_| {
                c_sec_c.set_visible(true);
                b_sec_c.set_visible(false);
                c_btn_c.add_css_class("active");
                b_btn_c.remove_css_class("active");
            }
        });

        tab_bookmarks_btn.connect_clicked({
            let c_sec_c = c_sec_c.clone();
            let b_sec_c = b_sec_c.clone();
            let c_btn_c = c_btn_c.clone();
            let b_btn_c = b_btn_c.clone();
            move |_| {
                c_sec_c.set_visible(false);
                b_sec_c.set_visible(true);
                c_btn_c.remove_css_class("active");
                b_btn_c.add_css_class("active");
            }
        });

        let current_track: Rc<RefCell<Option<Track>>> = Rc::new(RefCell::new(None));
        let current_position_ms = Rc::new(RefCell::new(0));
        let on_play_track: Rc<RefCell<Option<Box<dyn Fn(Track)>>>> = Rc::new(RefCell::new(None));
        let on_seek: Rc<RefCell<Option<Box<dyn Fn(f64)>>>> = Rc::new(RefCell::new(None));
        let on_post_comment: Rc<RefCell<Option<Box<dyn Fn(u64, String, u64)>>>> = Rc::new(RefCell::new(None));
        let on_select_user: Rc<RefCell<Option<Box<dyn Fn(u64)>>>> = Rc::new(RefCell::new(None));
        let on_add_bookmark: Rc<RefCell<Option<Box<dyn Fn(u64, u64, String)>>>> = Rc::new(RefCell::new(None));
        let on_delete_bookmark: Rc<RefCell<Option<Box<dyn Fn(i64)>>>> = Rc::new(RefCell::new(None));
        let on_stash_toggle: Rc<RefCell<Option<Box<dyn Fn()>>>> = Rc::new(RefCell::new(None));

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

        // Connect add bookmark button
        let add_bm_cb = Rc::clone(&on_add_bookmark);
        let track_bm = Rc::clone(&current_track);
        let pos_bm = Rc::clone(&current_position_ms);
        let bm_input_c = bookmark_input.clone();

        btn_add_bookmark.connect_clicked(move |_| {
            let text = bm_input_c.text().trim().to_string();
            let note = if text.is_empty() {
                "Bookmark".to_string()
            } else {
                text
            };
            if let Some(ref t) = *track_bm.borrow() {
                let pos = *pos_bm.borrow();
                if let Some(ref cb) = *add_bm_cb.borrow() {
                    cb(t.id, pos, note);
                }
                bm_input_c.set_text("");
            }
        });

        // Connect stash toggle
        let stash_cb = Rc::clone(&on_stash_toggle);
        btn_stash.connect_clicked(move |_| {
            if let Some(ref cb) = *stash_cb.borrow() {
                cb();
            }
        });

        Self {
            scrolled,
            container,
            artwork,
            title_label,
            artist_label,
            genre_label,
            btn_play,
            btn_like,
            btn_stash,
            waveform,
            comment_input,
            btn_post_comment,
            comments_list,
            bookmark_input,
            btn_add_bookmark,
            bookmarks_list,
            comments_section,
            bookmarks_section,
            tab_comments_btn,
            tab_bookmarks_btn,
            current_track,
            current_position_ms,
            on_play_track,
            on_seek,
            on_post_comment,
            on_select_user,
            on_add_bookmark,
            on_delete_bookmark,
            on_stash_toggle,
        }
    }

    pub fn widget(&self) -> &gtk::ScrolledWindow {
        &self.scrolled
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
        self.waveform.set_duration_ms(track.duration);
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
        self.bookmark_input.set_placeholder_text(Some(&format!(
            "Add personal note at {} (or press B)...",
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

    pub fn set_stashed(&self, stashed: bool) {
        if stashed {
            self.btn_stash.set_icon_name("emblem-ok-symbolic");
            self.btn_stash.add_css_class("btn-heart-liked");
            self.btn_stash.set_tooltip_text(Some("Stashed Offline (Click to remove)"));
        } else {
            self.btn_stash.set_icon_name("folder-download-symbolic");
            self.btn_stash.remove_css_class("btn-heart-liked");
            self.btn_stash.set_tooltip_text(Some("Stash Track Offline"));
        }
    }

    pub fn set_bookmarks(&self, bookmarks: Vec<crate::db::storage::Bookmark>, duration_ms: u64) {
        while let Some(child) = self.bookmarks_list.first_child() {
            self.bookmarks_list.remove(&child);
        }

        let mut markers = Vec::new();
        for b in &bookmarks {
            if duration_ms > 0 {
                let pct = (b.timestamp_ms as f64 / duration_ms as f64).clamp(0.0, 1.0);
                markers.push((pct, b.note.clone(), b.id));
            }
        }
        self.waveform.set_bookmarks(markers);

        if bookmarks.is_empty() {
            let empty_lbl = gtk::Label::builder()
                .label("No personal bookmarks for this track yet. Press B or write a note above to add one.")
                .css_classes(["player-artist"])
                .margin_top(20)
                .build();
            self.bookmarks_list.append(&empty_lbl);
            return;
        }

        for bm in bookmarks {
            let row = gtk::Box::builder()
                .orientation(gtk::Orientation::Horizontal)
                .spacing(12)
                .css_classes(["track-card"])
                .margin_top(4)
                .margin_bottom(4)
                .margin_start(6)
                .margin_end(6)
                .build();

            let ts_btn = gtk::Button::builder()
                .label(&format_milliseconds(bm.timestamp_ms))
                .css_classes(["filter-chip", "active"])
                .valign(gtk::Align::Center)
                .build();

            let seek_cb = Rc::clone(&self.on_seek);
            let ts_ms = bm.timestamp_ms;
            ts_btn.connect_clicked(move |_| {
                if duration_ms > 0 {
                    let pct = (ts_ms as f64 / duration_ms as f64).clamp(0.0, 1.0);
                    if let Some(ref cb) = *seek_cb.borrow() {
                        cb(pct);
                    }
                }
            });

            let note_lbl = gtk::Label::builder()
                .label(&bm.note)
                .css_classes(["player-title"])
                .halign(gtk::Align::Start)
                .hexpand(true)
                .ellipsize(gtk::pango::EllipsizeMode::End)
                .build();

            let time_lbl = gtk::Label::builder()
                .label(&bm.created_at)
                .css_classes(["time-label"])
                .halign(gtk::Align::End)
                .build();

            let del_btn = gtk::Button::builder()
                .icon_name("user-trash-symbolic")
                .css_classes(["btn-card-like"])
                .valign(gtk::Align::Center)
                .tooltip_text("Delete bookmark")
                .build();

            let del_cb = Rc::clone(&self.on_delete_bookmark);
            let bm_id = bm.id;
            del_btn.connect_clicked(move |_| {
                if let Some(ref cb) = *del_cb.borrow() {
                    cb(bm_id);
                }
            });

            row.append(&ts_btn);
            row.append(&note_lbl);
            row.append(&time_lbl);
            row.append(&del_btn);
            self.bookmarks_list.append(&row);
        }
    }

    pub fn connect_add_bookmark<F: Fn(u64, u64, String) + 'static>(&self, f: F) {
        *self.on_add_bookmark.borrow_mut() = Some(Box::new(f));
    }

    pub fn connect_delete_bookmark<F: Fn(i64) + 'static>(&self, f: F) {
        *self.on_delete_bookmark.borrow_mut() = Some(Box::new(f));
    }

    pub fn connect_stash<F: Fn() + 'static>(&self, f: F) {
        *self.on_stash_toggle.borrow_mut() = Some(Box::new(f));
    }

    pub fn current_track_id(&self) -> Option<u64> {
        self.current_track.borrow().as_ref().map(|t| t.id)
    }
}
