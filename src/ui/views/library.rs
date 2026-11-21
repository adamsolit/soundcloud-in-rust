use gtk4 as gtk;
use gtk::prelude::*;
use std::cell::RefCell;
use std::rc::Rc;

use crate::models::playlist::Playlist;
use crate::models::track::Track;
use crate::ui::views::playlist::PlaylistView;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LibraryTab {
    Likes,
    Playlists,
    History,
    Stash,
}

pub struct LibraryView {
    container: gtk::Box,
    stack: gtk::Stack,
    tracks_flow: gtk::Box,
    pub playlist_view: Rc<PlaylistView>,
    tab_likes_btn: gtk::Button,
    tab_playlists_btn: gtk::Button,
    tab_history_btn: gtk::Button,
    tab_stash_btn: gtk::Button,
    current_tab: Rc<RefCell<LibraryTab>>,
    on_play_track: Rc<RefCell<Option<Box<dyn Fn(Track)>>>>,
    on_play_queue: Rc<RefCell<Option<Box<dyn Fn(Vec<Track>, usize)>>>>,
    on_like_track: Rc<RefCell<Option<Box<dyn Fn(Track, bool)>>>>,
    on_tab_change: Rc<RefCell<Option<Box<dyn Fn(LibraryTab)>>>>,
    on_queue_track: Rc<RefCell<Option<Box<dyn Fn(Track)>>>>,
    on_select_track: Rc<RefCell<Option<Box<dyn Fn(Track)>>>>,
    on_artist_click: Rc<RefCell<Option<Box<dyn Fn(u64)>>>>,
    on_remove_stash: Rc<RefCell<Option<Box<dyn Fn(u64)>>>>,
    on_stash_track: Rc<RefCell<Option<Box<dyn Fn(Track)>>>>,
    is_stashed_fn: Rc<RefCell<Option<Box<dyn Fn(u64) -> bool>>>>,
    current_tracks: Rc<RefCell<Vec<Track>>>,
}

impl LibraryView {
    pub fn new() -> Self {
        let container = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(16)
            .margin_top(16)
            .margin_bottom(16)
            .margin_start(20)
            .margin_end(6)
            .hexpand(true)
            .vexpand(true)
            .build();

        // 1. Header & Tab Switcher
        let header_row = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(16)
            .margin_end(14)
            .build();

        let title = gtk::Label::builder()
            .label("Your Library")
            .css_classes(["brand-label"])
            .halign(gtk::Align::Start)
            .build();

        header_row.append(&title);
        container.append(&header_row);

        // Tab buttons: Liked Tracks | Playlists | Listening History
        let tabs_box = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(8)
            .margin_end(14)
            .build();

        let tab_likes_btn = gtk::Button::builder()
            .label("Liked Tracks")
            .css_classes(["filter-chip", "active"])
            .build();

        let tab_playlists_btn = gtk::Button::builder()
            .label("Playlists")
            .css_classes(["filter-chip"])
            .build();

        let tab_history_btn = gtk::Button::builder()
            .label("Listening History")
            .css_classes(["filter-chip"])
            .build();

        let tab_stash_btn = gtk::Button::builder()
            .label("Offline Stash")
            .css_classes(["filter-chip"])
            .build();

        tabs_box.append(&tab_likes_btn);
        tabs_box.append(&tab_playlists_btn);
        tabs_box.append(&tab_history_btn);
        tabs_box.append(&tab_stash_btn);
        container.append(&tabs_box);

        // 2. Main Content Stack
        let stack = gtk::Stack::builder()
            .transition_type(gtk::StackTransitionType::Crossfade)
            .transition_duration(120)
            .hexpand(true)
            .vexpand(true)
            .build();

        // Page A: Tracks list (Likes & History & Stash)
        let scrolled_tracks = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Never)
            .vscrollbar_policy(gtk::PolicyType::Automatic)
            .hexpand(true)
            .vexpand(true)
            .build();

        let tracks_flow = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(8)
            .margin_end(14)
            .hexpand(true)
            .build();

        scrolled_tracks.set_child(Some(&tracks_flow));
        stack.add_named(&scrolled_tracks, Some("tracks"));

        // Page B: Playlists view (Full playlist manager and detail viewer)
        let playlist_view = Rc::new(PlaylistView::new());
        stack.add_named(playlist_view.widget(), Some("playlists"));

        stack.set_visible_child_name("tracks");
        container.append(&stack);

        let current_tab = Rc::new(RefCell::new(LibraryTab::Likes));
        let on_play_track: Rc<RefCell<Option<Box<dyn Fn(Track)>>>> = Rc::new(RefCell::new(None));
        let on_play_queue: Rc<RefCell<Option<Box<dyn Fn(Vec<Track>, usize)>>>> = Rc::new(RefCell::new(None));
        let on_like_track: Rc<RefCell<Option<Box<dyn Fn(Track, bool)>>>> = Rc::new(RefCell::new(None));
        let on_tab_change: Rc<RefCell<Option<Box<dyn Fn(LibraryTab)>>>> = Rc::new(RefCell::new(None));
        let on_queue_track: Rc<RefCell<Option<Box<dyn Fn(Track)>>>> = Rc::new(RefCell::new(None));
        let on_select_track: Rc<RefCell<Option<Box<dyn Fn(Track)>>>> = Rc::new(RefCell::new(None));
        let on_artist_click: Rc<RefCell<Option<Box<dyn Fn(u64)>>>> = Rc::new(RefCell::new(None));
        let on_remove_stash: Rc<RefCell<Option<Box<dyn Fn(u64)>>>> = Rc::new(RefCell::new(None));
        let on_stash_track: Rc<RefCell<Option<Box<dyn Fn(Track)>>>> = Rc::new(RefCell::new(None));
        let is_stashed_fn: Rc<RefCell<Option<Box<dyn Fn(u64) -> bool>>>> = Rc::new(RefCell::new(None));
        let current_tracks = Rc::new(RefCell::new(Vec::new()));

        // Connect tab switching
        let stack_likes = stack.clone();
        let tab_c1 = Rc::clone(&current_tab);
        let tab_cb1 = Rc::clone(&on_tab_change);
        let b_l1 = tab_likes_btn.clone();
        let b_p1 = tab_playlists_btn.clone();
        let b_h1 = tab_history_btn.clone();
        let b_s1 = tab_stash_btn.clone();

        tab_likes_btn.connect_clicked(move |_| {
            b_l1.add_css_class("active");
            b_p1.remove_css_class("active");
            b_h1.remove_css_class("active");
            b_s1.remove_css_class("active");
            stack_likes.set_visible_child_name("tracks");
            *tab_c1.borrow_mut() = LibraryTab::Likes;
            if let Some(ref cb) = *tab_cb1.borrow() {
                cb(LibraryTab::Likes);
            }
        });

        let stack_pl = stack.clone();
        let tab_c2 = Rc::clone(&current_tab);
        let tab_cb2 = Rc::clone(&on_tab_change);
        let b_l2 = tab_likes_btn.clone();
        let b_p2 = tab_playlists_btn.clone();
        let b_h2 = tab_history_btn.clone();
        let b_s2 = tab_stash_btn.clone();

        tab_playlists_btn.connect_clicked(move |_| {
            b_p2.add_css_class("active");
            b_l2.remove_css_class("active");
            b_h2.remove_css_class("active");
            b_s2.remove_css_class("active");
            stack_pl.set_visible_child_name("playlists");
            *tab_c2.borrow_mut() = LibraryTab::Playlists;
            if let Some(ref cb) = *tab_cb2.borrow() {
                cb(LibraryTab::Playlists);
            }
        });

        let stack_hist = stack.clone();
        let tab_c3 = Rc::clone(&current_tab);
        let tab_cb3 = Rc::clone(&on_tab_change);
        let b_l3 = tab_likes_btn.clone();
        let b_p3 = tab_playlists_btn.clone();
        let b_h3 = tab_history_btn.clone();
        let b_s3 = tab_stash_btn.clone();

        tab_history_btn.connect_clicked(move |_| {
            b_h3.add_css_class("active");
            b_l3.remove_css_class("active");
            b_p3.remove_css_class("active");
            b_s3.remove_css_class("active");
            stack_hist.set_visible_child_name("tracks");
            *tab_c3.borrow_mut() = LibraryTab::History;
            if let Some(ref cb) = *tab_cb3.borrow() {
                cb(LibraryTab::History);
            }
        });

        let stack_stash = stack.clone();
        let tab_c4 = Rc::clone(&current_tab);
        let tab_cb4 = Rc::clone(&on_tab_change);
        let b_l4 = tab_likes_btn.clone();
        let b_p4 = tab_playlists_btn.clone();
        let b_h4 = tab_history_btn.clone();
        let b_s4 = tab_stash_btn.clone();

        tab_stash_btn.connect_clicked(move |_| {
            b_s4.add_css_class("active");
            b_l4.remove_css_class("active");
            b_p4.remove_css_class("active");
            b_h4.remove_css_class("active");
            stack_stash.set_visible_child_name("tracks");
            *tab_c4.borrow_mut() = LibraryTab::Stash;
            if let Some(ref cb) = *tab_cb4.borrow() {
                cb(LibraryTab::Stash);
            }
        });

        Self {
            container,
            stack,
            tracks_flow,
            playlist_view,
            tab_likes_btn,
            tab_playlists_btn,
            tab_history_btn,
            tab_stash_btn,
            current_tab,
            on_play_track,
            on_play_queue,
            on_like_track,
            on_tab_change,
            on_queue_track,
            on_select_track,
            on_artist_click,
            on_remove_stash,
            on_stash_track,
            is_stashed_fn,
            current_tracks,
        }
    }

    pub fn widget(&self) -> &gtk::Box {
        &self.container
    }

    pub fn set_tracks(&self, tracks: Vec<Track>) {
        while let Some(child) = self.tracks_flow.first_child() {
            self.tracks_flow.remove(&child);
        }

        *self.current_tracks.borrow_mut() = tracks.clone();

        if tracks.is_empty() {
            let tab = *self.current_tab.borrow();
            let msg = match tab {
                LibraryTab::Likes => "No liked tracks found. Click the star icon on any track to add it here.",
                LibraryTab::History => "No playback history recorded yet.",
                LibraryTab::Stash => "No offline stashed tracks yet. Click the download icon on any track to stash it for offline playback.",
                LibraryTab::Playlists => "",
            };

            let empty_lbl = gtk::Label::builder()
                .label(msg)
                .css_classes(["player-artist"])
                .margin_top(40)
                .build();
            self.tracks_flow.append(&empty_lbl);
            return;
        }

        for (index, track) in tracks.into_iter().enumerate() {
            let row = self.create_track_row(track, index);
            self.tracks_flow.append(&row);
        }
    }

    pub fn set_playlists(&self, playlists: Vec<Playlist>) {
        self.playlist_view.set_playlists(playlists);
    }

    fn create_track_row(&self, track: Track, index: usize) -> gtk::Box {
        let row = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(12)
            .css_classes(["track-card"])
            .hexpand(true)
            .build();

        let art_img = gtk::Image::builder()
            .pixel_size(44)
            .icon_name("audio-x-generic-symbolic")
            .css_classes(["track-card-artwork"])
            .valign(gtk::Align::Center)
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
            .width_request(32)
            .height_request(32)
            .valign(gtk::Align::Center)
            .build();

        let track_play = track.clone();
        let play_cb = Rc::clone(&self.on_play_track);
        let queue_cb = Rc::clone(&self.on_play_queue);
        let cur_tracks_c1 = Rc::clone(&self.current_tracks);
        play_btn.connect_clicked(move |_| {
            if let Some(ref cb) = *queue_cb.borrow() {
                let tracks = cur_tracks_c1.borrow().clone();
                cb(tracks, index);
            } else if let Some(ref cb) = *play_cb.borrow() {
                cb(track_play.clone());
            }
        });

        let info_box = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(2)
            .hexpand(true)
            .valign(gtk::Align::Center)
            .build();

        let title_lbl = gtk::Label::builder()
            .label(&track.title)
            .css_classes(["track-card-title"])
            .halign(gtk::Align::Start)
            .ellipsize(gtk::pango::EllipsizeMode::End)
            .build();

        let artist_lbl = gtk::Label::builder()
            .label(track.artist_name())
            .css_classes(["track-card-artist"])
            .halign(gtk::Align::Start)
            .build();

        info_box.append(&title_lbl);
        info_box.append(&artist_lbl);

        let dur_lbl = gtk::Label::builder()
            .label(&track.duration_formatted())
            .css_classes(["time-label"])
            .valign(gtk::Align::Center)
            .build();

        let is_liked = if *self.current_tab.borrow() == LibraryTab::Likes {
            true
        } else {
            track.user_favorite.unwrap_or(false)
        };

        let like_btn = gtk::Button::builder()
            .icon_name(if is_liked { "starred-symbolic" } else { "non-starred-symbolic" })
            .css_classes(["btn-card-like"])
            .tooltip_text(if is_liked { "Unlike track" } else { "Like track" })
            .valign(gtk::Align::Center)
            .build();
        if is_liked {
            like_btn.add_css_class("btn-heart-liked");
        }

        let is_liked_cell = std::cell::Cell::new(is_liked);
        let like_btn_c = like_btn.clone();
        let track_like = track.clone();
        let like_cb = Rc::clone(&self.on_like_track);
        let cur_tab_check = Rc::clone(&self.current_tab);
        let row_dim = row.clone();
        like_btn.connect_clicked(move |_| {
            let next = !is_liked_cell.get();
            is_liked_cell.set(next);
            if next {
                like_btn_c.add_css_class("btn-heart-liked");
                like_btn_c.set_icon_name("starred-symbolic");
                like_btn_c.set_tooltip_text(Some("Unlike track"));
                row_dim.set_opacity(1.0);
            } else {
                like_btn_c.remove_css_class("btn-heart-liked");
                like_btn_c.set_icon_name("non-starred-symbolic");
                like_btn_c.set_tooltip_text(Some("Like track"));
                if *cur_tab_check.borrow() == LibraryTab::Likes {
                    row_dim.set_opacity(0.35);
                }
            }
            if let Some(ref cb) = *like_cb.borrow() {
                cb(track_like.clone(), next);
            }
        });

        let click_area = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(12)
            .hexpand(true)
            .build();

        click_area.append(&art_img);
        click_area.append(&play_btn);
        click_area.append(&info_box);

        // Click click_area to play
        let click_g = gtk::GestureClick::new();
        let track_for_play = track.clone();
        let play_cb2 = Rc::clone(&self.on_play_track);
        let queue_cb2 = Rc::clone(&self.on_play_queue);
        let cur_tracks_c2 = Rc::clone(&self.current_tracks);
        click_g.connect_pressed(move |_g, _n, _x, _y| {
            if let Some(ref cb) = *queue_cb2.borrow() {
                let tracks = cur_tracks_c2.borrow().clone();
                cb(tracks, index);
            } else if let Some(ref cb) = *play_cb2.borrow() {
                cb(track_for_play.clone());
            }
        });
        click_area.add_controller(click_g);

        row.append(&click_area);
        row.append(&like_btn);
        row.append(&dur_lbl);

        // Right-click context menu
        let play_cb_ctx = Rc::clone(&self.on_play_track);
        let queue_cb_ctx = Rc::clone(&self.on_play_queue);
        let cur_tracks_ctx = Rc::clone(&self.current_tracks);
        let q_up_next_ctx = Rc::clone(&self.on_queue_track);
        let like_cb_ctx = Rc::clone(&self.on_like_track);
        let detail_cb_ctx = Rc::clone(&self.on_select_track);
        let artist_cb_ctx = Rc::clone(&self.on_artist_click);
        let stash_cb_ctx = Rc::clone(&self.on_stash_track);
        let is_st_ctx = Rc::clone(&self.is_stashed_fn);

        let actions = crate::ui::context_menu::TrackActions {
            on_play: Some(Rc::new(move |t| {
                if let Some(ref cb) = *queue_cb_ctx.borrow() {
                    let tracks = cur_tracks_ctx.borrow().clone();
                    cb(tracks, index);
                } else if let Some(ref cb) = *play_cb_ctx.borrow() {
                    cb(t);
                }
            })),
            on_add_to_queue: Some(Rc::new(move |t| {
                if let Some(ref cb) = *q_up_next_ctx.borrow() {
                    cb(t);
                }
            })),
            on_view_details: Some(Rc::new(move |t| {
                if let Some(ref cb) = *detail_cb_ctx.borrow() {
                    cb(t);
                }
            })),
            on_artist_profile: Some(Rc::new(move |u| {
                if let Some(ref cb) = *artist_cb_ctx.borrow() {
                    cb(u);
                }
            })),
            on_toggle_like: Some(Rc::new(move |t, s| {
                if let Some(ref cb) = *like_cb_ctx.borrow() {
                    cb(t, s);
                }
            })),
            on_stash_toggle: Some(Rc::new(move |t| {
                if let Some(ref cb) = *stash_cb_ctx.borrow() {
                    cb(t);
                }
            })),
            is_stashed: Some(Rc::new(move |id| {
                if let Some(ref cb) = *is_st_ctx.borrow() {
                    cb(id)
                } else {
                    false
                }
            })),
        };
        crate::ui::context_menu::attach_track_context_menu(&row, track, actions);

        row
    }

    pub fn connect_play_track<F: Fn(Track) + 'static>(&self, f: F) {
        *self.on_play_track.borrow_mut() = Some(Box::new(f));
    }

    pub fn connect_play_queue<F: Fn(Vec<Track>, usize) + 'static>(&self, f: F) {
        *self.on_play_queue.borrow_mut() = Some(Box::new(f));
    }

    pub fn connect_like_track<F: Fn(Track, bool) + 'static>(&self, f: F) {
        *self.on_like_track.borrow_mut() = Some(Box::new(f));
    }

    pub fn connect_tab_change<F: Fn(LibraryTab) + 'static>(&self, f: F) {
        *self.on_tab_change.borrow_mut() = Some(Box::new(f));
    }

    pub fn connect_queue_track<F: Fn(Track) + 'static>(&self, f: F) {
        *self.on_queue_track.borrow_mut() = Some(Box::new(f));
    }

    pub fn connect_select_track<F: Fn(Track) + 'static>(&self, f: F) {
        *self.on_select_track.borrow_mut() = Some(Box::new(f));
    }

    pub fn connect_artist_click<F: Fn(u64) + 'static>(&self, f: F) {
        *self.on_artist_click.borrow_mut() = Some(Box::new(f));
    }

    pub fn connect_remove_stash<F: Fn(u64) + 'static>(&self, f: F) {
        *self.on_remove_stash.borrow_mut() = Some(Box::new(f));
    }

    pub fn connect_stash_track<F: Fn(Track) + 'static>(&self, f: F) {
        *self.on_stash_track.borrow_mut() = Some(Box::new(f));
    }

    pub fn set_stash_checker<F: Fn(u64) -> bool + 'static>(&self, f: F) {
        *self.is_stashed_fn.borrow_mut() = Some(Box::new(f));
    }

    pub fn set_stashed_tracks(&self, items: Vec<(Track, String, u64)>) {
        while let Some(child) = self.tracks_flow.first_child() {
            self.tracks_flow.remove(&child);
        }

        let tracks: Vec<Track> = items.iter().map(|(t, _, _)| t.clone()).collect();
        *self.current_tracks.borrow_mut() = tracks;

        if items.is_empty() {
            let empty_lbl = gtk::Label::builder()
                .label("No offline stashed tracks yet. Click the download icon on any track to save it for offline playback.")
                .css_classes(["player-artist"])
                .margin_top(40)
                .build();
            self.tracks_flow.append(&empty_lbl);
            return;
        }

        let total_bytes: u64 = items.iter().map(|(_, _, sz)| *sz).sum();
        let mb = total_bytes as f64 / (1024.0 * 1024.0);

        let summary_box = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(12)
            .css_classes(["track-card"])
            .margin_bottom(8)
            .build();

        let icon = gtk::Image::builder()
            .icon_name("folder-download-symbolic")
            .pixel_size(24)
            .css_classes(["filter-chip", "active"])
            .build();

        let summary_lbl = gtk::Label::builder()
            .label(&format!("Offline Stash: {} tracks • {:.1} MB cached locally (instant zero-latency playback)", items.len(), mb))
            .css_classes(["brand-label"])
            .halign(gtk::Align::Start)
            .hexpand(true)
            .build();

        summary_box.append(&icon);
        summary_box.append(&summary_lbl);
        self.tracks_flow.append(&summary_box);

        for (index, (track, _path, size_bytes)) in items.into_iter().enumerate() {
            let row = self.create_stashed_track_row(track, size_bytes, index);
            self.tracks_flow.append(&row);
        }
    }

    fn create_stashed_track_row(&self, track: Track, size_bytes: u64, index: usize) -> gtk::Box {
        let row = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(12)
            .css_classes(["track-card"])
            .hexpand(true)
            .build();

        let art_img = gtk::Image::builder()
            .pixel_size(44)
            .icon_name("audio-x-generic-symbolic")
            .css_classes(["track-card-artwork"])
            .valign(gtk::Align::Center)
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
            .width_request(32)
            .height_request(32)
            .valign(gtk::Align::Center)
            .build();

        let track_play = track.clone();
        let play_cb = Rc::clone(&self.on_play_track);
        let queue_cb = Rc::clone(&self.on_play_queue);
        let cur_tracks_c1 = Rc::clone(&self.current_tracks);
        play_btn.connect_clicked(move |_| {
            if let Some(ref cb) = *queue_cb.borrow() {
                let tracks = cur_tracks_c1.borrow().clone();
                cb(tracks, index);
            } else if let Some(ref cb) = *play_cb.borrow() {
                cb(track_play.clone());
            }
        });

        let info_box = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(2)
            .hexpand(true)
            .valign(gtk::Align::Center)
            .build();

        let title_lbl = gtk::Label::builder()
            .label(&track.title)
            .css_classes(["track-card-title"])
            .halign(gtk::Align::Start)
            .ellipsize(gtk::pango::EllipsizeMode::End)
            .build();

        let artist_lbl = gtk::Label::builder()
            .label(track.artist_name())
            .css_classes(["track-card-artist"])
            .halign(gtk::Align::Start)
            .build();

        info_box.append(&title_lbl);
        info_box.append(&artist_lbl);

        let mb = size_bytes as f64 / (1024.0 * 1024.0);
        let size_lbl = gtk::Label::builder()
            .label(&format!("{:.1} MB", mb))
            .css_classes(["filter-chip"])
            .valign(gtk::Align::Center)
            .build();

        let dur_lbl = gtk::Label::builder()
            .label(&track.duration_formatted())
            .css_classes(["time-label"])
            .valign(gtk::Align::Center)
            .build();

        let del_btn = gtk::Button::builder()
            .icon_name("user-trash-symbolic")
            .css_classes(["btn-card-like"])
            .tooltip_text("Delete from Offline Stash")
            .valign(gtk::Align::Center)
            .build();

        let del_cb = Rc::clone(&self.on_remove_stash);
        let track_id = track.id;
        del_btn.connect_clicked(move |_| {
            if let Some(ref cb) = *del_cb.borrow() {
                cb(track_id);
            }
        });

        let click_area = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(12)
            .hexpand(true)
            .build();

        click_area.append(&art_img);
        click_area.append(&play_btn);
        click_area.append(&info_box);

        let click_g = gtk::GestureClick::new();
        let track_for_play = track.clone();
        let play_cb2 = Rc::clone(&self.on_play_track);
        let queue_cb2 = Rc::clone(&self.on_play_queue);
        let cur_tracks_c2 = Rc::clone(&self.current_tracks);
        click_g.connect_pressed(move |_g, _n, _x, _y| {
            if let Some(ref cb) = *queue_cb2.borrow() {
                let tracks = cur_tracks_c2.borrow().clone();
                cb(tracks, index);
            } else if let Some(ref cb) = *play_cb2.borrow() {
                cb(track_for_play.clone());
            }
        });
        click_area.add_controller(click_g);

        row.append(&click_area);
        row.append(&size_lbl);
        row.append(&dur_lbl);
        row.append(&del_btn);

        // Right-click context menu
        let play_cb_ctx = Rc::clone(&self.on_play_track);
        let queue_cb_ctx = Rc::clone(&self.on_play_queue);
        let cur_tracks_ctx = Rc::clone(&self.current_tracks);
        let q_up_next_ctx = Rc::clone(&self.on_queue_track);
        let like_cb_ctx = Rc::clone(&self.on_like_track);
        let detail_cb_ctx = Rc::clone(&self.on_select_track);
        let artist_cb_ctx = Rc::clone(&self.on_artist_click);
        let stash_cb_ctx = Rc::clone(&self.on_stash_track);
        let is_st_ctx = Rc::clone(&self.is_stashed_fn);

        let actions = crate::ui::context_menu::TrackActions {
            on_play: Some(Rc::new(move |t| {
                if let Some(ref cb) = *queue_cb_ctx.borrow() {
                    let tracks = cur_tracks_ctx.borrow().clone();
                    cb(tracks, index);
                } else if let Some(ref cb) = *play_cb_ctx.borrow() {
                    cb(t);
                }
            })),
            on_add_to_queue: Some(Rc::new(move |t| {
                if let Some(ref cb) = *q_up_next_ctx.borrow() {
                    cb(t);
                }
            })),
            on_view_details: Some(Rc::new(move |t| {
                if let Some(ref cb) = *detail_cb_ctx.borrow() {
                    cb(t);
                }
            })),
            on_artist_profile: Some(Rc::new(move |u| {
                if let Some(ref cb) = *artist_cb_ctx.borrow() {
                    cb(u);
                }
            })),
            on_toggle_like: Some(Rc::new(move |t, s| {
                if let Some(ref cb) = *like_cb_ctx.borrow() {
                    cb(t, s);
                }
            })),
            on_stash_toggle: Some(Rc::new(move |t| {
                if let Some(ref cb) = *stash_cb_ctx.borrow() {
                    cb(t);
                }
            })),
            is_stashed: Some(Rc::new(move |id| {
                if let Some(ref cb) = *is_st_ctx.borrow() {
                    cb(id)
                } else {
                    false
                }
            })),
        };
        crate::ui::context_menu::attach_track_context_menu(&row, track, actions);

        row
    }
}
