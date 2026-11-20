use gtk4 as gtk;
use gtk::prelude::*;
use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

use crate::models::search::{SearchFilter, SearchResultItem, SearchResults};
use crate::models::track::Track;
use crate::ui::views::stream::format_count;

pub struct SearchView {
    container: gtk::Box,
    search_entry: gtk::SearchEntry,
    filter_box: gtk::Box,
    results_box: gtk::Box,
    loading_spinner: gtk::Spinner,
    current_filter: Rc<RefCell<SearchFilter>>,
    current_query: Rc<RefCell<String>>,
    search_generation: Rc<std::cell::Cell<u64>>,
    on_search: Rc<RefCell<Option<Box<dyn Fn(String, SearchFilter)>>>>,
    on_play_track: Rc<RefCell<Option<Box<dyn Fn(Track)>>>>,
    on_like_track: Rc<RefCell<Option<Box<dyn Fn(Track, bool)>>>>,
    on_select_track: Rc<RefCell<Option<Box<dyn Fn(Track)>>>>,
    on_select_user: Rc<RefCell<Option<Box<dyn Fn(u64)>>>>,
    on_queue_track: Rc<RefCell<Option<Box<dyn Fn(Track)>>>>,
}

impl SearchView {
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

        // Top Search Bar
        let search_row = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(12)
            .margin_end(14)
            .hexpand(true)
            .build();

        let search_entry = gtk::SearchEntry::builder()
            .placeholder_text("Search for artists, bands, tracks, podcasts...")
            .css_classes(["sc-search-entry"])
            .hexpand(true)
            .build();

        let loading_spinner = gtk::Spinner::builder()
            .spinning(false)
            .valign(gtk::Align::Center)
            .build();

        search_row.append(&search_entry);
        search_row.append(&loading_spinner);
        container.append(&search_row);

        // Filter chips row
        let filter_box = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(8)
            .margin_end(14)
            .build();

        let filters = [
            (SearchFilter::All, "Everything"),
            (SearchFilter::Tracks, "Tracks"),
            (SearchFilter::Playlists, "Playlists / Sets"),
            (SearchFilter::Users, "People"),
        ];

        let current_filter = Rc::new(RefCell::new(SearchFilter::All));

        let current_query = Rc::new(RefCell::new(String::new()));
        let search_generation = Rc::new(std::cell::Cell::new(0u64));
        let on_search: Rc<RefCell<Option<Box<dyn Fn(String, SearchFilter)>>>> = Rc::new(RefCell::new(None));

        for (filter_type, label) in filters {
            let chip = gtk::Button::builder()
                .label(label)
                .css_classes(["filter-chip"])
                .build();

            if filter_type == SearchFilter::All {
                chip.add_css_class("active");
            }

            let filter_box_clone = filter_box.clone();
            let chip_clone = chip.clone();
            let cf = Rc::clone(&current_filter);
            let q_chip = Rc::clone(&current_query);
            let search_cb_chip = Rc::clone(&on_search);

            chip.connect_clicked(move |_| {
                let mut child = filter_box_clone.first_child();
                while let Some(w) = child {
                    w.remove_css_class("active");
                    child = w.next_sibling();
                }
                chip_clone.add_css_class("active");
                *cf.borrow_mut() = filter_type;

                // Re-run search immediately if there is an active query
                let q = q_chip.borrow().clone();
                if !q.is_empty() {
                    if let Some(ref cb) = *search_cb_chip.borrow() {
                        cb(q, filter_type);
                    }
                }
            });

            filter_box.append(&chip);
        }

        container.append(&filter_box);

        // Results listing
        let scrolled = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Never)
            .vscrollbar_policy(gtk::PolicyType::Automatic)
            .hexpand(true)
            .vexpand(true)
            .build();

        let results_box = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(10)
            .margin_end(14)
            .hexpand(true)
            .build();

        scrolled.set_child(Some(&results_box));
        container.append(&scrolled);

        // Connect search entry with panic-safe epoch debounce (zero panics across FFI boundaries)
        let search_cb = Rc::clone(&on_search);
        let q_store = Rc::clone(&current_query);
        let f_store = Rc::clone(&current_filter);
        let gen_store = Rc::clone(&search_generation);

        search_entry.connect_search_changed(move |entry| {
            let query = entry.text().trim().to_string();
            *q_store.borrow_mut() = query.clone();

            let next_gen = gen_store.get().wrapping_add(1);
            gen_store.set(next_gen);

            if query.is_empty() {
                return;
            }

            let search_cb_inner = Rc::clone(&search_cb);
            let f_store_inner = Rc::clone(&f_store);
            let gen_check = Rc::clone(&gen_store);
            let query_inner = query.clone();

            glib::timeout_add_local_once(Duration::from_millis(300), move || {
                if gen_check.get() == next_gen {
                    let filter = *f_store_inner.borrow();
                    if let Some(ref cb) = *search_cb_inner.borrow() {
                        cb(query_inner, filter);
                    }
                }
            });
        });

        let on_play_track = Rc::new(RefCell::new(None));
        let on_like_track = Rc::new(RefCell::new(None));
        let on_select_track = Rc::new(RefCell::new(None));
        let on_select_user = Rc::new(RefCell::new(None));
        let on_queue_track = Rc::new(RefCell::new(None));

        Self {
            container,
            search_entry,
            filter_box,
            results_box,
            loading_spinner,
            current_filter,
            current_query,
            search_generation,
            on_search,
            on_play_track,
            on_like_track,
            on_select_track,
            on_select_user,
            on_queue_track,
        }
    }

    pub fn focus_search(&self) {
        self.search_entry.grab_focus();
    }

    pub fn widget(&self) -> &gtk::Box {
        &self.container
    }

    pub fn set_loading(&self, loading: bool) {
        self.loading_spinner.set_spinning(loading);
    }

    pub fn set_results(&self, results: SearchResults) {
        while let Some(child) = self.results_box.first_child() {
            self.results_box.remove(&child);
        }

        if results.collection.is_empty() {
            let empty_lbl = gtk::Label::builder()
                .label("No matches found. Try searching for something else.")
                .css_classes(["player-artist"])
                .margin_top(40)
                .build();
            self.results_box.append(&empty_lbl);
            return;
        }

        for item in results.collection {
            match item {
                SearchResultItem::Track(track) => {
                    let card = self.create_track_row(track);
                    self.results_box.append(&card);
                }
                SearchResultItem::Playlist(playlist) => {
                    let card = self.create_playlist_row(playlist);
                    self.results_box.append(&card);
                }
                SearchResultItem::User(user) => {
                    let card = self.create_user_row(user);
                    self.results_box.append(&card);
                }
                SearchResultItem::Unknown => {}
            }
        }
    }

    fn create_track_row(&self, track: Track) -> gtk::Box {
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
            .width_request(34)
            .height_request(34)
            .valign(gtk::Align::Center)
            .build();

        let track_play = track.clone();
        let play_cb = Rc::clone(&self.on_play_track);
        play_btn.connect_clicked(move |_| {
            if let Some(ref cb) = *play_cb.borrow() {
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

        let is_liked = track.user_favorite.unwrap_or(false);
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
        like_btn.connect_clicked(move |_| {
            let next = !is_liked_cell.get();
            is_liked_cell.set(next);
            if next {
                like_btn_c.add_css_class("btn-heart-liked");
                like_btn_c.set_icon_name("starred-symbolic");
                like_btn_c.set_tooltip_text(Some("Unlike track"));
            } else {
                like_btn_c.remove_css_class("btn-heart-liked");
                like_btn_c.set_icon_name("non-starred-symbolic");
                like_btn_c.set_tooltip_text(Some("Like track"));
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

        // Click click_area to play track immediately (consistent across app)
        let click_g = gtk::GestureClick::new();
        let track_play_row = track.clone();
        let play_cb_row = Rc::clone(&self.on_play_track);
        click_g.connect_pressed(move |_g, _n, _x, _y| {
            if let Some(ref cb) = *play_cb_row.borrow() {
                cb(track_play_row.clone());
            }
        });
        click_area.add_controller(click_g);

        row.append(&click_area);
        row.append(&like_btn);
        row.append(&dur_lbl);

        // Click title to view track detail
        let title_g = gtk::GestureClick::new();
        let track_detail = track.clone();
        let detail_cb = Rc::clone(&self.on_select_track);
        title_g.connect_pressed(move |_g, _n, _x, _y| {
            if let Some(ref cb) = *detail_cb.borrow() {
                cb(track_detail.clone());
            }
        });
        title_lbl.add_controller(title_g);
        title_lbl.set_tooltip_text(Some("Click title to view comments & details"));

        // Right-click context menu
        let play_cb_ctx = Rc::clone(&self.on_play_track);
        let queue_cb_ctx = Rc::clone(&self.on_queue_track);
        let detail_cb_ctx = Rc::clone(&self.on_select_track);
        let artist_cb_ctx = Rc::clone(&self.on_select_user);
        let like_cb_ctx = Rc::clone(&self.on_like_track);

        let actions = crate::ui::context_menu::TrackActions {
            on_play: Some(Rc::new(move |t| {
                if let Some(ref cb) = *play_cb_ctx.borrow() {
                    cb(t);
                }
            })),
            on_add_to_queue: Some(Rc::new(move |t| {
                if let Some(ref cb) = *queue_cb_ctx.borrow() {
                    cb(t);
                }
            })),
            on_view_details: Some(Rc::new(move |t| {
                if let Some(ref cb) = *detail_cb_ctx.borrow() {
                    cb(t);
                }
            })),
            on_artist_profile: Some(Rc::new(move |uid| {
                if let Some(ref cb) = *artist_cb_ctx.borrow() {
                    cb(uid);
                }
            })),
            on_toggle_like: Some(Rc::new(move |t, s| {
                if let Some(ref cb) = *like_cb_ctx.borrow() {
                    cb(t, s);
                }
            })),
        };
        crate::ui::context_menu::attach_track_context_menu(&row, track, actions);

        row
    }

    fn create_playlist_row(&self, playlist: crate::models::playlist::Playlist) -> gtk::Box {
        let row = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(12)
            .css_classes(["track-card"])
            .hexpand(true)
            .build();

        let icon = gtk::Image::builder()
            .icon_name("media-playlist-consecutive-symbolic")
            .pixel_size(40)
            .css_classes(["brand-accent", "track-card-artwork"])
            .valign(gtk::Align::Center)
            .build();

        crate::ui::image_loader::load_into_image(
            &icon,
            playlist.artwork_url.as_deref(),
            "media-playlist-consecutive-symbolic",
        );

        let info_box = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(2)
            .hexpand(true)
            .valign(gtk::Align::Center)
            .build();

        let title_lbl = gtk::Label::builder()
            .label(&playlist.title)
            .css_classes(["track-card-title"])
            .halign(gtk::Align::Start)
            .build();

        let count_str = format!("Playlist • {} tracks", playlist.track_count.unwrap_or(0));
        let count_lbl = gtk::Label::builder()
            .label(&count_str)
            .css_classes(["track-card-artist"])
            .halign(gtk::Align::Start)
            .build();

        info_box.append(&title_lbl);
        info_box.append(&count_lbl);

        row.append(&icon);
        row.append(&info_box);
        row
    }

    fn create_user_row(&self, user: crate::models::user::UserProfile) -> gtk::Box {
        let row = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(12)
            .css_classes(["track-card"])
            .hexpand(true)
            .build();

        let avatar = gtk::Image::builder()
            .icon_name("avatar-default-symbolic")
            .pixel_size(40)
            .css_classes(["track-card-artwork"])
            .valign(gtk::Align::Center)
            .build();

        crate::ui::image_loader::load_into_image(
            &avatar,
            user.avatar_url.as_deref(),
            "avatar-default-symbolic",
        );

        let info_box = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(2)
            .hexpand(true)
            .valign(gtk::Align::Center)
            .build();

        let name_lbl = gtk::Label::builder()
            .label(&user.username)
            .css_classes(["track-card-title"])
            .halign(gtk::Align::Start)
            .build();

        let stats_str = format!(
            "Creator • {} followers • {} tracks",
            format_count(user.followers_count.unwrap_or(0)),
            user.track_count.unwrap_or(0)
        );

        let stats_lbl = gtk::Label::builder()
            .label(&stats_str)
            .css_classes(["track-card-artist"])
            .halign(gtk::Align::Start)
            .build();

        info_box.append(&name_lbl);
        info_box.append(&stats_lbl);

        row.append(&avatar);
        row.append(&info_box);

        // Click on artist card opens their artist profile
        let click_g = gtk::GestureClick::new();
        let user_id = user.id;
        let select_user_cb = Rc::clone(&self.on_select_user);
        click_g.connect_pressed(move |_g, _n, _x, _y| {
            if let Some(ref cb) = *select_user_cb.borrow() {
                cb(user_id);
            }
        });
        row.add_controller(click_g);
        row.set_tooltip_text(Some(&format!("View profile of @{}", user.username)));

        row
    }

    pub fn connect_search<F: Fn(String, SearchFilter) + 'static>(&self, f: F) {
        *self.on_search.borrow_mut() = Some(Box::new(f));
    }

    pub fn connect_play_track<F: Fn(Track) + 'static>(&self, f: F) {
        *self.on_play_track.borrow_mut() = Some(Box::new(f));
    }

    pub fn connect_like_track<F: Fn(Track, bool) + 'static>(&self, f: F) {
        *self.on_like_track.borrow_mut() = Some(Box::new(f));
    }

    pub fn connect_select_track<F: Fn(Track) + 'static>(&self, f: F) {
        *self.on_select_track.borrow_mut() = Some(Box::new(f));
    }

    pub fn connect_select_user<F: Fn(u64) + 'static>(&self, f: F) {
        *self.on_select_user.borrow_mut() = Some(Box::new(f));
    }

    pub fn connect_queue_track<F: Fn(Track) + 'static>(&self, f: F) {
        *self.on_queue_track.borrow_mut() = Some(Box::new(f));
    }
}
