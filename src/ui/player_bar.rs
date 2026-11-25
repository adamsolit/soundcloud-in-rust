use gtk4 as gtk;
use gtk::prelude::*;
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use tracing::info;

use crate::models::track::{format_milliseconds, Track};
use crate::ui::waveform_widget::WaveformWidget;

pub struct PlayerBar {
    container: gtk::Box,
    artwork: gtk::Image,
    title_label: gtk::Label,
    artist_label: gtk::Label,
    like_btn: gtk::Button,
    btn_play_pause: gtk::Button,
    btn_prev: gtk::Button,
    btn_next: gtk::Button,
    btn_shuffle: gtk::Button,
    btn_repeat: gtk::Button,
    btn_more: gtk::Button,
    more_stash_icon: gtk::Image,
    more_stash_label: gtk::Label,
    btn_more_stash: gtk::Button,
    speed_slider: gtk::Scale,
    speed_val_lbl: gtk::Label,
    sleep_status_lbl: gtk::Label,
    bookmark_popover: gtk::Popover,
    bm_prompt_entry: gtk::Entry,
    bm_prompt_title: gtk::Label,
    bm_current_pos_ms: Rc<Cell<u64>>,
    wave_row: gtk::Box,
    waveform: Rc<WaveformWidget>,
    time_current: gtk::Label,
    time_total: gtk::Label,
    volume_scale: gtk::Scale,
    vol_btn: gtk::Button,
    btn_queue: gtk::Button,
    btn_mini: gtk::Button,
    q_title_lbl: gtk::Label,
    q_items_flow: gtk::Box,
    current_track: Rc<RefCell<Option<Track>>>,
    is_playing: Rc<RefCell<bool>>,
    is_liked: Rc<RefCell<bool>>,
    is_stashed: Rc<RefCell<bool>>,
    saved_volume: Rc<Cell<f64>>,
    is_muted: Rc<Cell<bool>>,
    on_queue_jump: Rc<RefCell<Option<Box<dyn Fn(usize)>>>>,
    on_queue_remove: Rc<RefCell<Option<Box<dyn Fn(usize)>>>>,
    on_queue_clear: Rc<RefCell<Option<Box<dyn Fn()>>>>,
    on_toggle_mini: Rc<RefCell<Option<Box<dyn Fn()>>>>,
    on_speed_change: Rc<RefCell<Option<Box<dyn Fn(f64)>>>>,
    on_sleep_timer: Rc<RefCell<Option<Box<dyn Fn(Option<u32>, bool)>>>>,
    on_add_bookmark: Rc<RefCell<Option<Box<dyn Fn()>>>>,
    on_stash_toggle: Rc<RefCell<Option<Box<dyn Fn()>>>>,
    on_save_bookmark: Rc<RefCell<Option<Box<dyn Fn(u64, String)>>>>,
}

impl PlayerBar {
    pub fn new() -> Self {
        let container = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(4)
            .css_classes(["player-bar"])
            .hexpand(true)
            .build();

        // Top row: waveform and time labels
        let wave_row = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(12)
            .hexpand(true)
            .valign(gtk::Align::Center)
            .build();

        let time_current = gtk::Label::builder()
            .label("0:00")
            .css_classes(["time-label"])
            .width_chars(5)
            .xalign(1.0)
            .build();

        let waveform = Rc::new(WaveformWidget::new(38));

        let time_total = gtk::Label::builder()
            .label("0:00")
            .css_classes(["time-label"])
            .width_chars(5)
            .xalign(0.0)
            .build();

        wave_row.append(&time_current);
        wave_row.append(waveform.widget());
        wave_row.append(&time_total);

        // Bottom row: Info, Controls, Volume/Extras
        let controls_row = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(16)
            .hexpand(true)
            .valign(gtk::Align::Center)
            .build();

        // 1. Left section: Track info (ONLY artwork, title & artist, and like button)
        let info_box = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(12)
            .hexpand(false)
            .halign(gtk::Align::Start)
            .build();

        let artwork = gtk::Image::builder()
            .pixel_size(48)
            .icon_name("audio-x-generic-symbolic")
            .css_classes(["player-artwork"])
            .build();

        let titles_vbox = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(2)
            .valign(gtk::Align::Center)
            .build();

        let title_label = gtk::Label::builder()
            .label("No track selected")
            .css_classes(["player-title"])
            .xalign(0.0)
            .ellipsize(gtk::pango::EllipsizeMode::End)
            .max_width_chars(50)
            .build();

        let artist_label = gtk::Label::builder()
            .label("SoundCloud")
            .css_classes(["player-artist"])
            .xalign(0.0)
            .ellipsize(gtk::pango::EllipsizeMode::End)
            .max_width_chars(35)
            .build();

        titles_vbox.append(&title_label);
        titles_vbox.append(&artist_label);

        let like_btn = gtk::Button::builder()
            .icon_name("starred-symbolic")
            .css_classes(["btn-transport"])
            .tooltip_text("Like track")
            .valign(gtk::Align::Center)
            .build();

        info_box.append(&artwork);
        info_box.append(&titles_vbox);
        info_box.append(&like_btn);

        // 2. Center section: Transport Buttons
        let center_box = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(8)
            .hexpand(true)
            .halign(gtk::Align::Center)
            .valign(gtk::Align::Center)
            .build();

        let btn_shuffle = gtk::Button::builder()
            .icon_name("media-playlist-shuffle-symbolic")
            .css_classes(["btn-transport"])
            .tooltip_text("Shuffle")
            .build();

        let btn_prev = gtk::Button::builder()
            .icon_name("media-skip-backward-symbolic")
            .css_classes(["btn-transport"])
            .tooltip_text("Previous Track")
            .build();

        let btn_play_pause = gtk::Button::builder()
            .icon_name("media-playback-start-symbolic")
            .css_classes(["btn-transport-play"])
            .tooltip_text("Play")
            .build();

        let btn_next = gtk::Button::builder()
            .icon_name("media-skip-forward-symbolic")
            .css_classes(["btn-transport"])
            .tooltip_text("Next Track")
            .build();

        let btn_repeat = gtk::Button::builder()
            .icon_name("media-playlist-repeat-symbolic")
            .css_classes(["btn-transport"])
            .tooltip_text("Repeat")
            .build();

        center_box.append(&btn_shuffle);
        center_box.append(&btn_prev);
        center_box.append(&btn_play_pause);
        center_box.append(&btn_next);
        center_box.append(&btn_repeat);

        // 3. Right section: [ ... (More) ] [ Queue ] [ Mute ] [ Volume Slider ] [ Minimize ]
        let right_box = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(10)
            .hexpand(false)
            .halign(gtk::Align::End)
            .valign(gtk::Align::Center)
            .build();

        // --- A. Three-dot ... More Button & Popover ---
        let btn_more = gtk::Button::builder()
            .icon_name("view-more-horizontal-symbolic")
            .css_classes(["btn-transport"])
            .tooltip_text("More Options")
            .valign(gtk::Align::Center)
            .build();

        let more_popover = gtk::Popover::builder()
            .autohide(true)
            .has_arrow(true)
            .position(gtk::PositionType::Top)
            .build();
        more_popover.set_parent(&btn_more);

        let more_box = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(4)
            .margin_top(8)
            .margin_bottom(8)
            .margin_start(8)
            .margin_end(8)
            .width_request(260)
            .build();

        let make_more_item = |icon: &str, label: &str| -> (gtk::Button, gtk::Image, gtk::Label) {
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
            let btn = gtk::Button::builder()
                .child(&row)
                .css_classes(["flat", "context-menu-item"])
                .build();
            (btn, img, lbl)
        };

        // Item 1: Bookmark
        let (btn_more_bookmark, _, _) = make_more_item("bookmark-new-symbolic", "Add Bookmark at Position (B)");
        more_box.append(&btn_more_bookmark);

        // Item 2: Download / Stash
        let (btn_more_stash, more_stash_icon, more_stash_label) = make_more_item("folder-download-symbolic", "Download Track Offline");
        more_box.append(&btn_more_stash);

        // Item 3: Share link
        let (share_btn, _, _) = make_more_item("emblem-shared-symbolic", "Copy Track Link");
        more_box.append(&share_btn);

        // Separator
        let sep1 = gtk::Separator::builder()
            .orientation(gtk::Orientation::Horizontal)
            .margin_top(4)
            .margin_bottom(4)
            .build();
        more_box.append(&sep1);

        // Section: Playback Speed (0.5x - 2.5x)
        let speed_hdr = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .margin_start(6)
            .margin_end(6)
            .margin_top(2)
            .margin_bottom(2)
            .build();

        let speed_title = gtk::Label::builder()
            .label("PLAYBACK SPEED")
            .css_classes(["time-label"])
            .halign(gtk::Align::Start)
            .hexpand(true)
            .build();

        let speed_val_lbl = gtk::Label::builder()
            .label("1.00x")
            .css_classes(["time-label"])
            .halign(gtk::Align::End)
            .build();

        speed_hdr.append(&speed_title);
        speed_hdr.append(&speed_val_lbl);
        more_box.append(&speed_hdr);

        let speed_presets_box = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(4)
            .halign(gtk::Align::Center)
            .margin_top(2)
            .margin_bottom(4)
            .build();

        let speed_slider = gtk::Scale::with_range(gtk::Orientation::Horizontal, 0.5, 2.5, 0.05);
        speed_slider.set_value(1.0);
        speed_slider.set_hexpand(true);
        speed_slider.add_css_class("volume-scale");
        speed_slider.set_margin_start(6);
        speed_slider.set_margin_end(6);

        let presets: [f64; 5] = [0.75, 1.0, 1.25, 1.5, 2.0];
        for sp in presets {
            let chip = gtk::Button::builder()
                .label(&format!("{:.2}x", sp).replace(".00", "").replace("0x", "x"))
                .css_classes(["filter-chip"])
                .build();
            let s_slider = speed_slider.clone();
            chip.connect_clicked(move |_| {
                s_slider.set_value(sp);
            });
            speed_presets_box.append(&chip);
        }
        more_box.append(&speed_presets_box);
        more_box.append(&speed_slider);

        // Separator
        let sep2 = gtk::Separator::builder()
            .orientation(gtk::Orientation::Horizontal)
            .margin_top(4)
            .margin_bottom(4)
            .build();
        more_box.append(&sep2);

        // Section: Sleep Timer
        let sleep_hdr = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .margin_start(6)
            .margin_end(6)
            .margin_top(2)
            .margin_bottom(2)
            .build();

        let sleep_title = gtk::Label::builder()
            .label("SLEEP TIMER")
            .css_classes(["time-label"])
            .halign(gtk::Align::Start)
            .hexpand(true)
            .build();

        let sleep_status_lbl = gtk::Label::builder()
            .label("Off")
            .css_classes(["player-artist"])
            .halign(gtk::Align::End)
            .build();

        sleep_hdr.append(&sleep_title);
        sleep_hdr.append(&sleep_status_lbl);
        more_box.append(&sleep_hdr);

        let on_sleep_timer: Rc<RefCell<Option<Box<dyn Fn(Option<u32>, bool)>>>> = Rc::new(RefCell::new(None));

        let sleep_grid_row1 = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(4)
            .halign(gtk::Align::Center)
            .margin_top(2)
            .build();

        let sleep_grid_row2 = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(4)
            .halign(gtk::Align::Center)
            .margin_top(4)
            .margin_bottom(4)
            .build();

        let sleep_opts: Vec<(&str, Option<u32>, bool)> = vec![
            ("Off", None, false),
            ("15m", Some(15 * 60), false),
            ("30m", Some(30 * 60), false),
            ("45m", Some(45 * 60), false),
            ("60m", Some(60 * 60), false),
            ("End of Track", None, true),
        ];

        for (i, (lbl, secs, end_track)) in sleep_opts.into_iter().enumerate() {
            let chip = gtk::Button::builder()
                .label(lbl)
                .css_classes(["filter-chip"])
                .build();
            let sleep_cb_c = Rc::clone(&on_sleep_timer);
            let pop_c = more_popover.clone();
            chip.connect_clicked(move |_| {
                if let Some(ref cb) = *sleep_cb_c.borrow() {
                    cb(secs, end_track);
                }
                pop_c.popdown();
            });
            if i < 3 {
                sleep_grid_row1.append(&chip);
            } else {
                sleep_grid_row2.append(&chip);
            }
        }
        more_box.append(&sleep_grid_row1);
        more_box.append(&sleep_grid_row2);

        more_popover.set_child(Some(&more_box));

        let more_pop_c = more_popover.clone();
        btn_more.connect_clicked(move |_| {
            more_pop_c.popup();
        });

        // --- B. Bookmark Naming Prompt Popover ---
        let bookmark_popover = gtk::Popover::builder()
            .autohide(true)
            .has_arrow(true)
            .position(gtk::PositionType::Top)
            .build();
        bookmark_popover.set_parent(&wave_row);

        let bm_box = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(8)
            .margin_top(10)
            .margin_bottom(10)
            .margin_start(12)
            .margin_end(12)
            .width_request(280)
            .build();

        let bm_header = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(8)
            .build();

        let bm_icon = gtk::Image::builder()
            .icon_name("bookmark-new-symbolic")
            .pixel_size(16)
            .build();

        let bm_prompt_title = gtk::Label::builder()
            .label("Add Bookmark")
            .css_classes(["brand-label"])
            .halign(gtk::Align::Start)
            .hexpand(true)
            .build();

        bm_header.append(&bm_icon);
        bm_header.append(&bm_prompt_title);
        bm_box.append(&bm_header);

        let bm_prompt_entry = gtk::Entry::builder()
            .placeholder_text("Bookmark name (e.g. Intro, Drop, Solo)...")
            .css_classes(["sc-search-entry"])
            .build();
        bm_box.append(&bm_prompt_entry);

        let bm_btn_row = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(8)
            .halign(gtk::Align::End)
            .build();

        let bm_cancel_btn = gtk::Button::builder()
            .label("Cancel")
            .css_classes(["filter-chip"])
            .build();

        let bm_save_btn = gtk::Button::builder()
            .label("Save Bookmark")
            .css_classes(["filter-chip", "active"])
            .build();

        bm_btn_row.append(&bm_cancel_btn);
        bm_btn_row.append(&bm_save_btn);
        bm_box.append(&bm_btn_row);

        bookmark_popover.set_child(Some(&bm_box));

        let bm_pop_cancel = bookmark_popover.clone();
        bm_cancel_btn.connect_clicked(move |_| {
            bm_pop_cancel.popdown();
        });

        let bm_current_pos_ms = Rc::new(Cell::new(0u64));
        let on_save_bookmark: Rc<RefCell<Option<Box<dyn Fn(u64, String)>>>> = Rc::new(RefCell::new(None));

        let do_save = {
            let pop = bookmark_popover.clone();
            let entry = bm_prompt_entry.clone();
            let pos_c = Rc::clone(&bm_current_pos_ms);
            let save_cb = Rc::clone(&on_save_bookmark);
            move || {
                let text = entry.text().trim().to_string();
                let note = if text.is_empty() { "Bookmark".to_string() } else { text };
                let pos = pos_c.get();
                pop.popdown();
                if let Some(ref cb) = *save_cb.borrow() {
                    cb(pos, note);
                }
            }
        };

        let do_save_c1 = do_save.clone();
        bm_save_btn.connect_clicked(move |_| do_save_c1());

        let do_save_c2 = do_save.clone();
        bm_prompt_entry.connect_activate(move |_| do_save_c2());

        let bm_pop_closed = bookmark_popover.clone();
        bookmark_popover.connect_closed(move |pop| {
            if let Some(root) = pop.root().and_then(|r| r.downcast::<gtk::Window>().ok()) {
                if let Some(focused) = gtk::prelude::GtkWindowExt::focus(&root) {
                    if focused.is_ancestor(&bm_pop_closed) || &focused == bm_pop_closed.upcast_ref::<gtk::Widget>() {
                        gtk::prelude::GtkWindowExt::set_focus(&root, None::<&gtk::Widget>);
                    }
                }
            }
        });

        // --- C. Queue Flyout Button & Popover ---
        let btn_queue = gtk::Button::builder()
            .icon_name("view-list-bullet-symbolic")
            .css_classes(["btn-transport"])
            .tooltip_text("Up Next & Playback Queue")
            .valign(gtk::Align::Center)
            .build();

        let queue_popover = gtk::Popover::builder()
            .autohide(true)
            .has_arrow(true)
            .position(gtk::PositionType::Top)
            .build();
        queue_popover.set_parent(&btn_queue);

        let queue_container = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(10)
            .width_request(380)
            .height_request(420)
            .margin_top(8)
            .margin_bottom(8)
            .margin_start(10)
            .margin_end(10)
            .build();

        let q_header = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(8)
            .build();

        let q_title_lbl = gtk::Label::builder()
            .label("Up Next (0)")
            .css_classes(["brand-label"])
            .halign(gtk::Align::Start)
            .hexpand(true)
            .build();

        let q_clear_btn = gtk::Button::builder()
            .label("Clear Queue")
            .css_classes(["filter-chip"])
            .halign(gtk::Align::End)
            .build();

        q_header.append(&q_title_lbl);
        q_header.append(&q_clear_btn);
        queue_container.append(&q_header);

        let q_scroller = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Never)
            .vscrollbar_policy(gtk::PolicyType::Automatic)
            .hexpand(true)
            .vexpand(true)
            .build();

        let q_items_flow = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(6)
            .build();

        q_scroller.set_child(Some(&q_items_flow));
        queue_container.append(&q_scroller);
        queue_popover.set_child(Some(&queue_container));

        let q_pop_c = queue_popover.clone();
        btn_queue.connect_clicked(move |_| {
            q_pop_c.popup();
        });

        // --- D. Volume Mute Button & Volume Slider ---
        let vol_btn = gtk::Button::builder()
            .icon_name("audio-volume-high-symbolic")
            .css_classes(["btn-transport"])
            .tooltip_text("Mute")
            .valign(gtk::Align::Center)
            .build();

        let volume_scale = gtk::Scale::with_range(gtk::Orientation::Horizontal, 0.0, 1.0, 0.05);
        volume_scale.set_value(1.0);
        volume_scale.set_width_request(100);
        volume_scale.add_css_class("volume-scale");
        volume_scale.set_tooltip_text(Some("Volume"));

        let saved_volume = Rc::new(Cell::new(1.0));
        let is_muted = Rc::new(Cell::new(false));

        // Click on audio icon toggles mute / unmute
        let vol_scale_click = volume_scale.clone();
        let vol_btn_click = vol_btn.clone();
        let saved_vol_click = Rc::clone(&saved_volume);
        let is_muted_click = Rc::clone(&is_muted);

        vol_btn.connect_clicked(move |_| {
            if is_muted_click.get() {
                // Restore volume
                is_muted_click.set(false);
                let prev = saved_vol_click.get();
                let target = if prev <= 0.05 { 0.75 } else { prev };
                vol_scale_click.set_value(target);
                if target > 0.66 {
                    vol_btn_click.set_icon_name("audio-volume-high-symbolic");
                } else if target > 0.33 {
                    vol_btn_click.set_icon_name("audio-volume-medium-symbolic");
                } else {
                    vol_btn_click.set_icon_name("audio-volume-low-symbolic");
                }
                vol_btn_click.set_tooltip_text(Some("Mute"));
            } else {
                // Mute
                let cur = vol_scale_click.value();
                if cur > 0.05 {
                    saved_vol_click.set(cur);
                }
                is_muted_click.set(true);
                vol_scale_click.set_value(0.0);
                vol_btn_click.set_icon_name("audio-volume-muted-symbolic");
                vol_btn_click.set_tooltip_text(Some("Unmute"));
            }
        });

        // When user drags the volume slider, update mute state and icon dynamically
        let vol_btn_change = vol_btn.clone();
        let is_muted_change = Rc::clone(&is_muted);
        let saved_vol_change = Rc::clone(&saved_volume);

        volume_scale.connect_value_changed(move |scale| {
            let val = scale.value();
            if val <= 0.001 {
                is_muted_change.set(true);
                vol_btn_change.set_icon_name("audio-volume-muted-symbolic");
                vol_btn_change.set_tooltip_text(Some("Unmute"));
            } else {
                is_muted_change.set(false);
                saved_vol_change.set(val);
                if val > 0.66 {
                    vol_btn_change.set_icon_name("audio-volume-high-symbolic");
                } else if val > 0.33 {
                    vol_btn_change.set_icon_name("audio-volume-medium-symbolic");
                } else {
                    vol_btn_change.set_icon_name("audio-volume-low-symbolic");
                }
                vol_btn_change.set_tooltip_text(Some("Mute"));
            }
        });

        // --- E. Mini-Player Mode Button (at the very right, right of volume slider) ---
        let btn_mini = gtk::Button::builder()
            .icon_name("view-restore-symbolic")
            .css_classes(["btn-transport"])
            .tooltip_text("Toggle Mini-Player Mode (Ctrl+M)")
            .valign(gtk::Align::Center)
            .build();

        // Right side strictly: [... (more)] [queue] [mute] [volume slider] [minimize]
        right_box.append(&btn_more);
        right_box.append(&btn_queue);
        right_box.append(&vol_btn);
        right_box.append(&volume_scale);
        right_box.append(&btn_mini);

        // Assemble row
        controls_row.append(&info_box);
        controls_row.append(&center_box);
        controls_row.append(&right_box);

        container.append(&wave_row);
        container.append(&controls_row);

        let current_track: Rc<RefCell<Option<Track>>> = Rc::new(RefCell::new(None));
        let is_playing = Rc::new(RefCell::new(false));
        let is_liked = Rc::new(RefCell::new(false));

        // Connect share button in more menu to clipboard
        let track_share_clone = Rc::clone(&current_track);
        let more_pop_share = more_popover.clone();
        share_btn.connect_clicked(move |_btn| {
            more_pop_share.popdown();
            if let Some(ref track) = *track_share_clone.borrow() {
                if let Some(ref url) = track.permalink_url.as_deref() {
                    if let Some(display) = gdk4::Display::default() {
                        display.clipboard().set_text(url);
                        info!("Copied track link to clipboard: {}", url);
                    }
                }
            }
        });

        let is_stashed = Rc::new(RefCell::new(false));

        let on_queue_jump: Rc<RefCell<Option<Box<dyn Fn(usize)>>>> = Rc::new(RefCell::new(None));
        let on_queue_remove: Rc<RefCell<Option<Box<dyn Fn(usize)>>>> = Rc::new(RefCell::new(None));
        let on_queue_clear: Rc<RefCell<Option<Box<dyn Fn()>>>> = Rc::new(RefCell::new(None));
        let on_toggle_mini: Rc<RefCell<Option<Box<dyn Fn()>>>> = Rc::new(RefCell::new(None));
        let on_speed_change: Rc<RefCell<Option<Box<dyn Fn(f64)>>>> = Rc::new(RefCell::new(None));
        let on_add_bookmark: Rc<RefCell<Option<Box<dyn Fn()>>>> = Rc::new(RefCell::new(None));
        let on_stash_toggle: Rc<RefCell<Option<Box<dyn Fn()>>>> = Rc::new(RefCell::new(None));

        let clear_cb_c = Rc::clone(&on_queue_clear);
        q_clear_btn.connect_clicked(move |_| {
            if let Some(ref cb) = *clear_cb_c.borrow() {
                cb();
            }
        });

        let mini_cb_c = Rc::clone(&on_toggle_mini);
        btn_mini.connect_clicked(move |_| {
            if let Some(ref cb) = *mini_cb_c.borrow() {
                cb();
            }
        });

        let speed_val_clone = speed_val_lbl.clone();
        let speed_cb_c = Rc::clone(&on_speed_change);
        speed_slider.connect_value_changed(move |scale| {
            let val = scale.value();
            let val = if (val - 1.0).abs() < 0.03 { 1.0 } else { val };
            speed_val_clone.set_label(&format!("{:.2}x", val));
            if let Some(ref cb) = *speed_cb_c.borrow() {
                cb(val);
            }
        });

        let bm_cb_c = Rc::clone(&on_add_bookmark);
        let more_pop_bm = more_popover.clone();
        btn_more_bookmark.connect_clicked(move |_| {
            more_pop_bm.popdown();
            if let Some(ref cb) = *bm_cb_c.borrow() {
                cb();
            }
        });

        let stash_cb_c = Rc::clone(&on_stash_toggle);
        let more_pop_st = more_popover.clone();
        btn_more_stash.connect_clicked(move |_| {
            more_pop_st.popdown();
            if let Some(ref cb) = *stash_cb_c.borrow() {
                cb();
            }
        });

        Self {
            container,
            artwork,
            title_label,
            artist_label,
            like_btn,
            btn_play_pause,
            btn_prev,
            btn_next,
            btn_shuffle,
            btn_repeat,
            btn_more,
            more_stash_icon,
            more_stash_label,
            btn_more_stash,
            speed_slider,
            speed_val_lbl,
            sleep_status_lbl,
            bookmark_popover,
            bm_prompt_entry,
            bm_prompt_title,
            bm_current_pos_ms,
            wave_row,
            waveform,
            time_current,
            time_total,
            volume_scale,
            vol_btn,
            btn_queue,
            btn_mini,
            q_title_lbl,
            q_items_flow,
            current_track,
            is_playing,
            is_liked,
            is_stashed,
            saved_volume,
            is_muted,
            on_queue_jump,
            on_queue_remove,
            on_queue_clear,
            on_toggle_mini,
            on_speed_change,
            on_sleep_timer,
            on_add_bookmark,
            on_stash_toggle,
            on_save_bookmark,
        }
    }

    pub fn widget(&self) -> &gtk::Box {
        &self.container
    }

    pub fn set_track(&self, track: &Track, samples: Vec<f32>) {
        *self.current_track.borrow_mut() = Some(track.clone());
        self.title_label.set_label(&track.title);
        self.title_label.set_tooltip_text(Some(&track.title));
        let artist_str = track.artist_name();
        self.artist_label.set_label(artist_str);
        self.artist_label.set_tooltip_text(Some(artist_str));
        self.time_total.set_label(&track.duration_formatted());
        self.time_current.set_label("0:00");

        let art_url = track.best_artwork_url().or_else(|| track.thumbnail_url());
        crate::ui::image_loader::load_into_image(
            &self.artwork,
            art_url.as_deref(),
            "audio-x-generic-symbolic",
        );

        self.waveform.set_samples(samples);
        self.waveform.set_progress(0.0);
        self.waveform.set_duration_ms(track.duration);

        let liked = track.user_favorite.unwrap_or(false);
        *self.is_liked.borrow_mut() = liked;
        self.update_like_ui(liked);
    }

    pub fn update_playback_state(&self, playing: bool) {
        *self.is_playing.borrow_mut() = playing;
        if playing {
            self.btn_play_pause
                .set_icon_name("media-playback-pause-symbolic");
            self.btn_play_pause.set_tooltip_text(Some("Pause"));
        } else {
            self.btn_play_pause
                .set_icon_name("media-playback-start-symbolic");
            self.btn_play_pause.set_tooltip_text(Some("Play"));
        }
    }

    pub fn update_position(&self, position_ms: u64, duration_ms: u64) {
        self.time_current.set_label(&format_milliseconds(position_ms));
        if duration_ms > 0 {
            let pct = (position_ms as f64 / duration_ms as f64).clamp(0.0, 1.0);
            self.waveform.set_progress(pct);
        }
    }

    pub fn update_like_ui(&self, liked: bool) {
        if liked {
            self.like_btn.add_css_class("btn-heart-liked");
            self.like_btn.set_icon_name("starred-symbolic");
            self.like_btn.set_tooltip_text(Some("Unlike track"));
        } else {
            self.like_btn.remove_css_class("btn-heart-liked");
            self.like_btn.set_icon_name("non-starred-symbolic");
            self.like_btn.set_tooltip_text(Some("Like track"));
        }
    }

    pub fn prompt_add_bookmark(&self, position_ms: u64, duration_ms: u64) {
        self.bm_current_pos_ms.set(position_ms);
        self.bm_prompt_title.set_label(&format!("Add Bookmark at {}", format_milliseconds(position_ms)));
        self.bm_prompt_entry.set_text("");

        let pct = if duration_ms > 0 {
            (position_ms as f64 / duration_ms as f64).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let wave_w = self.waveform.widget().width() as f64;
        let local_x = if wave_w > 20.0 {
            (pct * wave_w).clamp(10.0, wave_w - 10.0)
        } else {
            50.0
        };
        let target_x = self
            .waveform
            .widget()
            .compute_point(&self.wave_row, &gtk::graphene::Point::new(local_x as f32, 0.0))
            .map(|pt| pt.x() as i32)
            .unwrap_or(local_x as i32);

        let rect = gdk4::Rectangle::new(target_x, 0, 1, 1);
        self.bookmark_popover.set_pointing_to(Some(&rect));

        let pop = self.bookmark_popover.clone();
        let entry = self.bm_prompt_entry.clone();
        glib::idle_add_local_once(move || {
            pop.popup();
            entry.grab_focus();
        });
    }

    pub fn connect_save_bookmark<F: Fn(u64, String) + 'static>(&self, f: F) {
        *self.on_save_bookmark.borrow_mut() = Some(Box::new(f));
    }

    pub fn connect_play_pause<F: Fn() + 'static>(&self, f: F) {
        self.btn_play_pause.connect_clicked(move |_| f());
    }

    pub fn connect_prev<F: Fn() + 'static>(&self, f: F) {
        self.btn_prev.connect_clicked(move |_| f());
    }

    pub fn connect_next<F: Fn() + 'static>(&self, f: F) {
        self.btn_next.connect_clicked(move |_| f());
    }

    pub fn set_repeat_mode(&self, mode: crate::audio::types::RepeatMode) {
        match mode {
            crate::audio::types::RepeatMode::Off => {
                self.btn_repeat.remove_css_class("active");
                self.btn_repeat.set_icon_name("media-playlist-repeat-symbolic");
                self.btn_repeat.set_tooltip_text(Some("Repeat: Off"));
            }
            crate::audio::types::RepeatMode::All => {
                self.btn_repeat.add_css_class("active");
                self.btn_repeat.set_icon_name("media-playlist-repeat-symbolic");
                self.btn_repeat.set_tooltip_text(Some("Repeat: All"));
            }
            crate::audio::types::RepeatMode::One => {
                self.btn_repeat.add_css_class("active");
                self.btn_repeat.set_icon_name("media-playlist-repeat-song-symbolic");
                self.btn_repeat.set_tooltip_text(Some("Repeat: One"));
            }
        }
    }

    pub fn set_shuffle(&self, shuffle: bool) {
        if shuffle {
            self.btn_shuffle.add_css_class("active");
            self.btn_shuffle.set_tooltip_text(Some("Shuffle: On"));
        } else {
            self.btn_shuffle.remove_css_class("active");
            self.btn_shuffle.set_tooltip_text(Some("Shuffle: Off"));
        }
    }

    pub fn connect_shuffle<F: Fn() -> bool + 'static>(&self, f: F) {
        let btn = self.btn_shuffle.clone();
        self.btn_shuffle.connect_clicked(move |_| {
            let active = f();
            if active {
                btn.add_css_class("active");
                btn.set_tooltip_text(Some("Shuffle: On"));
            } else {
                btn.remove_css_class("active");
                btn.set_tooltip_text(Some("Shuffle: Off"));
            }
        });
    }

    pub fn connect_repeat<F: Fn() -> crate::audio::types::RepeatMode + 'static>(&self, f: F) {
        let btn = self.btn_repeat.clone();
        self.btn_repeat.connect_clicked(move |_| {
            let mode = f();
            match mode {
                crate::audio::types::RepeatMode::Off => {
                    btn.remove_css_class("active");
                    btn.set_icon_name("media-playlist-repeat-symbolic");
                    btn.set_tooltip_text(Some("Repeat: Off"));
                }
                crate::audio::types::RepeatMode::All => {
                    btn.add_css_class("active");
                    btn.set_icon_name("media-playlist-repeat-symbolic");
                    btn.set_tooltip_text(Some("Repeat: All"));
                }
                crate::audio::types::RepeatMode::One => {
                    btn.add_css_class("active");
                    btn.set_icon_name("media-playlist-repeat-song-symbolic");
                    btn.set_tooltip_text(Some("Repeat: One"));
                }
            }
        });
    }

    pub fn connect_seek<F: Fn(f64) + 'static>(&self, f: F) {
        self.waveform.connect_seek(f);
    }

    pub fn connect_volume<F: Fn(f64) + 'static>(&self, f: F) {
        self.volume_scale.connect_value_changed(move |scale| {
            f(scale.value());
        });
    }

    pub fn connect_like<F: Fn(bool) + 'static>(&self, f: F) {
        let is_liked = Rc::clone(&self.is_liked);
        let like_btn = self.like_btn.clone();
        self.like_btn.connect_clicked(move |_| {
            let next = !*is_liked.borrow();
            *is_liked.borrow_mut() = next;
            if next {
                like_btn.add_css_class("btn-heart-liked");
                like_btn.set_icon_name("starred-symbolic");
            } else {
                like_btn.remove_css_class("btn-heart-liked");
                like_btn.set_icon_name("non-starred-symbolic");
            }
            f(next);
        });
    }

    pub fn waveform(&self) -> &WaveformWidget {
        &self.waveform
    }

    pub fn update_queue(&self, current: Option<&Track>, queue: &[Track]) {
        self.q_title_lbl.set_label(&format!("Up Next ({})", queue.len()));

        while let Some(child) = self.q_items_flow.first_child() {
            self.q_items_flow.remove(&child);
        }

        // 1. Now Playing item (if any)
        if let Some(curr) = current {
            let now_playing_hdr = gtk::Label::builder()
                .label("NOW PLAYING")
                .css_classes(["time-label"])
                .halign(gtk::Align::Start)
                .margin_top(4)
                .build();
            self.q_items_flow.append(&now_playing_hdr);

            let row = gtk::Box::builder()
                .orientation(gtk::Orientation::Horizontal)
                .spacing(10)
                .css_classes(["track-card"])
                .build();

            let art = gtk::Image::builder()
                .pixel_size(36)
                .icon_name("audio-x-generic-symbolic")
                .css_classes(["track-card-artwork"])
                .build();
            crate::ui::image_loader::load_into_image(&art, curr.thumbnail_url().as_deref(), "audio-x-generic-symbolic");

            let meta = gtk::Box::builder().orientation(gtk::Orientation::Vertical).spacing(2).hexpand(true).build();
            let t_lbl = gtk::Label::builder()
                .label(&curr.title)
                .css_classes(["track-card-title"])
                .xalign(0.0)
                .ellipsize(gtk::pango::EllipsizeMode::End)
                .tooltip_text(&curr.title)
                .build();
            let a_lbl = gtk::Label::builder().label(curr.artist_name()).css_classes(["track-card-artist"]).xalign(0.0).build();
            meta.append(&t_lbl);
            meta.append(&a_lbl);

            let d_lbl = gtk::Label::builder().label(&curr.duration_formatted()).css_classes(["time-label"]).valign(gtk::Align::Center).build();

            row.append(&art);
            row.append(&meta);
            row.append(&d_lbl);
            self.q_items_flow.append(&row);
        }

        // 2. Up Next items
        if queue.is_empty() {
            let empty_lbl = gtk::Label::builder()
                .label("Queue is empty. Right-click any track across the app to add it to Up Next.")
                .css_classes(["player-artist"])
                .margin_top(20)
                .wrap(true)
                .build();
            self.q_items_flow.append(&empty_lbl);
            return;
        }

        let up_next_hdr = gtk::Label::builder()
            .label("NEXT IN QUEUE")
            .css_classes(["time-label"])
            .halign(gtk::Align::Start)
            .margin_top(8)
            .build();
        self.q_items_flow.append(&up_next_hdr);

        for (idx, trk) in queue.iter().enumerate() {
            let row = gtk::Box::builder()
                .orientation(gtk::Orientation::Horizontal)
                .spacing(10)
                .css_classes(["track-card"])
                .build();

            let art = gtk::Image::builder()
                .pixel_size(36)
                .icon_name("audio-x-generic-symbolic")
                .css_classes(["track-card-artwork"])
                .build();
            crate::ui::image_loader::load_into_image(&art, trk.thumbnail_url().as_deref(), "audio-x-generic-symbolic");

            let meta = gtk::Box::builder().orientation(gtk::Orientation::Vertical).spacing(2).hexpand(true).build();
            let t_lbl = gtk::Label::builder()
                .label(&trk.title)
                .css_classes(["track-card-title"])
                .xalign(0.0)
                .ellipsize(gtk::pango::EllipsizeMode::End)
                .tooltip_text(&trk.title)
                .build();
            let a_lbl = gtk::Label::builder().label(trk.artist_name()).css_classes(["track-card-artist"]).xalign(0.0).build();
            meta.append(&t_lbl);
            meta.append(&a_lbl);

            let d_lbl = gtk::Label::builder().label(&trk.duration_formatted()).css_classes(["time-label"]).valign(gtk::Align::Center).build();

            let remove_btn = gtk::Button::builder()
                .icon_name("window-close-symbolic")
                .css_classes(["btn-card-like"])
                .valign(gtk::Align::Center)
                .tooltip_text("Remove from Queue")
                .build();

            let rem_cb = Rc::clone(&self.on_queue_remove);
            remove_btn.connect_clicked(move |_| {
                if let Some(ref cb) = *rem_cb.borrow() {
                    cb(idx);
                }
            });

            let jump_g = gtk::GestureClick::new();
            let jump_cb = Rc::clone(&self.on_queue_jump);
            jump_g.connect_pressed(move |_g, _n, _x, _y| {
                if let Some(ref cb) = *jump_cb.borrow() {
                    cb(idx);
                }
            });
            meta.add_controller(jump_g);
            meta.set_tooltip_text(Some("Click to jump to this track"));

            row.append(&art);
            row.append(&meta);
            row.append(&d_lbl);
            row.append(&remove_btn);

            self.q_items_flow.append(&row);
        }
    }

    pub fn connect_queue_jump<F: Fn(usize) + 'static>(&self, f: F) {
        *self.on_queue_jump.borrow_mut() = Some(Box::new(f));
    }

    pub fn connect_queue_remove<F: Fn(usize) + 'static>(&self, f: F) {
        *self.on_queue_remove.borrow_mut() = Some(Box::new(f));
    }

    pub fn connect_queue_clear<F: Fn() + 'static>(&self, f: F) {
        *self.on_queue_clear.borrow_mut() = Some(Box::new(f));
    }

    pub fn connect_toggle_mini<F: Fn() + 'static>(&self, f: F) {
        *self.on_toggle_mini.borrow_mut() = Some(Box::new(f));
    }

    pub fn set_stashed(&self, stashed: bool) {
        *self.is_stashed.borrow_mut() = stashed;
        if stashed {
            self.more_stash_icon.set_icon_name(Some("emblem-ok-symbolic"));
            self.more_stash_label.set_label("Remove from Offline Stash");
            self.btn_more_stash.add_css_class("btn-heart-liked");
        } else {
            self.more_stash_icon.set_icon_name(Some("folder-download-symbolic"));
            self.more_stash_label.set_label("Download Track Offline");
            self.btn_more_stash.remove_css_class("btn-heart-liked");
        }
    }

    pub fn set_speed(&self, speed: f64) {
        let label = if (speed - 1.0).abs() < 0.01 {
            "1.00x".to_string()
        } else {
            format!("{:.2}x", speed)
        };
        self.speed_val_lbl.set_label(&label);
        self.speed_slider.set_value(speed);
    }

    pub fn update_sleep_timer_status(&self, remaining_sec_opt: Option<u64>, is_end_of_track: bool) {
        if is_end_of_track {
            self.sleep_status_lbl.set_label("End of track");
        } else if let Some(rem) = remaining_sec_opt {
            let m = rem / 60;
            let s = rem % 60;
            self.sleep_status_lbl.set_label(&format!("{:02}:{:02}", m, s));
        } else {
            self.sleep_status_lbl.set_label("Off");
        }
    }

    pub fn connect_speed<F: Fn(f64) + 'static>(&self, f: F) {
        *self.on_speed_change.borrow_mut() = Some(Box::new(f));
    }

    pub fn connect_sleep_timer<F: Fn(Option<u32>, bool) + 'static>(&self, f: F) {
        *self.on_sleep_timer.borrow_mut() = Some(Box::new(f));
    }

    pub fn connect_add_bookmark<F: Fn() + 'static>(&self, f: F) {
        *self.on_add_bookmark.borrow_mut() = Some(Box::new(f));
    }

    pub fn connect_stash<F: Fn() + 'static>(&self, f: F) {
        *self.on_stash_toggle.borrow_mut() = Some(Box::new(f));
    }
}
