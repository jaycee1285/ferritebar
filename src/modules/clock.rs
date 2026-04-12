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
        let mut interval = tokio::time::interval(tokio::time::Duration::from_millis(1000));
        let mut prev_display = String::new();
        let mut display_buf = String::with_capacity(32);
        let mut tooltip_buf = String::with_capacity(64);
        loop {
            interval.tick().await;
            let now = chrono::Local::now();

            display_buf.clear();
            let _ = std::fmt::Write::write_fmt(
                &mut display_buf,
                format_args!("{}", now.format(&format)),
            );

            // Skip send if display hasn't changed
            if display_buf == prev_display {
                continue;
            }
            prev_display.clear();
            prev_display.push_str(&display_buf);

            tooltip_buf.clear();
            let _ = std::fmt::Write::write_fmt(
                &mut tooltip_buf,
                format_args!("{}", now.format(&tooltip_format)),
            );

            let data = ClockData {
                display: display_buf.clone(),
                tooltip: tooltip_buf.clone(),
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
        super::set_tooltip_text(container_ref.clone(), Some(&data.tooltip));
    });

    debug!("Clock module created");
    container.upcast()
}
