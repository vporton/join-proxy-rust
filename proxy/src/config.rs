use ic_agent::export::Principal;
use serde::Deserializer;
use serde_derive::Deserialize;
use serde::de::Error;
use std::{collections::HashMap, time::Duration};

#[derive(Clone, Deserialize, Debug)]
pub struct Callback {
    #[serde(deserialize_with = "deserialize_canister_id")]
    pub canister: Principal,
    pub func: String,
    #[serde(default="default_ic_local")]
    pub ic_local: bool,
    pub ic_url: Option<String>,
}

#[derive(Clone, Deserialize, Debug)]
pub struct UpstreamTimeouts {
    #[serde(default="default_upstream_connect_timeout", deserialize_with = "parse_duration_option")]
    pub connect_timeout: Option<Duration>,
    #[serde(default="default_upstream_read_timeout", deserialize_with = "parse_duration_option")]
    pub read_timeout: Option<Duration>,
    #[serde(default="default_upstream_total_timeout", deserialize_with = "parse_duration_option")]
    pub total_timeout: Option<Duration>,
}

#[derive(Clone, Deserialize, Debug)]
pub struct RequestHeaders {
    pub remove: Vec<String>,
    pub add: Vec<(String, String)>,
    pub remove_per_host: HashMap<String, Vec<String>>,
    pub add_per_host: HashMap<String, Vec<(String, String)>>,
}

#[derive(Clone, Deserialize, Debug)]
pub struct ResponseHeaders {
    pub remove: Vec<String>,
    pub add: Vec<(String, String)>,
    pub remove_per_host: HashMap<String, Vec<String>>,
    pub add_per_host: HashMap<String, Vec<(String, String)>>,
    #[serde(default="default_show_hit_miss")]
    pub show_hit_miss: bool,
    #[serde(default="default_add_forwarded_from_header")]
    pub add_forwarded_from_header: bool,
}

#[derive(Clone, Deserialize, Debug)]
pub struct CacheConfig {
    #[serde(deserialize_with = "parse_duration")]
    pub cache_timeout: Duration,
}

#[derive(Clone, Deserialize, Debug)]
pub struct Serve {
    #[serde(default="default_host")]
    pub host: String,
    #[serde(default="default_port")]
    pub port: u16,
    #[serde(default="default_https")]
    pub https: bool,
    pub cert_file: Option<String>,
    pub key_file: Option<String>,
}

#[derive(Clone, Deserialize, Debug)]
pub struct Config {
    pub serve: Serve,
    pub our_secret: Option<String>, // simple Bearer authentication
    pub cache: CacheConfig,
    pub request_headers: RequestHeaders,
    pub response_headers: ResponseHeaders,
    pub upstream_timeouts: UpstreamTimeouts,
    pub callback: Option<Callback>,
}

fn default_host() -> String {
    "localhost".to_string()
}

fn default_port() -> u16 {
    8080
}

fn default_https() -> bool {
    false
}

fn default_show_hit_miss() -> bool {
    false
}

fn default_upstream_connect_timeout() -> Option<Duration> {
    Some(Duration::from_secs(10))
}

fn default_upstream_read_timeout() -> Option<Duration> {
    Some(Duration::from_secs(60)) // I set it big, for the use case of OpenAI API
}

fn default_upstream_total_timeout() -> Option<Duration> {
    Some(Duration::from_secs(120)) // I set it big, for the use case of OpenAI API
}

fn default_add_forwarded_from_header() -> bool {
    false // Isn't it useless?
}

fn default_ic_local() -> bool {
    false
}

fn extract_duration<'de, D>(s: &str) -> Result<Duration, D::Error>
where
    D: Deserializer<'de>,
{
    let pos = s.find(|c: char| !c.is_numeric()).unwrap_or(s.len());
    let (value_str, unit) = s.split_at(pos);

    let value: u64 = value_str.parse().map_err(serde::de::Error::custom)?;

    match unit {
        "d" => Ok(Duration::from_secs(value*3600*24)),
        "h" => Ok(Duration::from_secs(value*3600)),
        "m" => Ok(Duration::from_secs(value*60)),
        "s" => Ok(Duration::from_secs(value)),
        "ms" => Ok(Duration::from_millis(value)),
        _ => Err(serde::de::Error::custom("Invalid duration unit")),
    }
}

fn parse_duration<'de, D>(deserializer: D) -> Result<Duration, D::Error>
where
    D: Deserializer<'de>,
{
    let s: String = serde::Deserialize::deserialize(deserializer)?;
    extract_duration::<D>(s.as_str())
}

fn parse_duration_option<'de, D>(deserializer: D) -> Result<Option<Duration>, D::Error>
where
    D: Deserializer<'de>,
{
    let opt: Option<String> = serde::Deserialize::deserialize(deserializer)?;
    Ok(if let Some(s) = opt {
        Some(extract_duration::<D>(s.as_str())?)
    } else {
        None
    })
}

fn deserialize_canister_id<'de, D>(deserializer: D) -> Result<Principal, D::Error>
where
    D: Deserializer<'de>,
{
    let input: String = serde::Deserialize::deserialize(deserializer)?;
    match Principal::from_text(input) {
        Ok(principal) => Ok(principal),
        Err(principal_error) =>
            Err(D::Error::custom(format!("Invalid principal: {}", principal_error))),
    }
}
