use gtk4 as gtk;
use gtk::prelude::*;
use std::rc::Rc;

use crate::models::track::Track;

#[derive(Clone, Default)]
pub struct TrackActions {
    pub on_play: Option<Rc<dyn Fn(Track)>>,
    pub on_add_to_queue: Option<Rc<dyn Fn(Track)>>,
    pub on_view_details: Option<Rc<dyn Fn(Track)>>,
    pub on_artist_profile: Option<Rc<dyn Fn(u64)>>,
    pub on_toggle_like: Option<Rc<dyn Fn(Track, bool)>>,
}

pub fn attach_track_context_menu(
    target: &impl IsA<gtk::Widget>,
    track: Track,
    actions: TrackActions,
) {
    let popover = gtk::Popover::builder()
        .autohide(true)
        .has_arrow(true)
        .position(gtk::PositionType::Bottom)
        .css_classes(["track-context-popover"])
        .build();
    popover.set_parent(target);

    let menu_box = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(2)
        .margin_top(4)
        .margin_bottom(4)
        .margin_start(4)
        .margin_end(4)
        .width_request(190)
        .css_classes(["context-menu-box"])
        .build();

    let make_item = |icon: &str, label: &str| -> gtk::Button {
        let row = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(10)
            .margin_start(6)
            .margin_end(6)
            .margin_top(4)
            .margin_bottom(4)
            .build();
        let img = gtk::Image::builder()
            .icon_name(icon)
            .pixel_size(16)
            .build();
        let lbl = gtk::Label::builder()
            .label(label)
            .halign(gtk::Align::Start)
            .hexpand(true)
            .build();
        row.append(&img);
        row.append(&lbl);
        gtk::Button::builder()
            .child(&row)
            .css_classes(["flat", "context-menu-item"])
            .build()
    };

    // 1. Play Now
    if let Some(on_play) = actions.on_play.clone() {
        let btn = make_item("media-playback-start-symbolic", "Play Now");
        let t = track.clone();
        let pop = popover.clone();
        btn.connect_clicked(move |_| {
            pop.popdown();
            on_play(t.clone());
        });
        menu_box.append(&btn);
    }

    // 2. Add to Up Next / Queue
    if let Some(on_queue) = actions.on_add_to_queue.clone() {
        let btn = make_item("media-playlist-consecutive-symbolic", "Add to Up Next");
        let t = track.clone();
        let pop = popover.clone();
        btn.connect_clicked(move |_| {
            pop.popdown();
            on_queue(t.clone());
        });
        menu_box.append(&btn);
    }

    // 3. View Details
    if let Some(on_view_details) = actions.on_view_details.clone() {
        let btn = make_item("format-justify-left-symbolic", "View Details");
        let t = track.clone();
        let pop = popover.clone();
        btn.connect_clicked(move |_| {
            pop.popdown();
            on_view_details(t.clone());
        });
        menu_box.append(&btn);
    }

    // 4. Go to Artist Profile
    if let Some(on_artist) = actions.on_artist_profile.clone() {
        if let Some(user_id) = track.user_id() {
            let btn = make_item("avatar-default-symbolic", "Go to Artist Profile");
            let pop = popover.clone();
            btn.connect_clicked(move |_| {
                pop.popdown();
                on_artist(user_id);
            });
            menu_box.append(&btn);
        }
    }

    // 5. Like / Unlike Track
    if let Some(on_like) = actions.on_toggle_like.clone() {
        let is_liked = track.user_favorite.unwrap_or(false);
        let label = if is_liked { "Unlike Track" } else { "Like Track" };
        let icon = if is_liked { "starred-symbolic" } else { "non-starred-symbolic" };
        let btn = make_item(icon, label);
        let t = track.clone();
        let pop = popover.clone();
        btn.connect_clicked(move |_| {
            pop.popdown();
            on_like(t.clone(), !is_liked);
        });
        menu_box.append(&btn);
    }

    // Separator before Copy Link
    let sep = gtk::Separator::builder()
        .orientation(gtk::Orientation::Horizontal)
        .margin_top(4)
        .margin_bottom(4)
        .build();
    menu_box.append(&sep);

    // 6. Copy Track Link
    let btn_copy = make_item("edit-copy-symbolic", "Copy Track Link");
    let url = track.permalink_url.clone().unwrap_or_else(|| {
        format!("https://soundcloud.com/track/{}", track.id)
    });
    let pop_copy = popover.clone();
    btn_copy.connect_clicked(move |_| {
        pop_copy.popdown();
        if let Some(display) = gdk4::Display::default() {
            display.clipboard().set_text(&url);
        }
    });
    menu_box.append(&btn_copy);

    popover.set_child(Some(&menu_box));

    // Secondary click (right click) gesture on target widget
    let gesture = gtk::GestureClick::builder()
        .button(gdk4::BUTTON_SECONDARY)
        .build();

    let pop_gesture = popover.clone();
    gesture.connect_pressed(move |_g, _n, x, y| {
        let rect = gdk4::Rectangle::new(x as i32, y as i32, 1, 1);
        pop_gesture.set_pointing_to(Some(&rect));
        pop_gesture.popup();
    });

    target.add_controller(gesture);
}
