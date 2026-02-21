use gtk::prelude::*;
use gtk_layer_shell::LayerShell;
use std::cell::Cell;
use std::rc::Rc;
use tokio::sync::mpsc;
use tracing::debug;

use crate::config::types::PowerConfig;

fn exec_command(cmd: &str) {
    let cmd = cmd.to_string();
    crate::spawn(async move {
        let _ = tokio::process::Command::new("sh")
            .arg("-c")
            .arg(&cmd)
            .spawn();
    });
}

fn make_power_btn(icon: &str, label: &str) -> gtk::Button {
    let btn = gtk::Button::new();
    let content = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let icon_label = gtk::Label::new(Some(icon));
    icon_label.add_css_class("module-label");
    icon_label.set_width_chars(2);
    let text_label = gtk::Label::new(Some(label));
    content.append(&icon_label);
    content.append(&text_label);
    btn.set_child(Some(&content));
    btn
}

/// Set up a standalone power menu window triggered by `ferritebar msg power`.
/// No bar module — just an overlay layer-shell surface that appears centered.
/// Keyboard navigation is handled manually since GTK focus doesn't work
/// reliably on layer-shell surfaces.
pub fn setup(app: &gtk::Application, config: &PowerConfig, bar_window: &gtk::ApplicationWindow) {
    let window = gtk::ApplicationWindow::builder()
        .application(app)
        .default_width(200)
        .default_height(0)
        .build();

    window.init_layer_shell();
    window.set_layer(gtk_layer_shell::Layer::Overlay);
    window.set_namespace(Some("ferritebar-power"));
    window.set_keyboard_mode(gtk_layer_shell::KeyboardMode::Exclusive);

    // Don't anchor to any edge — centers on screen
    window.set_anchor(gtk_layer_shell::Edge::Top, false);
    window.set_anchor(gtk_layer_shell::Edge::Bottom, false);
    window.set_anchor(gtk_layer_shell::Edge::Left, false);
    window.set_anchor(gtk_layer_shell::Edge::Right, false);

    let menu_box = gtk::Box::new(gtk::Orientation::Vertical, 2);
    menu_box.add_css_class("power-popover");

    // Collect buttons + commands so we can navigate by index
    let mut buttons: Vec<gtk::Button> = Vec::new();
    let mut commands: Vec<String> = Vec::new();

    let lock_btn = make_power_btn("\u{f023}", "1  Lock");
    buttons.push(lock_btn.clone());
    commands.push(config.lock_cmd.clone());
    menu_box.append(&lock_btn);

    let suspend_btn = make_power_btn("\u{f186}", "2  Suspend");
    buttons.push(suspend_btn.clone());
    commands.push(config.suspend_cmd.clone());
    menu_box.append(&suspend_btn);

    let reboot_btn = make_power_btn("\u{f2f1}", "3  Reboot");
    buttons.push(reboot_btn.clone());
    commands.push(config.reboot_cmd.clone());
    menu_box.append(&reboot_btn);

    let shutdown_btn = make_power_btn("\u{f011}", "4  Shutdown");
    buttons.push(shutdown_btn.clone());
    commands.push(config.shutdown_cmd.clone());
    menu_box.append(&shutdown_btn);

    if let Some(ref cmd) = config.logout_cmd {
        let logout_btn = make_power_btn("\u{f2f5}", "5  Logout");
        buttons.push(logout_btn.clone());
        commands.push(cmd.clone());
        menu_box.append(&logout_btn);
    }

    window.set_child(Some(&menu_box));

    let buttons = Rc::new(buttons);
    let commands = Rc::new(commands);
    let selected = Rc::new(Cell::new(0usize));

    // Highlight the selected button
    let buttons_hl = buttons.clone();
    let selected_hl = selected.clone();
    let highlight = Rc::new(move || {
        let idx = selected_hl.get();
        for (i, btn) in buttons_hl.iter().enumerate() {
            if i == idx {
                btn.add_css_class("active");
            } else {
                btn.remove_css_class("active");
            }
        }
    });

    // Dismiss helper: hide power menu, restore bar
    let bar_win = bar_window.clone();
    let power_win = window.clone();
    let dismiss = Rc::new(move || {
        power_win.set_visible(false);
        power_win.set_keyboard_mode(gtk_layer_shell::KeyboardMode::None);
        bar_win.set_visible(true);
    });

    // Wire click handlers on each button
    for (i, btn) in buttons.iter().enumerate() {
        let d = dismiss.clone();
        let cmd = commands[i].clone();
        btn.connect_clicked(move |_| { d(); exec_command(&cmd); });
    }

    // Keyboard navigation: Up/Down/Tab move selection, Enter activates, Escape dismisses
    let key_ctrl = gtk::EventControllerKey::new();
    let sel = selected.clone();
    let btns = buttons.clone();
    let hl = highlight.clone();
    let d = dismiss.clone();
    let count = btns.len();
    key_ctrl.connect_key_pressed(move |_, key, _, _| {
        match key {
            gtk::gdk::Key::Escape => {
                d();
                return glib::Propagation::Stop;
            }
            // Number keys: 1=Lock, 2=Suspend, 3=Reboot, 4=Shutdown, 5=Logout
            gtk::gdk::Key::_1 | gtk::gdk::Key::KP_1 => {
                if count > 0 { btns[0].emit_clicked(); }
                return glib::Propagation::Stop;
            }
            gtk::gdk::Key::_2 | gtk::gdk::Key::KP_2 => {
                if count > 1 { btns[1].emit_clicked(); }
                return glib::Propagation::Stop;
            }
            gtk::gdk::Key::_3 | gtk::gdk::Key::KP_3 => {
                if count > 2 { btns[2].emit_clicked(); }
                return glib::Propagation::Stop;
            }
            gtk::gdk::Key::_4 | gtk::gdk::Key::KP_4 => {
                if count > 3 { btns[3].emit_clicked(); }
                return glib::Propagation::Stop;
            }
            gtk::gdk::Key::_5 | gtk::gdk::Key::KP_5 => {
                if count > 4 { btns[4].emit_clicked(); }
                return glib::Propagation::Stop;
            }
            // Arrow/Tab navigation still available
            gtk::gdk::Key::Up | gtk::gdk::Key::KP_Up | gtk::gdk::Key::ISO_Left_Tab => {
                let cur = sel.get();
                sel.set(if cur == 0 { count - 1 } else { cur - 1 });
                hl();
                return glib::Propagation::Stop;
            }
            gtk::gdk::Key::Down | gtk::gdk::Key::KP_Down | gtk::gdk::Key::Tab => {
                let cur = sel.get();
                sel.set((cur + 1) % count);
                hl();
                return glib::Propagation::Stop;
            }
            gtk::gdk::Key::Return | gtk::gdk::Key::KP_Enter | gtk::gdk::Key::space => {
                btns[sel.get()].emit_clicked();
                return glib::Propagation::Stop;
            }
            _ => {}
        }
        glib::Propagation::Proceed
    });
    window.add_controller(key_ctrl);

    // Start hidden
    window.set_visible(false);

    // IPC: toggle on `ferritebar msg power`
    let (ipc_tx, ipc_rx) = mpsc::channel::<()>(4);
    let mut ipc_sub = crate::ipc::subscribe();
    crate::spawn(async move {
        loop {
            match ipc_sub.recv().await {
                Ok(msg) if msg == "power" => { let _ = ipc_tx.send(()).await; }
                Ok(_) => {}
                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
            }
        }
    });
    let w = window.clone();
    let bar_win = bar_window.clone();
    let d = dismiss.clone();
    crate::modules::recv_on_main_thread(ipc_rx, move |_| {
        if w.is_visible() {
            d();
        } else {
            // Reset selection to first item
            selected.set(0);
            highlight();
            // Hide bar so power menu is the only surface
            bar_win.set_visible(false);
            w.set_keyboard_mode(gtk_layer_shell::KeyboardMode::Exclusive);
            w.present();
        }
    });

    debug!("Power menu ready (IPC-only, keyboard-navigable)");
}
