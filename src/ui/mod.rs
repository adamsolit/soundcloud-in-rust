pub mod image_loader;
pub mod player_bar;
pub mod sidebar;
pub mod views;
pub mod waveform_widget;
pub mod window;


use gtk4 as gtk;

pub const APP_CSS: &str = include_str!("style.css");

pub fn load_css() {
    let provider = gtk::CssProvider::new();
    provider.load_from_string(APP_CSS);

    if let Some(display) = gdk4::Display::default() {
        gtk::style_context_add_provider_for_display(
            &display,
            &provider,
            gtk::STYLE_PROVIDER_PRIORITY_USER,
        );
    }
}
