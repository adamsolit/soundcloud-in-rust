use gtk4 as gtk;
use gtk::prelude::*;
use std::cell::RefCell;
use std::rc::Rc;

use crate::models::conversation::{Conversation, ConversationMessage};

pub struct MessagesView {
    container: gtk::Box,
    conversations_box: gtk::Box,
    thread_box: gtk::Box,
    thread_header: gtk::Box,
    thread_title: gtk::Label,
    thread_avatar: gtk::Image,
    msg_scrolled: gtk::ScrolledWindow,
    msg_list: gtk::Box,
    msg_entry: gtk::Entry,
    send_btn: gtk::Button,
    spinner: gtk::Spinner,
    current_other_user_id: Rc<RefCell<Option<u64>>>,
    current_user_id: Rc<RefCell<u64>>,
    on_select_conversation: Rc<RefCell<Option<Box<dyn Fn(u64)>>>>,
    on_send_message: Rc<RefCell<Option<Box<dyn Fn(u64, String)>>>>,
    on_refresh: Rc<RefCell<Option<Box<dyn Fn()>>>>,
}

impl MessagesView {
    pub fn new() -> Self {
        let container = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(0)
            .hexpand(true)
            .vexpand(true)
            .build();

        // 1. Left Sidebar: Conversations list
        let left_panel = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(12)
            .width_request(280)
            .css_classes(["sidebar-panel"])
            .build();

        let left_header = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(10)
            .margin_start(16)
            .margin_end(16)
            .margin_top(18)
            .margin_bottom(6)
            .build();

        let header_title = gtk::Label::builder()
            .label("Messages")
            .css_classes(["brand-label"])
            .halign(gtk::Align::Start)
            .hexpand(true)
            .build();

        let spinner = gtk::Spinner::builder()
            .spinning(false)
            .valign(gtk::Align::Center)
            .build();

        let refresh_btn = gtk::Button::builder()
            .icon_name("view-refresh-symbolic")
            .css_classes(["filter-chip"])
            .tooltip_text("Refresh messages")
            .build();

        left_header.append(&header_title);
        left_header.append(&spinner);
        left_header.append(&refresh_btn);
        left_panel.append(&left_header);

        let conv_scrolled = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Never)
            .vscrollbar_policy(gtk::PolicyType::Automatic)
            .hexpand(true)
            .vexpand(true)
            .build();

        let conversations_box = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(4)
            .margin_start(8)
            .margin_end(8)
            .hexpand(true)
            .build();

        conv_scrolled.set_child(Some(&conversations_box));
        left_panel.append(&conv_scrolled);
        container.append(&left_panel);

        // 2. Right Pane: Active Thread
        let thread_box = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(12)
            .margin_start(18)
            .margin_end(18)
            .margin_top(16)
            .margin_bottom(16)
            .hexpand(true)
            .vexpand(true)
            .build();

        // Thread header
        let thread_header = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(12)
            .css_classes(["account-card"])
            .visible(false)
            .build();

        let thread_avatar = gtk::Image::builder()
            .pixel_size(40)
            .icon_name("avatar-default-symbolic")
            .css_classes(["track-card-artwork"])
            .valign(gtk::Align::Center)
            .build();

        let thread_title = gtk::Label::builder()
            .label("")
            .css_classes(["brand-label"])
            .halign(gtk::Align::Start)
            .valign(gtk::Align::Center)
            .build();

        thread_header.append(&thread_avatar);
        thread_header.append(&thread_title);
        thread_box.append(&thread_header);

        // Thread messages list
        let msg_scrolled = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Never)
            .vscrollbar_policy(gtk::PolicyType::Automatic)
            .hexpand(true)
            .vexpand(true)
            .build();

        let msg_list = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(10)
            .hexpand(true)
            .build();

        // Placeholder for no conversation selected
        let empty_placeholder = gtk::Label::builder()
            .label("Select a conversation from the left to start messaging.")
            .css_classes(["player-artist"])
            .halign(gtk::Align::Center)
            .valign(gtk::Align::Center)
            .vexpand(true)
            .build();
        msg_list.append(&empty_placeholder);

        msg_scrolled.set_child(Some(&msg_list));
        thread_box.append(&msg_scrolled);

        // Compose row
        let compose_row = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(10)
            .hexpand(true)
            .visible(false)
            .build();

        let msg_entry = gtk::Entry::builder()
            .placeholder_text("Write a message...")
            .css_classes(["sc-search-entry"])
            .hexpand(true)
            .build();

        let send_btn = gtk::Button::builder()
            .label("Send")
            .icon_name("mail-send-symbolic")
            .css_classes(["btn-pill-play"])
            .build();

        compose_row.append(&msg_entry);
        compose_row.append(&send_btn);
        thread_box.append(&compose_row);
        container.append(&thread_box);

        let current_other_user_id = Rc::new(RefCell::new(None));
        let current_user_id = Rc::new(RefCell::new(0u64));
        let on_select_conversation: Rc<RefCell<Option<Box<dyn Fn(u64)>>>> = Rc::new(RefCell::new(None));
        let on_send_message: Rc<RefCell<Option<Box<dyn Fn(u64, String)>>>> = Rc::new(RefCell::new(None));
        let on_refresh: Rc<RefCell<Option<Box<dyn Fn()>>>> = Rc::new(RefCell::new(None));

        // Connect refresh
        let ref_cb = Rc::clone(&on_refresh);
        refresh_btn.connect_clicked(move |_| {
            if let Some(ref cb) = *ref_cb.borrow() {
                cb();
            }
        });

        // Connect send button and enter key
        let send_cb = Rc::clone(&on_send_message);
        let other_u_send = Rc::clone(&current_other_user_id);
        let m_entry = msg_entry.clone();

        let do_send = move || {
            let other_id = *other_u_send.borrow();
            let text = m_entry.text().trim().to_string();
            if let Some(target_id) = other_id {
                if !text.is_empty() {
                    m_entry.set_text("");
                    if let Some(ref cb) = *send_cb.borrow() {
                        cb(target_id, text);
                    }
                }
            }
        };

        let do_send_btn = do_send.clone();
        send_btn.connect_clicked(move |_| {
            do_send_btn();
        });

        msg_entry.connect_activate(move |_| {
            do_send();
        });

        Self {
            container,
            conversations_box,
            thread_box,
            thread_header,
            thread_title,
            thread_avatar,
            msg_scrolled,
            msg_list,
            msg_entry,
            send_btn,
            spinner,
            current_other_user_id,
            current_user_id,
            on_select_conversation,
            on_send_message,
            on_refresh,
        }
    }

    pub fn widget(&self) -> &gtk::Box {
        &self.container
    }

    pub fn set_current_user_id(&self, user_id: u64) {
        *self.current_user_id.borrow_mut() = user_id;
    }

    pub fn set_loading(&self, loading: bool) {
        self.spinner.set_spinning(loading);
    }

    pub fn set_conversations(&self, conversations: Vec<Conversation>) {
        while let Some(child) = self.conversations_box.first_child() {
            self.conversations_box.remove(&child);
        }

        if conversations.is_empty() {
            let empty_lbl = gtk::Label::builder()
                .label("No active messages.")
                .css_classes(["player-artist"])
                .margin_top(30)
                .build();
            self.conversations_box.append(&empty_lbl);
            return;
        }

        let my_id = *self.current_user_id.borrow();

        for conv in conversations {
            let card = gtk::Box::builder()
                .orientation(gtk::Orientation::Horizontal)
                .spacing(10)
                .css_classes(["sidebar-user-pill"])
                .hexpand(true)
                .build();

            let avatar = gtk::Image::builder()
                .pixel_size(36)
                .icon_name("avatar-default-symbolic")
                .css_classes(["track-card-artwork"])
                .valign(gtk::Align::Center)
                .build();

            crate::ui::image_loader::load_into_image(
                &avatar,
                conv.display_avatar(my_id).as_deref(),
                "avatar-default-symbolic",
            );

            let text_vbox = gtk::Box::builder()
                .orientation(gtk::Orientation::Vertical)
                .spacing(2)
                .hexpand(true)
                .valign(gtk::Align::Center)
                .build();

            let name_lbl = gtk::Label::builder()
                .label(&conv.display_title(my_id))
                .css_classes(["track-card-title"])
                .halign(gtk::Align::Start)
                .ellipsize(gtk::pango::EllipsizeMode::End)
                .build();

            let snippet = conv.last_message.as_ref()
                .map(|m| m.content.as_str())
                .unwrap_or("No messages yet");

            let snippet_lbl = gtk::Label::builder()
                .label(snippet)
                .css_classes(["player-artist"])
                .halign(gtk::Align::Start)
                .ellipsize(gtk::pango::EllipsizeMode::End)
                .build();

            text_vbox.append(&name_lbl);
            text_vbox.append(&snippet_lbl);
            card.append(&avatar);
            card.append(&text_vbox);

            // Click gesture to select conversation
            let other_user = conv.other_user(my_id);
            if let Some(other) = other_user {
                let other_id = other.id;
                let other_name = other.username.clone();
                let other_avatar = other.avatar_url.clone();
                let sel_cb = Rc::clone(&self.on_select_conversation);
                let title_ref = self.thread_title.clone();
                let avatar_ref = self.thread_avatar.clone();
                let header_ref = self.thread_header.clone();
                let cur_other = Rc::clone(&self.current_other_user_id);
                let compose_ref = self.msg_entry.parent().unwrap();

                let gesture = gtk::GestureClick::new();
                gesture.connect_pressed(move |_, _, _, _| {
                    *cur_other.borrow_mut() = Some(other_id);
                    title_ref.set_label(&other_name);
                    crate::ui::image_loader::load_into_image(
                        &avatar_ref,
                        other_avatar.as_deref(),
                        "avatar-default-symbolic",
                    );
                    header_ref.set_visible(true);
                    compose_ref.set_visible(true);

                    if let Some(ref cb) = *sel_cb.borrow() {
                        cb(other_id);
                    }
                });
                card.add_controller(gesture);
            }

            self.conversations_box.append(&card);
        }
    }

    pub fn set_messages(&self, messages: Vec<ConversationMessage>) {
        while let Some(child) = self.msg_list.first_child() {
            self.msg_list.remove(&child);
        }

        if messages.is_empty() {
            let empty_lbl = gtk::Label::builder()
                .label("No messages in this conversation yet. Send the first message below!")
                .css_classes(["player-artist"])
                .margin_top(40)
                .halign(gtk::Align::Center)
                .build();
            self.msg_list.append(&empty_lbl);
            return;
        }

        let my_id = *self.current_user_id.borrow();

        for msg in messages {
            let is_mine = msg.sender.as_ref().map(|u| u.id == my_id).unwrap_or(false);

            let row = gtk::Box::builder()
                .orientation(gtk::Orientation::Horizontal)
                .hexpand(true)
                .halign(if is_mine { gtk::Align::End } else { gtk::Align::Start })
                .build();

            let bubble = gtk::Box::builder()
                .orientation(gtk::Orientation::Vertical)
                .spacing(4)
                .css_classes([if is_mine { "filter-chip" } else { "track-card" }])
                .margin_start(if is_mine { 40 } else { 0 })
                .margin_end(if is_mine { 0 } else { 40 })
                .build();

            let text_lbl = gtk::Label::builder()
                .label(&msg.content)
                .wrap(true)
                .wrap_mode(gtk::pango::WrapMode::WordChar)
                .halign(gtk::Align::Start)
                .build();

            let time_short = msg.sent_at.split('T').next().unwrap_or(&msg.sent_at);
            let time_lbl = gtk::Label::builder()
                .label(time_short)
                .css_classes(["time-label"])
                .halign(if is_mine { gtk::Align::End } else { gtk::Align::Start })
                .build();

            bubble.append(&text_lbl);
            bubble.append(&time_lbl);
            row.append(&bubble);
            self.msg_list.append(&row);
        }

        // Auto-scroll to bottom of messages
        let adj = self.msg_scrolled.vadjustment();
        glib::timeout_add_local_once(std::time::Duration::from_millis(50), move || {
            adj.set_value(adj.upper());
        });
    }

    pub fn connect_select_conversation<F: Fn(u64) + 'static>(&self, f: F) {
        *self.on_select_conversation.borrow_mut() = Some(Box::new(f));
    }

    pub fn connect_send_message<F: Fn(u64, String) + 'static>(&self, f: F) {
        *self.on_send_message.borrow_mut() = Some(Box::new(f));
    }

    pub fn connect_refresh<F: Fn() + 'static>(&self, f: F) {
        *self.on_refresh.borrow_mut() = Some(Box::new(f));
    }
}
