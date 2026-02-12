use gtk::prelude::*;
use tokio::sync::mpsc;
use tracing::debug;

use crate::config::types::NetworkConfig;

#[derive(Debug)]
struct NetworkData {
    connected: bool,
    interface: String,
    kind: NetKind,
    ssid: Option<String>,
}

#[derive(Debug)]
enum NetKind {
    Wifi,
    Ethernet,
    None,
}

async fn read_network() -> NetworkData {
    // Try nmcli first (works with NetworkManager)
    if let Some(data) = try_nmcli().await {
        return data;
    }

    // Fallback: scan /sys/class/net for any 'up' interface
    if let Some(data) = try_sysfs().await {
        return data;
    }

    NetworkData {
        connected: false,
        interface: String::new(),
        kind: NetKind::None,
        ssid: None,
    }
}

async fn try_nmcli() -> Option<NetworkData> {
    let output = tokio::process::Command::new("nmcli")
        .args(["-t", "-f", "TYPE,STATE,CONNECTION,DEVICE", "device", "status"])
        .output()
        .await
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    for line in stdout.lines() {
        let parts: Vec<&str> = line.split(':').collect();
        if parts.len() < 4 {
            continue;
        }
        let dev_type = parts[0];
        let state = parts[1];
        let connection = parts[2];
        let device = parts[3];

        if state != "connected" {
            continue;
        }

        if dev_type == "wifi" {
            // Get SSID from active wifi connection
            let ssid = get_wifi_ssid(device).await.or_else(|| {
                if !connection.is_empty() && connection != "--" {
                    Some(connection.to_string())
                } else {
                    None
                }
            });
            return Some(NetworkData {
                connected: true,
                interface: device.to_string(),
                kind: NetKind::Wifi,
                ssid,
            });
        } else if dev_type == "ethernet" {
            return Some(NetworkData {
                connected: true,
                interface: device.to_string(),
                kind: NetKind::Ethernet,
                ssid: None,
            });
        }
    }

    None
}

async fn get_wifi_ssid(device: &str) -> Option<String> {
    let output = tokio::process::Command::new("nmcli")
        .args(["-t", "-f", "active,ssid", "dev", "wifi", "list", "ifname", device])
        .output()
        .await
        .ok()?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    for line in stdout.lines() {
        if let Some(ssid) = line.strip_prefix("yes:") {
            if !ssid.is_empty() {
                return Some(ssid.to_string());
            }
        }
    }
    None
}

async fn try_sysfs() -> Option<NetworkData> {
    let entries = std::fs::read_dir("/sys/class/net").ok()?;

    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if name == "lo" {
            continue;
        }
        if let Ok(state) = std::fs::read_to_string(entry.path().join("operstate")) {
            if state.trim() == "up" {
                // Detect wifi vs ethernet by checking for wireless subdir
                let is_wifi = entry.path().join("wireless").exists()
                    || name.starts_with('w');
                let kind = if is_wifi {
                    NetKind::Wifi
                } else {
                    NetKind::Ethernet
                };

                // Try to get SSID via iw for wifi interfaces
                let ssid = if is_wifi {
                    get_iw_ssid(&name).await
                } else {
                    None
                };

                return Some(NetworkData {
                    connected: true,
                    interface: name,
                    kind,
                    ssid,
                });
            }
        }
    }

    None
}

async fn get_iw_ssid(interface: &str) -> Option<String> {
    let output = tokio::process::Command::new("iw")
        .args(["dev", interface, "info"])
        .output()
        .await
        .ok()?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    for line in stdout.lines() {
        let trimmed = line.trim();
        if let Some(ssid) = trimmed.strip_prefix("ssid ") {
            return Some(ssid.to_string());
        }
    }
    None
}

fn network_icon(data: &NetworkData) -> &'static str {
    match data.kind {
        NetKind::Wifi if data.connected => "\u{f1eb}",     // fa-wifi
        NetKind::Ethernet if data.connected => "\u{f796}", // fa-ethernet
        _ => "\u{f071}",                                    // fa-triangle-exclamation
    }
}

pub fn build(config: &NetworkConfig) -> gtk::Widget {
    let (tx, rx) = mpsc::channel::<NetworkData>(8);

    let interval_secs = config.interval;

    crate::spawn(async move {
        let mut interval =
            tokio::time::interval(tokio::time::Duration::from_secs(interval_secs));
        loop {
            interval.tick().await;
            let data = read_network().await;
            if tx.send(data).await.is_err() {
                break;
            }
        }
    });

    let container = gtk::Box::new(gtk::Orientation::Horizontal, 4);
    container.add_css_class("module");
    container.add_css_class("network");

    let label = gtk::Label::new(None);
    label.add_css_class("module-label");
    container.append(&label);

    // Click handler
    if let Some(ref cmd) = config.on_click {
        let gesture = gtk::GestureClick::new();
        let on_click = cmd.clone();
        gesture.connect_released(move |_, _, _, _| {
            let cmd = on_click.clone();
            crate::spawn(async move {
                let _ = tokio::process::Command::new("sh")
                    .arg("-lc")
                    .arg(&cmd)
                    .spawn();
            });
        });
        container.add_controller(gesture);
    }

    let format = config.format.clone();

    let container_ref = container.clone();
    super::recv_on_main_thread(rx, move |data| {
        let icon = network_icon(&data);
        let text = format.replace("{icon}", icon);
        label.set_label(&text);

        if data.connected {
            container_ref.remove_css_class("disconnected");
            container_ref.add_css_class("connected");
        } else {
            container_ref.remove_css_class("connected");
            container_ref.add_css_class("disconnected");
        }

        let tooltip = match (&data.kind, &data.ssid) {
            (NetKind::Wifi, Some(ssid)) => format!("WiFi: {ssid} ({})", data.interface),
            (NetKind::Wifi, None) => format!("WiFi: {} (connected)", data.interface),
            (NetKind::Ethernet, _) => format!("Ethernet: {}", data.interface),
            _ => "Disconnected".to_string(),
        };
        container_ref.set_tooltip_text(Some(&tooltip));
    });

    debug!("Network module created");
    container.upcast()
}
