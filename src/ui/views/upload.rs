use gtk4 as gtk;
use gtk::prelude::*;
use std::cell::RefCell;
use std::rc::Rc;

use crate::api::upload::{TrackUploadMetadata, UploadProgress};

pub struct UploadView {
    container: gtk::Box,
    audio_path: Rc<RefCell<Option<String>>>,
    artwork_path: Rc<RefCell<Option<String>>>,
    audio_label: gtk::Label,
    title_entry: gtk::Entry,
    permalink_entry: gtk::Entry,
    genre_entry: gtk::Entry,
    tags_entry: gtk::Entry,
    desc_view: gtk::TextView,
    check_public: gtk::CheckButton,
    check_downloadable: gtk::CheckButton,
    license_dropdown: gtk::DropDown,
    release_date_entry: gtk::Entry,
    label_entry: gtk::Entry,
    isrc_entry: gtk::Entry,
    artwork_preview: gtk::Image,
    artwork_btn: gtk::Button,
    upload_btn: gtk::Button,
    progress_bar: gtk::ProgressBar,
    progress_label: gtk::Label,
    on_upload: Rc<RefCell<Option<Box<dyn Fn(TrackUploadMetadata)>>>>,
}

impl UploadView {
    pub fn new() -> Self {
        let container = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(16)
            .margin_top(16)
            .margin_bottom(16)
            .margin_start(24)
            .margin_end(24)
            .hexpand(true)
            .vexpand(true)
            .build();

        let title = gtk::Label::builder()
            .label("Upload Audio Track to SoundCloud")
            .css_classes(["brand-label"])
            .halign(gtk::Align::Start)
            .build();
        container.append(&title);

        let scrolled = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Never)
            .vscrollbar_policy(gtk::PolicyType::Automatic)
            .hexpand(true)
            .vexpand(true)
            .build();

        let form_box = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(16)
            .hexpand(true)
            .build();

        // 1. Drag and drop / File chooser zone
        let dropzone = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(8)
            .css_classes(["upload-dropzone"])
            .halign(gtk::Align::Fill)
            .build();

        let drop_icon = gtk::Image::builder()
            .icon_name("document-send-symbolic")
            .pixel_size(44)
            .css_classes(["brand-accent"])
            .build();

        let audio_label = gtk::Label::builder()
            .label("Choose an audio file from your device (MP3, WAV, FLAC, AAC, OGG)")
            .css_classes(["player-artist"])
            .build();

        let choose_file_btn = gtk::Button::builder()
            .label("Select Audio File")
            .css_classes(["filter-chip"])
            .halign(gtk::Align::Center)
            .build();

        dropzone.append(&drop_icon);
        dropzone.append(&audio_label);
        dropzone.append(&choose_file_btn);
        form_box.append(&dropzone);

        let audio_path = Rc::new(RefCell::new(None));
        let artwork_path = Rc::new(RefCell::new(None));

        // Connect audio file chooser
        let a_path_clone = Rc::clone(&audio_path);
        let a_lbl_clone = audio_label.clone();
        let cont_clone = container.clone();

        choose_file_btn.connect_clicked(move |_| {
            let window = cont_clone.root().and_downcast::<gtk::Window>();
            let dialog = gtk::FileDialog::builder()
                .title("Select Audio Track")
                .modal(true)
                .build();

            let a_path = Rc::clone(&a_path_clone);
            let a_lbl = a_lbl_clone.clone();

            dialog.open(window.as_ref(), gtk::gio::Cancellable::NONE, move |res| {
                if let Ok(file) = res {
                    if let Some(path) = file.path() {
                        let path_str = path.to_string_lossy().to_string();
                        let file_name = path.file_name().unwrap_or_default().to_string_lossy();
                        a_lbl.set_label(&format!("Audio: {}", file_name));
                        *a_path.borrow_mut() = Some(path_str);
                    }
                }
            });
        });

        // 2. Artwork & Basic Info Row
        let meta_section_label = gtk::Label::builder()
            .label("Basic Information")
            .css_classes(["track-card-title"])
            .halign(gtk::Align::Start)
            .build();
        form_box.append(&meta_section_label);

        let basic_row = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(16)
            .hexpand(true)
            .build();

        // Artwork preview box
        let artwork_box = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(8)
            .valign(gtk::Align::Start)
            .build();

        let artwork_preview = gtk::Image::builder()
            .pixel_size(120)
            .icon_name("audio-x-generic-symbolic")
            .css_classes(["track-card-artwork"])
            .build();

        let artwork_btn = gtk::Button::builder()
            .label("Choose Image")
            .css_classes(["filter-chip"])
            .halign(gtk::Align::Center)
            .build();

        artwork_box.append(&artwork_preview);
        artwork_box.append(&artwork_btn);
        basic_row.append(&artwork_box);

        // Fields next to artwork
        let basic_fields = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(10)
            .hexpand(true)
            .build();

        let title_entry = gtk::Entry::builder()
            .placeholder_text("Track Title * (Required)")
            .css_classes(["sc-search-entry"])
            .hexpand(true)
            .build();

        let permalink_entry = gtk::Entry::builder()
            .placeholder_text("Permalink / Slug (e.g. my-custom-track)")
            .css_classes(["sc-search-entry"])
            .hexpand(true)
            .build();

        let genre_entry = gtk::Entry::builder()
            .placeholder_text("Genre (e.g. Recitation, Spoken, Ambient, Classical)")
            .css_classes(["sc-search-entry"])
            .hexpand(true)
            .build();

        let tags_entry = gtk::Entry::builder()
            .placeholder_text("Tags (e.g. quran, tartil, podcast, 2026)")
            .css_classes(["sc-search-entry"])
            .hexpand(true)
            .build();

        basic_fields.append(&title_entry);
        basic_fields.append(&permalink_entry);
        basic_fields.append(&genre_entry);
        basic_fields.append(&tags_entry);
        basic_row.append(&basic_fields);
        form_box.append(&basic_row);

        // Connect artwork chooser
        let art_clone = Rc::clone(&artwork_path);
        let art_img_clone = artwork_preview.clone();
        let art_btn_clone = artwork_btn.clone();
        let cont_clone2 = container.clone();

        artwork_btn.connect_clicked(move |_| {
            let window = cont_clone2.root().and_downcast::<gtk::Window>();
            let dialog = gtk::FileDialog::builder()
                .title("Select Artwork Image")
                .modal(true)
                .build();

            let art_p = Rc::clone(&art_clone);
            let btn = art_btn_clone.clone();
            let img = art_img_clone.clone();

            dialog.open(window.as_ref(), gtk::gio::Cancellable::NONE, move |res| {
                if let Ok(file) = res {
                    if let Some(path) = file.path() {
                        let path_str = path.to_string_lossy().to_string();
                        let file_name = path.file_name().unwrap_or_default().to_string_lossy();
                        btn.set_label(&format!("Artwork: {}", file_name));
                        img.set_from_file(Some(&path));
                        *art_p.borrow_mut() = Some(path_str);
                    }
                }
            });
        });

        // 3. Description
        let desc_label = gtk::Label::builder()
            .label("Description")
            .css_classes(["track-card-title"])
            .halign(gtk::Align::Start)
            .build();
        form_box.append(&desc_label);

        let desc_view = gtk::TextView::builder()
            .css_classes(["sc-search-entry"])
            .height_request(80)
            .wrap_mode(gtk::WrapMode::Word)
            .build();
        form_box.append(&desc_view);

        // 4. Metadata & Publishing Details
        let pub_section_label = gtk::Label::builder()
            .label("Metadata & Credits")
            .css_classes(["track-card-title"])
            .halign(gtk::Align::Start)
            .build();
        form_box.append(&pub_section_label);

        let pub_grid = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(12)
            .hexpand(true)
            .build();

        let release_date_entry = gtk::Entry::builder()
            .placeholder_text("Release Date (YYYY-MM-DD)")
            .css_classes(["sc-search-entry"])
            .hexpand(true)
            .build();

        let label_entry = gtk::Entry::builder()
            .placeholder_text("Publisher / Record Label")
            .css_classes(["sc-search-entry"])
            .hexpand(true)
            .build();

        let isrc_entry = gtk::Entry::builder()
            .placeholder_text("ISRC Code")
            .css_classes(["sc-search-entry"])
            .hexpand(true)
            .build();

        pub_grid.append(&release_date_entry);
        pub_grid.append(&label_entry);
        pub_grid.append(&isrc_entry);
        form_box.append(&pub_grid);

        // 5. Permissions & Licensing
        let perm_section_label = gtk::Label::builder()
            .label("Permissions & Privacy")
            .css_classes(["track-card-title"])
            .halign(gtk::Align::Start)
            .build();
        form_box.append(&perm_section_label);

        let perm_box = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(10)
            .build();

        let check_public = gtk::CheckButton::builder()
            .label("Public (Track will be visible to everyone on SoundCloud)")
            .active(true)
            .build();

        let check_downloadable = gtk::CheckButton::builder()
            .label("Enable direct downloads (Listeners can download your original audio file)")
            .active(false)
            .build();

        let license_row = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(12)
            .build();

        let lic_lbl = gtk::Label::builder()
            .label("License:")
            .css_classes(["player-artist"])
            .valign(gtk::Align::Center)
            .build();

        let licenses = [
            "All Rights Reserved",
            "Creative Commons Attribution (CC-BY)",
            "Creative Commons Attribution-NonCommercial (CC-BY-NC)",
            "Creative Commons Attribution-NoDerivs (CC-BY-ND)",
            "Creative Commons Attribution-ShareAlike (CC-BY-SA)",
            "CC0 Public Domain",
        ];
        let license_model = gtk::StringList::new(&licenses);
        let license_dropdown = gtk::DropDown::builder()
            .model(&license_model)
            .selected(0)
            .css_classes(["filter-chip"])
            .build();

        license_row.append(&lic_lbl);
        license_row.append(&license_dropdown);

        perm_box.append(&check_public);
        perm_box.append(&check_downloadable);
        perm_box.append(&license_row);
        form_box.append(&perm_box);

        // 6. Action and Progress Bar
        let progress_bar = gtk::ProgressBar::builder()
            .fraction(0.0)
            .hexpand(true)
            .visible(false)
            .build();

        let progress_label = gtk::Label::builder()
            .label("")
            .css_classes(["player-artist"])
            .halign(gtk::Align::Start)
            .visible(false)
            .build();

        let upload_btn = gtk::Button::builder()
            .label("Upload Track to SoundCloud")
            .css_classes(["btn-pill-play"])
            .halign(gtk::Align::Start)
            .build();

        form_box.append(&progress_bar);
        form_box.append(&progress_label);
        form_box.append(&upload_btn);

        scrolled.set_child(Some(&form_box));
        container.append(&scrolled);

        let on_upload: Rc<RefCell<Option<Box<dyn Fn(TrackUploadMetadata)>>>> = Rc::new(RefCell::new(None));

        // Connect upload click
        let upload_cb = Rc::clone(&on_upload);
        let a_path_submit = Rc::clone(&audio_path);
        let art_path_submit = Rc::clone(&artwork_path);
        let t_entry = title_entry.clone();
        let p_entry = permalink_entry.clone();
        let g_entry = genre_entry.clone();
        let tg_entry = tags_entry.clone();
        let d_view = desc_view.clone();
        let chk_pub = check_public.clone();
        let chk_dl = check_downloadable.clone();
        let lic_drop = license_dropdown.clone();
        let rel_entry = release_date_entry.clone();
        let lbl_entry = label_entry.clone();
        let isrc_e = isrc_entry.clone();

        upload_btn.connect_clicked(move |_| {
            let audio = a_path_submit.borrow().clone();
            let title = t_entry.text().trim().to_string();

            if let Some(audio_file) = audio {
                if !title.is_empty() {
                    let permalink = {
                        let p = p_entry.text().trim().to_string();
                        if p.is_empty() { None } else { Some(p) }
                    };
                    let genre = {
                        let g = g_entry.text().trim().to_string();
                        if g.is_empty() { None } else { Some(g) }
                    };
                    let tags = {
                        let tg = tg_entry.text().trim().to_string();
                        if tg.is_empty() { None } else { Some(tg) }
                    };
                    let desc = {
                        let buffer = d_view.buffer();
                        let start = buffer.start_iter();
                        let end = buffer.end_iter();
                        let text = buffer.text(&start, &end, false).trim().to_string();
                        if text.is_empty() { None } else { Some(text) }
                    };
                    let release_date = {
                        let r = rel_entry.text().trim().to_string();
                        if r.is_empty() { None } else { Some(r) }
                    };
                    let label_name = {
                        let l = lbl_entry.text().trim().to_string();
                        if l.is_empty() { None } else { Some(l) }
                    };
                    let isrc = {
                        let i = isrc_e.text().trim().to_string();
                        if i.is_empty() { None } else { Some(i) }
                    };
                    let license = match lic_drop.selected() {
                        1 => Some("cc-by".to_string()),
                        2 => Some("cc-by-nc".to_string()),
                        3 => Some("cc-by-nd".to_string()),
                        4 => Some("cc-by-sa".to_string()),
                        5 => Some("cc0".to_string()),
                        _ => Some("all-rights-reserved".to_string()),
                    };

                    let meta = TrackUploadMetadata {
                        title,
                        permalink,
                        genre,
                        tags,
                        description: desc,
                        is_public: chk_pub.is_active(),
                        license,
                        downloadable: chk_dl.is_active(),
                        release_date,
                        label_name,
                        isrc,
                        audio_path: audio_file,
                        artwork_path: art_path_submit.borrow().clone(),
                    };

                    if let Some(ref cb) = *upload_cb.borrow() {
                        cb(meta);
                    }
                }
            }
        });

        Self {
            container,
            audio_path,
            artwork_path,
            audio_label,
            title_entry,
            permalink_entry,
            genre_entry,
            tags_entry,
            desc_view,
            check_public,
            check_downloadable,
            license_dropdown,
            release_date_entry,
            label_entry,
            isrc_entry,
            artwork_preview,
            artwork_btn,
            upload_btn,
            progress_bar,
            progress_label,
            on_upload,
        }
    }

    pub fn widget(&self) -> &gtk::Box {
        &self.container
    }

    pub fn set_progress(&self, progress: UploadProgress) {
        self.progress_bar.set_visible(true);
        self.progress_label.set_visible(true);
        self.progress_bar.set_fraction(progress.percentage);
        self.progress_label.set_label(&format!("{} ({:.0}%)", progress.stage, progress.percentage * 100.0));

        if progress.percentage >= 1.0 {
            self.upload_btn.set_sensitive(true);
        } else {
            self.upload_btn.set_sensitive(false);
        }
    }

    pub fn set_error(&self, err_msg: &str) {
        self.progress_bar.set_visible(false);
        self.progress_label.set_visible(true);
        self.progress_label.set_label(&format!("Upload error: {}", err_msg));
        self.upload_btn.set_sensitive(true);
    }

    pub fn connect_upload<F: Fn(TrackUploadMetadata) + 'static>(&self, f: F) {
        *self.on_upload.borrow_mut() = Some(Box::new(f));
    }
}
