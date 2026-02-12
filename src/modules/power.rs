use gtk::prelude::*;
use tracing::debug;

use crate::config::types::PowerConfig;

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

fn exec_command(cmd: &str) {
    let cmd = cmd.to_string();
    crate::spawn(async move {
        let _ = tokio::process::Command::new("sh")
            .arg("-c")
            .arg(&cmd)
            .spawn();
    });
}

pub fn build(config: &PowerConfig) -> gtk::Widget {
    let container = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    container.add_css_class("module");
    container.add_css_class("power");

    // Icon label using FA font stack
    let icon_label = gtk::Label::new(Some(&config.icon));
    icon_label.add_css_class("module-label");
    icon_label.add_css_class("power-icon");
    container.append(&icon_label);

    // Popover with power actions
    let popover = gtk::Popover::new();
    popover.set_parent(&container);

    let popover_box = gtk::Box::new(gtk::Orientation::Vertical, 2);
    popover_box.add_css_class("power-popover");

    let lock_btn = make_power_btn("\u{f023}", "Lock");       // fa-lock
    let suspend_btn = make_power_btn("\u{f186}", "Suspend");   // fa-moon
    let reboot_btn = make_power_btn("\u{f2f1}", "Reboot");     // fa-arrows-rotate
    let shutdown_btn = make_power_btn("\u{f011}", "Shutdown");  // fa-power-off

    popover_box.append(&lock_btn);
    popover_box.append(&suspend_btn);
    popover_box.append(&reboot_btn);
    popover_box.append(&shutdown_btn);

    if let Some(ref logout_cmd) = config.logout_cmd {
        let logout_btn = make_power_btn("\u{f2f5}", "Logout"); // fa-right-from-bracket
        let cmd = logout_cmd.clone();
        let popover_ref = popover.clone();
        logout_btn.connect_clicked(move |_| {
            popover_ref.popdown();
            exec_command(&cmd);
        });
        popover_box.append(&logout_btn);
    }

    popover.set_child(Some(&popover_box));

    // Connect click handlers (close popover on action)
    let lock_cmd = config.lock_cmd.clone();
    let p = popover.clone();
    lock_btn.connect_clicked(move |_| { p.popdown(); exec_command(&lock_cmd); });

    let suspend_cmd = config.suspend_cmd.clone();
    let p = popover.clone();
    suspend_btn.connect_clicked(move |_| { p.popdown(); exec_command(&suspend_cmd); });

    let reboot_cmd = config.reboot_cmd.clone();
    let p = popover.clone();
    reboot_btn.connect_clicked(move |_| { p.popdown(); exec_command(&reboot_cmd); });

    let shutdown_cmd = config.shutdown_cmd.clone();
    let p = popover.clone();
    shutdown_btn.connect_clicked(move |_| { p.popdown(); exec_command(&shutdown_cmd); });

    // Click on the container toggles the popover
    let gesture = gtk::GestureClick::new();
    let p = popover.clone();
    gesture.connect_released(move |_, _, _, _| {
        if p.is_visible() {
            p.popdown();
        } else {
            p.popup();
        }
    });
    container.add_controller(gesture);

    debug!("Power module created");
    container.upcast()
}
