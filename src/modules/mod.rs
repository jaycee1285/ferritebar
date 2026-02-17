pub mod clock;
pub mod memory;
mod meminfo;
pub mod swap;
pub mod script;
pub mod battery;
pub mod audio;
pub mod network;
pub mod power;
pub mod tray;
pub mod taskbar;
pub mod workspaces;

use gtk::prelude::*;
use tracing::debug;

use crate::bar::Bar;
use crate::config::types::{ModuleConfig, ModuleLayout};
use crate::theme::ThemeColors;
use std::cell::{Cell, RefCell};

const TOOLTIP_STATE_KEY: &str = "ferritebar-tooltip-state";

struct TooltipState {
    text: RefCell<String>,
    connected: Cell<bool>,
}

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

/// Set tooltip text using a custom widget to ensure our CSS applies.
pub fn set_tooltip_text<W: IsA<gtk::Widget>>(widget: W, text: Option<&str>) {
    let widget = widget.upcast::<gtk::Widget>();
    let state = ensure_tooltip_state(&widget);

    if let Some(text) = text {
        widget.set_has_tooltip(true);
        state.text.replace(text.to_string());

        if !state.connected.get() {
            widget.connect_query_tooltip(|widget, _, _, _, tooltip| {
                if let Some(state) = tooltip_state(widget) {
                    let text = state.text.borrow();
                    if text.is_empty() {
                        return false;
                    }

                    let label = gtk::Label::new(Some(text.as_str()));
                    label.add_css_class("ferrite-tooltip");
                    tooltip.set_custom(Some(&label));
                    return true;
                }
                false
            });
            state.connected.set(true);
        }
    } else {
        widget.set_has_tooltip(false);
        state.text.replace(String::new());
    }
}

fn tooltip_state(widget: &gtk::Widget) -> Option<&TooltipState> {
    // SAFETY: TooltipState is stored on the widget and lives for the widget's lifetime.
    unsafe { widget.data::<TooltipState>(TOOLTIP_STATE_KEY).map(|ptr| ptr.as_ref()) }
}

fn ensure_tooltip_state(widget: &gtk::Widget) -> &TooltipState {
    if let Some(state) = tooltip_state(widget) {
        return state;
    }

    // SAFETY: Data is only set once per widget, and TooltipState is owned by the widget.
    unsafe {
        widget.set_data(
            TOOLTIP_STATE_KEY,
            TooltipState {
                text: RefCell::new(String::new()),
                connected: Cell::new(false),
            },
        );
    }
    tooltip_state(widget).expect("tooltip state should be initialized")
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
        ModuleConfig::Workspaces(cfg) => Some(workspaces::build(cfg)),
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

    // Diagnostic: read computed label color after a brief delay (once realized)
    let start = bar.start_container().clone();
    let end = bar.end_container().clone();
    glib::timeout_add_local_once(std::time::Duration::from_millis(500), move || {
        for (name, container) in [("start", &start), ("end", &end)] {
            let mut child = container.first_child();
            while let Some(widget) = child {
                // Check label children for computed color
                if let Some(inner) = widget.first_child() {
                    let color = inner.color();
                    let css_classes = inner.css_classes();
                    debug!(
                        "Computed color for {name} widget (classes: {:?}): rgba({:.0},{:.0},{:.0},{:.2})",
                        css_classes,
                        color.red() * 255.0,
                        color.green() * 255.0,
                        color.blue() * 255.0,
                        color.alpha()
                    );
                }
                child = widget.next_sibling();
            }
        }
    });
}
