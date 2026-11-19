use gtk4 as gtk;
use gtk::gdk;
use gtk::glib;
use gtk::prelude::*;
use std::cell::RefCell;
use std::rc::Rc;

use crate::models::user::UserProfile;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NavTarget {
    Discover,
    Search,
    Library,
    Notifications,
    Messages,
    Upload,
    Settings,
}

pub struct Sidebar {
    container: gtk::Box,
    buttons: Vec<(NavTarget, gtk::Button)>,
    user_avatar: gtk::Image,
    user_name_lbl: gtk::Label,
    user_status_lbl: gtk::Label,
    on_navigate: Rc<RefCell<Option<Box<dyn Fn(NavTarget)>>>>,
}

impl Sidebar {
    pub fn new() -> Self {
        let container = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(6)
            .css_classes(["sidebar-panel"])
            .width_request(200)
            .build();

        // Brand logo header with official SoundCloud vector icon
        let brand_box = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(10)
            .margin_top(8)
            .margin_bottom(18)
            .margin_start(8)
            .build();

        let svg_bytes = glib::Bytes::from_static(include_bytes!("../../assets/soundcloud.svg"));
        let texture = gdk::Texture::from_bytes(&svg_bytes).ok();
        let icon = gtk::Image::builder()
            .pixel_size(28)
            .valign(gtk::Align::Center)
            .build();
        if let Some(ref t) = texture {
            icon.set_paintable(Some(t));
        } else {
            icon.set_icon_name(Some("weather-overcast-symbolic"));
        }

        let brand_label = gtk::Label::builder()
            .label("SoundCloud")
            .css_classes(["brand-label"])
            .halign(gtk::Align::Start)
            .valign(gtk::Align::Center)
            .build();

        brand_box.append(&icon);
        brand_box.append(&brand_label);
        container.append(&brand_box);

        let nav_items = [
            (NavTarget::Discover, "media-playback-start-symbolic", "Discover"),
            (NavTarget::Search, "system-search-symbolic", "Search"),
            (NavTarget::Library, "starred-symbolic", "Library"),
            (NavTarget::Upload, "document-send-symbolic", "Upload"),
            (NavTarget::Settings, "emblem-system-symbolic", "Settings"),
        ];

        let mut buttons = Vec::new();
        let on_navigate: Rc<RefCell<Option<Box<dyn Fn(NavTarget)>>>> = Rc::new(RefCell::new(None));

        for (target, icon_name, label_text) in nav_items {
            let btn_box = gtk::Box::builder()
                .orientation(gtk::Orientation::Horizontal)
                .spacing(12)
                .build();

            let btn_icon = gtk::Image::builder()
                .icon_name(icon_name)
                .pixel_size(18)
                .build();

            let label = gtk::Label::builder()
                .label(label_text)
                .halign(gtk::Align::Start)
                .hexpand(true)
                .build();

            btn_box.append(&btn_icon);
            btn_box.append(&label);

            let btn = gtk::Button::builder()
                .child(&btn_box)
                .css_classes(["nav-button"])
                .halign(gtk::Align::Fill)
                .build();

            if target == NavTarget::Discover {
                btn.add_css_class("active");
            }

            container.append(&btn);
            buttons.push((target, btn));
        }

        // Connect button activations
        for (target, btn) in &buttons {
            let t = *target;
            let nav_cb = Rc::clone(&on_navigate);
            btn.connect_clicked(move |_| {
                if let Some(ref cb) = *nav_cb.borrow() {
                    cb(t);
                }
            });
        }

        // Bottom spacer to push user pill down
        let spacer = gtk::Box::builder()
            .vexpand(true)
            .build();
        container.append(&spacer);

        // Bottom User Profile Pill
        let user_pill_box = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(10)
            .build();

        let user_avatar = gtk::Image::builder()
            .pixel_size(36)
            .icon_name("avatar-default-symbolic")
            .css_classes(["track-card-artwork"])
            .build();

        let user_text_vbox = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(2)
            .valign(gtk::Align::Center)
            .hexpand(true)
            .build();

        let user_name_lbl = gtk::Label::builder()
            .label("Guest Mode")
            .css_classes(["track-card-title"])
            .halign(gtk::Align::Start)
            .ellipsize(gtk::pango::EllipsizeMode::End)
            .build();

        let user_status_lbl = gtk::Label::builder()
            .label("Sign in via Settings")
            .css_classes(["badge-guest"])
            .halign(gtk::Align::Start)
            .build();

        user_text_vbox.append(&user_name_lbl);
        user_text_vbox.append(&user_status_lbl);

        user_pill_box.append(&user_avatar);
        user_pill_box.append(&user_text_vbox);

        let user_pill_btn = gtk::Button::builder()
            .child(&user_pill_box)
            .css_classes(["sidebar-user-pill"])
            .tooltip_text("Open Settings & Account")
            .build();

        let nav_cb_user = Rc::clone(&on_navigate);
        user_pill_btn.connect_clicked(move |_| {
            if let Some(ref cb) = *nav_cb_user.borrow() {
                cb(NavTarget::Settings);
            }
        });

        container.append(&user_pill_btn);

        Self {
            container,
            buttons,
            user_avatar,
            user_name_lbl,
            user_status_lbl,
            on_navigate,
        }
    }

    pub fn widget(&self) -> &gtk::Box {
        &self.container
    }

    pub fn set_active(&self, target: NavTarget) {
        for (t, btn) in &self.buttons {
            if *t == target {
                btn.add_css_class("active");
            } else {
                btn.remove_css_class("active");
            }
        }
    }

    pub fn update_user_profile(&self, user: Option<&UserProfile>) {
        if let Some(u) = user {
            self.user_name_lbl.set_label(&format!("@{}", u.username));
            self.user_status_lbl.set_label("Connected");
            self.user_status_lbl.remove_css_class("badge-guest");
            self.user_status_lbl.add_css_class("badge-connected");

            crate::ui::image_loader::load_into_image(
                &self.user_avatar,
                u.avatar_url.as_deref(),
                "avatar-default-symbolic",
            );
        } else {
            self.user_name_lbl.set_label("Guest Mode");
            self.user_status_lbl.set_label("Sign in via Settings");
            self.user_status_lbl.remove_css_class("badge-connected");
            self.user_status_lbl.add_css_class("badge-guest");
            self.user_avatar.set_icon_name(Some("avatar-default-symbolic"));
        }
    }

    pub fn connect_navigate<F: Fn(NavTarget) + 'static>(&self, f: F) {
        *self.on_navigate.borrow_mut() = Some(Box::new(f));
    }
}
