use crate::config::types::ThemeConfig;
use std::collections::HashMap;
use std::fmt::Write as _;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use tracing::{debug, warn};

#[derive(Debug, Clone)]
pub struct ThemeColors {
    pub bg: Box<str>,
    pub fg: Box<str>,
    pub text: Box<str>,
    pub menu_bg: Box<str>,
    pub menu_fg: Box<str>,
    pub selected_bg: Box<str>,
    pub selected_fg: Box<str>,
    pub success: Box<str>,
    pub warning: Box<str>,
    pub error: Box<str>,
}

impl Default for ThemeColors {
    fn default() -> Self {
        Self {
            bg: "#2e3440".into(),
            fg: "#d8dee9".into(),
            text: "#eceff4".into(),
            menu_bg: "#2e3440".into(),
            menu_fg: "#eceff4".into(),
            selected_bg: "#5e81ac".into(),
            selected_fg: "#eceff4".into(),
            success: "#a3be8c".into(),
            warning: "#facc15".into(),
            error: "#f87171".into(),
        }
    }
}

/// Resolve a color name from the parsed map, trying modern libadwaita names first,
/// then falling back to legacy GTK3 names.
fn resolve_color(defined: &HashMap<String, String>, names: &[&str]) -> Option<Box<str>> {
    for name in names {
        if let Some(c) = defined.get(*name) {
            return Some(c.clone().into_boxed_str());
        }
    }
    None
}

#[derive(Debug, Clone, Copy)]
struct Rgba {
    r: f64,
    g: f64,
    b: f64,
    a: f64,
}

impl Rgba {
    fn opaque(r: f64, g: f64, b: f64) -> Self {
        Self { r, g, b, a: 1.0 }
    }
}

#[derive(Debug, Clone)]
struct NamedColor {
    name: String,
    value: String,
    parsed: Rgba,
}

#[derive(Debug, Clone)]
struct MenuColorChoice {
    bg_name: String,
    bg_value: String,
    fg_name: String,
    fg_value: String,
    contrast: f64,
    meets_aa: bool,
}

const MENU_BG_CANDIDATES: &[&str] = &[
    "popover_bg_color",
    "view_bg_color",
    "window_bg_color",
    "theme_base_color",
    "theme_bg_color",
];

const MENU_FG_CANDIDATES: &[&str] = &[
    "popover_fg_color",
    "view_fg_color",
    "window_fg_color",
    "theme_text_color",
    "theme_fg_color",
];

static WCAG_REPORT_ENABLED: AtomicBool = AtomicBool::new(false);

pub fn set_wcag_report_enabled(enabled: bool) {
    WCAG_REPORT_ENABLED.store(enabled, Ordering::Relaxed);
}

fn parse_css_color(raw: &str) -> Option<Rgba> {
    let raw = raw.trim();
    if let Some(hex) = raw.strip_prefix('#') {
        return parse_hex_color(hex);
    }
    if let Some(inner) = raw.strip_prefix("rgba(").and_then(|s| s.strip_suffix(')')) {
        let parts: Vec<_> = inner.split(',').map(str::trim).collect();
        if parts.len() != 4 {
            return None;
        }
        return Some(Rgba {
            r: parse_rgb_channel(parts[0])?,
            g: parse_rgb_channel(parts[1])?,
            b: parse_rgb_channel(parts[2])?,
            a: parse_alpha_channel(parts[3])?,
        });
    }
    if let Some(inner) = raw.strip_prefix("rgb(").and_then(|s| s.strip_suffix(')')) {
        let parts: Vec<_> = inner.split(',').map(str::trim).collect();
        if parts.len() != 3 {
            return None;
        }
        return Some(Rgba {
            r: parse_rgb_channel(parts[0])?,
            g: parse_rgb_channel(parts[1])?,
            b: parse_rgb_channel(parts[2])?,
            a: 1.0,
        });
    }
    None
}

fn parse_hex_color(hex: &str) -> Option<Rgba> {
    let (r, g, b, a) = match hex.len() {
        3 => {
            let r = u8::from_str_radix(&hex[0..1].repeat(2), 16).ok()?;
            let g = u8::from_str_radix(&hex[1..2].repeat(2), 16).ok()?;
            let b = u8::from_str_radix(&hex[2..3].repeat(2), 16).ok()?;
            (r, g, b, 255)
        }
        4 => {
            let r = u8::from_str_radix(&hex[0..1].repeat(2), 16).ok()?;
            let g = u8::from_str_radix(&hex[1..2].repeat(2), 16).ok()?;
            let b = u8::from_str_radix(&hex[2..3].repeat(2), 16).ok()?;
            let a = u8::from_str_radix(&hex[3..4].repeat(2), 16).ok()?;
            (r, g, b, a)
        }
        6 => {
            let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
            let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
            let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
            (r, g, b, 255)
        }
        8 => {
            let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
            let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
            let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
            let a = u8::from_str_radix(&hex[6..8], 16).ok()?;
            (r, g, b, a)
        }
        _ => return None,
    };
    Some(Rgba {
        r: r as f64 / 255.0,
        g: g as f64 / 255.0,
        b: b as f64 / 255.0,
        a: a as f64 / 255.0,
    })
}

fn parse_rgb_channel(value: &str) -> Option<f64> {
    let value = value.trim();
    if let Some(percent) = value.strip_suffix('%') {
        let parsed = percent.trim().parse::<f64>().ok()?;
        return Some((parsed / 100.0).clamp(0.0, 1.0));
    }
    let parsed = value.parse::<f64>().ok()?;
    Some((parsed / 255.0).clamp(0.0, 1.0))
}

fn parse_alpha_channel(value: &str) -> Option<f64> {
    let value = value.trim();
    if let Some(percent) = value.strip_suffix('%') {
        let parsed = percent.trim().parse::<f64>().ok()?;
        return Some((parsed / 100.0).clamp(0.0, 1.0));
    }
    let parsed = value.parse::<f64>().ok()?;
    Some(parsed.clamp(0.0, 1.0))
}

fn composite_over(fg: Rgba, bg: Rgba) -> Rgba {
    let out_a = fg.a + bg.a * (1.0 - fg.a);
    if out_a <= f64::EPSILON {
        return Rgba::opaque(0.0, 0.0, 0.0);
    }
    let blend = |f: f64, b: f64| (f * fg.a + b * bg.a * (1.0 - fg.a)) / out_a;
    Rgba {
        r: blend(fg.r, bg.r),
        g: blend(fg.g, bg.g),
        b: blend(fg.b, bg.b),
        a: out_a,
    }
}

fn srgb_to_linear(channel: f64) -> f64 {
    if channel <= 0.04045 {
        channel / 12.92
    } else {
        ((channel + 0.055) / 1.055).powf(2.4)
    }
}

fn relative_luminance(color: Rgba) -> f64 {
    0.2126 * srgb_to_linear(color.r)
        + 0.7152 * srgb_to_linear(color.g)
        + 0.0722 * srgb_to_linear(color.b)
}

fn contrast_ratio(a: Rgba, b: Rgba) -> f64 {
    let l1 = relative_luminance(a);
    let l2 = relative_luminance(b);
    let (bright, dark) = if l1 >= l2 { (l1, l2) } else { (l2, l1) };
    (bright + 0.05) / (dark + 0.05)
}

fn collect_named_colors(defined: &HashMap<String, String>, names: &[&str]) -> Vec<NamedColor> {
    names.iter()
        .filter_map(|name| {
            let value = defined.get(*name)?;
            let parsed = parse_css_color(value)?;
            Some(NamedColor {
                name: (*name).to_string(),
                value: value.clone(),
                parsed,
            })
        })
        .collect()
}

fn pick_menu_color_choice(
    defined: &HashMap<String, String>,
    fallback_surface: &str,
) -> Option<(MenuColorChoice, String)> {
    let bg_candidates = collect_named_colors(defined, MENU_BG_CANDIDATES);
    let fg_candidates = collect_named_colors(defined, MENU_FG_CANDIDATES);
    if bg_candidates.is_empty() || fg_candidates.is_empty() {
        return None;
    }

    let base_surface = MENU_BG_CANDIDATES
        .iter()
        .find_map(|name| defined.get(*name).and_then(|value| parse_css_color(value)))
        .or_else(|| parse_css_color(fallback_surface))
        .unwrap_or_else(|| Rgba::opaque(0.18, 0.20, 0.25));

    let mut evaluations = Vec::new();
    for bg in &bg_candidates {
        let bg_opaque = composite_over(bg.parsed, base_surface);
        for fg in &fg_candidates {
            let fg_opaque = composite_over(fg.parsed, bg_opaque);
            let ratio = contrast_ratio(fg_opaque, bg_opaque);
            evaluations.push(MenuColorChoice {
                bg_name: bg.name.clone(),
                bg_value: bg.value.clone(),
                fg_name: fg.name.clone(),
                fg_value: fg.value.clone(),
                contrast: ratio,
                meets_aa: ratio >= 4.5,
            });
        }
    }

    evaluations.sort_by(|a, b| {
        b.meets_aa
            .cmp(&a.meets_aa)
            .then_with(|| {
                let direct_popover_a =
                    a.bg_name == "popover_bg_color" && a.fg_name == "popover_fg_color";
                let direct_popover_b =
                    b.bg_name == "popover_bg_color" && b.fg_name == "popover_fg_color";
                direct_popover_b.cmp(&direct_popover_a)
            })
            .then_with(|| b.contrast.total_cmp(&a.contrast))
    });

    let chosen = evaluations.first()?.clone();

    let mut report = String::new();
    let _ = writeln!(report, "# Ferritebar Theme Contrast Report");
    let _ = writeln!(report);
    let _ = writeln!(
        report,
        "Chosen menu pair: `{}` on `{}`",
        chosen.fg_name, chosen.bg_name
    );
    let _ = writeln!(
        report,
        "Contrast ratio: `{:.2}:1` ({})",
        chosen.contrast,
        if chosen.meets_aa {
            "passes WCAG AA"
        } else {
            "fails WCAG AA"
        }
    );
    let _ = writeln!(report, "Foreground value: `{}`", chosen.fg_value);
    let _ = writeln!(report, "Background value: `{}`", chosen.bg_value);
    let _ = writeln!(report);
    let _ = writeln!(report, "Top candidate pairs:");
    for eval in evaluations.iter().take(8) {
        let _ = writeln!(
            report,
            "- `{}` on `{}` => `{:.2}:1` {}",
            eval.fg_name,
            eval.bg_name,
            eval.contrast,
            if eval.meets_aa { "AA" } else { "fail" }
        );
    }
    let _ = writeln!(report);
    let _ = writeln!(report, "Parsed background candidates:");
    for bg in &bg_candidates {
        let _ = writeln!(report, "- `{}` = `{}`", bg.name, bg.value);
    }
    let _ = writeln!(report);
    let _ = writeln!(report, "Parsed foreground candidates:");
    for fg in &fg_candidates {
        let _ = writeln!(report, "- `{}` = `{}`", fg.name, fg.value);
    }

    Some((chosen, report))
}

fn write_theme_report(report: &str) {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("theme-contrast-report.md");
    if let Err(err) = std::fs::write(&path, report) {
        warn!("Failed to write theme report at {}: {err}", path.display());
    } else {
        debug!("Wrote theme report to {}", path.display());
    }
}

/// Extract colors from the active GTK4 theme CSS files
pub fn extract_colors(theme_config: &ThemeConfig) -> ThemeColors {
    let mut colors = ThemeColors::default();

    // Try to find and parse the GTK4 theme CSS
    if let Some(theme_css) = find_gtk4_theme_css() {
        let defined = parse_define_colors(&theme_css);

        // Background: libadwaita window_bg_color, then legacy theme_bg_color
        if let Some(c) = resolve_color(&defined, &["window_bg_color", "theme_bg_color"]) {
            colors.bg = c;
        }
        // Foreground: libadwaita window_fg_color, then legacy theme_fg_color
        if let Some(c) = resolve_color(&defined, &["window_fg_color", "theme_fg_color"]) {
            colors.fg = c;
        }
        // Text: view_fg_color, then legacy theme_text_color, then window_fg_color
        if let Some(c) = resolve_color(
            &defined,
            &["view_fg_color", "theme_text_color", "window_fg_color"],
        ) {
            colors.text = c;
        }
        // Menu/popup colors: choose among GTK menu/view/window candidates using measured contrast.
        if let Some((choice, report)) = pick_menu_color_choice(&defined, &colors.bg) {
            colors.menu_bg = choice.bg_value.into_boxed_str();
            colors.menu_fg = choice.fg_value.into_boxed_str();
            if WCAG_REPORT_ENABLED.load(Ordering::Relaxed) {
                write_theme_report(&report);
            }
        } else {
            // Fall back to direct priority when we can't parse enough colors to score candidates.
            if let Some(c) = resolve_color(&defined, MENU_BG_CANDIDATES) {
                colors.menu_bg = c;
            }
            if let Some(c) = resolve_color(&defined, MENU_FG_CANDIDATES) {
                colors.menu_fg = c;
            }
        }
        // Selected/accent bg: libadwaita accent_bg_color, then legacy theme_selected_bg_color
        if let Some(c) = resolve_color(&defined, &["accent_bg_color", "theme_selected_bg_color"]) {
            colors.selected_bg = c;
        }
        // Selected/accent fg: libadwaita accent_fg_color, then legacy theme_selected_fg_color
        if let Some(c) = resolve_color(&defined, &["accent_fg_color", "theme_selected_fg_color"]) {
            colors.selected_fg = c;
        }
        // Status colors from theme
        if let Some(c) = resolve_color(&defined, &["success_color"]) {
            colors.success = c;
        }
        if let Some(c) = resolve_color(&defined, &["warning_color"]) {
            colors.warning = c;
        }
        if let Some(c) = resolve_color(&defined, &["error_color", "destructive_color"]) {
            colors.error = c;
        }

        debug!(
            "Extracted {} @define-color entries from GTK4 theme",
            defined.len()
        );
        debug!("Resolved colors: bg={}, fg={}, text={}, menu_bg={}, menu_fg={}, selected_bg={}, selected_fg={}, success={}, warning={}, error={}",
            colors.bg, colors.fg, colors.text, colors.menu_bg, colors.menu_fg, colors.selected_bg, colors.selected_fg,
            colors.success, colors.warning, colors.error);
    } else {
        warn!("Could not find GTK4 theme CSS, using ALL defaults");
    }

    // User overrides from config (highest priority)
    if let Some(ref c) = theme_config.success_color {
        colors.success = c.clone().into_boxed_str();
    }
    if let Some(ref c) = theme_config.warning_color {
        colors.warning = c.clone().into_boxed_str();
    }
    if let Some(ref c) = theme_config.error_color {
        colors.error = c.clone().into_boxed_str();
    }
    if let Some(ref c) = theme_config.menu_bg_color {
        colors.menu_bg = c.clone().into_boxed_str();
    }
    if let Some(ref c) = theme_config.menu_fg_color {
        colors.menu_fg = c.clone().into_boxed_str();
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
                debug!(
                    "Using user gtk.css at {} ({} bytes)",
                    user_css.display(),
                    css.len()
                );
                return Some(css);
            } else {
                warn!(
                    "User gtk.css exists at {} but has no @define-color directives",
                    user_css.display()
                );
            }
        } else {
            warn!(
                "User gtk.css exists at {} but failed to read",
                user_css.display()
            );
        }
    } else {
        debug!("No user gtk.css at {}", user_css.display());
    }

    // Search theme directories
    if let Some(name) = theme_name {
        let search_paths = vec![
            config_dir
                .join("themes")
                .join(&name)
                .join("gtk-4.0")
                .join("gtk.css"),
            PathBuf::from("/usr/share/themes")
                .join(&name)
                .join("gtk-4.0")
                .join("gtk.css"),
            PathBuf::from(std::env::var("HOME").unwrap_or_default())
                .join(".themes")
                .join(&name)
                .join("gtk-4.0")
                .join("gtk.css"),
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
                let value = rest[space_idx..]
                    .trim()
                    .trim_end_matches(';')
                    .trim()
                    .to_string();
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
pub fn generate_css(
    colors: &ThemeColors,
    bar_height: u32,
    font: &str,
    font_size_override: Option<u32>,
) -> String {
    format!(
        r#"
window {{
    background-color: transparent;
}}

.bar-container {{
    padding: 0;
    margin: 0;
}}

.module {{
    padding: 0 4px;
    margin: 0 1px;
}}

.module label.module-label {{
    color: {selected_bg};
    font-family: "Font Awesome 7 Free Solid", "Font Awesome 7 Free", "{font}", sans-serif;
    font-size: {font_size}px;
}}

.clock label.module-label {{
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
    font-family: "{font}", sans-serif;
    min-height: {bar_h}px;
}}

.taskbar-button.inactive {{
    color: {selected_bg};
}}

.workspace-button {{
    padding: 0 6px;
    border-radius: 4px;
    border: none;
    background: transparent;
    color: {fg};
    min-height: {bar_h}px;
}}

.taskbar-button:hover {{
    background-color: alpha({selected_bg}, 0.3);
}}

.workspace-button:hover {{
    background-color: alpha({selected_bg}, 0.3);
}}

.taskbar-button.active {{
    background-color: alpha({selected_bg}, 0.5);
    color: {fg};
}}

.workspace-button.active {{
    background-color: alpha({selected_bg}, 0.5);
    color: {selected_fg};
}}

.workspace-button.urgent {{
    background-color: alpha({error}, 0.25);
    color: {error};
}}

.workspace-button.hidden {{
    opacity: 0.6;
}}

.power-popover {{
    background-color: alpha({menu_bg}, 0.96);
    border: 1px solid alpha({menu_fg}, 0.2);
    border-radius: 8px;
    padding: 4px;
}}

.power-popover button {{
    background: transparent;
    border: none;
    color: {menu_fg};
    padding: 8px 16px;
    border-radius: 4px;
}}

.power-popover button:hover {{
    background-color: alpha({selected_bg}, 0.3);
}}

.power-popover button.active {{
    background-color: {selected_bg};
    color: {selected_fg};
}}

.toggle-menu {{
    background-color: alpha({menu_bg}, 0.96);
    border: 1px solid alpha({menu_fg}, 0.2);
    border-radius: 8px;
    padding: 8px 4px;
}}

.toggle-menu label {{
    color: {menu_fg};
    font-family: "{font}", sans-serif;
    font-size: {font_size}px;
    padding: 2px 8px;
}}

.toggle-menu label.active {{
    background-color: {selected_bg};
    color: {selected_fg};
    border-radius: 4px;
}}

.toggle-menu label.toggle-header {{
    color: alpha({menu_fg}, 0.6);
    font-size: {tooltip_font_size}px;
    font-weight: bold;
    padding: 4px 8px 2px 8px;
}}

.toggle-menu label.toggle-header-active {{
    color: {selected_bg};
}}

.toggle-menu label.toggle-empty {{
    color: alpha({menu_fg}, 0.3);
    font-style: italic;
}}

.toggle-menu separator {{
    margin: 4px 8px;
    min-height: 1px;
    background-color: alpha({menu_fg}, 0.15);
}}

.tray-menu {{
    background-color: alpha({menu_bg}, 0.96);
    border: 1px solid alpha({menu_fg}, 0.2);
    border-radius: 8px;
    padding: 4px;
}}

.tray-menu button {{
    background: transparent;
    border: none;
    color: {menu_fg};
    padding: 6px 12px;
    border-radius: 4px;
    min-height: 0;
}}

.tray-menu button:hover {{
    background-color: alpha({selected_bg}, 0.18);
    color: {menu_fg}
}}

.tray-menu button:disabled {{
    color: alpha({menu_fg}, 0.4);
}}

.tray-menu separator {{
    margin: 2px 4px;
    min-height: 1px;
    background-color: alpha({menu_fg}, 0.15);
}}

.tray-menu .submenu-header {{
    color: alpha({menu_fg}, 0.7);
    padding: 4px 12px 2px 12px;
    font-size: 0.9em;
}}

.tray-menu .submenu-item {{
    padding-left: 24px;
}}

.tray-menu .toggle-on {{
    font-weight: bold;
}}

.connected label.module-label {{
    color: {success};
}}

.disconnected label.module-label {{
    color: {warning};
}}

.muted label.module-label {{
    color: alpha({fg}, 0.8);
}}

.charging label.module-label {{
    color: {selected_bg};
}}

.low label.module-label {{
    color: {warning};
}}

.critical label.module-label {{
    color: {error};
}}

tooltip, tooltip.background {{
    background-color: alpha({menu_bg}, 0.96);
    color: {menu_fg};
    border: 1px solid alpha({menu_fg}, 0.2);
    border-radius: 4px;
}}

tooltip label {{
    color: {selected_bg};
    text-transform: none;
    font-variant: normal;
    font-family: "{font}", sans-serif;
    font-size: {tooltip_font_size}px;
}}

.ferrite-tooltip {{
    text-transform: none;
    font-variant: normal;
    font-family: "{font}", sans-serif;
    font-size: {tooltip_font_size}px;
    color: {selected_bg};
}}
"#,
        fg = colors.fg,
        menu_bg = colors.menu_bg,
        menu_fg = colors.menu_fg,
        selected_bg = colors.selected_bg,
        selected_fg = colors.selected_fg,
        success = colors.success,
        warning = colors.warning,
        error = colors.error,
        font = font,
        font_size = font_size_override.unwrap_or((bar_height as f64 * 0.55).max(14.0) as u32),
        tooltip_font_size =
            (font_size_override.unwrap_or((bar_height as f64 * 0.55).max(14.0) as u32) as f64
                * 0.75) as u32,
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
