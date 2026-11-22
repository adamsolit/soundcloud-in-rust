use cloudpulse::app;
use cloudpulse::ui;
use libadwaita as adw;
use libadwaita::prelude::*;
use std::cell::RefCell;
use std::rc::Rc;
use tracing::{error, info};

const APP_ID: &str = "org.cloudpulse.SoundCloud";

fn main() {
    // Initialize structured logging
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info,cloudpulse=debug")),
        )
        .init();

    // Create and enter Tokio multi-threaded runtime context with capped worker threads for low memory footprint
    let tokio_rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .max_blocking_threads(8)
        .thread_stack_size(2 * 1024 * 1024)
        .enable_all()
        .build()
        .expect("Failed to build Tokio runtime");
    let _tokio_guard = tokio_rt.enter();

    info!("Starting CloudPulse Native Linux SoundCloud Client...");

    let app = adw::Application::builder()
        .application_id(APP_ID)
        .flags(gtk4::gio::ApplicationFlags::FLAGS_NONE)
        .build();

    let app_holder: Rc<RefCell<Option<app::Application>>> = Rc::new(RefCell::new(None));

    app.connect_startup(|_| {
        info!("GTK connect_startup called");
        ensure_desktop_integration();
        ui::load_css();
    });

    let app_holder_activate = Rc::clone(&app_holder);
    app.connect_activate(move |adw_app| {
        info!("GTK connect_activate called");
        if app_holder_activate.borrow().is_none() {
            match app::Application::new(adw_app) {
                Ok(application) => {
                    info!("Presenting main application window");
                    application.window.window().present();
                    *app_holder_activate.borrow_mut() = Some(application);
                }
                Err(e) => {
                    error!("Fatal application initialization error: {}", e);
                }
            }
        } else if let Some(ref application) = *app_holder_activate.borrow() {
            application.window.window().present();
        }
    });

    info!("Calling app.run()...");
    let exit_status = app.run();
    info!("app.run() finished with status: {:?}", exit_status);
}

fn ensure_desktop_integration() {
    let Some(home) = std::env::var_os("HOME").map(std::path::PathBuf::from) else { return; };
    let app_dir = home.join(".local/share/applications");
    let icon_dir = home.join(".local/share/icons/hicolor/scalable/apps");
    let desktop_id_file = app_dir.join(format!("{}.desktop", APP_ID));
    let desktop_alias_file = app_dir.join("soundcloud.desktop");
    let icon_id_file = icon_dir.join(format!("{}.svg", APP_ID));
    let icon_alias_file = icon_dir.join("soundcloud.svg");

    let _ = std::fs::create_dir_all(&icon_dir);
    let icon_bytes = include_bytes!("../assets/soundcloud_app_icon.svg");
    if !icon_id_file.exists() {
        let _ = std::fs::write(&icon_id_file, icon_bytes);
    }
    if !icon_alias_file.exists() {
        let _ = std::fs::write(&icon_alias_file, icon_bytes);
    }

    if let Some(display) = gdk4::Display::default() {
        let theme = gtk4::IconTheme::for_display(&display);
        theme.add_search_path(&icon_dir);
    }

    let _ = std::fs::create_dir_all(&app_dir);
    let current_exe = std::env::current_exe()
        .unwrap_or_else(|_| std::path::PathBuf::from("soundcloud"));
    let exe_str = current_exe.to_string_lossy();

    let desktop_content = format!(
        "[Desktop Entry]\n\
         Name=SoundCloud\n\
         Comment=Native SoundCloud client for Linux & Wayland\n\
         Exec={}\n\
         Icon={}\n\
         Terminal=false\n\
         Type=Application\n\
         Categories=AudioVideo;Audio;Player;\n\
         Keywords=SoundCloud;Streaming;Audio;Waveform;Player;\n\
         StartupNotify=true\n\
         StartupWMClass={}\n",
        exe_str, APP_ID, APP_ID
    );

    let _ = std::fs::write(&desktop_id_file, &desktop_content);
    let _ = std::fs::write(&desktop_alias_file, &desktop_content);
    let _ = std::process::Command::new("update-desktop-database")
        .arg(&app_dir)
        .status();
}
