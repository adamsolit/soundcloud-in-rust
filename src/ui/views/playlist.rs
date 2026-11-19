use gtk4 as gtk;
use gtk::prelude::*;
use std::cell::RefCell;
use std::rc::Rc;

use crate::models::playlist::{CreatePlaylistPayload, Playlist};
use crate::models::track::Track;

pub struct PlaylistView {
    container: gtk::Box,
    view_stack: gtk::Stack,
    playlists_flow: gtk::Box,
    // Detail view widgets
    detail_artwork: gtk::Image,
    detail_title: gtk::Label,
    detail_author: gtk::Label,
    detail_meta: gtk::Label,
    detail_tracks_flow: gtk::Box,
    detail_spinner: gtk::Spinner,
    current_detail_playlist: Rc<RefCell<Option<Playlist>>>,
    // Callbacks
    on_play_track: Rc<RefCell<Option<Box<dyn Fn(Track)>>>>,
    on_play_queue: Rc<RefCell<Option<Box<dyn Fn(Vec<Track>, usize)>>>>,
    on_select_playlist: Rc<RefCell<Option<Box<dyn Fn(u64)>>>>,
    on_create_playlist: Rc<RefCell<Option<Box<dyn Fn(CreatePlaylistPayload)>>>>,
}

impl PlaylistView {
    pub fn new() -> Self {
        let container = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(12)
            .hexpand(true)
            .vexpand(true)
            .build();

        let view_stack = gtk::Stack::builder()
            .transition_type(gtk::StackTransitionType::Crossfade)
            .transition_duration(120)
            .hexpand(true)
            .vexpand(true)
            .build();

        // ==========================================
        // PAGE 1: PLAYLISTS GRID / LIST
        // ==========================================
        let list_page = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(14)
            .hexpand(true)
            .vexpand(true)
            .build();

        let list_header = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(12)
            .build();

        let title = gtk::Label::builder()
            .label("Your Playlists & Sets")
            .css_classes(["brand-label"])
            .halign(gtk::Align::Start)
            .hexpand(true)
            .build();

        let btn_new_playlist = gtk::Button::builder()
            .label("+ New Playlist")
            .css_classes(["filter-chip", "active"])
            .build();

        list_header.append(&title);
        list_header.append(&btn_new_playlist);
        list_page.append(&list_header);

        let scrolled_list = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Never)
            .vscrollbar_policy(gtk::PolicyType::Automatic)
            .hexpand(true)
            .vexpand(true)
            .build();

        let playlists_flow = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(10)
            .hexpand(true)
            .build();

        scrolled_list.set_child(Some(&playlists_flow));
        list_page.append(&scrolled_list);

        // ==========================================
        // PAGE 2: PLAYLIST DETAIL VIEW
        // ==========================================
        let detail_page = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(16)
            .hexpand(true)
            .vexpand(true)
            .build();

        // Top navigation bar: "← Back to Playlists"
        let detail_nav_bar = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(12)
            .build();

        let btn_back = gtk::Button::builder()
            .label("Back to Playlists")
            .icon_name("go-previous-symbolic")
            .css_classes(["filter-chip"])
            .build();

        let detail_spinner = gtk::Spinner::builder()
            .spinning(false)
            .valign(gtk::Align::Center)
            .build();

        detail_nav_bar.append(&btn_back);
        detail_nav_bar.append(&detail_spinner);
        detail_page.append(&detail_nav_bar);

        // Playlist Hero Card
        let hero_card = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(18)
            .css_classes(["account-card"])
            .build();

        let detail_artwork = gtk::Image::builder()
            .pixel_size(110)
            .icon_name("media-playlist-consecutive-symbolic")
            .css_classes(["player-artwork"])
            .build();

        let hero_meta = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(6)
            .valign(gtk::Align::Center)
            .hexpand(true)
            .build();

        let detail_title = gtk::Label::builder()
            .label("Playlist Title")
            .css_classes(["brand-label"])
            .halign(gtk::Align::Start)
            .ellipsize(gtk::pango::EllipsizeMode::End)
            .build();

        let detail_author = gtk::Label::builder()
            .label("Curator")
            .css_classes(["track-card-artist"])
            .halign(gtk::Align::Start)
            .build();

        let detail_meta = gtk::Label::builder()
            .label("0 tracks")
            .css_classes(["time-label"])
            .halign(gtk::Align::Start)
            .build();

        let play_actions_row = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(8)
            .halign(gtk::Align::Start)
            .valign(gtk::Align::Center)
            .build();

        let btn_play_all = gtk::Button::builder()
            .label("Play All")
            .icon_name("media-playback-start-symbolic")
            .css_classes(["btn-pill-play"])
            .height_request(36)
            .build();

        let btn_shuffle = gtk::ToggleButton::builder()
            .icon_name("media-playlist-shuffle-symbolic")
            .css_classes(["filter-chip"])
            .tooltip_text("Shuffle: OFF (sequential playback order)")
            .height_request(36)
            .build();

        btn_shuffle.connect_toggled(|btn| {
            if btn.is_active() {
                btn.add_css_class("active");
                btn.set_tooltip_text(Some("Shuffle: ON (random playback order)"));
            } else {
                btn.remove_css_class("active");
                btn.set_tooltip_text(Some("Shuffle: OFF (sequential playback order)"));
            }
        });

        play_actions_row.append(&btn_play_all);
        play_actions_row.append(&btn_shuffle);

        hero_meta.append(&detail_title);
        hero_meta.append(&detail_author);
        hero_meta.append(&detail_meta);
        hero_meta.append(&play_actions_row);

        hero_card.append(&detail_artwork);
        hero_card.append(&hero_meta);
        detail_page.append(&hero_card);

        // Scrolled tracks list in playlist
        let scrolled_tracks = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Never)
            .vscrollbar_policy(gtk::PolicyType::Automatic)
            .hexpand(true)
            .vexpand(true)
            .build();

        let detail_tracks_flow = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(6)
            .hexpand(true)
            .build();

        scrolled_tracks.set_child(Some(&detail_tracks_flow));
        detail_page.append(&scrolled_tracks);

        view_stack.add_named(&list_page, Some("list"));
        view_stack.add_named(&detail_page, Some("detail"));
        view_stack.set_visible_child_name("list");
        container.append(&view_stack);

        // Connect Back button
        let vs_clone = view_stack.clone();
        btn_back.connect_clicked(move |_| {
            vs_clone.set_visible_child_name("list");
        });

        let current_detail_playlist: Rc<RefCell<Option<Playlist>>> = Rc::new(RefCell::new(None));
        let on_play_track: Rc<RefCell<Option<Box<dyn Fn(Track)>>>> = Rc::new(RefCell::new(None));
        let on_play_queue: Rc<RefCell<Option<Box<dyn Fn(Vec<Track>, usize)>>>> = Rc::new(RefCell::new(None));
        let on_select_playlist: Rc<RefCell<Option<Box<dyn Fn(u64)>>>> = Rc::new(RefCell::new(None));
        let on_create_playlist: Rc<RefCell<Option<Box<dyn Fn(CreatePlaylistPayload)>>>> = Rc::new(RefCell::new(None));

        // Connect Play All button
        let cur_pl_clone = Rc::clone(&current_detail_playlist);
        let play_q_cb = Rc::clone(&on_play_queue);
        let btn_shuffle_clone = btn_shuffle.clone();
        btn_play_all.connect_clicked(move |_| {
            if let Some(ref pl) = *cur_pl_clone.borrow() {
                if !pl.tracks.is_empty() {
                    let mut tracks = pl.tracks.clone();
                    if btn_shuffle_clone.is_active() {
                        fastrand::shuffle(&mut tracks);
                    }
                    if let Some(ref cb) = *play_q_cb.borrow() {
                        cb(tracks, 0);
                    }
                }
            }
        });

        // Connect New Playlist dialog
        let create_cb = Rc::clone(&on_create_playlist);
        let cont_clone = container.clone();
        btn_new_playlist.connect_clicked(move |_| {
            show_create_playlist_dialog(&cont_clone, Rc::clone(&create_cb));
        });

        Self {
            container,
            view_stack,
            playlists_flow,
            detail_artwork,
            detail_title,
            detail_author,
            detail_meta,
            detail_tracks_flow,
            detail_spinner,
            current_detail_playlist,
            on_play_track,
            on_play_queue,
            on_select_playlist,
            on_create_playlist,
        }
    }

    pub fn widget(&self) -> &gtk::Box {
        &self.container
    }

    pub fn set_playlists(&self, playlists: Vec<Playlist>) {
        while let Some(child) = self.playlists_flow.first_child() {
            self.playlists_flow.remove(&child);
        }

        if playlists.is_empty() {
            let empty_lbl = gtk::Label::builder()
                .label("You have no playlists yet. Click '+ New Playlist' to create one.")
                .css_classes(["player-artist"])
                .margin_top(40)
                .build();
            self.playlists_flow.append(&empty_lbl);
            return;
        }

        for playlist in playlists {
            let card = self.create_playlist_card(playlist);
            self.playlists_flow.append(&card);
        }
    }

    fn create_playlist_card(&self, playlist: Playlist) -> gtk::Box {
        let card = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(14)
            .css_classes(["track-card"])
            .hexpand(true)
            .build();

        // 1. Cover artwork
        let cover_img = gtk::Image::builder()
            .pixel_size(56)
            .icon_name("media-playlist-consecutive-symbolic")
            .css_classes(["track-card-artwork"])
            .valign(gtk::Align::Center)
            .build();

        let cover_url = playlist.cover_url();
        crate::ui::image_loader::load_into_image(
            &cover_img,
            cover_url.as_deref(),
            "media-playlist-consecutive-symbolic",
        );

        // 2. Info box
        let info_box = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(3)
            .hexpand(true)
            .valign(gtk::Align::Center)
            .build();

        let title_lbl = gtk::Label::builder()
            .label(&playlist.title)
            .css_classes(["track-card-title"])
            .halign(gtk::Align::Start)
            .ellipsize(gtk::pango::EllipsizeMode::End)
            .build();

        let count = playlist.track_count.unwrap_or(playlist.tracks.len() as u64);
        let author_str = format!("{} • {} tracks", playlist.author_name(), count);
        let author_lbl = gtk::Label::builder()
            .label(&author_str)
            .css_classes(["track-card-artist"])
            .halign(gtk::Align::Start)
            .build();

        info_box.append(&title_lbl);
        info_box.append(&author_lbl);

        // 3. Open button indicator
        let open_btn = gtk::Button::builder()
            .icon_name("go-next-symbolic")
            .css_classes(["btn-transport"])
            .valign(gtk::Align::Center)
            .build();

        card.append(&cover_img);
        card.append(&info_box);
        card.append(&open_btn);

        // Click on playlist card opens detail view
        let click_gesture = gtk::GestureClick::new();
        let pl_id = playlist.id;
        let select_cb = Rc::clone(&self.on_select_playlist);
        click_gesture.connect_pressed(move |_g, _n, _x, _y| {
            if let Some(ref cb) = *select_cb.borrow() {
                cb(pl_id);
            }
        });
        card.add_controller(click_gesture);

        let pl_id2 = playlist.id;
        let select_cb2 = Rc::clone(&self.on_select_playlist);
        open_btn.connect_clicked(move |_| {
            if let Some(ref cb) = *select_cb2.borrow() {
                cb(pl_id2);
            }
        });

        card
    }

    /// Sets and opens the playlist detail view with all tracks
    pub fn open_playlist_detail(&self, playlist: Playlist) {
        *self.current_detail_playlist.borrow_mut() = Some(playlist.clone());

        self.detail_title.set_label(&playlist.title);
        self.detail_author.set_label(&format!("By {}", playlist.author_name()));

        let count = playlist.track_count.unwrap_or(playlist.tracks.len() as u64);
        self.detail_meta.set_label(&format!("Playlist • {} tracks", count));

        let cover_url = playlist.cover_url();
        crate::ui::image_loader::load_into_image(
            &self.detail_artwork,
            cover_url.as_deref(),
            "media-playlist-consecutive-symbolic",
        );

        // Populate all tracks
        while let Some(child) = self.detail_tracks_flow.first_child() {
            self.detail_tracks_flow.remove(&child);
        }

        if playlist.tracks.is_empty() {
            let empty_lbl = gtk::Label::builder()
                .label("This playlist has no tracks.")
                .css_classes(["player-artist"])
                .margin_top(20)
                .build();
            self.detail_tracks_flow.append(&empty_lbl);
        } else {
            let all_tracks = playlist.tracks.clone();
            for (idx, track) in playlist.tracks.iter().enumerate() {
                let row = self.create_detail_track_row(track.clone(), idx, all_tracks.clone());
                self.detail_tracks_flow.append(&row);
            }
        }

        self.view_stack.set_visible_child_name("detail");
    }

    fn create_detail_track_row(&self, track: Track, index: usize, all_tracks: Vec<Track>) -> gtk::Box {
        let row = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(12)
            .css_classes(["track-card"])
            .hexpand(true)
            .build();

        // 1. Track Index
        let idx_lbl = gtk::Label::builder()
            .label(&format!("{:02}", index + 1))
            .css_classes(["time-label"])
            .width_chars(3)
            .valign(gtk::Align::Center)
            .build();

        // 2. Play button
        let play_btn = gtk::Button::builder()
            .icon_name("media-playback-start-symbolic")
            .css_classes(["btn-transport"])
            .valign(gtk::Align::Center)
            .build();

        let queue_cb = Rc::clone(&self.on_play_queue);
        let all_tracks_clone = all_tracks.clone();
        play_btn.connect_clicked(move |_| {
            if let Some(ref cb) = *queue_cb.borrow() {
                cb(all_tracks_clone.clone(), index);
            }
        });

        // 3. Title & Artist
        let meta_vbox = gtk::Box::builder()
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

        meta_vbox.append(&title_lbl);
        meta_vbox.append(&artist_lbl);

        // 4. Duration
        let dur_lbl = gtk::Label::builder()
            .label(&track.duration_formatted())
            .css_classes(["time-label"])
            .valign(gtk::Align::Center)
            .build();

        row.append(&idx_lbl);
        row.append(&play_btn);
        row.append(&meta_vbox);
        row.append(&dur_lbl);

        // Click row to play
        let row_gesture = gtk::GestureClick::new();
        let queue_cb2 = Rc::clone(&self.on_play_queue);
        let all_tracks_clone2 = all_tracks;
        row_gesture.connect_pressed(move |_g, _n, _x, _y| {
            if let Some(ref cb) = *queue_cb2.borrow() {
                cb(all_tracks_clone2.clone(), index);
            }
        });
        row.add_controller(row_gesture);

        row
    }

    pub fn set_loading(&self, loading: bool) {
        self.detail_spinner.set_spinning(loading);
    }

    pub fn connect_play_track<F: Fn(Track) + 'static>(&self, f: F) {
        *self.on_play_track.borrow_mut() = Some(Box::new(f));
    }

    pub fn connect_play_queue<F: Fn(Vec<Track>, usize) + 'static>(&self, f: F) {
        *self.on_play_queue.borrow_mut() = Some(Box::new(f));
    }

    pub fn connect_select_playlist<F: Fn(u64) + 'static>(&self, f: F) {
        *self.on_select_playlist.borrow_mut() = Some(Box::new(f));
    }

    pub fn connect_create_playlist<F: Fn(CreatePlaylistPayload) + 'static>(&self, f: F) {
        *self.on_create_playlist.borrow_mut() = Some(Box::new(f));
    }
}

fn show_create_playlist_dialog(
    parent: &gtk::Box,
    on_create: Rc<RefCell<Option<Box<dyn Fn(CreatePlaylistPayload)>>>>,
) {
    let window = parent.root().and_downcast::<gtk::Window>();
    let dialog = gtk::Window::builder()
        .title("Create New Playlist")
        .modal(true)
        .transient_for(window.as_ref().unwrap())
        .default_width(380)
        .default_height(240)
        .css_classes(["account-card"])
        .build();

    let vbox = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(12)
        .margin_top(16)
        .margin_bottom(16)
        .margin_start(16)
        .margin_end(16)
        .build();

    let lbl = gtk::Label::builder()
        .label("Playlist Title")
        .css_classes(["track-card-title"])
        .halign(gtk::Align::Start)
        .build();

    let title_entry = gtk::Entry::builder()
        .placeholder_text("e.g. My Favorites")
        .css_classes(["sc-search-entry"])
        .build();

    let desc_lbl = gtk::Label::builder()
        .label("Description (optional)")
        .css_classes(["track-card-artist"])
        .halign(gtk::Align::Start)
        .build();

    let desc_entry = gtk::Entry::builder()
        .placeholder_text("Description...")
        .css_classes(["sc-search-entry"])
        .build();

    let check_private = gtk::CheckButton::builder()
        .label("Private Playlist")
        .active(false)
        .build();

    let btn_submit = gtk::Button::builder()
        .label("Create Playlist")
        .css_classes(["filter-chip", "active"])
        .build();

    let diag_clone = dialog.clone();
    let title_clone = title_entry.clone();
    let desc_clone = desc_entry.clone();
    let priv_clone = check_private.clone();

    btn_submit.connect_clicked(move |_| {
        let t = title_clone.text().trim().to_string();
        if !t.is_empty() {
            let desc = desc_clone.text().trim().to_string();
            let is_priv = priv_clone.is_active();
            let payload = CreatePlaylistPayload {
                title: t,
                description: if desc.is_empty() { None } else { Some(desc) },
                sharing: if is_priv { "private".to_string() } else { "public".to_string() },
                tracks: Vec::new(),
            };
            if let Some(ref cb) = *on_create.borrow() {
                cb(payload);
            }
        }
        diag_clone.close();
    });

    vbox.append(&lbl);
    vbox.append(&title_entry);
    vbox.append(&desc_lbl);
    vbox.append(&desc_entry);
    vbox.append(&check_private);
    vbox.append(&btn_submit);

    dialog.set_child(Some(&vbox));
    dialog.present();
}
