use gtk4 as gtk;
use gtk::prelude::*;
use std::cell::RefCell;
use std::rc::Rc;

use crate::models::user::UserProfile;
use crate::ui::views::stream::format_count;

pub struct SettingsView {
    container: gtk::Box,
    // Account section widgets
    account_stack: gtk::Stack,
    guest_box: gtk::Box,
    connected_box: gtk::Box,
    user_avatar: gtk::Image,
    user_name_lbl: gtk::Label,
    user_handle_lbl: gtk::Label,
    user_meta_lbl: gtk::Label,
    oauth_entry: gtk::PasswordEntry,
    btn_open_browser: gtk::Button,
    btn_auto_detect: gtk::Button,
    btn_connect: gtk::Button,
    btn_sync: gtk::Button,
    btn_sign_out: gtk::Button,
    // Profile customization widgets
    avatar_edit_img: gtk::Image,
    btn_choose_avatar: gtk::Button,
    selected_avatar_path: Rc<RefCell<Option<String>>>,
    edit_username_entry: gtk::Entry,
    edit_city_entry: gtk::Entry,
    edit_country_entry: gtk::Entry,
    edit_bio_view: gtk::TextView,
    btn_save_profile: gtk::Button,
    // Client ID section widgets
    client_id_entry: gtk::Entry,
    btn_save_cid: gtk::Button,
    btn_discover_cid: gtk::Button,
    // Cache section
    btn_clear_cache: gtk::Button,
    // Playback & Desktop Preferences
    switch_normalization: gtk::Switch,
    switch_notifications: gtk::Switch,
    // Theme selection buttons
    btn_theme_anthropic: gtk::Button,
    btn_theme_obsidian: gtk::Button,
    // Status
    status_label: gtk::Label,
    spinner: gtk::Spinner,
    // Callbacks
    on_theme_changed: Rc<RefCell<Option<Box<dyn Fn(crate::ui::theme::AppTheme)>>>>,
    on_connect_token: Rc<RefCell<Option<Box<dyn Fn(String)>>>>,
    on_auto_detect_cookie: Rc<RefCell<Option<Box<dyn Fn()>>>>,
    on_sign_out: Rc<RefCell<Option<Box<dyn Fn()>>>>,
    on_sync_library: Rc<RefCell<Option<Box<dyn Fn()>>>>,
    on_save_cid: Rc<RefCell<Option<Box<dyn Fn(String)>>>>,
    on_discover_cid: Rc<RefCell<Option<Box<dyn Fn()>>>>,
    on_clear_cache: Rc<RefCell<Option<Box<dyn Fn()>>>>,
    on_toggle_normalization: Rc<RefCell<Option<Box<dyn Fn(bool)>>>>,
    on_toggle_notifications: Rc<RefCell<Option<Box<dyn Fn(bool)>>>>,
    on_update_profile: Rc<RefCell<Option<Box<dyn Fn(String, Option<String>, Option<String>, Option<String>, Option<String>)>>>>,
}

impl SettingsView {
    pub fn new() -> Self {
        let container = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(18)
            .margin_top(18)
            .margin_bottom(18)
            .margin_start(24)
            .margin_end(8)
            .hexpand(true)
            .vexpand(true)
            .build();

        let header_row = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(14)
            .margin_end(16)
            .build();

        let title = gtk::Label::builder()
            .label("Account & Preferences")
            .css_classes(["brand-label"])
            .halign(gtk::Align::Start)
            .build();

        let spinner = gtk::Spinner::builder()
            .spinning(false)
            .valign(gtk::Align::Center)
            .build();

        header_row.append(&title);
        header_row.append(&spinner);
        container.append(&header_row);

        let scrolled = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Never)
            .vscrollbar_policy(gtk::PolicyType::Automatic)
            .hexpand(true)
            .vexpand(true)
            .build();

        let form_box = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(18)
            .margin_end(16)
            .hexpand(true)
            .build();

        // 1. ACCOUNT SECTION (Stack: Guest View vs Connected Profile)
        let account_stack = gtk::Stack::builder()
            .transition_type(gtk::StackTransitionType::Crossfade)
            .transition_duration(150)
            .build();

        // 1A. GUEST / LOGIN CARD
        let guest_box = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(14)
            .css_classes(["account-card"])
            .build();

        let guest_header = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(12)
            .build();

        let guest_icon = gtk::Image::builder()
            .icon_name("avatar-default-symbolic")
            .pixel_size(44)
            .css_classes(["track-card-artwork"])
            .build();

        let guest_meta = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(2)
            .valign(gtk::Align::Center)
            .build();

        let guest_title = gtk::Label::builder()
            .label("SoundCloud Account Session")
            .css_classes(["player-title"])
            .halign(gtk::Align::Start)
            .build();

        let guest_badge = gtk::Label::builder()
            .label("● Currently Guest Mode (Public Streaming Active)")
            .css_classes(["badge-guest"])
            .halign(gtk::Align::Start)
            .build();

        guest_meta.append(&guest_title);
        guest_meta.append(&guest_badge);
        guest_header.append(&guest_icon);
        guest_header.append(&guest_meta);
        guest_box.append(&guest_header);

        // Step-by-step instructions box
        let guide_box = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(8)
            .css_classes(["login-guide-box"])
            .build();

        let guide_title = gtk::Label::builder()
            .label("How to Connect Your SoundCloud Account:")
            .css_classes(["player-title"])
            .halign(gtk::Align::Start)
            .build();

        let guide_step1 = gtk::Label::builder()
            .label("1. Log in to soundcloud.com in your web browser (Zen, Firefox, or Chromium).")
            .css_classes(["player-artist"])
            .halign(gtk::Align::Start)
            .build();

        let guide_step2 = gtk::Label::builder()
            .label("2. Open Developer Tools (F12) → Storage (or Application tab) → Cookies → https://soundcloud.com.")
            .css_classes(["player-artist"])
            .halign(gtk::Align::Start)
            .build();

        let guide_step3 = gtk::Label::builder()
            .label("3. Copy the value of the 'oauth_token' cookie (e.g. 2-293444-10294829-xX...) and paste below.")
            .css_classes(["player-artist"])
            .halign(gtk::Align::Start)
            .build();

        let guide_step4 = gtk::Label::builder()
            .label("Or simply click 'Auto-Detect from Browser' to find an active session automatically!")
            .css_classes(["player-artist"])
            .halign(gtk::Align::Start)
            .build();

        guide_box.append(&guide_title);
        guide_box.append(&guide_step1);
        guide_box.append(&guide_step2);
        guide_box.append(&guide_step3);
        guide_box.append(&guide_step4);
        guest_box.append(&guide_box);

        // Action buttons row (Open browser & Auto-Detect)
        let helper_actions_row = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(10)
            .build();

        let btn_open_browser = gtk::Button::builder()
            .label("Open soundcloud.com/signin")
            .css_classes(["filter-chip"])
            .tooltip_text("Opens SoundCloud login page in your default browser")
            .build();

        let btn_auto_detect = gtk::Button::builder()
            .label("Auto-Detect from Browser Cookies")
            .css_classes(["filter-chip"])
            .tooltip_text("Scans local browser profiles for an active soundcloud.com oauth_token")
            .build();

        helper_actions_row.append(&btn_open_browser);
        helper_actions_row.append(&btn_auto_detect);
        guest_box.append(&helper_actions_row);

        // Entry and connect button
        let oauth_entry = gtk::PasswordEntry::builder()
            .placeholder_text("Paste your SoundCloud oauth_token cookie here...")
            .css_classes(["sc-search-entry"])
            .show_peek_icon(true)
            .build();

        let btn_connect = gtk::Button::builder()
            .label("Verify & Connect Account")
            .css_classes(["filter-chip", "active"])
            .halign(gtk::Align::Start)
            .tooltip_text("Verifies the token against SoundCloud /me and saves securely to system keyring")
            .build();

        guest_box.append(&oauth_entry);
        guest_box.append(&btn_connect);

        // 1B. CONNECTED PROFILE CARD
        let connected_box = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(16)
            .css_classes(["account-card"])
            .build();

        let user_header_row = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(16)
            .build();

        let user_avatar = gtk::Image::builder()
            .pixel_size(64)
            .icon_name("avatar-default-symbolic")
            .css_classes(["track-card-artwork"])
            .valign(gtk::Align::Center)
            .build();

        let user_text_vbox = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(3)
            .valign(gtk::Align::Center)
            .hexpand(true)
            .build();

        let user_name_lbl = gtk::Label::builder()
            .label("SoundCloud User")
            .css_classes(["brand-label"])
            .halign(gtk::Align::Start)
            .build();

        let user_handle_lbl = gtk::Label::builder()
            .label("@user • Connected")
            .css_classes(["badge-connected"])
            .halign(gtk::Align::Start)
            .build();

        let user_meta_lbl = gtk::Label::builder()
            .label("0 followers • 0 liked tracks")
            .css_classes(["player-artist"])
            .halign(gtk::Align::Start)
            .build();

        user_text_vbox.append(&user_name_lbl);
        user_text_vbox.append(&user_handle_lbl);
        user_text_vbox.append(&user_meta_lbl);

        user_header_row.append(&user_avatar);
        user_header_row.append(&user_text_vbox);
        connected_box.append(&user_header_row);

        let connected_actions_row = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(12)
            .build();

        let btn_sync = gtk::Button::builder()
            .label("Sync Personal Likes & Playlists")
            .css_classes(["filter-chip", "active"])
            .tooltip_text("Pulls your personal liked tracks and playlists from SoundCloud")
            .build();

        let btn_sign_out = gtk::Button::builder()
            .label("Sign Out / Disconnect")
            .css_classes(["filter-chip"])
            .tooltip_text("Clears saved session credentials from the system keyring")
            .build();

        connected_actions_row.append(&btn_sync);
        connected_actions_row.append(&btn_sign_out);
        connected_box.append(&connected_actions_row);

        // Profile customization section inside connected_box
        let edit_profile_title = gtk::Label::builder()
            .label("Customize SoundCloud Profile")
            .css_classes(["track-card-title"])
            .halign(gtk::Align::Start)
            .margin_top(12)
            .build();
        connected_box.append(&edit_profile_title);

        let avatar_edit_row = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(12)
            .build();

        let avatar_edit_img = gtk::Image::builder()
            .pixel_size(48)
            .icon_name("avatar-default-symbolic")
            .css_classes(["track-card-artwork"])
            .build();

        let btn_choose_avatar = gtk::Button::builder()
            .label("Change Avatar Image")
            .css_classes(["filter-chip"])
            .valign(gtk::Align::Center)
            .build();

        let selected_avatar_path = Rc::new(RefCell::new(None::<String>));

        avatar_edit_row.append(&avatar_edit_img);
        avatar_edit_row.append(&btn_choose_avatar);
        connected_box.append(&avatar_edit_row);

        let av_path_clone = Rc::clone(&selected_avatar_path);
        let av_img_clone = avatar_edit_img.clone();
        let cont_c = container.clone();

        btn_choose_avatar.connect_clicked(move |_| {
            let win = cont_c.root().and_downcast::<gtk::Window>();
            let dialog = gtk::FileDialog::builder()
                .title("Select New Avatar Image")
                .modal(true)
                .build();
            let av_p = Rc::clone(&av_path_clone);
            let img = av_img_clone.clone();

            dialog.open(win.as_ref(), gtk::gio::Cancellable::NONE, move |res| {
                if let Ok(file) = res {
                    if let Some(path) = file.path() {
                        let path_str = path.to_string_lossy().to_string();
                        img.set_from_file(Some(&path));
                        *av_p.borrow_mut() = Some(path_str);
                    }
                }
            });
        });

        let edit_username_entry = gtk::Entry::builder()
            .placeholder_text("Display Name / Username")
            .css_classes(["sc-search-entry"])
            .build();

        let location_row = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(10)
            .build();

        let edit_city_entry = gtk::Entry::builder()
            .placeholder_text("City (e.g. Cairo)")
            .css_classes(["sc-search-entry"])
            .hexpand(true)
            .build();

        let edit_country_entry = gtk::Entry::builder()
            .placeholder_text("Country Code (e.g. EG)")
            .css_classes(["sc-search-entry"])
            .width_request(140)
            .build();

        location_row.append(&edit_city_entry);
        location_row.append(&edit_country_entry);

        let bio_label = gtk::Label::builder()
            .label("Bio / Description:")
            .css_classes(["player-artist"])
            .halign(gtk::Align::Start)
            .build();

        let edit_bio_view = gtk::TextView::builder()
            .css_classes(["sc-search-entry"])
            .height_request(70)
            .wrap_mode(gtk::WrapMode::Word)
            .build();

        let btn_save_profile = gtk::Button::builder()
            .label("Save Profile Changes")
            .css_classes(["btn-pill-play"])
            .halign(gtk::Align::Start)
            .build();

        connected_box.append(&edit_username_entry);
        connected_box.append(&location_row);
        connected_box.append(&bio_label);
        connected_box.append(&edit_bio_view);
        connected_box.append(&btn_save_profile);

        account_stack.add_named(&guest_box, Some("guest"));
        account_stack.add_named(&connected_box, Some("connected"));
        account_stack.set_visible_child_name("guest");
        form_box.append(&account_stack);

        // 2. CLIENT-ID & API SETTINGS CARD
        let cid_group = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(10)
            .css_classes(["account-card"])
            .build();

        let cid_title = gtk::Label::builder()
            .label("SoundCloud Public Client ID")
            .css_classes(["player-title"])
            .halign(gtk::Align::Start)
            .build();

        let cid_desc = gtk::Label::builder()
            .label("Used by the native engine to query public charts, tracks, and streams. Auto-discovered dynamically.")
            .css_classes(["player-artist"])
            .halign(gtk::Align::Start)
            .build();

        let client_id_entry = gtk::Entry::builder()
            .placeholder_text("Active Client ID")
            .css_classes(["sc-search-entry"])
            .build();

        let cid_btn_row = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(10)
            .build();

        let btn_save_cid = gtk::Button::builder()
            .label("Save Client ID")
            .css_classes(["filter-chip"])
            .build();

        let btn_discover_cid = gtk::Button::builder()
            .label("Auto-Discover Client ID from soundcloud.com")
            .css_classes(["filter-chip"])
            .build();

        cid_btn_row.append(&btn_save_cid);
        cid_btn_row.append(&btn_discover_cid);

        cid_group.append(&cid_title);
        cid_group.append(&cid_desc);
        cid_group.append(&client_id_entry);
        cid_group.append(&cid_btn_row);
        form_box.append(&cid_group);

        // 3. STORAGE & CACHE CARD
        let cache_group = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(10)
            .css_classes(["account-card"])
            .build();

        let cache_title = gtk::Label::builder()
            .label("Local Database & Artwork Cache")
            .css_classes(["player-title"])
            .halign(gtk::Align::Start)
            .build();

        let cache_desc = gtk::Label::builder()
            .label("High-speed SQLite WAL cache stores track metadata and waveforms. Downloaded artwork is cached locally on disk.")
            .css_classes(["player-artist"])
            .halign(gtk::Align::Start)
            .build();

        let btn_clear_cache = gtk::Button::builder()
            .label("Clear Artwork & Metadata Cache")
            .css_classes(["filter-chip"])
            .halign(gtk::Align::Start)
            .build();

        cache_group.append(&cache_title);
        cache_group.append(&cache_desc);
        cache_group.append(&btn_clear_cache);
        form_box.append(&cache_group);

        // 3B. AESTHETIC DESIGN & THEME CARD (Anthropic Classical vs Obsidian Dark)
        let theme_group = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(12)
            .css_classes(["account-card"])
            .build();

        let theme_title = gtk::Label::builder()
            .label("Aesthetic Design & Theme")
            .css_classes(["player-title"])
            .halign(gtk::Align::Start)
            .build();

        let theme_desc = gtk::Label::builder()
            .label("Choose between the Anthropic Hellenic editorial look (terracotta accents, classical serifs, warm alabaster) or the legacy monochrome dark skin.")
            .css_classes(["player-artist"])
            .halign(gtk::Align::Start)
            .wrap(true)
            .build();

        let theme_btn_box = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(10)
            .build();

        let btn_theme_anthropic = gtk::Button::builder()
            .label("Anthropic Classical (Hellenic Editorial)")
            .css_classes(["filter-chip", "active"])
            .build();

        let btn_theme_obsidian = gtk::Button::builder()
            .label("Obsidian Dark (Legacy Monochrome)")
            .css_classes(["filter-chip"])
            .build();

        theme_btn_box.append(&btn_theme_anthropic);
        theme_btn_box.append(&btn_theme_obsidian);

        theme_group.append(&theme_title);
        theme_group.append(&theme_desc);
        theme_group.append(&theme_btn_box);
        form_box.append(&theme_group);

        let on_theme_changed: Rc<RefCell<Option<Box<dyn Fn(crate::ui::theme::AppTheme)>>>> = Rc::new(RefCell::new(None));

        {
            let on_c = Rc::clone(&on_theme_changed);
            let b_ant = btn_theme_anthropic.clone();
            let b_obs = btn_theme_obsidian.clone();
            btn_theme_anthropic.connect_clicked(move |_| {
                b_ant.add_css_class("active");
                b_obs.remove_css_class("active");
                if let Some(ref cb) = *on_c.borrow() {
                    cb(crate::ui::theme::AppTheme::AnthropicClassical);
                }
            });
        }

        {
            let on_c = Rc::clone(&on_theme_changed);
            let b_ant = btn_theme_anthropic.clone();
            let b_obs = btn_theme_obsidian.clone();
            btn_theme_obsidian.connect_clicked(move |_| {
                b_obs.add_css_class("active");
                b_ant.remove_css_class("active");
                if let Some(ref cb) = *on_c.borrow() {
                    cb(crate::ui::theme::AppTheme::ObsidianDark);
                }
            });
        }

        // 4. AUDIO & DESKTOP PREFERENCES CARD
        let prefs_group = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(14)
            .css_classes(["account-card"])
            .build();

        let prefs_title = gtk::Label::builder()
            .label("Audio & System Preferences")
            .css_classes(["player-title"])
            .halign(gtk::Align::Start)
            .build();

        // Row 1: Loudness Normalization
        let norm_row = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(12)
            .build();

        let norm_text_box = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(2)
            .hexpand(true)
            .build();

        let norm_lbl = gtk::Label::builder()
            .label("Loudness Normalization (dynaudnorm)")
            .css_classes(["track-card-title"])
            .halign(gtk::Align::Start)
            .build();

        let norm_sub = gtk::Label::builder()
            .label("Real-time EBU R128 dynamic normalization to eliminate jarring volume discrepancies between tracks.")
            .css_classes(["player-artist"])
            .halign(gtk::Align::Start)
            .wrap(true)
            .build();

        norm_text_box.append(&norm_lbl);
        norm_text_box.append(&norm_sub);

        let switch_normalization = gtk::Switch::builder()
            .valign(gtk::Align::Center)
            .build();

        norm_row.append(&norm_text_box);
        norm_row.append(&switch_normalization);

        // Row 2: Desktop Notifications
        let notif_row = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(12)
            .build();

        let notif_text_box = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(2)
            .hexpand(true)
            .build();

        let notif_lbl = gtk::Label::builder()
            .label("Desktop Notifications")
            .css_classes(["track-card-title"])
            .halign(gtk::Align::Start)
            .build();

        let notif_sub = gtk::Label::builder()
            .label("Display native FreeDesktop notifications when tracks change while running in the background.")
            .css_classes(["player-artist"])
            .halign(gtk::Align::Start)
            .wrap(true)
            .build();

        notif_text_box.append(&notif_lbl);
        notif_text_box.append(&notif_sub);

        let switch_notifications = gtk::Switch::builder()
            .valign(gtk::Align::Center)
            .active(true)
            .build();

        notif_row.append(&notif_text_box);
        notif_row.append(&switch_notifications);

        prefs_group.append(&prefs_title);
        prefs_group.append(&norm_row);
        prefs_group.append(&notif_row);
        form_box.append(&prefs_group);

        // Status Feedback Banner
        let status_label = gtk::Label::builder()
            .label("Ready")
            .css_classes(["player-artist"])
            .halign(gtk::Align::Start)
            .wrap(true)
            .build();
        form_box.append(&status_label);

        scrolled.set_child(Some(&form_box));
        container.append(&scrolled);

        // Wiring Callbacks
        let on_connect_token: Rc<RefCell<Option<Box<dyn Fn(String)>>>> = Rc::new(RefCell::new(None));
        let on_auto_detect_cookie: Rc<RefCell<Option<Box<dyn Fn()>>>> = Rc::new(RefCell::new(None));
        let on_sign_out: Rc<RefCell<Option<Box<dyn Fn()>>>> = Rc::new(RefCell::new(None));
        let on_sync_library: Rc<RefCell<Option<Box<dyn Fn()>>>> = Rc::new(RefCell::new(None));
        let on_save_cid: Rc<RefCell<Option<Box<dyn Fn(String)>>>> = Rc::new(RefCell::new(None));
        let on_discover_cid: Rc<RefCell<Option<Box<dyn Fn()>>>> = Rc::new(RefCell::new(None));
        let on_clear_cache: Rc<RefCell<Option<Box<dyn Fn()>>>> = Rc::new(RefCell::new(None));

        // Connect button
        let conn_cb = Rc::clone(&on_connect_token);
        let oauth_c = oauth_entry.clone();
        btn_connect.connect_clicked(move |_| {
            let tok = oauth_c.text().trim().to_string();
            if !tok.is_empty() {
                if let Some(ref cb) = *conn_cb.borrow() {
                    cb(tok);
                }
            }
        });

        // Open browser button
        btn_open_browser.connect_clicked(move |_| {
            let _ = gtk::gio::AppInfo::launch_default_for_uri("https://soundcloud.com/signin", None::<&gtk::gio::AppLaunchContext>);
        });

        // Auto-detect button
        let auto_cb = Rc::clone(&on_auto_detect_cookie);
        btn_auto_detect.connect_clicked(move |_| {
            if let Some(ref cb) = *auto_cb.borrow() {
                cb();
            }
        });

        // Sign out button
        let sign_out_cb = Rc::clone(&on_sign_out);
        btn_sign_out.connect_clicked(move |_| {
            if let Some(ref cb) = *sign_out_cb.borrow() {
                cb();
            }
        });

        // Sync button
        let sync_cb = Rc::clone(&on_sync_library);
        btn_sync.connect_clicked(move |_| {
            if let Some(ref cb) = *sync_cb.borrow() {
                cb();
            }
        });

        // Save CID
        let save_cid_cb = Rc::clone(&on_save_cid);
        let cid_c = client_id_entry.clone();
        btn_save_cid.connect_clicked(move |_| {
            let cid = cid_c.text().trim().to_string();
            if !cid.is_empty() {
                if let Some(ref cb) = *save_cid_cb.borrow() {
                    cb(cid);
                }
            }
        });

        // Discover CID
        let disc_cid_cb = Rc::clone(&on_discover_cid);
        btn_discover_cid.connect_clicked(move |_| {
            if let Some(ref cb) = *disc_cid_cb.borrow() {
                cb();
            }
        });

        // Clear cache
        let clear_c_cb = Rc::clone(&on_clear_cache);
        btn_clear_cache.connect_clicked(move |_| {
            if let Some(ref cb) = *clear_c_cb.borrow() {
                cb();
            }
        });

        // Profile update
        let on_update_profile: Rc<RefCell<Option<Box<dyn Fn(String, Option<String>, Option<String>, Option<String>, Option<String>)>>>> = Rc::new(RefCell::new(None));
        let update_p_cb = Rc::clone(&on_update_profile);
        let u_entry_c = edit_username_entry.clone();
        let city_e_c = edit_city_entry.clone();
        let ctry_e_c = edit_country_entry.clone();
        let bio_v_c = edit_bio_view.clone();
        let av_path_sub = Rc::clone(&selected_avatar_path);

        btn_save_profile.connect_clicked(move |_| {
            let username = u_entry_c.text().trim().to_string();
            if !username.is_empty() {
                let city = {
                    let c = city_e_c.text().trim().to_string();
                    if c.is_empty() { None } else { Some(c) }
                };
                let country = {
                    let co = ctry_e_c.text().trim().to_string();
                    if co.is_empty() { None } else { Some(co) }
                };
                let bio = {
                    let buffer = bio_v_c.buffer();
                    let start = buffer.start_iter();
                    let end = buffer.end_iter();
                    let t = buffer.text(&start, &end, false).trim().to_string();
                    if t.is_empty() { None } else { Some(t) }
                };
                let avatar = av_path_sub.borrow().clone();

                if let Some(ref cb) = *update_p_cb.borrow() {
                    cb(username, bio, city, country, avatar);
                }
            }
        });

        // Preferences callbacks
        let on_toggle_normalization: Rc<RefCell<Option<Box<dyn Fn(bool)>>>> = Rc::new(RefCell::new(None));
        let on_toggle_notifications: Rc<RefCell<Option<Box<dyn Fn(bool)>>>> = Rc::new(RefCell::new(None));

        let norm_cb = Rc::clone(&on_toggle_normalization);
        switch_normalization.connect_state_set(move |_, state| {
            if let Some(ref cb) = *norm_cb.borrow() {
                cb(state);
            }
            glib::Propagation::Proceed
        });

        let notif_pref_cb = Rc::clone(&on_toggle_notifications);
        switch_notifications.connect_state_set(move |_, state| {
            if let Some(ref cb) = *notif_pref_cb.borrow() {
                cb(state);
            }
            glib::Propagation::Proceed
        });

        Self {
            container,
            account_stack,
            guest_box,
            connected_box,
            user_avatar,
            user_name_lbl,
            user_handle_lbl,
            user_meta_lbl,
            oauth_entry,
            btn_open_browser,
            btn_auto_detect,
            btn_connect,
            btn_sync,
            btn_sign_out,
            avatar_edit_img,
            btn_choose_avatar,
            selected_avatar_path,
            edit_username_entry,
            edit_city_entry,
            edit_country_entry,
            edit_bio_view,
            btn_save_profile,
            client_id_entry,
            btn_save_cid,
            btn_discover_cid,
            btn_clear_cache,
            switch_normalization,
            switch_notifications,
            btn_theme_anthropic,
            btn_theme_obsidian,
            status_label,
            spinner,
            on_theme_changed,
            on_connect_token,
            on_auto_detect_cookie,
            on_sign_out,
            on_sync_library,
            on_save_cid,
            on_discover_cid,
            on_clear_cache,
            on_toggle_normalization,
            on_toggle_notifications,
            on_update_profile,
        }
    }

    pub fn widget(&self) -> &gtk::Box {
        &self.container
    }

    pub fn set_user_profile(&self, user: Option<&UserProfile>) {
        if let Some(u) = user {
            self.user_name_lbl.set_label(&u.username);
            let handle = u.permalink.as_deref().unwrap_or(&u.username);
            self.user_handle_lbl.set_label(&format!("@{} • Connected", handle));

            let followers = format_count(u.followers_count.unwrap_or(0));
            let likes = format_count(u.likes_count.unwrap_or(0));
            let loc = u.display_location().unwrap_or_else(|| "SoundCloud".to_string());
            self.user_meta_lbl.set_label(&format!("{} • {} followers • {} liked tracks", loc, followers, likes));

            crate::ui::image_loader::load_into_image(
                &self.user_avatar,
                u.avatar_url.as_deref(),
                "avatar-default-symbolic",
            );

            // Populate profile edit form
            self.edit_username_entry.set_text(&u.username);
            self.edit_city_entry.set_text(u.city.as_deref().unwrap_or(""));
            self.edit_country_entry.set_text(u.country.as_deref().unwrap_or(""));
            self.edit_bio_view.buffer().set_text(u.description.as_deref().unwrap_or(""));
            crate::ui::image_loader::load_into_image(
                &self.avatar_edit_img,
                u.avatar_url.as_deref(),
                "avatar-default-symbolic",
            );

            self.account_stack.set_visible_child_name("connected");
        } else {
            self.oauth_entry.set_text("");
            self.account_stack.set_visible_child_name("guest");
        }
    }

    pub fn set_client_id(&self, cid: &str) {
        self.client_id_entry.set_text(cid);
    }

    pub fn set_token_input(&self, tok: &str) {
        self.oauth_entry.set_text(tok);
    }

    pub fn set_status(&self, status: &str, is_error: bool) {
        self.status_label.set_label(status);
        if is_error {
            self.status_label.add_css_class("brand-accent");
        } else {
            self.status_label.remove_css_class("brand-accent");
        }
    }

    pub fn set_loading(&self, loading: bool) {
        self.spinner.set_spinning(loading);
    }

    pub fn connect_token<F: Fn(String) + 'static>(&self, f: F) {
        *self.on_connect_token.borrow_mut() = Some(Box::new(f));
    }

    pub fn connect_auto_detect<F: Fn() + 'static>(&self, f: F) {
        *self.on_auto_detect_cookie.borrow_mut() = Some(Box::new(f));
    }

    pub fn connect_sign_out<F: Fn() + 'static>(&self, f: F) {
        *self.on_sign_out.borrow_mut() = Some(Box::new(f));
    }

    pub fn connect_sync_library<F: Fn() + 'static>(&self, f: F) {
        *self.on_sync_library.borrow_mut() = Some(Box::new(f));
    }

    pub fn connect_save_cid<F: Fn(String) + 'static>(&self, f: F) {
        *self.on_save_cid.borrow_mut() = Some(Box::new(f));
    }

    pub fn connect_discover_cid<F: Fn() + 'static>(&self, f: F) {
        *self.on_discover_cid.borrow_mut() = Some(Box::new(f));
    }

    pub fn connect_clear_cache<F: Fn() + 'static>(&self, f: F) {
        *self.on_clear_cache.borrow_mut() = Some(Box::new(f));
    }

    pub fn set_audio_normalization(&self, active: bool) {
        self.switch_normalization.set_state(active);
    }

    pub fn set_desktop_notifications(&self, active: bool) {
        self.switch_notifications.set_state(active);
    }

    pub fn connect_audio_normalization<F: Fn(bool) + 'static>(&self, f: F) {
        *self.on_toggle_normalization.borrow_mut() = Some(Box::new(f));
    }

    pub fn connect_desktop_notifications<F: Fn(bool) + 'static>(&self, f: F) {
        *self.on_toggle_notifications.borrow_mut() = Some(Box::new(f));
    }

    pub fn connect_update_profile<F: Fn(String, Option<String>, Option<String>, Option<String>, Option<String>) + 'static>(&self, f: F) {
        *self.on_update_profile.borrow_mut() = Some(Box::new(f));
    }

    pub fn connect_theme_changed<F: Fn(crate::ui::theme::AppTheme) + 'static>(&self, f: F) {
        *self.on_theme_changed.borrow_mut() = Some(Box::new(f));
    }

    pub fn set_active_theme(&self, theme: crate::ui::theme::AppTheme) {
        match theme {
            crate::ui::theme::AppTheme::AnthropicClassical => {
                self.btn_theme_anthropic.add_css_class("active");
                self.btn_theme_obsidian.remove_css_class("active");
            }
            crate::ui::theme::AppTheme::ObsidianDark => {
                self.btn_theme_obsidian.add_css_class("active");
                self.btn_theme_anthropic.remove_css_class("active");
            }
        }
    }
}
