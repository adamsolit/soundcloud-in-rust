pub mod context_menu;
pub mod image_loader;
pub mod mini_player;
pub mod player_bar;
pub mod shortcuts_dialog;
pub mod sidebar;
pub mod theme;
pub mod views;
pub mod waveform_widget;
pub mod window;

pub const APP_CSS: &str = include_str!("style.css");

pub fn load_css() {
    theme::apply_theme(theme::current_theme());
}

#[cfg(test)]
mod tests {
    use super::*;
    use gtk4 as gtk;

    #[test]
    fn test_css_loads_validly() {
        let _ = gtk::init();
        let provider = gtk::CssProvider::new();
        provider.load_from_string(theme::CSS_ANTHROPIC);

        let provider_obsidian = gtk::CssProvider::new();
        provider_obsidian.load_from_string(theme::CSS_OBSIDIAN);
    }
}
