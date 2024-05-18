use ic_agent::export::Principal;
use serde::Deserializer;
use serde_derive::Deserialize;
use serde::de::Error;
use std::time::Duration;

#[derive(Clone, Deserialize, Debug)]
pub struct Config {
    #[serde(default="default_port")]
    pub port: u16,
    pub our_secret: Option<String>, // simple Bearer authentication
    pub upstream_prefix: Option<String>,
    pub cache_timeout: Duration,
    pub remove_request_headers: Vec<String>,
    pub add_request_headers: Vec<(String, String)>,
    pub remove_response_headers: Vec<String>,
    pub add_response_headers: Vec<(String, String)>,
    #[serde(default="default_show_hit_miss")]
    pub show_hit_miss: bool,
    #[serde(default="default_upstream_connect_timeout")]
    pub upstream_connect_timeout: Duration,
    #[serde(default="default_upstream_read_timeout")]
    pub upstream_read_timeout: Duration,
    #[serde(default="default_add_forwarded_from_header")]
    pub add_forwarded_from_header: bool,
    pub ic_url: Option<String>,
    #[serde(default="default_ic_local")]
    pub ic_local: bool,
    #[serde(deserialize_with = "deserialize_canister_id")]
    pub signing_canister_id: Option<Principal>,
}

fn default_port() -> u16 {
    8080
}

fn default_show_hit_miss() -> bool {
    true
}

fn default_upstream_connect_timeout() -> Duration {
    Duration::from_secs(10)
}

fn default_upstream_read_timeout() -> Duration {
    Duration::from_secs(60) // I set it big, for the use case of OpenAI API
}

fn default_add_forwarded_from_header() -> bool {
    false // Isn't it useless?
}

fn default_ic_local() -> bool {
    false
}

fn deserialize_canister_id<'de, D>(deserializer: D) -> Result<Option<Principal>, D::Error>
where
    D: Deserializer<'de>,
{
    let input: Option<String> = serde::Deserialize::deserialize(deserializer)?;
    if let Some(input) = input {
        match Principal::from_text(input) {
            Ok(principal) => Ok(Some(principal)),
            Err(principal_error) =>
                Err(D::Error::custom(format!("Invalid principal: {}", principal_error))),
        }
    } else {
        Ok(None)
    }
}
