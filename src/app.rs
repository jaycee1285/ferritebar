use tracing::info;

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

    // Extract theme colors and generate CSS
    let colors = theme::extract_colors(&cfg.theme);
    let css = theme::generate_css(&colors, cfg.bar.height, &cfg.theme.font);

    // Apply CSS globally
    let provider = gtk::CssProvider::new();
    provider.load_from_string(&css);
    let display = gtk::gdk::Display::default().expect("Could not get default display");
    gtk::style_context_add_provider_for_display(
        &display,
        &provider,
        gtk::STYLE_PROVIDER_PRIORITY_USER + 1,
    );

    // Set icon theme if configured
    set_icon_theme(&cfg.theme.icon_theme);

    // Create bar
    let bar = Bar::new(app, &cfg.bar);

    // Populate modules
    modules::populate_bar(&bar, &cfg.modules, &colors);

    bar.show();

    // Start config file watcher for hot-reload
    let reload_rx = config::watch_config(config_path.clone());

    let bar_ref = bar;
    modules::recv_on_main_thread(reload_rx, move |()| {
        info!("Reloading config...");

        let cfg = config::load_config(&config_path);
        let colors = theme::extract_colors(&cfg.theme);
        let css = theme::generate_css(&colors, cfg.bar.height, &cfg.theme.font);

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
