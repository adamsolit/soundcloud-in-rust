#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

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

    // Windows startup environment setup
    #[cfg(target_os = "windows")]
    {
        if let Ok(exe) = std::env::current_exe() {
            if let Some(dir) = exe.parent() {
                // Ensure PATH contains the application folder so mpv.exe and all DLLs are resolved
                if let Some(path) = std::env::var_os("PATH") {
                    let mut paths = std::env::split_paths(&path).collect::<Vec<_>>();
                    paths.insert(0, dir.to_path_buf());
                    if let Ok(new_path) = std::env::join_paths(paths) {
                        unsafe {
                            std::env::set_var("PATH", new_path);
                        }
                    }
                }

                // GSettings Schemas
                let schema_dir = dir.join("share").join("glib-2.0").join("schemas");
                if schema_dir.exists() && std::env::var_os("GSETTINGS_SCHEMA_DIR").is_none() {
                    unsafe {
                        std::env::set_var("GSETTINGS_SCHEMA_DIR", &schema_dir);
                    }
                }

                // GDK Pixbuf loaders cache
                let loader_cache = dir.join("lib").join("gdk-pixbuf-2.0").join("2.10.0").join("loaders.cache");
                if loader_cache.exists() && std::env::var_os("GDK_PIXBUF_MODULE_FILE").is_none() {
                    unsafe {
                        std::env::set_var("GDK_PIXBUF_MODULE_FILE", &loader_cache);
                    }
                }

                // High performance Next-Gen GL renderer
                if std::env::var_os("GSK_RENDERER").is_none() {
                    unsafe {
                        std::env::set_var("GSK_RENDERER", "ngl");
                    }
                }

                // Register bundled fonts into Windows process font table
                let fonts_dir = dir.join("share").join("fonts");
                if fonts_dir.exists() {
                    load_bundled_fonts(&fonts_dir);
                }
            }
        }
    }

    // Create and enter Tokio multi-threaded runtime context
    let tokio_rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .max_blocking_threads(8)
        .thread_stack_size(2 * 1024 * 1024)
        .enable_all()
        .build()
        .expect("Failed to build Tokio runtime");
    let _tokio_guard = tokio_rt.enter();

    info!("Starting SoundCloud Desktop Client...");

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

#[cfg(target_os = "windows")]
fn load_bundled_fonts(fonts_dir: &std::path::Path) {
    use std::os::windows::ffi::OsStrExt;
    unsafe extern "system" {
        fn AddFontResourceExW(lpszFilename: *const u16, fl: u32, pdv: *mut std::ffi::c_void) -> i32;
    }
    const FR_PRIVATE: u32 = 0x10;

    if let Ok(entries) = std::fs::read_dir(fonts_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("");
            if ext.eq_ignore_ascii_case("ttf") || ext.eq_ignore_ascii_case("otf") || ext.eq_ignore_ascii_case("ttc") {
                let wide: Vec<u16> = path.as_os_str().encode_wide().chain(std::iter::once(0)).collect();
                unsafe {
                    let ret = AddFontResourceExW(wide.as_ptr(), FR_PRIVATE, std::ptr::null_mut());
                    if ret > 0 {
                        info!("Loaded bundled font: {:?}", path.file_name().unwrap_or_default());
                    }
                }
            }
        }
    }
}

#[cfg(target_os = "windows")]
fn ensure_desktop_integration() {
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let icon_dir = dir.join("share").join("icons");
            if icon_dir.exists() {
                if let Some(display) = gdk4::Display::default() {
                    let theme = gtk4::IconTheme::for_display(&display);
                    theme.add_search_path(&icon_dir);
                    theme.add_search_path(&icon_dir.join("hicolor"));
                    theme.add_search_path(&icon_dir.join("Adwaita"));
                }
            }
        }
    }
}

#[cfg(target_os = "linux")]
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

#[cfg(not(any(target_os = "linux", target_os = "windows")))]
fn ensure_desktop_integration() {}
