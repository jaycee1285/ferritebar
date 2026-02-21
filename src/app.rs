use tracing::{debug, info};

use crate::bar::Bar;
use crate::config;
use crate::modules;
use crate::theme;

fn set_icon_theme(icon_theme: &Option<String>) {
    if let Some(ref name) = icon_theme {
        if let Some(settings) = gtk::Settings::default() {
            settings.set_gtk_icon_theme_name(Some(name));
        }

        // Force the display IconTheme to pick up the change
        if let Some(display) = gtk::gdk::Display::default() {
            let icon_theme_obj = gtk::IconTheme::for_display(&display);
            // Use GObject property instead of set_theme_name (which asserts on singletons)
            use glib::object::ObjectExt;
            icon_theme_obj.set_property("theme-name", name);
        }

        info!("Set icon theme to {name}");
    }
}

pub fn activate(app: &gtk::Application) {
    let config_path = config::default_config_path();
    let cfg = config::load_config(&config_path);

    info!("Ferritebar starting");

    crate::ipc::start_listener();

    // Log font availability for debugging (via fontconfig)
    crate::spawn(async {
        if let Ok(output) = tokio::process::Command::new("fc-list")
            .arg("--format=%{family}\n")
            .output()
            .await
        {
            let families = String::from_utf8_lossy(&output.stdout);
            for name in ["Font Awesome 7", "Font Awesome 6", "Font Awesome 5"] {
                let found = families.lines().any(|l| l.contains(name));
                debug!("Font check: \"{name}\" -> {}", if found { "FOUND" } else { "NOT FOUND" });
            }
        }
    });

    // Extract theme colors and generate CSS
    let colors = theme::extract_colors(&cfg.theme);
    let css = theme::generate_css(&colors, cfg.bar.height, &cfg.theme.font, cfg.theme.font_size);

    debug!("Extracted theme colors: {colors:?}");
    debug!("Generated CSS ({} bytes):\n{css}", css.len());

    // Apply CSS globally
    let provider = gtk::CssProvider::new();
    provider.connect_parsing_error(|_provider, section, error| {
        let location = section.start_location();
        tracing::warn!(
            "CSS parse error at line {}:{}: {}",
            location.lines() + 1,
            location.chars() + 1,
            error,
        );
    });
    provider.load_from_string(&css);
    let priority = gtk::STYLE_PROVIDER_PRIORITY_USER + 1;
    let display = gtk::gdk::Display::default().expect("Could not get default display");
    gtk::style_context_add_provider_for_display(
        &display,
        &provider,
        priority,
    );
    info!("CSS provider registered at priority {priority} (USER={}, THEME={})",
        gtk::STYLE_PROVIDER_PRIORITY_USER, gtk::STYLE_PROVIDER_PRIORITY_THEME);

    // Set icon theme if configured
    set_icon_theme(&cfg.theme.icon_theme);

    // Create bar
    let bar = Bar::new(app, &cfg.bar);

    // Populate modules
    modules::populate_bar(&bar, &cfg.modules, &colors);

    // Standalone power menu (IPC-only, not a bar module)
    crate::power_menu::setup(app, &cfg.power, bar.window());

    bar.show();

    // Start config file watcher for hot-reload
    let reload_rx = config::watch_config(config_path.clone());

    let bar_ref = bar;
    modules::recv_on_main_thread(reload_rx, move |()| {
        info!("Reloading config...");

        let cfg = config::load_config(&config_path);
        let colors = theme::extract_colors(&cfg.theme);
        let css = theme::generate_css(&colors, cfg.bar.height, &cfg.theme.font, cfg.theme.font_size);

        debug!("Reload: extracted colors: {colors:?}");
        debug!("Reload: generated CSS ({} bytes):\n{css}", css.len());

        // Reload CSS (instant theme updates)
        provider.load_from_string(&css);

        // Update icon theme before rebuilding
        set_icon_theme(&cfg.theme.icon_theme);

        // Clear and rebuild modules
        bar_ref.clear();
        modules::populate_bar(&bar_ref, &cfg.modules, &colors);

        info!("Config reloaded");
    });

    info!("Ferritebar ready");
}
