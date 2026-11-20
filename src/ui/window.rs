use gtk4 as gtk;
use libadwaita as adw;
use libadwaita::prelude::*;
use std::rc::Rc;

use crate::ui::player_bar::PlayerBar;
use crate::ui::sidebar::{NavTarget, Sidebar};
use crate::ui::views::{
    ArtistProfileView, LibraryView, MessagesView, NotificationsView, SearchView, SettingsView,
    StreamView, TrackDetailView, UploadView,
};

pub struct MainWindow {
    window: adw::ApplicationWindow,
    sidebar: Rc<Sidebar>,
    view_stack: gtk::Stack,
    player_bar: Rc<PlayerBar>,
    pub profile_avatar: gtk::Image,
    pub btn_profile: gtk::Button,
    pub previous_view: Rc<std::cell::RefCell<String>>,
    pub stream_view: Rc<StreamView>,
    pub search_view: Rc<SearchView>,
    pub library_view: Rc<LibraryView>,
    pub artist_profile_view: Rc<ArtistProfileView>,
    pub notifications_view: Rc<NotificationsView>,
    pub messages_view: Rc<MessagesView>,
    pub upload_view: Rc<UploadView>,
    pub track_detail_view: Rc<TrackDetailView>,
    pub settings_view: Rc<SettingsView>,
}

impl MainWindow {
    pub fn new(app: &adw::Application) -> Self {
        gtk::Window::set_default_icon_name("soundcloud");

        let window = adw::ApplicationWindow::builder()
            .application(app)
            .title("SoundCloud")
            .icon_name("soundcloud")
            .default_width(1120)
            .default_height(760)
            .css_classes(["cloudpulse-main-window"])
            .build();

        // Enforce dark mode scheme
        let style_manager = adw::StyleManager::default();
        style_manager.set_color_scheme(adw::ColorScheme::ForceDark);

        let root_box = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .hexpand(true)
            .vexpand(true)
            .build();

        // Minimal HeaderBar
        let header = adw::HeaderBar::builder()
            .title_widget(&adw::WindowTitle::new("SoundCloud", ""))
            .show_end_title_buttons(true)
            .show_start_title_buttons(true)
            .build();

        // Top-right header buttons for Profile, Notifications & Messages
        let profile_avatar = gtk::Image::builder()
            .pixel_size(26)
            .icon_name("avatar-default-symbolic")
            .css_classes(["header-profile-avatar"])
            .valign(gtk::Align::Center)
            .halign(gtk::Align::Center)
            .build();

        let btn_profile = gtk::Button::builder()
            .child(&profile_avatar)
            .css_classes(["header-profile-btn"])
            .tooltip_text("Account & Preferences")
            .valign(gtk::Align::Center)
            .build();

        let btn_messages = gtk::Button::builder()
            .icon_name("mail-unread-symbolic")
            .css_classes(["btn-transport"])
            .tooltip_text("Messages")
            .valign(gtk::Align::Center)
            .build();

        let btn_notifications = gtk::Button::builder()
            .icon_name("preferences-system-notifications-symbolic")
            .css_classes(["btn-transport"])
            .tooltip_text("Notifications")
            .valign(gtk::Align::Center)
            .build();

        let notifications_view = Rc::new(NotificationsView::new());
        let messages_view = Rc::new(MessagesView::new());

        // Notifications Popover Overlay
        let notif_popover = gtk::Popover::builder()
            .autohide(true)
            .has_arrow(true)
            .position(gtk::PositionType::Bottom)
            .build();
        notif_popover.set_parent(&btn_notifications);

        let notif_box = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .width_request(400)
            .height_request(480)
            .build();
        notif_box.append(notifications_view.widget());
        notif_popover.set_child(Some(&notif_box));

        let notif_pop_clone = notif_popover.clone();
        btn_notifications.connect_clicked(move |_| {
            notif_pop_clone.popup();
        });

        // Messages Popover Overlay
        let msg_popover = gtk::Popover::builder()
            .autohide(true)
            .has_arrow(true)
            .position(gtk::PositionType::Bottom)
            .build();
        msg_popover.set_parent(&btn_messages);

        let msg_box = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .width_request(580)
            .height_request(520)
            .build();
        msg_box.append(messages_view.widget());
        msg_popover.set_child(Some(&msg_box));

        let msg_pop_clone = msg_popover.clone();
        btn_messages.connect_clicked(move |_| {
            msg_pop_clone.popup();
        });

        // pack_end places widgets from right to left: Profile (right), Notifications (mid), Messages (left)
        header.pack_end(&btn_profile);
        header.pack_end(&btn_notifications);
        header.pack_end(&btn_messages);

        root_box.append(&header);

        // Body: Sidebar + Main Content View Stack
        let body_box = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .hexpand(true)
            .vexpand(true)
            .build();

        let sidebar = Rc::new(Sidebar::new());
        sidebar.widget().set_hexpand(false);
        sidebar.widget().set_hexpand_set(true);
        body_box.append(sidebar.widget());

        let view_stack = gtk::Stack::builder()
            .transition_type(gtk::StackTransitionType::Crossfade)
            .transition_duration(150)
            .hexpand(true)
            .vexpand(true)
            .build();

        let stream_view = Rc::new(StreamView::new());
        let search_view = Rc::new(SearchView::new());
        let library_view = Rc::new(LibraryView::new());
        let artist_profile_view = Rc::new(ArtistProfileView::new());
        let upload_view = Rc::new(UploadView::new());
        let track_detail_view = Rc::new(TrackDetailView::new());
        let settings_view = Rc::new(SettingsView::new());

        view_stack.add_named(stream_view.widget(), Some("discover"));
        view_stack.add_named(search_view.widget(), Some("search"));
        view_stack.add_named(library_view.widget(), Some("library"));
        view_stack.add_named(artist_profile_view.widget(), Some("artist_profile"));
        view_stack.add_named(upload_view.widget(), Some("upload"));
        view_stack.add_named(track_detail_view.widget(), Some("track_detail"));
        view_stack.add_named(settings_view.widget(), Some("settings"));

        view_stack.set_visible_child_name("discover");
        body_box.append(&view_stack);
        root_box.append(&body_box);

        // Persistent bottom player bar
        let player_bar = Rc::new(PlayerBar::new());
        root_box.append(player_bar.widget());

        window.set_content(Some(&root_box));

        let previous_view = Rc::new(std::cell::RefCell::new("discover".to_string()));

        // Connect sidebar navigation
        let stack_clone = view_stack.clone();
        let sidebar_clone = Rc::clone(&sidebar);
        let prev_view_clone = Rc::clone(&previous_view);
        sidebar.connect_navigate(move |target| {
            let view_name = match target {
                NavTarget::Discover => "discover",
                NavTarget::Search => "search",
                NavTarget::Library => "library",
                NavTarget::Upload => "upload",
                NavTarget::Settings => "settings",
                _ => return,
            };
            *prev_view_clone.borrow_mut() = view_name.to_string();
            stack_clone.set_visible_child_name(view_name);
            sidebar_clone.set_active(target);
        });

        // Connect top-right profile button to open Account & Settings
        let stack_for_prof = view_stack.clone();
        let sidebar_for_prof = Rc::clone(&sidebar);
        let prev_view_prof = Rc::clone(&previous_view);
        btn_profile.connect_clicked(move |_| {
            *prev_view_prof.borrow_mut() = "settings".to_string();
            stack_for_prof.set_visible_child_name("settings");
            sidebar_for_prof.set_active(NavTarget::Settings);
        });

        Self {
            window,
            sidebar,
            view_stack,
            player_bar,
            profile_avatar,
            btn_profile,
            previous_view,
            stream_view,
            search_view,
            library_view,
            artist_profile_view,
            notifications_view,
            messages_view,
            upload_view,
            track_detail_view,
            settings_view,
        }
    }

    pub fn update_user_profile(&self, user: Option<&crate::models::user::UserProfile>) {
        if let Some(u) = user {
            self.btn_profile
                .set_tooltip_text(Some(&format!("@{} (Account & Preferences)", u.username)));
            crate::ui::image_loader::load_into_image(
                &self.profile_avatar,
                u.avatar_url.as_deref(),
                "avatar-default-symbolic",
            );
        } else {
            self.btn_profile
                .set_tooltip_text(Some("Account & Preferences (Sign In)"));
            self.profile_avatar
                .set_icon_name(Some("avatar-default-symbolic"));
        }
    }

    pub fn present(&self) {
        self.window.present();
    }

    pub fn window(&self) -> &adw::ApplicationWindow {
        &self.window
    }

    pub fn player_bar(&self) -> Rc<PlayerBar> {
        Rc::clone(&self.player_bar)
    }

    pub fn sidebar(&self) -> Rc<Sidebar> {
        Rc::clone(&self.sidebar)
    }

    pub fn show_view(&self, name: &str) {
        if let Some(cur) = self.view_stack.visible_child_name() {
            let cur_str = cur.as_str();
            if cur_str != name && cur_str != "artist_profile" {
                *self.previous_view.borrow_mut() = cur_str.to_string();
            }
        }
        self.view_stack.set_visible_child_name(name);
    }

    pub fn go_back(&self) {
        let prev = self.previous_view.borrow().clone();
        if !prev.is_empty() {
            self.view_stack.set_visible_child_name(&prev);
        } else {
            self.view_stack.set_visible_child_name("discover");
        }
    }
}
