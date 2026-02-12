pub mod clock;
pub mod memory;
pub mod swap;
pub mod script;
pub mod battery;
pub mod audio;
pub mod network;
pub mod power;
pub mod tray;
pub mod taskbar;

use gtk::prelude::*;
use tracing::debug;

use crate::bar::Bar;
use crate::config::types::{ModuleConfig, ModuleLayout};
use crate::theme::ThemeColors;

/// Bridge an async mpsc receiver to the GTK main thread
pub fn recv_on_main_thread<T: 'static>(
    mut rx: tokio::sync::mpsc::Receiver<T>,
    mut callback: impl FnMut(T) + 'static,
) {
    glib::spawn_future_local(async move {
        while let Some(data) = rx.recv().await {
            callback(data);
        }
    });
}

/// Create a module widget from config and append to appropriate container
fn build_module(config: &ModuleConfig, colors: &ThemeColors) -> Option<gtk::Widget> {
    match config {
        ModuleConfig::Clock(cfg) => Some(clock::build(cfg)),
        ModuleConfig::Battery(cfg) => Some(battery::build(cfg)),
        ModuleConfig::Audio(cfg) => Some(audio::build(cfg)),
        ModuleConfig::Network(cfg) => Some(network::build(cfg)),
        ModuleConfig::Memory(cfg) => Some(memory::build(cfg, colors)),
        ModuleConfig::Swap(cfg) => Some(swap::build(cfg, colors)),
        ModuleConfig::Power(cfg) => Some(power::build(cfg)),
        ModuleConfig::Script(cfg) => Some(script::build(cfg)),
        ModuleConfig::Tray(cfg) => Some(tray::build(cfg)),
        ModuleConfig::Taskbar(cfg) => Some(taskbar::build(cfg)),
    }
}

/// Populate bar containers with modules from config
pub fn populate_bar(bar: &Bar, layout: &ModuleLayout, colors: &ThemeColors) {
    for module_cfg in &layout.left {
        if let Some(widget) = build_module(module_cfg, colors) {
            bar.start_container().append(&widget);
        }
    }

    for module_cfg in &layout.center {
        if let Some(widget) = build_module(module_cfg, colors) {
            bar.center_container().append(&widget);
        }
    }

    for module_cfg in &layout.right {
        if let Some(widget) = build_module(module_cfg, colors) {
            bar.end_container().append(&widget);
        }
    }

    let total = layout.left.len() + layout.center.len() + layout.right.len();
    debug!("Populated {total} modules");
}
