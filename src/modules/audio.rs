use std::fmt::Write;

use gtk::prelude::*;
use tokio::sync::mpsc;
use tracing::debug;

use crate::config::types::AudioConfig;

#[derive(Debug)]
struct AudioData {
    volume: u32,
    muted: bool,
}

/// Read audio state via wpctl (PipeWire/WirePlumber)
async fn read_audio() -> Option<AudioData> {
    let output = tokio::process::Command::new("wpctl")
        .args(["get-volume", "@DEFAULT_SINK@"])
        .output()
        .await
        .ok()?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    // Output format: "Volume: 0.50" or "Volume: 0.50 [MUTED]"
    let muted = stdout.contains("[MUTED]");
    let volume = stdout
        .split_whitespace()
        .nth(1)?
        .parse::<f64>()
        .ok()
        .map(|v| (v * 100.0) as u32)?;

    Some(AudioData { volume, muted })
}

fn audio_icon(volume: u32, muted: bool) -> &'static str {
    if muted {
        "\u{f6a9}" // fa-volume-xmark
    } else if volume > 66 {
        "\u{f028}" // fa-volume-high
    } else if volume > 33 {
        "\u{f027}" // fa-volume-low
    } else if volume > 0 {
        "\u{f026}" // fa-volume-off
    } else {
        "\u{f6a9}" // fa-volume-xmark
    }
}

pub fn build(config: &AudioConfig) -> gtk::Widget {
    let (tx, rx) = mpsc::channel::<AudioData>(8);

    // Spawn controller - poll every 1s
    crate::spawn(async move {
        let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(1));
        loop {
            interval.tick().await;
            if let Some(data) = read_audio().await {
                if tx.send(data).await.is_err() {
                    break;
                }
            }
        }
    });

    // Build widget
    let container = gtk::Box::new(gtk::Orientation::Horizontal, 4);
    container.add_css_class("module");
    container.add_css_class("audio");

    let label = gtk::Label::new(None);
    label.add_css_class("module-label");
    container.append(&label);

    let format = config.format.clone();

    // Click to mute
    let gesture = gtk::GestureClick::new();
    let on_click = config.on_click.clone();
    gesture.connect_released(move |_, _, _, _| {
        let cmd = on_click.clone();
        crate::spawn(async move {
            let _ = tokio::process::Command::new("sh")
                .arg("-c")
                .arg(&cmd)
                .spawn();
        });
    });
    container.add_controller(gesture);

    // Bridge to GTK
    let container_ref = container.clone();
    let mut buf = String::with_capacity(32);
    let mut tooltip_buf = String::with_capacity(32);
    super::recv_on_main_thread(rx, move |data| {
        let icon = audio_icon(data.volume, data.muted);

        buf.clear();
        for part in format.split('{') {
            if let Some(rest) = part.strip_prefix("icon}") {
                buf.push_str(icon);
                buf.push_str(rest);
            } else if let Some(rest) = part.strip_prefix("volume}") {
                let _ = write!(buf, "{}", data.volume);
                buf.push_str(rest);
            } else {
                buf.push_str(part);
            }
        }
        label.set_label(&buf);

        if data.muted {
            container_ref.add_css_class("muted");
        } else {
            container_ref.remove_css_class("muted");
        }

        tooltip_buf.clear();
        let _ = write!(tooltip_buf, "Volume: {}%", data.volume);
        if data.muted {
            tooltip_buf.push_str(" (Muted)");
        }
        super::set_tooltip_text(container_ref.clone(), Some(&tooltip_buf));
    });

    debug!("Audio module created");
    container.upcast()
}
