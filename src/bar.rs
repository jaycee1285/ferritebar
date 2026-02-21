use gtk::prelude::*;
use gtk_layer_shell::LayerShell;
use tracing::debug;

use crate::config::types::{BarConfig, Position};

pub struct Bar {
    window: gtk::ApplicationWindow,
    start: gtk::Box,
    center: gtk::Box,
    end: gtk::Box,
}

impl Bar {
    pub fn new(app: &gtk::Application, config: &BarConfig) -> Self {
        let window = gtk::ApplicationWindow::builder()
            .application(app)
            .default_width(0)
            .default_height(config.height as i32)
            .build();

        // Initialize layer shell
        window.init_layer_shell();
        window.set_layer(gtk_layer_shell::Layer::Top);
        window.set_namespace(Some("ferritebar"));
        window.auto_exclusive_zone_enable();
        // Allow popups (power menu, tooltips) to grab keyboard when needed
        window.set_keyboard_mode(gtk_layer_shell::KeyboardMode::OnDemand);

        // Anchor to edges based on position
        match config.position {
            Position::Top => {
                window.set_anchor(gtk_layer_shell::Edge::Top, true);
            }
            Position::Bottom => {
                window.set_anchor(gtk_layer_shell::Edge::Bottom, true);
            }
        }
        window.set_anchor(gtk_layer_shell::Edge::Left, true);
        window.set_anchor(gtk_layer_shell::Edge::Right, true);

        // Apply margins
        window.set_margin(gtk_layer_shell::Edge::Top, config.margin.top);
        window.set_margin(gtk_layer_shell::Edge::Bottom, config.margin.bottom);
        window.set_margin(gtk_layer_shell::Edge::Left, config.margin.left);
        window.set_margin(gtk_layer_shell::Edge::Right, config.margin.right);

        // Create layout containers
        let start = gtk::Box::new(gtk::Orientation::Horizontal, 4);
        start.set_halign(gtk::Align::Start);

        let center = gtk::Box::new(gtk::Orientation::Horizontal, 4);
        center.set_halign(gtk::Align::Center);

        let end = gtk::Box::new(gtk::Orientation::Horizontal, 4);
        end.set_halign(gtk::Align::End);

        let center_box = gtk::CenterBox::new();
        center_box.set_start_widget(Some(&start));
        center_box.set_center_widget(Some(&center));
        center_box.set_end_widget(Some(&end));
        center_box.add_css_class("bar-container");

        window.set_child(Some(&center_box));

        debug!(
            "Created bar: position={:?}, height={}",
            config.position, config.height
        );

        Self {
            window,
            start,
            center,
            end,
        }
    }

    pub fn start_container(&self) -> &gtk::Box {
        &self.start
    }

    pub fn center_container(&self) -> &gtk::Box {
        &self.center
    }

    pub fn end_container(&self) -> &gtk::Box {
        &self.end
    }

    /// Remove all module widgets from containers (for reload)
    pub fn clear(&self) {
        while let Some(child) = self.start.first_child() {
            self.start.remove(&child);
        }
        while let Some(child) = self.center.first_child() {
            self.center.remove(&child);
        }
        while let Some(child) = self.end.first_child() {
            self.end.remove(&child);
        }
        debug!("Cleared all bar modules");
    }

    pub fn window(&self) -> &gtk::ApplicationWindow {
        &self.window
    }

    pub fn show(&self) {
        self.window.present();
    }
}
