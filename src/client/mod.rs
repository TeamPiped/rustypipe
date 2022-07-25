use std::time::Instant;

use anyhow::{anyhow, bail, Context, Result};
use fancy_regex::Regex;
use log::{debug, error, info, warn};
use once_cell::sync::Lazy;
use rand::Rng;
use reqwest::{header, Client, ClientBuilder, Request};
use serde::Serialize;
use tokio::sync::Mutex;

use crate::util;

pub enum ClientType {
    Desktop,
    DesktopMusic,
    TvHtml5Embed,
    Android,
    Ios,
}

#[derive(Serialize, Debug)]
#[serde(rename = "camelCase")]
struct BaseRequest {
    context: ContextYT,
}

#[derive(Serialize, Debug)]
#[serde(rename = "camelCase")]
struct ContextYT {
    client: ClientInfo,
    /// only used on desktop
    #[serde(skip_serializing_if = "Option::is_none")]
    request: Option<RequestYT>,
    user: User,
    /// only used for the embedded player
    #[serde(skip_serializing_if = "Option::is_none")]
    third_party: Option<ThirdParty>,
}

#[derive(Serialize, Debug)]
#[serde(rename = "camelCase")]
struct ClientInfo {
    client_name: String,
    client_version: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    client_screen: Option<String>,
    platform: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    original_url: Option<String>,
    /// Language (`en`, `de`)
    hl: String,
    /// Country (`US`, `DE`)
    gl: String,
}

#[derive(Serialize, Debug)]
#[serde(rename = "camelCase")]
struct RequestYT {
    internal_experiment_flags: Vec<String>,
    use_ssl: bool,
}

impl Default for RequestYT {
    fn default() -> Self {
        Self {
            internal_experiment_flags: vec![],
            use_ssl: true,
        }
    }
}

#[derive(Serialize, Debug, Default)]
#[serde(rename = "camelCase")]
struct User {
    // TO DO: provide a way to enable restricted mode with:
    // "enableSafetyMode": true
    locked_safety_mode: bool,
}

#[derive(Serialize, Debug)]
#[serde(rename = "camelCase")]
struct ThirdParty {
    embed_url: String,
}

struct DesktopClientData {
    last_update: Option<Instant>,
    client_version: String,
}

impl Default for DesktopClientData {
    fn default() -> Self {
        Self {
            last_update: None,
            client_version: DESKTOP_CLIENT_VERSION.to_owned(),
        }
    }
}

impl DesktopClientData {
    fn is_old(&self) -> bool {
        self.last_update.is_none()
            || Instant::now()
                .duration_since(self.last_update.unwrap())
                .as_secs()
                > 86400
    }
}

const DEFAULT_UA: &str =
    "Mozilla/5.0 (Windows NT 10.0; Win64; rv:107.0) Gecko/20100101 Firefox/107.0";

const CONSENT_COOKIE: &str = "CONSENT";
const CONSENT_COOKIE_YES: &str = "YES+yt.462272069.de+FX+";
const CONSENT_COOKIE_NO: &str = "PENDING+";

const DESKTOP_CLIENT_VERSION: &str = "2.20220721.05.00_1";
const DESKTOP_API_KEY: &str = "AIzaSyAO_FJ2SlqU8Q4STEHLGCilw_Y9_11qcW8";

const CONTENT_PLAYBACK_NONCE_ALPHABET: &[u8; 64] =
    b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";

pub struct RustyTube {
    http: Client,
    desktop_client_data: Mutex<DesktopClientData>,

    lang: String,
    country: String,

    consent_cookie_yes: String,
    consent_cookie_no: String,
}

impl RustyTube {
    #[must_use]
    pub fn new() -> Self {
        Self::new_with_ua("en", "US", DEFAULT_UA)
    }

    #[must_use]
    pub fn new_with_ua(lang: &str, country: &str, user_agent: &str) -> Self {
        let http = ClientBuilder::new()
            .user_agent(user_agent)
            .build()
            .expect("unable to build the HTTP client");

        let mut rng = rand::thread_rng();

        Self {
            http,
            desktop_client_data: Mutex::new(DesktopClientData::default()),
            lang: lang.to_owned(),
            country: country.to_owned(),
            consent_cookie_yes: format!(
                "{}={}{}",
                CONSENT_COOKIE,
                CONSENT_COOKIE_YES,
                rng.gen_range(100..1000)
            ),
            consent_cookie_no: format!(
                "{}={}{}",
                CONSENT_COOKIE,
                CONSENT_COOKIE_NO,
                rng.gen_range(100..1000)
            ),
        }
    }

    async fn get_client_version(&self) -> String {
        let mut client_data = self.desktop_client_data.lock().await;

        if client_data.is_old() {
            let client_version = self.extract_client_version_from_swjs().await;
            let new_version = match client_version {
                Ok(client_version) => match client_version {
                    Some(client_version) => {
                        debug!("Updated desktop client version to {}", client_version);
                        client_version
                    }
                    None => {
                        warn!("Could not find desktop client version in sw.js");
                        DESKTOP_CLIENT_VERSION.to_owned()
                    }
                },
                Err(e) => {
                    warn!("Could not extract desktop client version, Error: {}", e);
                    DESKTOP_CLIENT_VERSION.to_owned()
                }
            };

            *client_data = DesktopClientData {
                client_version: new_version,
                last_update: Some(Instant::now()),
            }
        }

        client_data.client_version.to_string()
    }

    async fn extract_client_version_from_swjs(&self) -> Result<Option<String>> {
        let swjs = self
            .exec_request(
                self.http
                    .get("https://www.youtube.com/sw.js")
                    .header(header::ORIGIN, "https://www.youtube.com")
                    .header(header::REFERER, "https://www.youtube.com")
                    .header(header::COOKIE, self.consent_cookie_yes.to_owned())
                    .build()
                    .unwrap(),
            )
            .await
            .context("Failed to download sw.js")?;

        static CLIENT_VERSION_PATTERNS: Lazy<[Regex; 3]> = Lazy::new(|| {
            [
                Regex::new("INNERTUBE_CONTEXT_CLIENT_VERSION\":\"([0-9\\.]+?)\"").unwrap(),
                Regex::new("innertube_context_client_version\":\"([0-9\\.]+?)\"").unwrap(),
                Regex::new("client.version=([0-9\\.]+)").unwrap(),
            ]
        });

        /*
        static API_KEY_PATTERNS: Lazy<[Regex; 2]> = Lazy::new(|| {
            [
                Regex::new("INNERTUBE_API_KEY\":\"([0-9a-zA-Z_-]+?)\"").unwrap(),
                Regex::new("innertubeApiKey\":\"([0-9a-zA-Z_-]+?)\"").unwrap(),
            ]
        });*/

        Ok(util::get_cg_from_regexes(
            CLIENT_VERSION_PATTERNS.iter(),
            &swjs,
            1,
        ))
    }

    fn generate_content_playback_nonce() -> String {
        util::random_string(CONTENT_PLAYBACK_NONCE_ALPHABET, 16)
    }

    fn generate_t_parameter() -> String {
        util::random_string(CONTENT_PLAYBACK_NONCE_ALPHABET, 12)
    }

    async fn exec_request(&self, request: Request) -> Result<String> {
        let resp = self.http.execute(request).await?.error_for_status()?;
        Ok(resp.text().await?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use test_log::test;

    #[test(tokio::test)]
    async fn t_extract_client_version_from_swjs() {
        let rt = RustyTube::new();
        let version = rt.extract_client_version_from_swjs().await.unwrap();

        let version = version.unwrap();

        // Client version changes often, notify during test so the hardcoded version can be updated
        if version != DESKTOP_CLIENT_VERSION {
            println!(
                "INFO: YT Desktop Client was updated, new version: {}",
                version
            );
        }
    }

    #[test(tokio::test)]
    async fn t_get_client_version() {
        error!("Checking whether it still works...");
        let rt = RustyTube::new();
        let client_version = rt.get_client_version().await;
        assert!(client_version.len() > 10);
    }

    #[test]
    fn json_test() {
        let request = BaseRequest {
            context: ContextYT {
                client: ClientInfo {
                    client_name: "WEB".to_owned(),
                    client_version: "x".to_owned(),
                    client_screen: None,
                    platform: "DESKTOP".to_owned(),
                    original_url: Some("https://www.youtube.com".to_owned()),
                    hl: "de".to_owned(),
                    gl: "DE".to_owned(),
                },
                request: Some(RequestYT::default()),
                user: User::default(),
                third_party: None,
            },
        };

        let request_str = serde_json::to_string_pretty(&request).unwrap();
        println!("{}", request_str);
    }
}
