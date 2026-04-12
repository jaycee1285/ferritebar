use gtk::prelude::*;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::sync::mpsc;
use tracing::{debug, warn};

use crate::config::types::AgentUsageConfig;

const CLAUDE_CLIENT_ID: &str = "9d1c250a-e61b-44d9-88ed-5944d1962f5e";
const CODEX_CLIENT_ID: &str = "app_EMoamEEZ73f0CkXaXp7hrann";

#[derive(Debug, Deserialize, Serialize)]
struct CredentialsFile {
    claude: Option<ClaudeCredentials>,
    codex: Option<CodexCredentials>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
struct ClaudeCredentials {
    access_token: String,
    refresh_token: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
struct CodexCredentials {
    access_token: String,
    refresh_token: String,
    account_id: String,
}

#[derive(Debug)]
struct UsageWindow {
    label: String,
    remaining_pct: f64,
    resets_at: Option<String>, // formatted as M/DD
}

#[derive(Debug)]
struct ServiceUsage {
    name: String,
    windows: Vec<UsageWindow>,
}

#[derive(Debug)]
struct AgentUsageData {
    tooltip: String,
}

pub fn build(config: &AgentUsageConfig) -> gtk::Widget {
    let (tx, rx) = mpsc::channel::<AgentUsageData>(8);

    let data_path = config.data_path.clone();
    let interval_secs = config.interval.max(60);

    crate::spawn(async move {
        let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(interval_secs));
        loop {
            interval.tick().await;

            let tooltip = fetch_all_usage(&data_path).await;

            if tx.send(AgentUsageData { tooltip }).await.is_err() {
                break;
            }
        }
    });

    let container = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    container.add_css_class("module");
    container.add_css_class("agent-usage");

    let label = gtk::Label::new(Some(&config.icon));
    label.add_css_class("module-label");
    container.append(&label);

    super::set_tooltip_text(container.clone(), Some("Loading agent usage..."));

    let container_ref = container.clone();
    super::recv_on_main_thread(rx, move |data| {
        super::set_tooltip_text(container_ref.clone(), Some(&data.tooltip));
    });

    debug!("Agent usage module created");
    container.upcast()
}

async fn fetch_all_usage(data_path: &str) -> String {
    let expanded = expand_path(data_path);
    let contents = match tokio::fs::read_to_string(&expanded).await {
        Ok(c) => c,
        Err(err) => return format!("Agent Usage\nFailed to read {}: {err}", expanded.display()),
    };

    let mut creds: CredentialsFile = match serde_json::from_str(&contents) {
        Ok(c) => c,
        Err(err) => return format!("Agent Usage\nFailed to parse {}: {err}", expanded.display()),
    };

    let mut lines = Vec::new();
    let mut creds_changed = false;

    // Codex
    match &creds.codex {
        Some(codex_creds) => {
            match fetch_codex_usage(codex_creds).await {
                Ok(usage) => lines.push(format_service(&usage)),
                Err(ref err) if err.contains("401") || err.contains("unauthorized") => {
                    // Try refresh
                    match refresh_codex_token(codex_creds).await {
                        Ok(new_creds) => {
                            match fetch_codex_usage(&new_creds).await {
                                Ok(usage) => {
                                    lines.push(format_service(&usage));
                                    creds.codex = Some(new_creds);
                                    creds_changed = true;
                                }
                                Err(err) => lines.push(format!("Codex: {err}")),
                            }
                        }
                        Err(err) => lines.push(format!("Codex: refresh failed: {err}")),
                    }
                }
                Err(err) => lines.push(format!("Codex: {err}")),
            }
        }
        None => lines.push("Codex: not configured".to_string()),
    }

    // Claude
    match &creds.claude {
        Some(claude_creds) => {
            match fetch_claude_usage(claude_creds).await {
                Ok(usage) => lines.push(format_service(&usage)),
                Err(ref err) if err.contains("401") || err.contains("unauthorized") => {
                    match refresh_claude_token(claude_creds).await {
                        Ok(new_creds) => {
                            match fetch_claude_usage(&new_creds).await {
                                Ok(usage) => {
                                    lines.push(format_service(&usage));
                                    creds.claude = Some(new_creds);
                                    creds_changed = true;
                                }
                                Err(err) => lines.push(format!("Claude: {err}")),
                            }
                        }
                        Err(err) => lines.push(format!("Claude: refresh failed: {err}")),
                    }
                }
                Err(err) => lines.push(format!("Claude: {err}")),
            }
        }
        None => lines.push("Claude: not configured".to_string()),
    }

    if creds_changed {
        if let Err(err) = save_credentials(&expanded, &creds).await {
            warn!("Failed to save refreshed tokens: {err}");
        }
    }

    lines.join("\n")
}

async fn fetch_codex_usage(creds: &CodexCredentials) -> Result<ServiceUsage, String> {
    let response = curl_json(
        "https://chatgpt.com/backend-api/wham/usage",
        vec![
            "-H".to_string(),
            format!("Authorization: Bearer {}", creds.access_token),
            "-H".to_string(),
            format!("ChatGPT-Account-Id: {}", creds.account_id),
            "-H".to_string(),
            "User-Agent: ferritebar".to_string(),
        ],
    )
    .await?;

    let rate_limit = response
        .get("rate_limit")
        .ok_or_else(|| "no rate_limit in response".to_string())?;

    let mut windows = Vec::new();

    if let Some(primary) = rate_limit.get("primary_window") {
        let used_pct = primary
            .get("used_percent")
            .and_then(value_as_f64)
            .unwrap_or(0.0);
        let remaining = 100.0 - used_pct;
        let resets = primary
            .get("reset_at")
            .and_then(Value::as_i64)
            .map(format_unix_timestamp);
        windows.push(UsageWindow {
            label: "5hr".to_string(),
            remaining_pct: remaining,
            resets_at: resets,
        });
    }

    if let Some(secondary) = rate_limit.get("secondary_window") {
        let used_pct = secondary
            .get("used_percent")
            .and_then(value_as_f64)
            .unwrap_or(0.0);
        let remaining = 100.0 - used_pct;
        let resets = secondary
            .get("reset_at")
            .and_then(Value::as_i64)
            .map(format_unix_timestamp);
        windows.push(UsageWindow {
            label: "Week".to_string(),
            remaining_pct: remaining,
            resets_at: resets,
        });
    }

    Ok(ServiceUsage {
        name: "Codex".to_string(),
        windows,
    })
}

async fn fetch_claude_usage(creds: &ClaudeCredentials) -> Result<ServiceUsage, String> {
    let response = curl_json(
        "https://api.anthropic.com/api/oauth/usage",
        vec![
            "-H".to_string(),
            format!("Authorization: Bearer {}", creds.access_token),
            "-H".to_string(),
            "anthropic-beta: oauth-2025-04-20".to_string(),
        ],
    )
    .await?;

    let mut windows = Vec::new();

    if let Some(five_hour) = response.get("five_hour") {
        let utilization = five_hour
            .get("utilization")
            .and_then(value_as_f64)
            .unwrap_or(0.0);
        let remaining = (1.0 - utilization) * 100.0;
        let resets = five_hour
            .get("resets_at")
            .and_then(Value::as_str)
            .and_then(|s| parse_iso_to_date(s));
        windows.push(UsageWindow {
            label: "5hr".to_string(),
            remaining_pct: remaining,
            resets_at: resets,
        });
    }

    if let Some(seven_day) = response.get("seven_day") {
        let utilization = seven_day
            .get("utilization")
            .and_then(value_as_f64)
            .unwrap_or(0.0);
        let remaining = (1.0 - utilization) * 100.0;
        let resets = seven_day
            .get("resets_at")
            .and_then(Value::as_str)
            .and_then(|s| parse_iso_to_date(s));
        windows.push(UsageWindow {
            label: "Week".to_string(),
            remaining_pct: remaining,
            resets_at: resets,
        });
    }

    Ok(ServiceUsage {
        name: "Claude".to_string(),
        windows,
    })
}

async fn refresh_claude_token(creds: &ClaudeCredentials) -> Result<ClaudeCredentials, String> {
    debug!("Refreshing Claude token");
    let response = curl_json(
        "https://platform.claude.com/v1/oauth/token",
        vec![
            "-X".to_string(),
            "POST".to_string(),
            "-H".to_string(),
            "Content-Type: application/x-www-form-urlencoded".to_string(),
            "-d".to_string(),
            format!(
                "grant_type=refresh_token&refresh_token={}&client_id={}",
                creds.refresh_token, CLAUDE_CLIENT_ID
            ),
        ],
    )
    .await?;

    let access_token = response
        .get("access_token")
        .and_then(Value::as_str)
        .ok_or_else(|| "no access_token in refresh response".to_string())?
        .to_string();

    let refresh_token = response
        .get("refresh_token")
        .and_then(Value::as_str)
        .unwrap_or(&creds.refresh_token)
        .to_string();

    Ok(ClaudeCredentials {
        access_token,
        refresh_token,
    })
}

async fn refresh_codex_token(creds: &CodexCredentials) -> Result<CodexCredentials, String> {
    debug!("Refreshing Codex token");
    let body = serde_json::json!({
        "client_id": CODEX_CLIENT_ID,
        "grant_type": "refresh_token",
        "refresh_token": creds.refresh_token,
        "scope": "openid profile email"
    });

    let response = curl_json(
        "https://auth.openai.com/oauth/token",
        vec![
            "-X".to_string(),
            "POST".to_string(),
            "-H".to_string(),
            "Content-Type: application/json".to_string(),
            "-d".to_string(),
            body.to_string(),
        ],
    )
    .await?;

    let access_token = response
        .get("access_token")
        .and_then(Value::as_str)
        .ok_or_else(|| "no access_token in refresh response".to_string())?
        .to_string();

    let refresh_token = response
        .get("refresh_token")
        .and_then(Value::as_str)
        .unwrap_or(&creds.refresh_token)
        .to_string();

    Ok(CodexCredentials {
        access_token,
        refresh_token,
        account_id: creds.account_id.clone(),
    })
}

async fn save_credentials(
    path: &std::path::Path,
    creds: &CredentialsFile,
) -> Result<(), String> {
    let json = serde_json::to_string_pretty(creds).map_err(|e| e.to_string())?;
    tokio::fs::write(path, json)
        .await
        .map_err(|e| e.to_string())
}

fn format_service(usage: &ServiceUsage) -> String {
    let parts: Vec<String> = usage
        .windows
        .iter()
        .map(|w| {
            let base = format!("{} {:.0}%", w.label, w.remaining_pct);
            match &w.resets_at {
                Some(date) => format!("{base} Resets {date}"),
                None => base,
            }
        })
        .collect();

    format!("{} {}", usage.name, parts.join(" | "))
}

fn format_unix_timestamp(ts: i64) -> String {
    let dt = chrono::DateTime::from_timestamp(ts, 0);
    match dt {
        Some(dt) => {
            use chrono::Datelike;
            let local = dt.with_timezone(&chrono::Local);
            format!("{}/{}", local.month(), local.day())
        }
        None => "??".to_string(),
    }
}

fn parse_iso_to_date(iso: &str) -> Option<String> {
    let dt = chrono::DateTime::parse_from_rfc3339(iso).ok()?;
    use chrono::Datelike;
    let local = dt.with_timezone(&chrono::Local);
    Some(format!("{}/{}", local.month(), local.day()))
}

async fn curl_json(url: &str, args: Vec<String>) -> Result<Value, String> {
    let output = tokio::process::Command::new("curl")
        .arg("-fsSL")
        .args(args)
        .arg(url)
        .output()
        .await
        .map_err(|err| format!("curl failed: {err}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(if stderr.is_empty() {
            format!("request failed with status {}", output.status)
        } else {
            stderr
        });
    }

    serde_json::from_slice(&output.stdout).map_err(|err| format!("invalid JSON: {err}"))
}

fn value_as_f64(value: &Value) -> Option<f64> {
    value
        .as_f64()
        .or_else(|| value.as_i64().map(|n| n as f64))
        .or_else(|| value.as_u64().map(|n| n as f64))
        .or_else(|| value.as_str().and_then(|s| s.parse::<f64>().ok()))
}

fn expand_path(path: &str) -> std::path::PathBuf {
    if let Some(rest) = path.strip_prefix("~/") {
        if let Ok(home) = std::env::var("HOME") {
            return std::path::PathBuf::from(home).join(rest);
        }
    }
    std::path::PathBuf::from(path)
}
