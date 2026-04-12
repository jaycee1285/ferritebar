use gtk::prelude::*;
use gtk_layer_shell::LayerShell;
use std::cell::RefCell;
use std::rc::Rc;
use tokio::sync::mpsc;
use tracing::debug;

use crate::config;
use crate::config::types::ModuleConfig;

/// An item in either the Shown or Hidden column.
#[derive(Clone)]
struct ToggleItem {
    module: ModuleConfig,
    /// Which section it came from (left/center/right)
    origin: String,
}

#[derive(Clone, Copy, PartialEq)]
enum Column {
    Shown,
    Hidden,
}

struct MenuState {
    shown: Vec<ToggleItem>,
    hidden: Vec<ToggleItem>,
    column: Column,
    index: usize,
}

impl MenuState {
    fn active_list(&self) -> &[ToggleItem] {
        match self.column {
            Column::Shown => &self.shown,
            Column::Hidden => &self.hidden,
        }
    }

    fn active_len(&self) -> usize {
        self.active_list().len()
    }

    fn clamp_index(&mut self) {
        let len = self.active_len();
        if len == 0 {
            self.index = 0;
        } else if self.index >= len {
            self.index = len - 1;
        }
    }

    /// Move the selected item to the other column
    fn toggle_selected(&mut self) {
        match self.column {
            Column::Shown => {
                if !self.shown.is_empty() {
                    let item = self.shown.remove(self.index);
                    self.hidden.push(item);
                    self.clamp_index();
                }
            }
            Column::Hidden => {
                if !self.hidden.is_empty() {
                    let item = self.hidden.remove(self.index);
                    self.shown.push(item);
                    self.clamp_index();
                }
            }
        }
    }
}

fn make_label(text: &str, active: bool) -> gtk::Label {
    let label = gtk::Label::new(Some(text));
    label.set_halign(gtk::Align::Start);
    label.set_margin_start(8);
    label.set_margin_end(8);
    label.set_margin_top(2);
    label.set_margin_bottom(2);
    if active {
        label.add_css_class("active");
    }
    label
}

fn rebuild_ui(container: &gtk::Box, state: &MenuState) {
    // Clear existing children
    while let Some(child) = container.first_child() {
        container.remove(&child);
    }

    // Shown header
    let shown_header = gtk::Label::new(Some("Shown"));
    shown_header.add_css_class("toggle-header");
    shown_header.set_halign(gtk::Align::Start);
    shown_header.set_margin_start(8);
    if state.column == Column::Shown {
        shown_header.add_css_class("toggle-header-active");
    }
    container.append(&shown_header);

    for (i, item) in state.shown.iter().enumerate() {
        let active = state.column == Column::Shown && state.index == i;
        let label = make_label(&format!("  {}", item.module.display_name()), active);
        container.append(&label);
    }

    if state.shown.is_empty() {
        let empty = gtk::Label::new(Some("  (none)"));
        empty.set_halign(gtk::Align::Start);
        empty.set_margin_start(8);
        empty.add_css_class("toggle-empty");
        container.append(&empty);
    }

    // Separator
    let sep = gtk::Separator::new(gtk::Orientation::Horizontal);
    sep.set_margin_top(4);
    sep.set_margin_bottom(4);
    container.append(&sep);

    // Hidden header
    let hidden_header = gtk::Label::new(Some("Hidden"));
    hidden_header.add_css_class("toggle-header");
    hidden_header.set_halign(gtk::Align::Start);
    hidden_header.set_margin_start(8);
    if state.column == Column::Hidden {
        hidden_header.add_css_class("toggle-header-active");
    }
    container.append(&hidden_header);

    for (i, item) in state.hidden.iter().enumerate() {
        let active = state.column == Column::Hidden && state.index == i;
        let label = make_label(&format!("  {}", item.module.display_name()), active);
        container.append(&label);
    }

    if state.hidden.is_empty() {
        let empty = gtk::Label::new(Some("  (none)"));
        empty.set_halign(gtk::Align::Start);
        empty.set_margin_start(8);
        empty.add_css_class("toggle-empty");
        container.append(&empty);
    }
}

/// Apply the current toggle state back to config.toml
fn apply_state(state: &MenuState) {
    let config_path = config::default_config_path();
    let mut cfg = config::load_config(&config_path);

    // Clear all module lists
    cfg.modules.left.clear();
    cfg.modules.center.clear();
    cfg.modules.right.clear();
    cfg.modules.hidden.clear();

    // Re-populate shown modules into their original sections
    for item in &state.shown {
        match item.origin.as_str() {
            "left" => cfg.modules.left.push(item.module.clone()),
            "center" => cfg.modules.center.push(item.module.clone()),
            _ => cfg.modules.right.push(item.module.clone()),
        }
    }

    // Hidden modules
    for item in &state.hidden {
        cfg.modules.hidden.push(item.module.clone());
    }

    config::save_config(&config_path, &cfg);
}

pub fn setup(app: &gtk::Application, cfg: &config::types::Config, bar_window: &gtk::ApplicationWindow) {
    let window = gtk::ApplicationWindow::builder()
        .application(app)
        .default_width(250)
        .default_height(0)
        .build();

    window.init_layer_shell();
    window.set_layer(gtk_layer_shell::Layer::Overlay);
    window.set_namespace(Some("ferritebar-toggle"));
    window.set_keyboard_mode(gtk_layer_shell::KeyboardMode::Exclusive);

    // Center on screen
    window.set_anchor(gtk_layer_shell::Edge::Top, false);
    window.set_anchor(gtk_layer_shell::Edge::Bottom, false);
    window.set_anchor(gtk_layer_shell::Edge::Left, false);
    window.set_anchor(gtk_layer_shell::Edge::Right, false);

    let menu_box = gtk::Box::new(gtk::Orientation::Vertical, 0);
    menu_box.add_css_class("toggle-menu");
    window.set_child(Some(&menu_box));

    // Build initial state from config
    let mut shown: Vec<ToggleItem> = Vec::new();
    for m in &cfg.modules.left {
        shown.push(ToggleItem { module: m.clone(), origin: "left".to_string() });
    }
    for m in &cfg.modules.center {
        shown.push(ToggleItem { module: m.clone(), origin: "center".to_string() });
    }
    for m in &cfg.modules.right {
        shown.push(ToggleItem { module: m.clone(), origin: "right".to_string() });
    }

    let mut hidden: Vec<ToggleItem> = Vec::new();
    for m in &cfg.modules.hidden {
        hidden.push(ToggleItem {
            module: m.clone(),
            origin: m.default_section().to_string(),
        });
    }

    let state = Rc::new(RefCell::new(MenuState {
        shown,
        hidden,
        column: Column::Shown,
        index: 0,
    }));

    // Initial render
    rebuild_ui(&menu_box, &state.borrow());

    // Dismiss helper
    let bar_win = bar_window.clone();
    let toggle_win = window.clone();
    let dismiss = Rc::new(move || {
        toggle_win.set_visible(false);
        toggle_win.set_keyboard_mode(gtk_layer_shell::KeyboardMode::None);
        bar_win.set_visible(true);
    });

    // Keyboard navigation
    let key_ctrl = gtk::EventControllerKey::new();
    let s = state.clone();
    let mb = menu_box.clone();
    let d = dismiss.clone();
    key_ctrl.connect_key_pressed(move |_, key, _, _| {
        let mut state = s.borrow_mut();
        match key {
            gtk::gdk::Key::Escape => {
                drop(state);
                d();
                return glib::Propagation::Stop;
            }
            gtk::gdk::Key::Tab | gtk::gdk::Key::ISO_Left_Tab => {
                // Switch columns
                state.column = match state.column {
                    Column::Shown => Column::Hidden,
                    Column::Hidden => Column::Shown,
                };
                state.clamp_index();
                rebuild_ui(&mb, &state);
                return glib::Propagation::Stop;
            }
            gtk::gdk::Key::Up | gtk::gdk::Key::KP_Up => {
                if state.active_len() > 0 {
                    if state.index == 0 {
                        state.index = state.active_len() - 1;
                    } else {
                        state.index -= 1;
                    }
                    rebuild_ui(&mb, &state);
                }
                return glib::Propagation::Stop;
            }
            gtk::gdk::Key::Down | gtk::gdk::Key::KP_Down => {
                if state.active_len() > 0 {
                    state.index = (state.index + 1) % state.active_len();
                    rebuild_ui(&mb, &state);
                }
                return glib::Propagation::Stop;
            }
            gtk::gdk::Key::Right | gtk::gdk::Key::space => {
                // Move selected item to other column
                state.toggle_selected();
                rebuild_ui(&mb, &state);
                return glib::Propagation::Stop;
            }
            gtk::gdk::Key::Left => {
                // Also move (in case user thinks left = "move back")
                state.toggle_selected();
                rebuild_ui(&mb, &state);
                return glib::Propagation::Stop;
            }
            gtk::gdk::Key::Return | gtk::gdk::Key::KP_Enter => {
                // Apply and dismiss
                apply_state(&state);
                drop(state);
                d();
                return glib::Propagation::Stop;
            }
            _ => {}
        }
        glib::Propagation::Proceed
    });
    window.add_controller(key_ctrl);

    // Start hidden
    window.set_visible(false);

    // IPC: toggle on `ferritebar msg toggle`
    let (ipc_tx, ipc_rx) = mpsc::channel::<()>(4);
    let mut ipc_sub = crate::ipc::subscribe();
    crate::spawn(async move {
        loop {
            match ipc_sub.recv().await {
                Ok(msg) if msg == "toggle" => {
                    let _ = ipc_tx.send(()).await;
                }
                Ok(_) => {}
                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
            }
        }
    });

    let w = window.clone();
    let bar_win = bar_window.clone();
    let d = dismiss.clone();
    let st = state.clone();
    let mb2 = menu_box.clone();
    crate::modules::recv_on_main_thread(ipc_rx, move |_| {
        if w.is_visible() {
            d();
        } else {
            // Reload state from current config before showing
            let config_path = config::default_config_path();
            let cfg = config::load_config(&config_path);

            let mut shown: Vec<ToggleItem> = Vec::new();
            for m in &cfg.modules.left {
                shown.push(ToggleItem { module: m.clone(), origin: "left".to_string() });
            }
            for m in &cfg.modules.center {
                shown.push(ToggleItem { module: m.clone(), origin: "center".to_string() });
            }
            for m in &cfg.modules.right {
                shown.push(ToggleItem { module: m.clone(), origin: "right".to_string() });
            }

            let mut hidden: Vec<ToggleItem> = Vec::new();
            for m in &cfg.modules.hidden {
                hidden.push(ToggleItem {
                    module: m.clone(),
                    origin: m.default_section().to_string(),
                });
            }

            let mut state = st.borrow_mut();
            state.shown = shown;
            state.hidden = hidden;
            state.column = Column::Shown;
            state.index = 0;
            rebuild_ui(&mb2, &state);
            drop(state);

            bar_win.set_visible(false);
            w.set_keyboard_mode(gtk_layer_shell::KeyboardMode::Exclusive);
            w.present();
        }
    });

    debug!("Toggle menu ready (IPC: ferritebar msg toggle)");
}
