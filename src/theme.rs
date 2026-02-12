use crate::config::types::ThemeConfig;
use std::collections::HashMap;
use std::path::PathBuf;
use tracing::{debug, warn};

#[derive(Debug, Clone)]
pub struct ThemeColors {
    pub bg: String,
    pub fg: String,
    pub text: String,
    pub selected_bg: String,
    pub selected_fg: String,
    pub success: String,
    pub warning: String,
    pub error: String,
}

impl Default for ThemeColors {
    fn default() -> Self {
        Self {
            bg: "#2e3440".to_string(),
            fg: "#d8dee9".to_string(),
            text: "#eceff4".to_string(),
            selected_bg: "#5e81ac".to_string(),
            selected_fg: "#eceff4".to_string(),
            success: "#a3be8c".to_string(),
            warning: "#facc15".to_string(),
            error: "#f87171".to_string(),
        }
    }
}

/// Extract colors from the active GTK4 theme CSS files
pub fn extract_colors(theme_config: &ThemeConfig) -> ThemeColors {
    let mut colors = ThemeColors::default();

    // Try to find and parse the GTK4 theme CSS
    if let Some(theme_css) = find_gtk4_theme_css() {
        let defined = parse_define_colors(&theme_css);

        if let Some(c) = defined.get("theme_bg_color") {
            colors.bg = c.clone();
        }
        if let Some(c) = defined.get("theme_fg_color") {
            colors.fg = c.clone();
        }
        if let Some(c) = defined.get("theme_text_color") {
            colors.text = c.clone();
        }
        if let Some(c) = defined.get("theme_selected_bg_color") {
            colors.selected_bg = c.clone();
        }
        if let Some(c) = defined.get("theme_selected_fg_color") {
            colors.selected_fg = c.clone();
        }

        debug!("Extracted {} colors from GTK4 theme", defined.len());
    } else {
        warn!("Could not find GTK4 theme CSS, using defaults");
    }

    // User overrides from config
    if let Some(ref c) = theme_config.success_color {
        colors.success = c.clone();
    }
    if let Some(ref c) = theme_config.warning_color {
        colors.warning = c.clone();
    }
    if let Some(ref c) = theme_config.error_color {
        colors.error = c.clone();
    }

    colors
}

/// Find the active GTK4 theme's CSS file
fn find_gtk4_theme_css() -> Option<String> {
    // Check GTK4 settings.ini for theme name
    let config_dir = std::env::var("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            let home = std::env::var("HOME").expect("HOME not set");
            PathBuf::from(home).join(".config")
        });

    // Read gtk-4.0/settings.ini for theme name
    let settings_path = config_dir.join("gtk-4.0").join("settings.ini");
    let theme_name = if settings_path.exists() {
        std::fs::read_to_string(&settings_path)
            .ok()
            .and_then(|contents| {
                contents
                    .lines()
                    .find(|l| l.starts_with("gtk-theme-name"))
                    .and_then(|l| l.split('=').nth(1))
                    .map(|s| s.trim().to_string())
            })
    } else {
        None
    };

    // Check for user gtk.css first (direct custom CSS)
    let user_css = config_dir.join("gtk-4.0").join("gtk.css");
    if user_css.exists() {
        if let Ok(css) = std::fs::read_to_string(&user_css) {
            if css.contains("@define-color") {
                debug!("Using user gtk.css at {}", user_css.display());
                return Some(css);
            }
        }
    }

    // Search theme directories
    if let Some(name) = theme_name {
        let search_paths = vec![
            config_dir.join("themes").join(&name).join("gtk-4.0").join("gtk.css"),
            PathBuf::from("/usr/share/themes").join(&name).join("gtk-4.0").join("gtk.css"),
            PathBuf::from(std::env::var("HOME").unwrap_or_default())
                .join(".themes").join(&name).join("gtk-4.0").join("gtk.css"),
        ];

        for path in search_paths {
            if path.exists() {
                if let Ok(css) = std::fs::read_to_string(&path) {
                    debug!("Using theme CSS at {}", path.display());
                    return Some(css);
                }
            }
        }
    }

    None
}

/// Parse @define-color declarations from GTK CSS, resolving @references
fn parse_define_colors(css: &str) -> HashMap<String, String> {
    let mut colors = HashMap::new();

    for line in css.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("@define-color") {
            let rest = rest.trim();
            if let Some(space_idx) = rest.find(|c: char| c.is_whitespace()) {
                let name = rest[..space_idx].trim().to_string();
                let value = rest[space_idx..].trim().trim_end_matches(';').trim().to_string();
                colors.insert(name, value);
            }
        }
    }

    // Resolve @references (e.g. @define-color theme_bg_color @bg_color)
    let snapshot = colors.clone();
    for value in colors.values_mut() {
        if let Some(ref_name) = value.strip_prefix('@') {
            if let Some(resolved) = snapshot.get(ref_name) {
                *value = resolved.clone();
            }
        }
    }

    colors
}

/// Generate programmatic CSS from theme colors
pub fn generate_css(colors: &ThemeColors, bar_height: u32, font: &str) -> String {
    format!(
        r#"
window {{
    background-color: transparent;
    color: {fg};
}}

.bar-container {{
    padding: 0;
    margin: 0;
}}

.module {{
    padding: 0 4px;
    margin: 0 1px;
}}

.module-label {{
    color: {fg};
    font-family: "Font Awesome 7 Free Solid", "Font Awesome 7 Free", "Material Design Icons", "Symbols Nerd Font", "{font}", sans-serif;
    font-size: {font_size}px;
}}

.clock .module-label, .taskbar-button {{
    font-family: "{font}", sans-serif;
}}

.module:hover {{
    background-color: alpha({selected_bg}, 0.3);
    border-radius: 4px;
}}

.compact {{
    padding: 0 3px;
}}

.memory, .swap {{
    padding: 0 2px;
    margin: 0;
}}

.taskbar-button {{
    padding: 0 8px;
    border-radius: 4px;
    border: none;
    background: transparent;
    color: {fg};
    min-height: {bar_h}px;
}}

.taskbar-button:hover {{
    background-color: alpha({selected_bg}, 0.3);
}}

.taskbar-button.active {{
    background-color: alpha({selected_bg}, 0.5);
    color: {selected_fg};
}}

.power {{
    padding: 0 6px;
}}

.power-icon {{
    color: {fg};
}}

.power:hover .power-icon {{
    color: {error};
}}

.power-popover {{
    background-color: alpha({bg}, 0.85);
    border: 1px solid alpha({fg}, 0.2);
    border-radius: 8px;
    padding: 4px;
}}

.power-popover button {{
    background: transparent;
    border: none;
    color: {fg};
    padding: 8px 16px;
    border-radius: 4px;
}}

.power-popover button:hover {{
    background-color: alpha({selected_bg}, 0.3);
}}

.connected {{
    color: {fg};
}}

.disconnected {{
    color: alpha({fg}, 0.4);
}}

.muted {{
    color: alpha({fg}, 0.4);
}}

.charging {{
    color: {success};
}}

.low {{
    color: {warning};
}}

.critical {{
    color: {error};
}}

tooltip, tooltip.background {{
    background-color: alpha({bg}, 0.85);
    color: {fg};
    border: 1px solid alpha({fg}, 0.2);
    border-radius: 4px;
}}
"#,
        bg = colors.bg,
        fg = colors.fg,
        selected_bg = colors.selected_bg,
        selected_fg = colors.selected_fg,
        success = colors.success,
        warning = colors.warning,
        error = colors.error,
        font = font,
        font_size = (bar_height as f64 * 0.55).max(14.0) as u32,
        bar_h = bar_height,
    )
}

/// Parse a hex color string into (r, g, b) floats 0.0-1.0
pub fn hex_to_rgb(hex: &str) -> Option<(f64, f64, f64)> {
    let hex = hex.trim_start_matches('#');
    if hex.len() != 6 {
        return None;
    }
    let r = u8::from_str_radix(&hex[0..2], 16).ok()? as f64 / 255.0;
    let g = u8::from_str_radix(&hex[2..4], 16).ok()? as f64 / 255.0;
    let b = u8::from_str_radix(&hex[4..6], 16).ok()? as f64 / 255.0;
    Some((r, g, b))
}
