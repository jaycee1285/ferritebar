use std::fmt::Write;

use gtk::prelude::*;
use serde::Deserialize;
use tokio::sync::mpsc;
use tracing::{debug, warn};

use crate::config::types::WeatherConfig;

#[derive(Debug)]
struct WeatherData {
    temperature: i64,
    condition: String,
    humidity: Option<f64>,
    wind_speed: Option<f64>,
    location_name: String,
}

#[derive(Deserialize)]
struct ZippopotamResponse {
    places: Vec<ZippopotamPlace>,
}

#[derive(Deserialize)]
struct ZippopotamPlace {
    #[serde(rename = "place name")]
    place_name: String,
    latitude: String,
    longitude: String,
}

#[derive(Deserialize)]
struct NWSPointResponse {
    properties: NWSPointProperties,
}

#[derive(Deserialize)]
struct NWSPointProperties {
    #[serde(rename = "gridId")]
    grid_id: String,
    #[serde(rename = "gridX")]
    grid_x: i64,
    #[serde(rename = "gridY")]
    grid_y: i64,
    #[serde(rename = "observationStations")]
    observation_stations: String,
}

#[derive(Deserialize)]
struct ForecastResponse {
    properties: ForecastProperties,
}

#[derive(Deserialize)]
struct ForecastProperties {
    periods: Vec<ForecastPeriod>,
}

#[derive(Deserialize)]
struct ForecastPeriod {
    temperature: i64,
    #[serde(rename = "temperatureUnit")]
    temperature_unit: String,
    #[serde(rename = "shortForecast")]
    short_forecast: String,
}

#[derive(Deserialize)]
struct StationsResponse {
    features: Vec<StationFeature>,
}

#[derive(Deserialize)]
struct StationFeature {
    properties: StationProperties,
}

#[derive(Deserialize)]
struct StationProperties {
    #[serde(rename = "stationIdentifier")]
    station_identifier: String,
}

#[derive(Deserialize)]
struct ObservationResponse {
    properties: ObservationProperties,
}

#[derive(Deserialize)]
struct ObservationProperties {
    temperature: ObservationValue<f64>,
    #[serde(rename = "relativeHumidity")]
    relative_humidity: Option<ObservationValue<f64>>,
    #[serde(rename = "windSpeed")]
    wind_speed: Option<ObservationValue<f64>>,
}

#[derive(Deserialize)]
struct ObservationValue<T> {
    value: Option<T>,
}

const USER_AGENT: &str = "ferritebar/0.1 (github.com/jaycee1285/ferritebar)";

async fn curl_json<T: serde::de::DeserializeOwned>(url: &str, headers: &[(&str, &str)]) -> Result<T, String> {
    let mut args = vec!["-fsSL".to_string()];
    for (key, val) in headers {
        args.push("-H".to_string());
        args.push(format!("{key}: {val}"));
    }
    args.push(url.to_string());

    let output = tokio::process::Command::new("curl")
        .args(&args)
        .output()
        .await
        .map_err(|e| format!("curl failed: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(if stderr.is_empty() {
            format!("HTTP error {}", output.status)
        } else {
            stderr
        });
    }

    serde_json::from_slice(&output.stdout).map_err(|e| format!("JSON parse: {e}"))
}

async fn resolve_zip(zip: &str) -> Result<(f64, f64, String), String> {
    let url = format!("https://api.zippopotam.us/us/{zip}");
    let resp: ZippopotamResponse = curl_json(&url, &[]).await?;
    let place = resp.places.first().ok_or("ZIP not found")?;
    let lat: f64 = place.latitude.parse().map_err(|e| format!("lat parse: {e}"))?;
    let lon: f64 = place.longitude.parse().map_err(|e| format!("lon parse: {e}"))?;
    Ok((lat, lon, place.place_name.clone()))
}

async fn fetch_weather(lat: f64, lon: f64, location_name: &str) -> Result<WeatherData, String> {
    let headers = [("User-Agent", USER_AGENT)];

    let point_url = format!("https://api.weather.gov/points/{lat},{lon}");
    let point: NWSPointResponse = curl_json(&point_url, &headers).await?;

    let forecast_url = format!(
        "https://api.weather.gov/gridpoints/{}/{},{}/forecast",
        point.properties.grid_id, point.properties.grid_x, point.properties.grid_y
    );

    let forecast: ForecastResponse = curl_json(&forecast_url, &headers).await?;
    let period = forecast.properties.periods.first().ok_or("No forecast data")?;

    let mut temp_c = period.temperature;
    if period.temperature_unit == "F" {
        temp_c = ((period.temperature as f64 - 32.0) / 1.8).round() as i64;
    }

    let mut data = WeatherData {
        temperature: temp_c,
        condition: period.short_forecast.clone(),
        humidity: None,
        wind_speed: None,
        location_name: location_name.to_string(),
    };

    // Try current observation for more accurate data
    if let Ok(stations) = curl_json::<StationsResponse>(&point.properties.observation_stations, &headers).await {
        if let Some(station) = stations.features.first() {
            let obs_url = format!(
                "https://api.weather.gov/stations/{}/observations/latest",
                station.properties.station_identifier
            );
            if let Ok(obs) = curl_json::<ObservationResponse>(&obs_url, &headers).await {
                if let Some(t) = obs.properties.temperature.value {
                    data.temperature = t.round() as i64;
                }
                data.humidity = obs.properties.relative_humidity.and_then(|h| h.value);
                data.wind_speed = obs.properties.wind_speed.and_then(|w| w.value);
            }
        }
    }

    Ok(data)
}

/// Font Awesome 6 weather icons (bundled in ferritebar's Nix fontconfig)
fn weather_icon(condition: &str) -> &'static str {
    let c = condition.to_lowercase();
    if c.contains("sunny") || c.contains("clear") {
        "\u{f185}" // fa-sun
    } else if c.contains("partly") || c.contains("mostly sunny") {
        "\u{f6c4}" // fa-cloud-sun
    } else if c.contains("cloud") || c.contains("overcast") {
        "\u{f0c2}" // fa-cloud
    } else if c.contains("thunder") || c.contains("storm") {
        "\u{f0e7}" // fa-bolt
    } else if c.contains("rain") || c.contains("showers") {
        if c.contains("light") {
            "\u{f73d}" // fa-cloud-rain
        } else {
            "\u{f740}" // fa-cloud-showers-heavy
        }
    } else if c.contains("snow") || c.contains("cold") {
        "\u{f2dc}" // fa-snowflake
    } else if c.contains("fog") || c.contains("mist") || c.contains("haze") {
        "\u{f75f}" // fa-smog
    } else if c.contains("wind") {
        "\u{f72e}" // fa-wind
    } else if c.contains("hot") {
        "\u{f185}" // fa-sun
    } else {
        "\u{f0c2}" // fa-cloud as generic fallback
    }
}

fn format_temp(temp_c: i64, unit: &str) -> String {
    if unit.to_uppercase().starts_with('C') {
        format!("{temp_c}°C")
    } else {
        let temp_f = (temp_c as f64 * 1.8 + 32.0).round() as i64;
        format!("{temp_f}°F")
    }
}

pub fn build(config: &WeatherConfig) -> gtk::Widget {
    let (tx, rx) = mpsc::channel::<Result<WeatherData, String>>(8);

    let zip = config.zip.clone();
    let lat = config.lat;
    let lon = config.lon;
    let interval_secs = config.interval.max(300); // Minimum 5 minutes

    crate::spawn(async move {
        // Resolve location once
        let (lat, lon, name) = if let Some(ref zip) = zip {
            match resolve_zip(zip).await {
                Ok(loc) => loc,
                Err(e) => {
                    let _ = tx.send(Err(format!("ZIP resolve: {e}"))).await;
                    // Retry after interval
                    tokio::time::sleep(tokio::time::Duration::from_secs(interval_secs)).await;
                    return;
                }
            }
        } else if let (Some(lat), Some(lon)) = (lat, lon) {
            (lat, lon, format!("{lat:.2}, {lon:.2}"))
        } else {
            let _ = tx.send(Err("No zip or lat/lon configured".to_string())).await;
            return;
        };

        let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(interval_secs));
        loop {
            interval.tick().await;
            let result = fetch_weather(lat, lon, &name).await;
            if let Err(ref e) = result {
                warn!("Weather fetch failed: {e}");
            }
            if tx.send(result).await.is_err() {
                break;
            }
        }
    });

    let container = gtk::Box::new(gtk::Orientation::Horizontal, 4);
    container.add_css_class("module");
    container.add_css_class("weather");

    let label = gtk::Label::new(Some("\u{f0c2}")); // fa-cloud placeholder
    label.add_css_class("module-label");
    container.append(&label);

    let unit = config.unit.clone().unwrap_or_else(|| "F".to_string());

    let container_ref = container.clone();
    let mut tooltip_buf = String::with_capacity(128);
    super::recv_on_main_thread(rx, move |result| {
        match result {
            Ok(data) => {
                let icon = weather_icon(&data.condition);
                let temp = format_temp(data.temperature, &unit);
                // Icon only on the bar
                label.set_label(icon);

                // Temperature + details in tooltip
                tooltip_buf.clear();
                let _ = write!(tooltip_buf, "{temp}  {}", data.condition);
                let _ = write!(tooltip_buf, "\n{}", data.location_name);
                if let Some(h) = data.humidity {
                    let _ = write!(tooltip_buf, "\nHumidity: {h:.0}%");
                }
                if let Some(w) = data.wind_speed {
                    let mph = w * 2.237;
                    let _ = write!(tooltip_buf, "\nWind: {mph:.0} mph");
                }
                super::set_tooltip_text(container_ref.clone(), Some(&tooltip_buf));
                container_ref.remove_css_class("weather-error");
            }
            Err(e) => {
                label.set_label("\u{f0c2}");
                super::set_tooltip_text(container_ref.clone(), Some(&format!("Weather error: {e}")));
                container_ref.add_css_class("weather-error");
            }
        }
    });

    debug!("Weather module created");
    container.upcast()
}
