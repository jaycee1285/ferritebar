use gtk::prelude::*;
use tokio::sync::mpsc;
use tracing::debug;

use crate::config::types::MemoryConfig;
use crate::theme::ThemeColors;
use crate::widgets::mini_bar::MiniBar;

#[derive(Debug)]
struct MemoryData {
    used_bytes: u64,
    total_bytes: u64,
    fraction: f64,
}

fn read_memory() -> Option<MemoryData> {
    let contents = std::fs::read_to_string("/proc/meminfo").ok()?;
    let mut total: u64 = 0;
    let mut available: u64 = 0;

    for line in contents.lines() {
        if let Some(rest) = line.strip_prefix("MemTotal:") {
            total = parse_kb(rest)?;
        } else if let Some(rest) = line.strip_prefix("MemAvailable:") {
            available = parse_kb(rest)?;
        }
    }

    if total == 0 {
        return None;
    }

    let used = total.saturating_sub(available);
    let fraction = used as f64 / total as f64;

    Some(MemoryData {
        used_bytes: used * 1024,
        total_bytes: total * 1024,
        fraction,
    })
}

fn parse_kb(s: &str) -> Option<u64> {
    s.trim()
        .trim_end_matches("kB")
        .trim()
        .parse::<u64>()
        .ok()
}

fn format_bytes(bytes: u64) -> String {
    let gib = bytes as f64 / (1024.0 * 1024.0 * 1024.0);
    if gib >= 1.0 {
        format!("{gib:.1} GiB")
    } else {
        let mib = bytes as f64 / (1024.0 * 1024.0);
        format!("{mib:.0} MiB")
    }
}

pub fn build(config: &MemoryConfig, colors: &ThemeColors) -> gtk::Widget {
    let (tx, rx) = mpsc::channel::<MemoryData>(8);

    let interval_secs = config.interval;

    crate::spawn(async move {
        let mut interval =
            tokio::time::interval(tokio::time::Duration::from_secs(interval_secs));
        loop {
            interval.tick().await;
            if let Some(data) = read_memory() {
                if tx.send(data).await.is_err() {
                    break;
                }
            }
        }
    });

    let container = gtk::Box::new(gtk::Orientation::Horizontal, 2);
    container.add_css_class("module");
    container.add_css_class("memory");
    container.set_margin_start(0);
    container.set_margin_end(0);

    // Icon
    let icon_label = gtk::Label::new(Some("\u{f538}")); // fa-memory
    icon_label.add_css_class("module-label");
    container.append(&icon_label);

    // Vertical mini bar
    let mini_bar = MiniBar::new(config.bar_width, config.bar_height, colors, true);
    container.append(mini_bar.widget());

    let container_ref = container.clone();
    super::recv_on_main_thread(rx, move |data| {
        mini_bar.set_fraction(data.fraction);

        let tooltip = format!(
            "Memory: {} / {} ({:.0}%)",
            format_bytes(data.used_bytes),
            format_bytes(data.total_bytes),
            data.fraction * 100.0
        );
        container_ref.set_tooltip_text(Some(&tooltip));
    });

    debug!("Memory module created");
    container.upcast()
}
