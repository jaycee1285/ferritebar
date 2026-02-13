use std::fmt::Write;

use gtk::prelude::*;
use tokio::sync::mpsc;
use tracing::debug;

use crate::config::types::SwapConfig;
use crate::theme::ThemeColors;
use crate::widgets::mini_bar::MiniBar;

use super::meminfo;

#[derive(Debug)]
struct SwapData {
    used_bytes: u64,
    total_bytes: u64,
    fraction: f64,
}

fn read_swap() -> Option<SwapData> {
    let info = meminfo::read_meminfo()?;

    if info.swap_total == 0 {
        return Some(SwapData {
            used_bytes: 0,
            total_bytes: 0,
            fraction: 0.0,
        });
    }

    let used = info.swap_total.saturating_sub(info.swap_free);
    let fraction = used as f64 / info.swap_total as f64;

    Some(SwapData {
        used_bytes: used * 1024,
        total_bytes: info.swap_total * 1024,
        fraction,
    })
}

pub fn build(config: &SwapConfig, colors: &ThemeColors) -> gtk::Widget {
    let (tx, rx) = mpsc::channel::<SwapData>(8);

    let interval_secs = config.interval;

    crate::spawn(async move {
        let mut interval =
            tokio::time::interval(tokio::time::Duration::from_secs(interval_secs));
        loop {
            interval.tick().await;
            if let Some(data) = read_swap() {
                if tx.send(data).await.is_err() {
                    break;
                }
            }
        }
    });

    let container = gtk::Box::new(gtk::Orientation::Horizontal, 2);
    container.add_css_class("module");
    container.add_css_class("swap");
    container.set_margin_start(0);
    container.set_margin_end(0);

    // Icon
    let icon_label = gtk::Label::new(Some("\u{f0a0}")); // fa-hdd
    icon_label.add_css_class("module-label");
    container.append(&icon_label);

    // Vertical mini bar
    let mini_bar = MiniBar::new(config.bar_width, config.bar_height, colors, true);
    container.append(mini_bar.widget());

    let container_ref = container.clone();
    let mut tooltip_buf = String::with_capacity(64);
    super::recv_on_main_thread(rx, move |data| {
        mini_bar.set_fraction(data.fraction);

        tooltip_buf.clear();
        if data.total_bytes == 0 {
            tooltip_buf.push_str("Swap: disabled");
        } else {
            tooltip_buf.push_str("Swap: ");
            meminfo::format_bytes_into(&mut tooltip_buf, data.used_bytes);
            tooltip_buf.push_str(" / ");
            meminfo::format_bytes_into(&mut tooltip_buf, data.total_bytes);
            let _ = write!(tooltip_buf, " ({:.0}%)", data.fraction * 100.0);
        }
        container_ref.set_tooltip_text(Some(&tooltip_buf));
    });

    debug!("Swap module created");
    container.upcast()
}
