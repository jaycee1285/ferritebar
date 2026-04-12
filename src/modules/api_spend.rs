use gtk::prelude::*;
use serde::Deserialize;
use serde_json::Value;
use tokio::sync::mpsc;
use tracing::debug;

use crate::config::types::ApiSpendConfig;

#[derive(Debug, Clone)]
struct ProviderFileConfig {
    api_key: String,
    limit: f64,
}

#[derive(Debug, Deserialize)]
struct ApiFileConfig {
    openai: Option<ProviderEntry>,
    anthropic: Option<ProviderEntry>,
    openrouter: Option<ProviderEntry>,
}

#[derive(Debug, Deserialize)]
struct ProviderEntry {
    api_key: String,
    limit: f64,
}

#[derive(Debug)]
struct ApiSpendData {
    tooltip: String,
}

pub fn build(config: &ApiSpendConfig) -> gtk::Widget {
    let (tx, rx) = mpsc::channel::<ApiSpendData>(8);

    let data_path = config.data_path.clone();
    let interval_secs = config.interval.max(60);

    crate::spawn(async move {
        let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(interval_secs));
        loop {
            interval.tick().await;

            let tooltip = match load_provider_config(&data_path).await {
                Ok(provider_cfg) => fetch_tooltip(provider_cfg).await,
                Err(err) => format!("API spend\n{err}"),
            };

            if tx.send(ApiSpendData { tooltip }).await.is_err() {
                break;
            }
        }
    });

    let container = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    container.add_css_class("module");
    container.add_css_class("api-spend");

    let label = gtk::Label::new(Some(&config.icon));
    label.add_css_class("module-label");
    container.append(&label);

    super::set_tooltip_text(container.clone(), Some("Loading API spend..."));

    let container_ref = container.clone();
    super::recv_on_main_thread(rx, move |data| {
        super::set_tooltip_text(container_ref.clone(), Some(&data.tooltip));
    });

    debug!("API spend module created");
    container.upcast()
}

async fn load_provider_config(path: &str) -> Result<ApiFileConfig, String> {
    let expanded = expand_path(path);
    let contents = tokio::fs::read_to_string(&expanded)
        .await
        .map_err(|err| format!("Failed to read {}: {err}", expanded.display()))?;

    serde_json::from_str(&contents)
        .map_err(|err| format!("Failed to parse {}: {err}", expanded.display()))
}

async fn fetch_tooltip(config: ApiFileConfig) -> String {
    let openai_cfg = config.openai.and_then(normalize_provider);
    let anthropic_cfg = config.anthropic.and_then(normalize_provider);
    let openrouter_cfg = config.openrouter.and_then(normalize_provider);

    let (openai, anthropic, openrouter) = tokio::join!(
        fetch_openai(openai_cfg),
        fetch_anthropic(anthropic_cfg),
        fetch_openrouter(openrouter_cfg)
    );

    format!(
        "OpenAI {}\nAnthropic {}\nOpen Router {}",
        format_provider_result(openai),
        format_provider_result(anthropic),
        format_provider_result(openrouter),
    )
}

fn normalize_provider(entry: ProviderEntry) -> Option<ProviderFileConfig> {
    let api_key = entry.api_key.trim().to_string();
    if api_key.is_empty() {
        return None;
    }

    Some(ProviderFileConfig {
        api_key,
        limit: entry.limit,
    })
}

async fn fetch_openai(config: Option<ProviderFileConfig>) -> Result<(f64, f64), String> {
    let Some(config) = config else {
        return Err("not configured / n/a".to_string());
    };

    let mut spent = 0.0;
    let mut page: Option<String> = None;

    loop {
        let mut args = vec![
            "-H".to_string(),
            format!("Authorization: Bearer {}", config.api_key),
            "-H".to_string(),
            "Content-Type: application/json".to_string(),
            "--get".to_string(),
            "--data-urlencode".to_string(),
            format!("start_time={}", openai_start_time()),
            "--data-urlencode".to_string(),
            "bucket_width=1d".to_string(),
            "--data-urlencode".to_string(),
            "limit=180".to_string(),
        ];

        if let Some(ref page_cursor) = page {
            args.push("--data-urlencode".to_string());
            args.push(format!("page={page_cursor}"));
        }

        let response = curl_json("https://api.openai.com/v1/organization/costs", args).await?;

        spent += response
            .get("data")
            .and_then(Value::as_array)
            .map(|buckets| buckets.iter().filter_map(openai_bucket_total).sum::<f64>())
            .ok_or_else(|| "unexpected response".to_string())?;

        let has_more = response
            .get("has_more")
            .and_then(Value::as_bool)
            .unwrap_or(false);

        page = response
            .get("next_page")
            .and_then(Value::as_str)
            .map(ToString::to_string);

        if !has_more || page.is_none() {
            break;
        }
    }

    Ok((spent, config.limit))
}

async fn fetch_anthropic(config: Option<ProviderFileConfig>) -> Result<(f64, f64), String> {
    let Some(config) = config else {
        return Err("not configured / n/a".to_string());
    };

    let ending_at = chrono::Utc::now().to_rfc3339();
    let response = curl_json(
        "https://api.anthropic.com/v1/organizations/cost_report",
        vec![
            "-H".to_string(),
            format!("x-api-key: {}", config.api_key),
            "-H".to_string(),
            "anthropic-version: 2023-06-01".to_string(),
            "--get".to_string(),
            "--data-urlencode".to_string(),
            "starting_at=1970-01-01T00:00:00Z".to_string(),
            "--data-urlencode".to_string(),
            format!("ending_at={ending_at}"),
        ],
    )
    .await?;

    let spent = anthropic_total(&response).ok_or_else(|| "unexpected response".to_string())?;
    Ok((spent, config.limit))
}

async fn fetch_openrouter(config: Option<ProviderFileConfig>) -> Result<(f64, f64), String> {
    let Some(config) = config else {
        return Err("not configured / n/a".to_string());
    };

    let response = curl_json(
        "https://openrouter.ai/api/v1/key",
        vec![
            "-H".to_string(),
            format!("Authorization: Bearer {}", config.api_key),
        ],
    )
    .await?;

    let spent = response
        .get("data")
        .and_then(|data| data.get("usage"))
        .and_then(value_as_f64)
        .or_else(|| response.get("usage").and_then(value_as_f64))
        .ok_or_else(|| "unexpected response".to_string())?;

    Ok((spent, config.limit))
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

fn openai_bucket_total(bucket: &Value) -> Option<f64> {
    let results = bucket.get("results").and_then(Value::as_array)?;
    Some(results.iter().filter_map(openai_amount_from_row).sum::<f64>())
}

fn openai_amount_from_row(row: &Value) -> Option<f64> {
    row.get("amount")
        .and_then(|amount| amount.get("value"))
        .and_then(value_as_f64)
}

fn anthropic_total(value: &Value) -> Option<f64> {
    value
        .get("total_cost_usd")
        .and_then(value_as_f64)
        .or_else(|| {
            value.get("total_cost").and_then(|total| {
                total
                    .get("usd")
                    .and_then(value_as_f64)
                    .or_else(|| total.get("value").and_then(value_as_f64))
                    .or_else(|| value_as_f64(total))
            })
        })
        .or_else(|| {
            value.get("data").and_then(Value::as_array).map(|rows| {
                rows.iter()
                    .filter_map(|row| {
                        row.get("cost_usd")
                            .and_then(value_as_f64)
                            .or_else(|| row.get("amount_usd").and_then(value_as_f64))
                            .or_else(|| row.get("amount").and_then(extract_nested_amount))
                            .or_else(|| row.get("cost").and_then(extract_nested_amount))
                    })
                    .sum::<f64>()
            })
        })
}

fn extract_nested_amount(value: &Value) -> Option<f64> {
    value
        .get("usd")
        .and_then(value_as_f64)
        .or_else(|| value.get("value").and_then(value_as_f64))
        .or_else(|| value_as_f64(value))
}

fn value_as_f64(value: &Value) -> Option<f64> {
    value
        .as_f64()
        .or_else(|| value.as_i64().map(|n| n as f64))
        .or_else(|| value.as_u64().map(|n| n as f64))
        .or_else(|| value.as_str().and_then(|s| s.parse::<f64>().ok()))
}

fn format_provider_result(result: Result<(f64, f64), String>) -> String {
    match result {
        Ok((spent, limit)) => format!("${spent:.2} / ${limit:.2}"),
        Err(err) => err,
    }
}

fn openai_start_time() -> u64 {
    chrono::NaiveDate::from_ymd_opt(2020, 1, 1)
        .and_then(|date| date.and_hms_opt(0, 0, 0))
        .map(|dt| dt.and_utc().timestamp() as u64)
        .unwrap_or(1_577_836_800)
}

fn expand_path(path: &str) -> std::path::PathBuf {
    if let Some(rest) = path.strip_prefix("~/") {
        if let Ok(home) = std::env::var("HOME") {
            return std::path::PathBuf::from(home).join(rest);
        }
    }

    std::path::PathBuf::from(path)
}
