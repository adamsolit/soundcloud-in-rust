use gtk4 as gtk;
use gtk::prelude::*;
use std::cell::RefCell;
use std::rc::Rc;

use crate::models::activity::ActivityItem;
use crate::models::track::Track;

pub struct NotificationsView {
    container: gtk::Box,
    list_box: gtk::Box,
    loading_spinner: gtk::Spinner,
    refresh_btn: gtk::Button,
    on_play_track: Rc<RefCell<Option<Box<dyn Fn(Track)>>>>,
    on_refresh: Rc<RefCell<Option<Box<dyn Fn()>>>>,
}

impl NotificationsView {
    pub fn new() -> Self {
        let container = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(16)
            .margin_top(18)
            .margin_bottom(18)
            .margin_start(24)
            .margin_end(24)
            .hexpand(true)
            .vexpand(true)
            .build();

        // Header
        let header_row = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(12)
            .build();

        let title = gtk::Label::builder()
            .label("Notifications")
            .css_classes(["brand-label"])
            .halign(gtk::Align::Start)
            .hexpand(true)
            .build();

        let loading_spinner = gtk::Spinner::builder()
            .spinning(false)
            .valign(gtk::Align::Center)
            .build();

        let refresh_btn = gtk::Button::builder()
            .icon_name("view-refresh-symbolic")
            .css_classes(["filter-chip"])
            .tooltip_text("Refresh activities")
            .build();

        header_row.append(&title);
        header_row.append(&loading_spinner);
        header_row.append(&refresh_btn);
        container.append(&header_row);

        // Scrolled notifications list
        let scrolled = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Never)
            .vscrollbar_policy(gtk::PolicyType::Automatic)
            .hexpand(true)
            .vexpand(true)
            .build();

        let list_box = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(8)
            .hexpand(true)
            .build();

        scrolled.set_child(Some(&list_box));
        container.append(&scrolled);

        let on_play_track: Rc<RefCell<Option<Box<dyn Fn(Track)>>>> = Rc::new(RefCell::new(None));
        let on_refresh: Rc<RefCell<Option<Box<dyn Fn()>>>> = Rc::new(RefCell::new(None));

        let ref_cb = Rc::clone(&on_refresh);
        refresh_btn.connect_clicked(move |_| {
            if let Some(ref cb) = *ref_cb.borrow() {
                cb();
            }
        });

        Self {
            container,
            list_box,
            loading_spinner,
            refresh_btn,
            on_play_track,
            on_refresh,
        }
    }

    pub fn widget(&self) -> &gtk::Box {
        &self.container
    }

    pub fn set_loading(&self, loading: bool) {
        self.loading_spinner.set_spinning(loading);
        self.refresh_btn.set_sensitive(!loading);
    }

    pub fn set_activities(&self, activities: Vec<ActivityItem>) {
        while let Some(child) = self.list_box.first_child() {
            self.list_box.remove(&child);
        }

        if activities.is_empty() {
            let empty_lbl = gtk::Label::builder()
                .label("No recent notifications or activities.")
                .css_classes(["player-artist"])
                .margin_top(40)
                .build();
            self.list_box.append(&empty_lbl);
            return;
        }

        for item in activities {
            let card = self.create_activity_card(item);
            self.list_box.append(&card);
        }
    }

    fn create_activity_card(&self, item: ActivityItem) -> gtk::Box {
        let card = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(14)
            .css_classes(["track-card"])
            .hexpand(true)
            .build();

        // User avatar
        let avatar_img = gtk::Image::builder()
            .pixel_size(44)
            .icon_name("avatar-default-symbolic")
            .css_classes(["track-card-artwork"])
            .valign(gtk::Align::Center)
            .build();

        if let Some(ref user) = item.user {
            crate::ui::image_loader::load_into_image(
                &avatar_img,
                user.avatar_url.as_deref(),
                "avatar-default-symbolic",
            );
        }
        card.append(&avatar_img);

        // Content description
        let text_vbox = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(4)
            .valign(gtk::Align::Center)
            .hexpand(true)
            .build();

        let desc_lbl = gtk::Label::builder()
            .label(&item.action_summary())
            .css_classes(["track-card-title"])
            .halign(gtk::Align::Start)
            .ellipsize(gtk::pango::EllipsizeMode::End)
            .build();

        let time_formatted = item.created_at.split('T').next().unwrap_or(&item.created_at);
        let time_lbl = gtk::Label::builder()
            .label(time_formatted)
            .css_classes(["time-label"])
            .halign(gtk::Align::Start)
            .build();

        text_vbox.append(&desc_lbl);
        text_vbox.append(&time_lbl);
        card.append(&text_vbox);

        // If activity has an associated track, show quick play button
        if let Some(target_t) = item.target_track() {
            let play_btn = gtk::Button::builder()
                .icon_name("media-playback-start-symbolic")
                .css_classes(["btn-transport-play"])
                .width_request(34)
                .height_request(34)
                .valign(gtk::Align::Center)
                .tooltip_text("Play track")
                .build();

            let play_cb = Rc::clone(&self.on_play_track);
            play_btn.connect_clicked(move |_| {
                if let Some(ref cb) = *play_cb.borrow() {
                    cb(target_t.clone());
                }
            });
            card.append(&play_btn);
        }

        card
    }

    pub fn connect_play_track<F: Fn(Track) + 'static>(&self, f: F) {
        *self.on_play_track.borrow_mut() = Some(Box::new(f));
    }

    pub fn connect_refresh<F: Fn() + 'static>(&self, f: F) {
        *self.on_refresh.borrow_mut() = Some(Box::new(f));
    }
}
