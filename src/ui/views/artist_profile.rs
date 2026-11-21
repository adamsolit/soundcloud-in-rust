use gtk4 as gtk;
use gtk::prelude::*;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

use crate::api::waveform::generate_synthetic_waveform;
use crate::models::track::Track;
use crate::models::user::UserProfile;
use crate::ui::views::stream::format_count;
use crate::ui::waveform_widget::WaveformWidget;

pub struct ArtistProfileView {
    container: gtk::Box,
    scrolled: gtk::ScrolledWindow,
    avatar_img: gtk::Image,
    name_lbl: gtk::Label,
    handle_lbl: gtk::Label,
    location_lbl: gtk::Label,
    stats_lbl: gtk::Label,
    bio_box: gtk::Box,
    bio_lbl: gtk::Label,
    btn_follow: gtk::Button,
    btn_play_all: gtk::Button,
    spinner: gtk::Spinner,
    tracks_flow: gtk::Box,
    current_artist: Rc<RefCell<Option<UserProfile>>>,
    current_tracks: Rc<RefCell<Vec<Track>>>,
    is_following: Rc<Cell<bool>>,
    on_play_track: Rc<RefCell<Option<Box<dyn Fn(Track)>>>>,
    on_play_queue: Rc<RefCell<Option<Box<dyn Fn(Vec<Track>, usize)>>>>,
    on_like_track: Rc<RefCell<Option<Box<dyn Fn(Track, bool)>>>>,
    on_play_all: Rc<RefCell<Option<Box<dyn Fn(Vec<Track>)>>>>,
    on_select_track: Rc<RefCell<Option<Box<dyn Fn(Track)>>>>,
    on_toggle_follow: Rc<RefCell<Option<Box<dyn Fn(u64, bool)>>>>,
    on_queue_track: Rc<RefCell<Option<Box<dyn Fn(Track)>>>>,
    on_stash_track: Rc<RefCell<Option<Box<dyn Fn(Track)>>>>,
    is_stashed_fn: Rc<RefCell<Option<Box<dyn Fn(u64) -> bool>>>>,
    on_back: Rc<RefCell<Option<Box<dyn Fn()>>>>,
}

impl ArtistProfileView {
    pub fn new() -> Self {
        let scrolled = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Never)
            .vscrollbar_policy(gtk::PolicyType::Automatic)
            .hexpand(true)
            .vexpand(true)
            .build();

        let container = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(18)
            .margin_top(16)
            .margin_bottom(24)
            .margin_start(24)
            .margin_end(24)
            .hexpand(true)
            .build();

        // 1. Navigation Header Row (Back button + Spinner)
        let nav_row = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(12)
            .build();

        let btn_back = gtk::Button::builder()
            .label("Back")
            .icon_name("go-previous-symbolic")
            .css_classes(["filter-chip"])
            .halign(gtk::Align::Start)
            .build();

        let spinner = gtk::Spinner::builder()
            .spinning(false)
            .valign(gtk::Align::Center)
            .build();

        nav_row.append(&btn_back);
        nav_row.append(&spinner);
        container.append(&nav_row);

        // 2. Artist Hero Card
        let hero_card = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(20)
            .css_classes(["account-card"])
            .hexpand(true)
            .build();

        let avatar_img = gtk::Image::builder()
            .pixel_size(96)
            .icon_name("avatar-default-symbolic")
            .css_classes(["artist-avatar", "track-card-artwork"])
            .valign(gtk::Align::Center)
            .build();

        let hero_info = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(6)
            .valign(gtk::Align::Center)
            .hexpand(true)
            .build();

        let name_row = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(8)
            .build();

        let name_lbl = gtk::Label::builder()
            .label("Artist Name")
            .css_classes(["brand-label"])
            .halign(gtk::Align::Start)
            .build();

        name_row.append(&name_lbl);

        let handle_lbl = gtk::Label::builder()
            .label("@artist")
            .css_classes(["track-card-artist"])
            .halign(gtk::Align::Start)
            .build();

        let location_lbl = gtk::Label::builder()
            .label("")
            .css_classes(["time-label"])
            .halign(gtk::Align::Start)
            .build();

        let stats_lbl = gtk::Label::builder()
            .label("0 followers • 0 tracks")
            .css_classes(["track-card-artist"])
            .halign(gtk::Align::Start)
            .build();

        // Actions: Play All and Follow
        let actions_row = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(10)
            .margin_top(4)
            .halign(gtk::Align::Start)
            .build();

        let btn_play_all = gtk::Button::builder()
            .label("Play All")
            .icon_name("media-playback-start-symbolic")
            .css_classes(["btn-pill-play"])
            .height_request(36)
            .build();

        let btn_follow = gtk::Button::builder()
            .label("Follow")
            .icon_name("list-add-symbolic")
            .css_classes(["filter-chip"])
            .height_request(36)
            .build();

        actions_row.append(&btn_play_all);
        actions_row.append(&btn_follow);

        hero_info.append(&name_row);
        hero_info.append(&handle_lbl);
        hero_info.append(&location_lbl);
        hero_info.append(&stats_lbl);
        hero_info.append(&actions_row);

        hero_card.append(&avatar_img);
        hero_card.append(&hero_info);
        container.append(&hero_card);

        // 3. Bio / Description Card (collapsible / conditionally visible)
        let bio_box = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(6)
            .css_classes(["login-guide-box"])
            .visible(false)
            .build();

        let bio_title = gtk::Label::builder()
            .label("About")
            .css_classes(["track-card-title"])
            .halign(gtk::Align::Start)
            .build();

        let bio_lbl = gtk::Label::builder()
            .label("")
            .css_classes(["track-card-artist"])
            .halign(gtk::Align::Start)
            .wrap(true)
            .wrap_mode(gtk::pango::WrapMode::WordChar)
            .build();

        bio_box.append(&bio_title);
        bio_box.append(&bio_lbl);
        container.append(&bio_box);

        // 4. Tracks Section
        let section_header = gtk::Label::builder()
            .label("Tracks & Recitations")
            .css_classes(["brand-label"])
            .halign(gtk::Align::Start)
            .margin_top(8)
            .build();
        container.append(&section_header);

        let tracks_flow = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(10)
            .hexpand(true)
            .build();
        container.append(&tracks_flow);

        scrolled.set_child(Some(&container));

        let current_artist: Rc<RefCell<Option<UserProfile>>> = Rc::new(RefCell::new(None));
        let current_tracks = Rc::new(RefCell::new(Vec::new()));
        let is_following = Rc::new(Cell::new(false));

        let on_play_track: Rc<RefCell<Option<Box<dyn Fn(Track)>>>> = Rc::new(RefCell::new(None));
        let on_play_queue: Rc<RefCell<Option<Box<dyn Fn(Vec<Track>, usize)>>>> = Rc::new(RefCell::new(None));
        let on_like_track: Rc<RefCell<Option<Box<dyn Fn(Track, bool)>>>> = Rc::new(RefCell::new(None));
        let on_play_all: Rc<RefCell<Option<Box<dyn Fn(Vec<Track>)>>>> = Rc::new(RefCell::new(None));
        let on_select_track: Rc<RefCell<Option<Box<dyn Fn(Track)>>>> = Rc::new(RefCell::new(None));
        let on_toggle_follow: Rc<RefCell<Option<Box<dyn Fn(u64, bool)>>>> = Rc::new(RefCell::new(None));
        let on_queue_track: Rc<RefCell<Option<Box<dyn Fn(Track)>>>> = Rc::new(RefCell::new(None));
        let on_stash_track: Rc<RefCell<Option<Box<dyn Fn(Track)>>>> = Rc::new(RefCell::new(None));
        let is_stashed_fn: Rc<RefCell<Option<Box<dyn Fn(u64) -> bool>>>> = Rc::new(RefCell::new(None));
        let on_back: Rc<RefCell<Option<Box<dyn Fn()>>>> = Rc::new(RefCell::new(None));

        // Connect Back button
        let back_cb = Rc::clone(&on_back);
        btn_back.connect_clicked(move |_| {
            if let Some(ref cb) = *back_cb.borrow() {
                cb();
            }
        });

        // Connect Play All button
        let tracks_holder = Rc::clone(&current_tracks);
        let play_all_cb = Rc::clone(&on_play_all);
        btn_play_all.connect_clicked(move |_| {
            let trs = tracks_holder.borrow().clone();
            if !trs.is_empty() {
                if let Some(ref cb) = *play_all_cb.borrow() {
                    cb(trs);
                }
            }
        });

        // Connect Follow toggle button
        let artist_holder = Rc::clone(&current_artist);
        let following_state = Rc::clone(&is_following);
        let follow_cb = Rc::clone(&on_toggle_follow);
        let btn_follow_c = btn_follow.clone();

        btn_follow.connect_clicked(move |_| {
            if let Some(ref artist) = *artist_holder.borrow() {
                let current = following_state.get();
                let new_state = !current;
                following_state.set(new_state);

                if new_state {
                    btn_follow_c.set_label("Following");
                    btn_follow_c.set_icon_name("object-select-symbolic");
                    btn_follow_c.add_css_class("active");
                } else {
                    btn_follow_c.set_label("Follow");
                    btn_follow_c.set_icon_name("list-add-symbolic");
                    btn_follow_c.remove_css_class("active");
                }

                if let Some(ref cb) = *follow_cb.borrow() {
                    cb(artist.id, new_state);
                }
            }
        });

        Self {
            container,
            scrolled,
            avatar_img,
            name_lbl,
            handle_lbl,
            location_lbl,
            stats_lbl,
            bio_box,
            bio_lbl,
            btn_follow,
            btn_play_all,
            spinner,
            tracks_flow,
            current_artist,
            current_tracks,
            is_following,
            on_play_track,
            on_play_queue,
            on_like_track,
            on_play_all,
            on_select_track,
            on_toggle_follow,
            on_queue_track,
            on_stash_track,
            is_stashed_fn,
            on_back,
        }
    }

    pub fn widget(&self) -> &gtk::ScrolledWindow {
        &self.scrolled
    }

    pub fn set_loading(&self, loading: bool) {
        self.spinner.set_spinning(loading);
    }

    pub fn set_artist(&self, user: &UserProfile) {
        *self.current_artist.borrow_mut() = Some(user.clone());

        self.name_lbl.set_label(&user.username);
        self.handle_lbl.set_label(&format!("@{}", user.permalink.as_deref().unwrap_or(&user.username)));

        let location = user.display_location().unwrap_or_default();
        if !location.is_empty() {
            self.location_lbl.set_label(&location);
            self.location_lbl.set_visible(true);
        } else {
            self.location_lbl.set_visible(false);
        }

        let followers = format_count(user.followers_count.unwrap_or(0));
        let tracks_count = user.track_count.unwrap_or(0);
        let followings = format_count(user.followings_count.unwrap_or(0));
        self.stats_lbl.set_label(&format!(
            "{} followers • {} following • {} tracks",
            followers, followings, tracks_count
        ));

        // Load avatar
        crate::ui::image_loader::load_into_image(
            &self.avatar_img,
            user.avatar_url.as_deref(),
            "avatar-default-symbolic",
        );

        // Bio
        if let Some(ref bio) = user.description {
            let trimmed = bio.trim();
            if !trimmed.is_empty() {
                self.bio_lbl.set_label(trimmed);
                self.bio_box.set_visible(true);
            } else {
                self.bio_box.set_visible(false);
            }
        } else {
            self.bio_box.set_visible(false);
        }
    }

    pub fn set_following(&self, following: bool) {
        self.is_following.set(following);
        if following {
            self.btn_follow.set_label("Following");
            self.btn_follow.set_icon_name("object-select-symbolic");
            self.btn_follow.add_css_class("active");
        } else {
            self.btn_follow.set_label("Follow");
            self.btn_follow.set_icon_name("list-add-symbolic");
            self.btn_follow.remove_css_class("active");
        }
    }

    pub fn set_tracks(&self, tracks: Vec<Track>) {
        *self.current_tracks.borrow_mut() = tracks.clone();

        while let Some(child) = self.tracks_flow.first_child() {
            self.tracks_flow.remove(&child);
        }

        if tracks.is_empty() {
            let empty_lbl = gtk::Label::builder()
                .label("No public tracks uploaded by this artist.")
                .css_classes(["player-artist"])
                .margin_top(30)
                .build();
            self.tracks_flow.append(&empty_lbl);
            return;
        }

        for (index, track) in tracks.into_iter().enumerate() {
            let card = self.create_track_card(track, index);
            self.tracks_flow.append(&card);
        }
    }

    fn create_track_card(&self, track: Track, index: usize) -> gtk::Box {
        let card = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(14)
            .css_classes(["track-card"])
            .hexpand(true)
            .build();

        // 1. Play Button & Artwork Overlay
        let art_overlay = gtk::Overlay::builder()
            .valign(gtk::Align::Center)
            .halign(gtk::Align::Center)
            .build();

        let art_img = gtk::Image::builder()
            .pixel_size(52)
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
            .width_request(34)
            .height_request(34)
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

        // 2. Info Box & Waveform
        let info_box = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(4)
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

        let dur_lbl = gtk::Label::builder()
            .label(&track.duration_formatted())
            .css_classes(["time-label"])
            .halign(gtk::Align::End)
            .build();

        let is_liked = track.user_favorite.unwrap_or(false);
        let like_icon = if is_liked { "starred-symbolic" } else { "non-starred-symbolic" };
        let like_tooltip = if is_liked { "Unlike track" } else { "Like track" };

        let like_btn = gtk::Button::builder()
            .icon_name(like_icon)
            .css_classes(["btn-card-like"])
            .valign(gtk::Align::Center)
            .tooltip_text(like_tooltip)
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

        title_row.append(&title_lbl);
        title_row.append(&like_btn);
        title_row.append(&dur_lbl);

        let wave = WaveformWidget::new(28);
        let samples = generate_synthetic_waveform(track.id, 80);
        wave.set_samples(samples);

        // Stats row
        let stats_row = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(14)
            .build();

        let plays = format!("{} plays", format_count(track.playback_count.unwrap_or(0)));
        let likes = format!("{} likes", format_count(track.likes_count.unwrap_or(0)));

        for text in [plays, likes] {
            let stat_lbl = gtk::Label::builder()
                .label(&text)
                .css_classes(["track-card-stats"])
                .build();
            stats_row.append(&stat_lbl);
        }

        info_box.append(&title_row);
        info_box.append(wave.widget());
        info_box.append(&stats_row);
        card.append(&info_box);

        // Click artwork or waveform to play track immediately
        let click_g = gtk::GestureClick::new();
        let track_row_play = track.clone();
        let play_cb_row = Rc::clone(&self.on_play_track);
        let queue_cb_row = Rc::clone(&self.on_play_queue);
        let cur_tracks_c1 = Rc::clone(&self.current_tracks);
        click_g.connect_pressed(move |_g, _n, _x, _y| {
            if let Some(ref cb) = *queue_cb_row.borrow() {
                let tracks = cur_tracks_c1.borrow().clone();
                cb(tracks, index);
            } else if let Some(ref cb) = *play_cb_row.borrow() {
                cb(track_row_play.clone());
            }
        });
        art_overlay.add_controller(click_g);

        let wave_g = gtk::GestureClick::new();
        let track_wave_play = track.clone();
        let play_cb_wave = Rc::clone(&self.on_play_track);
        let queue_cb_wave = Rc::clone(&self.on_play_queue);
        let cur_tracks_c2 = Rc::clone(&self.current_tracks);
        wave_g.connect_pressed(move |_g, _n, _x, _y| {
            if let Some(ref cb) = *queue_cb_wave.borrow() {
                let tracks = cur_tracks_c2.borrow().clone();
                cb(tracks, index);
            } else if let Some(ref cb) = *play_cb_wave.borrow() {
                cb(track_wave_play.clone());
            }
        });
        wave.widget().add_controller(wave_g);

        // Click title to view detail
        let title_g = gtk::GestureClick::new();
        let track_detail_for_click = track.clone();
        let detail_cb = Rc::clone(&self.on_select_track);
        title_g.connect_pressed(move |_g, _n, _x, _y| {
            if let Some(ref cb) = *detail_cb.borrow() {
                cb(track_detail_for_click.clone());
            }
        });
        title_lbl.add_controller(title_g);
        title_lbl.set_tooltip_text(Some("Click title to view comments & details"));

        // Right-click context menu
        let play_cb_ctx = Rc::clone(&self.on_play_track);
        let queue_cb_ctx = Rc::clone(&self.on_play_queue);
        let cur_tracks_ctx = Rc::clone(&self.current_tracks);
        let q_up_next_ctx = Rc::clone(&self.on_queue_track);
        let detail_cb_ctx = Rc::clone(&self.on_select_track);
        let like_cb_ctx = Rc::clone(&self.on_like_track);
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
            on_artist_profile: None,
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
        crate::ui::context_menu::attach_track_context_menu(&card, track, actions);

        card
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

    pub fn connect_play_all<F: Fn(Vec<Track>) + 'static>(&self, f: F) {
        *self.on_play_all.borrow_mut() = Some(Box::new(f));
    }

    pub fn connect_select_track<F: Fn(Track) + 'static>(&self, f: F) {
        *self.on_select_track.borrow_mut() = Some(Box::new(f));
    }

    pub fn connect_toggle_follow<F: Fn(u64, bool) + 'static>(&self, f: F) {
        *self.on_toggle_follow.borrow_mut() = Some(Box::new(f));
    }

    pub fn connect_queue_track<F: Fn(Track) + 'static>(&self, f: F) {
        *self.on_queue_track.borrow_mut() = Some(Box::new(f));
    }

    pub fn connect_stash_track<F: Fn(Track) + 'static>(&self, f: F) {
        *self.on_stash_track.borrow_mut() = Some(Box::new(f));
    }

    pub fn set_stash_checker<F: Fn(u64) -> bool + 'static>(&self, f: F) {
        *self.is_stashed_fn.borrow_mut() = Some(Box::new(f));
    }

    pub fn connect_back<F: Fn() + 'static>(&self, f: F) {
        *self.on_back.borrow_mut() = Some(Box::new(f));
    }
}
