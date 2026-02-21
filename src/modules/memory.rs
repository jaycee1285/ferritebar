use std::fmt::Write;

use gtk::prelude::*;
use tokio::sync::mpsc;
use tracing::debug;

use crate::config::types::MemoryConfig;
use crate::theme::ThemeColors;
use crate::widgets::mini_bar::MiniBar;

use super::meminfo;

#[derive(Debug)]
struct MemoryData {
    used_bytes: u64,
    total_bytes: u64,
    fraction: f64,
}

fn read_memory() -> Option<MemoryData> {
    let info = meminfo::read_meminfo()?;

    if info.mem_total == 0 {
        return None;
    }

    let used = info.mem_total.saturating_sub(info.mem_available);
    let fraction = used as f64 / info.mem_total as f64;

    Some(MemoryData {
        used_bytes: used * 1024,
        total_bytes: info.mem_total * 1024,
        fraction,
    })
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
    let mut tooltip_buf = String::with_capacity(64);
    super::recv_on_main_thread(rx, move |data| {
        mini_bar.set_fraction(data.fraction);

        tooltip_buf.clear();
        tooltip_buf.push_str("Memory: ");
        meminfo::format_bytes_into(&mut tooltip_buf, data.used_bytes);
        tooltip_buf.push_str(" / ");
        meminfo::format_bytes_into(&mut tooltip_buf, data.total_bytes);
        let _ = write!(tooltip_buf, " ({:.0}%)", data.fraction * 100.0);
        super::set_tooltip_text(container_ref.clone(), Some(&tooltip_buf));
    });

    // IPC: toggle visibility when `ferritebar msg memory-toggle` is called
    let (ipc_tx, ipc_rx) = mpsc::channel::<()>(4);
    let mut ipc_sub = crate::ipc::subscribe();
    crate::spawn(async move {
        loop {
            match ipc_sub.recv().await {
                Ok(msg) if msg == "memory-toggle" => {
                    let _ = ipc_tx.send(()).await;
                }
                Ok(_) => {}
                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
            }
        }
    });
    let container_ipc = container.clone();
    super::recv_on_main_thread(ipc_rx, move |_| {
        container_ipc.set_visible(!container_ipc.is_visible());
    });

    debug!("Memory module created");
    container.upcast()
}
