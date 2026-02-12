use gtk::prelude::*;
use tokio::sync::mpsc;
use tracing::debug;

use crate::config::types::ClockConfig;

struct ClockData {
    display: String,
    tooltip: String,
}

pub fn build(config: &ClockConfig) -> gtk::Widget {
    let (tx, rx) = mpsc::channel::<ClockData>(8);

    let format = config.format.clone();
    let tooltip_format = config.tooltip_format.clone();

    // Spawn controller
    crate::spawn(async move {
        let mut interval = tokio::time::interval(tokio::time::Duration::from_millis(500));
        loop {
            interval.tick().await;
            let now = chrono::Local::now();
            let data = ClockData {
                display: now.format(&format).to_string(),
                tooltip: now.format(&tooltip_format).to_string(),
            };
            if tx.send(data).await.is_err() {
                break;
            }
        }
    });

    // Build widget
    let container = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    container.add_css_class("module");
    container.add_css_class("clock");

    let label = gtk::Label::new(None);
    label.add_css_class("module-label");
    container.append(&label);

    // Handle clicks
    if let Some(ref cmd) = config.on_click {
        let gesture = gtk::GestureClick::new();
        let cmd = cmd.clone();
        gesture.connect_released(move |_, _, _, _| {
            let cmd = cmd.clone();
            crate::spawn(async move {
                let _ = tokio::process::Command::new("sh")
                    .arg("-c")
                    .arg(&cmd)
                    .spawn();
            });
        });
        container.add_controller(gesture);
    }

    // Bridge to GTK
    let container_ref = container.clone();
    super::recv_on_main_thread(rx, move |data| {
        label.set_label(&data.display);
        container_ref.set_tooltip_text(Some(&data.tooltip));
    });

    debug!("Clock module created");
    container.upcast()
}
