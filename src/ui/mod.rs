pub mod context_menu;
pub mod image_loader;
pub mod mini_player;
pub mod player_bar;
pub mod shortcuts_dialog;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_css_loads_validly() {
        let _ = gtk::init();
        let provider = gtk::CssProvider::new();
        provider.load_from_string(APP_CSS);
    }
}
