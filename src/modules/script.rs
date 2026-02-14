use gtk::prelude::*;
use tokio::sync::mpsc;
use tracing::debug;

use crate::config::types::ScriptConfig;

#[derive(Debug)]
struct ScriptOutput {
    text: String,
    tooltip: Option<String>,
    class: Option<String>,
}

async fn run_script(exec: &str) -> Option<ScriptOutput> {
    let output = tokio::process::Command::new("sh")
        .arg("-c")
        .arg(exec)
        .output()
        .await
        .ok()?;

    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if stdout.is_empty() {
        return None;
    }

    // Try JSON parse first
    if let Ok(json) = serde_json::from_str::<serde_json::Value>(&stdout) {
        let text = json
            .get("text")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let tooltip = json
            .get("tooltip")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let class = json
            .get("class")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        Some(ScriptOutput {
            text,
            tooltip,
            class,
        })
    } else {
        // Plain text fallback
        Some(ScriptOutput {
            text: stdout,
            tooltip: None,
            class: None,
        })
    }
}

pub fn build(config: &ScriptConfig) -> gtk::Widget {
    let (tx, rx) = mpsc::channel::<ScriptOutput>(8);

    let exec = config.exec.clone();
    let interval_secs = config.interval;

    crate::spawn(async move {
        let mut interval =
            tokio::time::interval(tokio::time::Duration::from_secs(interval_secs));
        loop {
            interval.tick().await;
            if let Some(data) = run_script(&exec).await {
                if tx.send(data).await.is_err() {
                    break;
                }
            }
        }
    });

    let container = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    container.add_css_class("module");
    container.add_css_class("script");
    container.add_css_class(&format!("script-{}", config.name));

    // Optional static icon
    if let Some(ref icon) = config.icon {
        let icon_label = gtk::Label::new(Some(icon));
        icon_label.add_css_class("module-label");
        container.append(&icon_label);
    }

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

    let container_ref = container.clone();
    let mut prev_class: Option<String> = None;

    super::recv_on_main_thread(rx, move |mut data| {
        label.set_label(&data.text);

        if let Some(ref tooltip) = data.tooltip {
            super::set_tooltip_text(container_ref.clone(), Some(tooltip));
        } else {
            super::set_tooltip_text(container_ref.clone(), None);
        }

        // Remove previous dynamic class
        if let Some(ref old) = prev_class {
            container_ref.remove_css_class(old);
        }

        // Add new dynamic class
        if let Some(ref class) = data.class {
            container_ref.add_css_class(class);
        }

        // Take ownership instead of cloning
        prev_class = std::mem::take(&mut data.class);
    });

    debug!("Script module '{}' created", config.name);
    container.upcast()
}
