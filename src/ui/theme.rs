use gtk4 as gtk;
use std::cell::RefCell;
use std::sync::atomic::{AtomicU8, Ordering};

pub const CSS_ANTHROPIC: &str = include_str!("theme_anthropic.css");
pub const CSS_OBSIDIAN: &str = include_str!("theme_obsidian.css");

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppTheme {
    AnthropicClassical,
    ObsidianDark,
}

impl AppTheme {
    pub fn as_str(&self) -> &'static str {
        match self {
            AppTheme::AnthropicClassical => "anthropic",
            AppTheme::ObsidianDark => "obsidian",
        }
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            AppTheme::AnthropicClassical => "Anthropic Classical (Hellenic Editorial)",
            AppTheme::ObsidianDark => "Obsidian Dark (Legacy Monochrome)",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "obsidian" => AppTheme::ObsidianDark,
            _ => AppTheme::AnthropicClassical,
        }
    }
}

static CURRENT_THEME: AtomicU8 = AtomicU8::new(0); // 0 = Anthropic, 1 = Obsidian

thread_local! {
    static THEME_PROVIDER: RefCell<Option<gtk::CssProvider>> = const { RefCell::new(None) };
}

pub fn current_theme() -> AppTheme {
    if CURRENT_THEME.load(Ordering::Relaxed) == 1 {
        AppTheme::ObsidianDark
    } else {
        AppTheme::AnthropicClassical
    }
}

pub fn apply_theme(theme: AppTheme) {
    let code = match theme {
        AppTheme::AnthropicClassical => 0,
        AppTheme::ObsidianDark => 1,
    };
    CURRENT_THEME.store(code, Ordering::Relaxed);

    let css = match theme {
        AppTheme::AnthropicClassical => CSS_ANTHROPIC,
        AppTheme::ObsidianDark => CSS_OBSIDIAN,
    };

    THEME_PROVIDER.with(|provider_cell| {
        let mut opt = provider_cell.borrow_mut();
        if opt.is_none() {
            let p = gtk::CssProvider::new();
            if let Some(display) = gdk4::Display::default() {
                gtk::style_context_add_provider_for_display(
                    &display,
                    &p,
                    gtk::STYLE_PROVIDER_PRIORITY_USER + 20,
                );
            }
            *opt = Some(p);
        }
        if let Some(ref p) = *opt {
            p.load_from_string(css);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_theme_switch_and_defaults() {
        assert_eq!(AppTheme::from_str("anthropic"), AppTheme::AnthropicClassical);
        assert_eq!(AppTheme::from_str("obsidian"), AppTheme::ObsidianDark);
        assert_eq!(AppTheme::from_str("unknown"), AppTheme::AnthropicClassical);
        assert_eq!(AppTheme::AnthropicClassical.as_str(), "anthropic");
        assert_eq!(AppTheme::ObsidianDark.as_str(), "obsidian");
    }
}
