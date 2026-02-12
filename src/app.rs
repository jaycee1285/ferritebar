use tracing::info;

use crate::bar::Bar;
use crate::config;
use crate::modules;
use crate::theme;

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
        gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );

    // Set icon theme if configured
    if let Some(ref icon_theme) = cfg.theme.icon_theme {
        if let Some(settings) = gtk::Settings::default() {
            settings.set_gtk_icon_theme_name(Some(icon_theme));
            info!("Set icon theme to {icon_theme}");
        }
    }

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

        // Update icon theme
        if let Some(ref icon_theme) = cfg.theme.icon_theme {
            if let Some(settings) = gtk::Settings::default() {
                settings.set_gtk_icon_theme_name(Some(icon_theme));
            }
        }

        // Clear and rebuild modules
        bar_ref.clear();
        modules::populate_bar(&bar_ref, &cfg.modules, &colors);

        info!("Config reloaded");
    });

    info!("Ferritebar ready");
}
