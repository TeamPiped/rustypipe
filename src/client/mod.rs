mod player;
mod response;

use std::{sync::Arc, time::Instant};

use anyhow::{anyhow, bail, Context, Result};
use async_trait::async_trait;
use fancy_regex::Regex;
use log::{debug, warn};
use once_cell::sync::Lazy;
use rand::Rng;
use reqwest::{header, Client, ClientBuilder, Method, Request, RequestBuilder, Response};
use serde::Serialize;
use tokio::sync::Mutex;

use crate::{deobfuscate::Deobfuscator, util};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ClientType {
    Desktop,
    DesktopMusic,
    TvHtml5Embed,
    Android,
    Ios,
}

impl ClientType {
    pub fn is_web(self) -> bool {
        self == Self::Desktop || self == Self::DesktopMusic || self == Self::TvHtml5Embed
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContextYT {
    client: ClientInfo,
    /// only used on desktop
    #[serde(skip_serializing_if = "Option::is_none")]
    request: Option<RequestYT>,
    user: User,
    /// only used for the embedded player
    #[serde(skip_serializing_if = "Option::is_none")]
    third_party: Option<ThirdParty>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ClientInfo {
    client_name: String,
    client_version: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    client_screen: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    device_model: Option<String>,
    platform: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    original_url: Option<String>,
    /// Language (`en`, `de`)
    hl: String,
    /// Country (`US`, `DE`)
    gl: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
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

#[derive(Clone, Debug, Serialize, Default)]
#[serde(rename_all = "camelCase")]
struct User {
    // TO DO: provide a way to enable restricted mode with:
    // "enableSafetyMode": true
    locked_safety_mode: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ThirdParty {
    embed_url: String,
}

const DEFAULT_UA: &str =
    "Mozilla/5.0 (Windows NT 10.0; Win64; rv:107.0) Gecko/20100101 Firefox/107.0";

const CONSENT_COOKIE: &str = "CONSENT";
const CONSENT_COOKIE_YES: &str = "YES+yt.462272069.de+FX+";
const CONSENT_COOKIE_NO: &str = "PENDING+";

const YOUTUBEI_V1_URL: &str = "https://www.youtube.com/youtubei/v1/";
const YOUTUBEI_V1_GAPIS_URL: &str = "https://youtubei.googleapis.com/youtubei/v1/";

const DISABLE_PRETTY_PRINT_PARAMETER: &str = "&prettyPrint=false";

const DESKTOP_CLIENT_VERSION: &str = "2.20220721.05.00_1";
const DESKTOP_API_KEY: &str = "AIzaSyAO_FJ2SlqU8Q4STEHLGCilw_Y9_11qcW8";
const TVHTML5_CLIENT_VERSION: &str = "2.0";

const MOBILE_CLIENT_VERSION: &str = "17.10.35";
const ANDROID_API_KEY: &str = "AIzaSyA8eiZmM1FaDVjRy-df2KTyQ_vz_yYM39w";
const IOS_API_KEY: &str = "AIzaSyB-63vPrdThhKuerbB2N_l7Kwwcxj6yUAc";
const IOS_DEVICE_MODEL: &str = "iPhone14,5";

pub struct RustyTube {
    pub locale: Arc<Locale>,
    desktop_client: Arc<DesktopClient>,
    android_client: Arc<AndroidClient>,
    ios_client: Arc<IosClient>,
}

#[derive(Clone)]
pub struct Locale {
    lang: String,
    country: String,
}

impl RustyTube {
    #[must_use]
    pub fn new() -> Self {
        Self::new_with_ua("en", "US")
    }

    #[must_use]
    pub fn new_with_ua(lang: &str, country: &str) -> Self {
        let locale = Arc::new(Locale {
            lang: lang.to_owned(),
            country: country.to_owned(),
        });

        Self {
            locale: locale.clone(),
            desktop_client: Arc::new(DesktopClient::new(locale.clone())),
            android_client: Arc::new(AndroidClient::new(locale.clone())),
            ios_client: Arc::new(IosClient::new(locale)),
        }
    }

    pub fn get_ytclient(&self, client_type: ClientType) -> Arc<dyn YTClient> {
        match client_type {
            ClientType::Desktop => self.desktop_client.clone(),
            ClientType::DesktopMusic => todo!(),
            ClientType::TvHtml5Embed => todo!(),
            ClientType::Android => self.android_client.clone(),
            ClientType::Ios => self.ios_client.clone(),
        }
    }
}

#[async_trait]
pub trait YTClient {
    // fn new(locale: Arc<Locale>) -> Self;

    async fn get_context(&self, localized: bool) -> ContextYT;
    async fn request_builder(&self, method: Method, url: &str) -> RequestBuilder;
    async fn exec_request(&self, request: Request) -> Result<Response>;
    async fn exec_request_text(&self, request: Request) -> Result<String>;
}

pub struct DesktopClient {
    locale: Arc<Locale>,
    http: Client,
    data: Mutex<DesktopClientData>,
    consent_cookie_yes: String,
    consent_cookie_no: String,
    deobf: Deobfuscator,
}

#[derive(Debug)]
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

#[async_trait]
impl YTClient for DesktopClient {
    async fn get_context(&self, localized: bool) -> ContextYT {
        ContextYT {
            client: ClientInfo {
                client_name: "WEB".to_owned(),
                client_version: self.get_client_version().await,
                client_screen: None,
                device_model: None,
                platform: "DESKTOP".to_owned(),
                original_url: Some("https://www.youtube.com".to_owned()),
                hl: match localized {
                    true => self.locale.lang.to_owned(),
                    false => "en".to_owned(),
                },
                gl: match localized {
                    true => self.locale.country.to_owned(),
                    false => "US".to_owned(),
                },
            },
            request: Some(RequestYT::default()),
            user: User::default(),
            third_party: None,
        }
    }

    async fn request_builder(&self, method: Method, endpoint: &str) -> RequestBuilder {
        self.http
            .request(
                method,
                format!(
                    "{}{}?key={}{}",
                    YOUTUBEI_V1_URL, endpoint, DESKTOP_API_KEY, DISABLE_PRETTY_PRINT_PARAMETER
                ),
            )
            .header(header::ORIGIN, "https://www.youtube.com")
            .header(header::REFERER, "https://www.youtube.com")
            .header(header::COOKIE, self.consent_cookie_no.to_owned())
            .header("X-YouTube-Client-Name", "1")
            .header("X-YouTube-Client-Version", self.get_client_version().await)
    }

    async fn exec_request(&self, request: Request) -> Result<Response> {
        Ok(self.http.execute(request).await?.error_for_status()?)
    }

    async fn exec_request_text(&self, request: Request) -> Result<String> {
        Ok(self.exec_request(request).await?.text().await?)
    }
}

impl DesktopClient {
    fn new(locale: Arc<Locale>) -> Self {
        let mut rng = rand::thread_rng();

        let http = ClientBuilder::new()
            .user_agent(DEFAULT_UA)
            .gzip(true)
            .brotli(true)
            .build()
            .expect("unable to build the HTTP client");

        let deobf = Deobfuscator::new(http.clone());

        Self {
            locale,
            http,
            data: Mutex::new(DesktopClientData::default()),
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
            deobf,
        }
    }

    async fn extract_client_version_from_swjs(&self) -> Result<Option<String>> {
        let swjs = self
            .exec_request_text(
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

        Ok(util::get_cg_from_regexes(
            CLIENT_VERSION_PATTERNS.iter(),
            &swjs,
            1,
        ))
    }

    async fn get_client_version(&self) -> String {
        let mut client_data = self.data.lock().await;

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
        client_data.client_version.to_owned()
    }
}

pub struct AndroidClient {
    locale: Arc<Locale>,
    http: Client,
}

#[async_trait]
impl YTClient for AndroidClient {
    async fn get_context(&self, localized: bool) -> ContextYT {
        ContextYT {
            client: ClientInfo {
                client_name: "ANDROID".to_owned(),
                client_version: MOBILE_CLIENT_VERSION.to_owned(),
                client_screen: None,
                device_model: None,
                platform: "MOBILE".to_owned(),
                original_url: None,
                hl: match localized {
                    true => self.locale.lang.to_owned(),
                    false => "en".to_owned(),
                },
                gl: match localized {
                    true => self.locale.country.to_owned(),
                    false => "US".to_owned(),
                },
            },
            request: None,
            user: User::default(),
            third_party: None,
        }
    }

    async fn request_builder(&self, method: Method, endpoint: &str) -> RequestBuilder {
        self.http
            .request(
                method,
                format!(
                    "{}{}?key={}{}",
                    YOUTUBEI_V1_GAPIS_URL,
                    endpoint,
                    ANDROID_API_KEY,
                    DISABLE_PRETTY_PRINT_PARAMETER
                ),
            )
            .header("X-Goog-Api-Format-Version", "2")
    }

    async fn exec_request(&self, request: Request) -> Result<Response> {
        Ok(self.http.execute(request).await?.error_for_status()?)
    }

    async fn exec_request_text(&self, request: Request) -> Result<String> {
        Ok(self.exec_request(request).await?.text().await?)
    }
}

impl AndroidClient {
    fn new(locale: Arc<Locale>) -> Self {
        let http = ClientBuilder::new()
            .user_agent(format!(
                "com.google.android.youtube/{} (Linux; U; Android 12; {}) gzip",
                MOBILE_CLIENT_VERSION, locale.country
            ))
            .gzip(true)
            .brotli(true)
            .build()
            .expect("unable to build the HTTP client");

        Self { locale, http }
    }
}

pub struct IosClient {
    locale: Arc<Locale>,
    http: Client,
}

#[async_trait]
impl YTClient for IosClient {
    async fn get_context(&self, localized: bool) -> ContextYT {
        ContextYT {
            client: ClientInfo {
                client_name: "IOS".to_owned(),
                client_version: MOBILE_CLIENT_VERSION.to_owned(),
                client_screen: None,
                device_model: Some(IOS_DEVICE_MODEL.to_owned()),
                platform: "MOBILE".to_owned(),
                original_url: None,
                hl: match localized {
                    true => self.locale.lang.to_owned(),
                    false => "en".to_owned(),
                },
                gl: match localized {
                    true => self.locale.country.to_owned(),
                    false => "US".to_owned(),
                },
            },
            request: None,
            user: User::default(),
            third_party: None,
        }
    }

    async fn request_builder(&self, method: Method, endpoint: &str) -> RequestBuilder {
        self.http
            .request(
                method,
                format!(
                    "{}{}?key={}{}",
                    YOUTUBEI_V1_GAPIS_URL,
                    endpoint,
                    ANDROID_API_KEY,
                    DISABLE_PRETTY_PRINT_PARAMETER
                ),
            )
            .header("X-Goog-Api-Format-Version", "2")
    }

    async fn exec_request(&self, request: Request) -> Result<Response> {
        Ok(self.http.execute(request).await?.error_for_status()?)
    }

    async fn exec_request_text(&self, request: Request) -> Result<String> {
        Ok(self.exec_request(request).await?.text().await?)
    }
}

impl IosClient {
    fn new(locale: Arc<Locale>) -> Self {
        let http = ClientBuilder::new()
            .user_agent(format!(
                "com.google.ios.youtube/{} ({}; U; CPU iOS 15_4 like Mac OS X; {})",
                MOBILE_CLIENT_VERSION, IOS_DEVICE_MODEL, locale.country
            ))
            .gzip(true)
            .brotli(true)
            .build()
            .expect("unable to build the HTTP client");

        Self { locale, http }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use test_log::test;

    /*
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
    */
}
