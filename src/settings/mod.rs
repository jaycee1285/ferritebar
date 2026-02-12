use gtk::prelude::*;
use tracing::{error, info};

use crate::config;

/// Open the settings window for editing the config TOML
pub fn open(app: &gtk::Application) {
    let config_path = config::default_config_path();

    let window = gtk::ApplicationWindow::builder()
        .application(app)
        .title("Ferritebar Settings")
        .default_width(700)
        .default_height(500)
        .build();

    let vbox = gtk::Box::new(gtk::Orientation::Vertical, 0);

    // Header bar with save button
    let header = gtk::HeaderBar::new();
    let save_button = gtk::Button::with_label("Save");
    save_button.add_css_class("suggested-action");
    header.pack_end(&save_button);

    let open_external = gtk::Button::with_label("Open in Editor");
    header.pack_end(&open_external);

    window.set_titlebar(Some(&header));

    // Info bar showing file path
    let path_label = gtk::Label::new(Some(&format!("Editing: {}", config_path.display())));
    path_label.set_margin_start(8);
    path_label.set_margin_end(8);
    path_label.set_margin_top(4);
    path_label.set_margin_bottom(4);
    path_label.set_halign(gtk::Align::Start);
    path_label.add_css_class("dim-label");
    vbox.append(&path_label);

    // Text view for TOML editing
    let scrolled = gtk::ScrolledWindow::builder()
        .vexpand(true)
        .hexpand(true)
        .build();

    let text_view = gtk::TextView::new();
    text_view.set_monospace(true);
    text_view.set_left_margin(8);
    text_view.set_right_margin(8);
    text_view.set_top_margin(8);
    text_view.set_bottom_margin(8);
    text_view.set_wrap_mode(gtk::WrapMode::None);

    // Load current config content
    let buffer = text_view.buffer();
    match std::fs::read_to_string(&config_path) {
        Ok(contents) => buffer.set_text(&contents),
        Err(e) => {
            error!("Could not read config: {e}");
            buffer.set_text(&format!("# Could not read config: {e}\n# Creating new config\n\n"));
        }
    }

    scrolled.set_child(Some(&text_view));
    vbox.append(&scrolled);

    // Status bar
    let status = gtk::Label::new(Some(""));
    status.set_margin_start(8);
    status.set_margin_end(8);
    status.set_margin_top(4);
    status.set_margin_bottom(4);
    status.set_halign(gtk::Align::Start);
    vbox.append(&status);

    window.set_child(Some(&vbox));

    // Save button handler
    let config_path_save = config_path.clone();
    let buffer_ref = buffer.clone();
    let status_ref = status.clone();
    save_button.connect_clicked(move |_| {
        let text = buffer_ref.text(&buffer_ref.start_iter(), &buffer_ref.end_iter(), false);
        let text_str = text.as_str();

        // Validate TOML before saving
        match toml::from_str::<config::Config>(text_str) {
            Ok(_) => {
                // Ensure parent dir exists
                if let Some(parent) = config_path_save.parent() {
                    let _ = std::fs::create_dir_all(parent);
                }
                match std::fs::write(&config_path_save, text_str) {
                    Ok(()) => {
                        info!("Config saved to {}", config_path_save.display());
                        status_ref.set_text("Saved. Config will reload automatically.");
                        status_ref.remove_css_class("error");
                    }
                    Err(e) => {
                        error!("Failed to save config: {e}");
                        status_ref.set_text(&format!("Error saving: {e}"));
                        status_ref.add_css_class("error");
                    }
                }
            }
            Err(e) => {
                status_ref.set_text(&format!("Invalid TOML: {e}"));
                status_ref.add_css_class("error");
            }
        }
    });

    // Open in external editor
    let config_path_ext = config_path.clone();
    open_external.connect_clicked(move |_| {
        let editor = std::env::var("EDITOR").unwrap_or_else(|_| "xdg-open".to_string());
        if let Err(e) = std::process::Command::new(&editor)
            .arg(&config_path_ext)
            .spawn()
        {
            error!("Failed to open editor ({editor}): {e}");
        }
    });

    window.present();
    info!("Settings window opened");
}
