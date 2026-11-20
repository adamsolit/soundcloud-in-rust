use gtk4 as gtk;
use gtk::gdk;
use gtk::glib;
use gtk::prelude::*;
use std::cell::{Cell, RefCell};
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
    labels: Vec<gtk::Label>,
    btn_boxes: Vec<gtk::Box>,
    brand_box: gtk::Box,
    brand_icon: gtk::Image,
    brand_label: gtk::Label,
    btn_toggle: gtk::Button,
    is_collapsed: Rc<Cell<bool>>,
    on_navigate: Rc<RefCell<Option<Box<dyn Fn(NavTarget)>>>>,
    on_collapse_changed: Rc<RefCell<Option<Box<dyn Fn(bool)>>>>,
    apply_collapse: Rc<dyn Fn(bool)>,
}

impl Sidebar {
    pub fn new() -> Self {
        let is_collapsed = Rc::new(Cell::new(false));

        let container = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(6)
            .css_classes(["sidebar-panel"])
            .width_request(158)
            .hexpand(false)
            .build();
        container.set_hexpand_set(true);

        // Brand logo header with official SoundCloud vector icon & collapse button
        let brand_box = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(8)
            .margin_top(4)
            .margin_bottom(14)
            .margin_start(2)
            .margin_end(2)
            .hexpand(false)
            .build();
        brand_box.set_hexpand_set(true);

        let svg_bytes = glib::Bytes::from_static(include_bytes!("../../assets/soundcloud.svg"));
        let texture = gdk::Texture::from_bytes(&svg_bytes).ok();
        let brand_icon = gtk::Image::builder()
            .pixel_size(24)
            .valign(gtk::Align::Center)
            .build();
        if let Some(ref t) = texture {
            brand_icon.set_paintable(Some(t));
        } else {
            brand_icon.set_icon_name(Some("weather-overcast-symbolic"));
        }

        let brand_label = gtk::Label::builder()
            .label("SoundCloud")
            .css_classes(["brand-label"])
            .halign(gtk::Align::Start)
            .valign(gtk::Align::Center)
            .hexpand(false)
            .build();
        brand_label.set_hexpand_set(true);

        let btn_toggle = gtk::Button::builder()
            .icon_name("pan-start-symbolic")
            .css_classes(["btn-sidebar-toggle"])
            .tooltip_text("Collapse Sidebar (Ctrl+B)")
            .valign(gtk::Align::Center)
            .halign(gtk::Align::End)
            .hexpand(false)
            .build();
        btn_toggle.set_hexpand_set(true);

        brand_box.append(&brand_icon);
        brand_box.append(&brand_label);
        brand_box.append(&btn_toggle);
        container.append(&brand_box);

        let nav_items = [
            (NavTarget::Discover, "media-playback-start-symbolic", "Discover"),
            (NavTarget::Search, "system-search-symbolic", "Search"),
            (NavTarget::Library, "starred-symbolic", "Library"),
            (NavTarget::Upload, "document-send-symbolic", "Upload"),
            (NavTarget::Settings, "emblem-system-symbolic", "Settings"),
        ];

        let mut buttons = Vec::new();
        let mut labels = Vec::new();
        let mut btn_boxes = Vec::new();
        let on_navigate: Rc<RefCell<Option<Box<dyn Fn(NavTarget)>>>> = Rc::new(RefCell::new(None));
        let on_collapse_changed: Rc<RefCell<Option<Box<dyn Fn(bool)>>>> = Rc::new(RefCell::new(None));

        for (target, icon_name, label_text) in nav_items {
            let btn_box = gtk::Box::builder()
                .orientation(gtk::Orientation::Horizontal)
                .spacing(10)
                .halign(gtk::Align::Start)
                .hexpand(false)
                .build();
            btn_box.set_hexpand_set(true);

            let btn_icon = gtk::Image::builder()
                .icon_name(icon_name)
                .pixel_size(18)
                .valign(gtk::Align::Center)
                .build();

            let label = gtk::Label::builder()
                .label(label_text)
                .halign(gtk::Align::Start)
                .valign(gtk::Align::Center)
                .hexpand(false)
                .build();
            label.set_hexpand_set(true);

            btn_box.append(&btn_icon);
            btn_box.append(&label);

            let btn = gtk::Button::builder()
                .child(&btn_box)
                .css_classes(["nav-button"])
                .halign(gtk::Align::Fill)
                .hexpand(false)
                .tooltip_text(label_text)
                .build();
            btn.set_hexpand_set(true);

            if target == NavTarget::Discover {
                btn.add_css_class("active");
            }

            container.append(&btn);
            buttons.push((target, btn));
            labels.push(label);
            btn_boxes.push(btn_box);
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

        // Bottom spacer to push content up cleanly
        let spacer = gtk::Box::builder()
            .vexpand(true)
            .hexpand(false)
            .build();
        spacer.set_hexpand_set(true);
        container.append(&spacer);

        // Define unified collapse state applier
        let is_col_apply = Rc::clone(&is_collapsed);
        let container_apply = container.clone();
        let brand_icon_apply = brand_icon.clone();
        let brand_label_apply = brand_label.clone();
        let brand_box_apply = brand_box.clone();
        let btn_toggle_apply = btn_toggle.clone();
        let labels_apply = labels.clone();
        let btn_boxes_apply = btn_boxes.clone();
        let on_col_cb = Rc::clone(&on_collapse_changed);

        let apply_collapse: Rc<dyn Fn(bool)> = Rc::new(move |collapsed: bool| {
            is_col_apply.set(collapsed);
            if collapsed {
                container_apply.set_width_request(54);
                container_apply.add_css_class("collapsed");
                brand_icon_apply.set_visible(false);
                brand_label_apply.set_visible(false);
                brand_box_apply.set_halign(gtk::Align::Center);
                btn_toggle_apply.set_icon_name("pan-end-symbolic");
                btn_toggle_apply.set_tooltip_text(Some("Expand Sidebar (Ctrl+B)"));
                for l in &labels_apply {
                    l.set_visible(false);
                }
                for b in &btn_boxes_apply {
                    b.set_halign(gtk::Align::Center);
                }
            } else {
                container_apply.set_width_request(158);
                container_apply.remove_css_class("collapsed");
                brand_icon_apply.set_visible(true);
                brand_label_apply.set_visible(true);
                brand_box_apply.set_halign(gtk::Align::Fill);
                btn_toggle_apply.set_icon_name("pan-start-symbolic");
                btn_toggle_apply.set_tooltip_text(Some("Collapse Sidebar (Ctrl+B)"));
                for l in &labels_apply {
                    l.set_visible(true);
                }
                for b in &btn_boxes_apply {
                    b.set_halign(gtk::Align::Start);
                }
            }
            container_apply.queue_resize();
            if let Some(parent) = container_apply.parent() {
                parent.queue_resize();
            }
            if let Some(ref cb) = *on_col_cb.borrow() {
                cb(collapsed);
            }
        });

        // Connect toggle button
        let apply_toggle = Rc::clone(&apply_collapse);
        let is_col_toggle = Rc::clone(&is_collapsed);
        btn_toggle.connect_clicked(move |_| {
            let next = !is_col_toggle.get();
            apply_toggle(next);
        });

        Self {
            container,
            buttons,
            labels,
            btn_boxes,
            brand_box,
            brand_icon,
            brand_label,
            btn_toggle,
            is_collapsed,
            on_navigate,
            on_collapse_changed,
            apply_collapse,
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

    pub fn toggle(&self) {
        let next = !self.is_collapsed.get();
        self.set_collapsed(next);
    }

    pub fn set_collapsed(&self, collapsed: bool) {
        (self.apply_collapse)(collapsed);
    }

    pub fn is_collapsed(&self) -> bool {
        self.is_collapsed.get()
    }

    pub fn connect_collapse_changed<F: Fn(bool) + 'static>(&self, f: F) {
        *self.on_collapse_changed.borrow_mut() = Some(Box::new(f));
    }

    pub fn update_user_profile(&self, _user: Option<&UserProfile>) {
        // User profile is now rendered in top-right headerbar
    }

    pub fn connect_navigate<F: Fn(NavTarget) + 'static>(&self, f: F) {
        *self.on_navigate.borrow_mut() = Some(Box::new(f));
    }
}
