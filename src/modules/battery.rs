use gtk::prelude::*;
use tokio::sync::mpsc;
use tracing::debug;

use crate::config::types::BatteryConfig;

#[derive(Debug)]
struct BatteryData {
    percentage: u8,
    status: String,
}

fn read_battery(path: &str) -> Option<BatteryData> {
    let capacity = std::fs::read_to_string(format!("{path}/capacity"))
        .ok()?
        .trim()
        .parse::<u8>()
        .ok()?;
    let status = std::fs::read_to_string(format!("{path}/status"))
        .ok()?
        .trim()
        .to_string();
    Some(BatteryData {
        percentage: capacity,
        status,
    })
}

fn battery_icon(percentage: u8, charging: bool, max_charge: u8) -> &'static str {
    if charging {
        return "\u{f1e6}"; // fa-plug
    }
    // Scale percentage relative to max_charge for icon selection
    let effective = if max_charge > 0 && max_charge < 100 {
        ((percentage as u16) * 100 / max_charge as u16).min(100) as u8
    } else {
        percentage
    };
    match effective {
        90..=100 => "\u{F240}",
        60..=89 => "\u{F241}",
        30..=59 => "\u{F242}",
        10..=29 => "\u{F243}",
        _ => "\u{F244}",
    }
}

pub fn build(config: &BatteryConfig) -> gtk::Widget {
    let (tx, rx) = mpsc::channel::<BatteryData>(8);

    let path = config.path.clone();
    let interval_secs = config.interval;

    // Spawn controller
    crate::spawn(async move {
        let mut interval =
            tokio::time::interval(tokio::time::Duration::from_secs(interval_secs));
        loop {
            interval.tick().await;
            if let Some(data) = read_battery(&path) {
                if tx.send(data).await.is_err() {
                    break;
                }
            }
        }
    });

    // Build widget — single label, no doubling
    let container = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    container.add_css_class("module");
    container.add_css_class("battery");

    let label = gtk::Label::new(None);
    label.add_css_class("module-label");
    container.append(&label);

    let format = config.format.clone();
    let max_charge = config.max_charge;

    // Bridge to GTK
    let container_ref = container.clone();
    super::recv_on_main_thread(rx, move |data| {
        let charging = data.status == "Charging";
        let icon = battery_icon(data.percentage, charging, max_charge);

        let text = format
            .replace("{icon}", icon)
            .replace("{percentage}", &data.percentage.to_string())
            .replace("{status}", &data.status);
        label.set_label(&text);

        // Update CSS classes
        container_ref.remove_css_class("charging");
        container_ref.remove_css_class("low");
        container_ref.remove_css_class("critical");

        // Scale thresholds relative to max_charge
        let effective_pct = if max_charge > 0 && max_charge < 100 {
            ((data.percentage as u16) * 100 / max_charge as u16).min(100) as u8
        } else {
            data.percentage
        };

        if charging {
            container_ref.add_css_class("charging");
        } else if effective_pct < 10 {
            container_ref.add_css_class("critical");
        } else if effective_pct < 20 {
            container_ref.add_css_class("low");
        }

        // Tooltip with percentage and status
        let tooltip = if max_charge < 100 {
            format!(
                "Battery: {}% / {}% max ({})",
                data.percentage, max_charge, data.status
            )
        } else {
            format!("Battery: {}% ({})", data.percentage, data.status)
        };
        container_ref.set_tooltip_text(Some(&tooltip));
    });

    debug!("Battery module created");
    container.upcast()
}
