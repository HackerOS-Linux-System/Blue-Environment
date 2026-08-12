use serde::{Deserialize, Serialize};
use std::time::Duration;
use tauri_plugin_http::reqwest::Client;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct WeatherResult {
    pub temp_c: f64,
    pub feels_like_c: f64,
    pub code: i64,
    pub city: String,
    pub humidity: Option<f64>,
    pub wind_kph: Option<f64>,
    pub high_c: Option<f64>,
    pub low_c: Option<f64>,
}

#[derive(Debug, Deserialize)]
struct GeoHit {
    latitude: f64,
    longitude: f64,
    name: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GeocodingResponse {
    results: Option<Vec<GeoHit>>,
}

fn client() -> Client {
    Client::builder()
        .timeout(Duration::from_secs(6))
        .user_agent("BlueEnvironment/1.0 (+https://blue-environment.local)")
        .build()
        .unwrap_or_else(|_| Client::new())
}

/// Geocode a manually-entered city name (e.g. "Katowice,PL") via
/// Open-Meteo's own geocoding API — no separate provider needed here since
/// it isn't rate-limited the way IP-geolocation lookups are.
async fn geocode_city(city: &str) -> Option<(f64, f64, String)> {
    let url = format!(
        "https://geocoding-api.open-meteo.com/v1/search?count=1&name={}",
        urlencoding_light(city)
    );
    let res = client().get(&url).send().await.ok()?;
    let body: GeocodingResponse = res.json().await.ok()?;
    let hit = body.results?.into_iter().next()?;
    Some((hit.latitude, hit.longitude, hit.name.unwrap_or_else(|| city.to_string())))
}

/// Fallback chain of IP-geolocation providers. Each is tried in order and
/// the first one that returns usable coordinates wins. All three have
/// generous free tiers and none require an API key, so no secrets are
/// needed here.
async fn geolocate_ip() -> Option<(f64, f64, String)> {
    // 1. ipapi.co — kept first since it's what the original code used and
    //    usually gives the nicest city names, but we no longer depend on
    //    it being reachable.
    if let Some(r) = try_ipapi_co().await {
        return Some(r);
    }
    // 2. ipwho.is — HTTPS, no key required, decent free-tier limits.
    //    (Note: we deliberately only use HTTPS providers here — the
    //    Tauri `http:default` capability in capabilities/default.json is
    //    scoped to `https://*`, so an HTTP-only provider like the free
    //    tier of ip-api.com would just be rejected by the permission
    //    system rather than failing over cleanly.)
    if let Some(r) = try_ipwho_is().await {
        return Some(r);
    }
    None
}

#[derive(Debug, Deserialize)]
struct IpApiCo {
    latitude: Option<f64>,
    longitude: Option<f64>,
    city: Option<String>,
}
async fn try_ipapi_co() -> Option<(f64, f64, String)> {
    let res = client().get("https://ipapi.co/json/").send().await.ok()?;
    if !res.status().is_success() {
        return None;
    }
    let body: IpApiCo = res.json().await.ok()?;
    Some((body.latitude?, body.longitude?, body.city.unwrap_or_else(|| "Unknown".into())))
}

#[derive(Debug, Deserialize)]
struct IpWhoIs {
    success: Option<bool>,
    latitude: Option<f64>,
    longitude: Option<f64>,
    city: Option<String>,
}
async fn try_ipwho_is() -> Option<(f64, f64, String)> {
    let res = client().get("https://ipwho.is/").send().await.ok()?;
    let body: IpWhoIs = res.json().await.ok()?;
    if body.success == Some(false) {
        return None;
    }
    Some((body.latitude?, body.longitude?, body.city.unwrap_or_else(|| "Unknown".into())))
}

#[derive(Debug, Deserialize)]
struct ForecastResponse {
    current_weather: Option<CurrentWeather>,
    hourly: Option<Hourly>,
    daily: Option<Daily>,
}
#[derive(Debug, Deserialize)]
struct CurrentWeather {
    temperature: f64,
    weathercode: i64,
    windspeed: Option<f64>,
}
#[derive(Debug, Deserialize)]
struct Hourly {
    relativehumidity_2m: Option<Vec<f64>>,
}
#[derive(Debug, Deserialize)]
struct Daily {
    temperature_2m_max: Option<Vec<f64>>,
    temperature_2m_min: Option<Vec<f64>>,
}

/// Tiny percent-encoder so we don't need to pull in the `urlencoding`
/// crate just for one query param.
fn urlencoding_light(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            b' ' => out.push('+'),
            _ => out.push_str(&format!("%{:02X}", b)),
        }
    }
    out
}

#[tauri::command]
pub async fn get_weather(city_override: Option<String>) -> Result<WeatherResult, String> {
    let city_override = city_override.unwrap_or_default();
    let (lat, lon, city) = if !city_override.trim().is_empty() {
        geocode_city(city_override.trim())
            .await
            .ok_or_else(|| format!("Could not geocode city '{}'", city_override))?
    } else {
        geolocate_ip()
            .await
            .ok_or_else(|| "All IP geolocation providers failed or timed out".to_string())?
    };

    let url = format!(
        "https://api.open-meteo.com/v1/forecast?latitude={}&longitude={}&current_weather=true&hourly=relativehumidity_2m&daily=temperature_2m_max,temperature_2m_min&timezone=auto",
        lat, lon
    );
    let res = client()
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("Forecast request failed: {e}"))?;
    if !res.status().is_success() {
        return Err(format!("Forecast request returned HTTP {}", res.status()));
    }
    let body: ForecastResponse = res
        .json()
        .await
        .map_err(|e| format!("Failed to parse forecast response: {e}"))?;
    let cw = body
        .current_weather
        .ok_or_else(|| "Forecast response missing current_weather".to_string())?;

    let now_hour = chrono_hour_now();
    let humidity = body
        .hourly
        .and_then(|h| h.relativehumidity_2m)
        .and_then(|v| v.get(now_hour).copied());
    let high = body.daily.as_ref().and_then(|d| d.temperature_2m_max.as_ref()).and_then(|v| v.first().copied());
    let low = body.daily.as_ref().and_then(|d| d.temperature_2m_min.as_ref()).and_then(|v| v.first().copied());

    Ok(WeatherResult {
        temp_c: cw.temperature,
        feels_like_c: cw.temperature, // Open-Meteo's `current_weather` block has no apparent-temp field on the free tier.
        code: cw.weathercode,
        city,
        humidity,
        wind_kph: cw.windspeed,
        high_c: high,
        low_c: low,
    })
}

/// Local hour-of-day (0-23), without pulling in the `chrono` crate — we
/// only need it as an index into Open-Meteo's hourly array, which is
/// already returned in the location's local time because we pass
/// `timezone=auto`.
fn chrono_hour_now() -> usize {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    ((secs / 3600) % 24) as usize
}
