pub mod player;
pub mod playlist;

mod response;

use std::fmt::Debug;
use std::sync::Arc;

use anyhow::{anyhow, Context, Result};
use fancy_regex::Regex;
use once_cell::sync::Lazy;
use rand::Rng;
use reqwest::{header, Client, ClientBuilder, Method, RequestBuilder};
use serde::{de::DeserializeOwned, Deserialize, Serialize};

use crate::{
    cache::Cache,
    deobfuscate::Deobfuscator,
    model::{Country, Language},
    report::{Level, Report, Reporter, YamlFileReporter},
};

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum ClientType {
    Desktop,
    DesktopMusic,
    TvHtml5Embed,
    Android,
    Ios,
}

const CLIENT_TYPES: [ClientType; 5] = [
    ClientType::Desktop,
    ClientType::DesktopMusic,
    ClientType::TvHtml5Embed,
    ClientType::Android,
    ClientType::Ios,
];

impl ClientType {
    fn is_web(&self) -> bool {
        match self {
            ClientType::Desktop | ClientType::DesktopMusic | ClientType::TvHtml5Embed => true,
            ClientType::Android | ClientType::Ios => false,
        }
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
    hl: Language,
    gl: Country,
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

const DEFAULT_UA: &str = "Mozilla/5.0 (X11; Linux x86_64; rv:102.0) Gecko/20100101 Firefox/102.0";

const CONSENT_COOKIE: &str = "CONSENT";
const CONSENT_COOKIE_YES: &str = "YES+yt.462272069.de+FX+";

const YOUTUBEI_V1_URL: &str = "https://www.youtube.com/youtubei/v1/";
const YOUTUBEI_V1_GAPIS_URL: &str = "https://youtubei.googleapis.com/youtubei/v1/";
const YOUTUBE_MUSIC_V1_URL: &str = "https://music.youtube.com/youtubei/v1/";

const DISABLE_PRETTY_PRINT_PARAMETER: &str = "&prettyPrint=false";

const DESKTOP_CLIENT_VERSION: &str = "2.20220909.00.00";
const DESKTOP_API_KEY: &str = "AIzaSyAO_FJ2SlqU8Q4STEHLGCilw_Y9_11qcW8";
const TVHTML5_CLIENT_VERSION: &str = "2.0";
const DESKTOP_MUSIC_API_KEY: &str = "AIzaSyC9XL3ZjWddXya6X74dJoCTL-WEYFDNX30";
const DESKTOP_MUSIC_CLIENT_VERSION: &str = "1.20220831.01.02";

const MOBILE_CLIENT_VERSION: &str = "17.29.35";
const ANDROID_API_KEY: &str = "AIzaSyA8eiZmM1FaDVjRy-df2KTyQ_vz_yYM39w";
const IOS_API_KEY: &str = "AIzaSyB-63vPrdThhKuerbB2N_l7Kwwcxj6yUAc";
const IOS_DEVICE_MODEL: &str = "iPhone14,5";

static CLIENT_VERSION_REGEXES: Lazy<[Regex; 1]> =
    Lazy::new(|| [Regex::new("INNERTUBE_CONTEXT_CLIENT_VERSION\":\"([0-9\\.]+?)\"").unwrap()]);

#[derive(Clone)]
pub struct RustyPipe {
    inner: Arc<RustyPipeRef>,
    opts: RustyPipeOpts,
}

struct RustyPipeRef {
    http: Client,
    cache: Cache,
    reporter: Option<Box<dyn Reporter>>,
    user_agent: String,
    consent_cookie: String,
}

#[derive(Clone)]
struct RustyPipeOpts {
    lang: Language,
    country: Country,
    report: bool,
}

impl Default for RustyPipe {
    fn default() -> Self {
        Self::new(
            Some(Cache::from_json_file("RustyPipeCache.json")),
            Some(Box::new(YamlFileReporter::default())),
            None,
        )
    }
}

impl Default for RustyPipeOpts {
    fn default() -> Self {
        Self {
            lang: Language::En,
            country: Country::Us,
            report: false,
        }
    }
}

impl RustyPipe {
    pub fn new(
        cache: Option<Cache>,
        reporter: Option<Box<dyn Reporter>>,
        user_agent: Option<String>,
    ) -> Self {
        let cache = cache.unwrap_or_else(|| Cache::default());
        let user_agent = user_agent.unwrap_or(DEFAULT_UA.to_owned());

        let http = ClientBuilder::new()
            .user_agent(user_agent.to_owned())
            .gzip(true)
            .brotli(true)
            .build()
            .expect("unable to build the HTTP client");

        RustyPipe {
            inner: Arc::new(RustyPipeRef {
                http,
                cache,
                reporter,
                user_agent,
                consent_cookie: format!(
                    "{}={}{}",
                    CONSENT_COOKIE,
                    CONSENT_COOKIE_YES,
                    rand::thread_rng().gen_range(100..1000)
                ),
            }),
            opts: RustyPipeOpts::default(),
        }
    }

    pub fn lang(mut self, lang: Language) -> Self {
        self.opts.lang = lang;
        self
    }

    pub fn country(mut self, country: Country) -> Self {
        self.opts.country = country;
        self
    }

    pub fn report(mut self, report: bool) -> Self {
        self.opts.report = report;
        self
    }

    async fn get_context(&self, ctype: ClientType, localized: bool) -> ContextYT {
        let hl = match localized {
            true => self.opts.lang,
            false => Language::En,
        };
        let gl = match localized {
            true => self.opts.country,
            false => Country::Us,
        };

        match ctype {
            ClientType::Desktop => ContextYT {
                client: ClientInfo {
                    client_name: "WEB".to_owned(),
                    client_version: DESKTOP_CLIENT_VERSION.to_owned(),
                    client_screen: None,
                    device_model: None,
                    platform: "DESKTOP".to_owned(),
                    original_url: Some("https://www.youtube.com/".to_owned()),
                    hl,
                    gl,
                },
                request: Some(RequestYT::default()),
                user: User::default(),
                third_party: None,
            },
            ClientType::DesktopMusic => ContextYT {
                client: ClientInfo {
                    client_name: "WEB_REMIX".to_owned(),
                    client_version: DESKTOP_MUSIC_CLIENT_VERSION.to_owned(),
                    client_screen: None,
                    device_model: None,
                    platform: "DESKTOP".to_owned(),
                    original_url: Some("https://music.youtube.com/".to_owned()),
                    hl,
                    gl,
                },
                request: Some(RequestYT::default()),
                user: User::default(),
                third_party: None,
            },
            ClientType::TvHtml5Embed => ContextYT {
                client: ClientInfo {
                    client_name: "TVHTML5_SIMPLY_EMBEDDED_PLAYER".to_owned(),
                    client_version: TVHTML5_CLIENT_VERSION.to_owned(),
                    client_screen: Some("EMBED".to_owned()),
                    device_model: None,
                    platform: "TV".to_owned(),
                    original_url: None,
                    hl,
                    gl,
                },
                request: Some(RequestYT::default()),
                user: User::default(),
                third_party: Some(ThirdParty {
                    embed_url: "https://www.youtube.com/".to_owned(),
                }),
            },
            ClientType::Android => ContextYT {
                client: ClientInfo {
                    client_name: "ANDROID".to_owned(),
                    client_version: MOBILE_CLIENT_VERSION.to_owned(),
                    client_screen: None,
                    device_model: None,
                    platform: "MOBILE".to_owned(),
                    original_url: None,
                    hl,
                    gl,
                },
                request: None,
                user: User::default(),
                third_party: None,
            },
            ClientType::Ios => ContextYT {
                client: ClientInfo {
                    client_name: "IOS".to_owned(),
                    client_version: MOBILE_CLIENT_VERSION.to_owned(),
                    client_screen: None,
                    device_model: Some(IOS_DEVICE_MODEL.to_owned()),
                    platform: "MOBILE".to_owned(),
                    original_url: None,
                    hl,
                    gl,
                },
                request: None,
                user: User::default(),
                third_party: None,
            },
        }
    }

    async fn request_builder(
        &self,
        ctype: ClientType,
        method: Method,
        endpoint: &str,
    ) -> RequestBuilder {
        match ctype {
            ClientType::Desktop => self
                .inner
                .http
                .request(
                    method,
                    format!(
                        "{}{}?key={}{}",
                        YOUTUBEI_V1_URL, endpoint, DESKTOP_API_KEY, DISABLE_PRETTY_PRINT_PARAMETER
                    ),
                )
                .header(header::ORIGIN, "https://www.youtube.com")
                .header(header::REFERER, "https://www.youtube.com")
                .header(header::COOKIE, self.inner.consent_cookie.to_owned())
                .header("X-YouTube-Client-Name", "1")
                .header("X-YouTube-Client-Version", DESKTOP_CLIENT_VERSION),
            ClientType::DesktopMusic => self
                .inner
                .http
                .request(
                    method,
                    format!(
                        "{}{}?key={}{}",
                        YOUTUBE_MUSIC_V1_URL,
                        endpoint,
                        DESKTOP_MUSIC_API_KEY,
                        DISABLE_PRETTY_PRINT_PARAMETER
                    ),
                )
                .header(header::ORIGIN, "https://music.youtube.com")
                .header(header::REFERER, "https://music.youtube.com")
                .header(header::COOKIE, self.inner.consent_cookie.to_owned())
                .header("X-YouTube-Client-Name", "67")
                .header("X-YouTube-Client-Version", DESKTOP_MUSIC_CLIENT_VERSION),
            ClientType::TvHtml5Embed => self
                .inner
                .http
                .request(
                    method,
                    format!(
                        "{}{}?key={}{}",
                        YOUTUBEI_V1_URL, endpoint, DESKTOP_API_KEY, DISABLE_PRETTY_PRINT_PARAMETER
                    ),
                )
                .header(header::ORIGIN, "https://www.youtube.com")
                .header(header::REFERER, "https://www.youtube.com")
                .header("X-YouTube-Client-Name", "1")
                .header("X-YouTube-Client-Version", TVHTML5_CLIENT_VERSION),
            ClientType::Android => self
                .inner
                .http
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
                .header(
                    header::USER_AGENT,
                    format!(
                        "com.google.android.youtube/{} (Linux; U; Android 12; {}) gzip",
                        MOBILE_CLIENT_VERSION, self.opts.country
                    ),
                )
                .header("X-Goog-Api-Format-Version", "2"),
            ClientType::Ios => self
                .inner
                .http
                .request(
                    method,
                    format!(
                        "{}{}?key={}{}",
                        YOUTUBEI_V1_GAPIS_URL,
                        endpoint,
                        IOS_API_KEY,
                        DISABLE_PRETTY_PRINT_PARAMETER
                    ),
                )
                .header(
                    header::USER_AGENT,
                    format!(
                        "com.google.ios.youtube/{} ({}; U; CPU iOS 15_4 like Mac OS X; {})",
                        MOBILE_CLIENT_VERSION, IOS_DEVICE_MODEL, self.opts.country
                    ),
                )
                .header("X-Goog-Api-Format-Version", "2"),
        }
    }

    async fn execute_request<
        R: DeserializeOwned + MapResponse<M> + Debug,
        M,
        B: Serialize + ?Sized,
    >(
        &self,
        ctype: ClientType,
        operation: &str,
        method: Method,
        endpoint: &str,
        id: &str,
        body: &B,
        deobf: Option<&Deobfuscator>,
    ) -> Result<M> {
        let request = self
            .request_builder(ctype, method.clone(), endpoint)
            .await
            .json(body)
            .build()?;

        let request_url = request.url().to_string();
        let request_headers = request.headers().to_owned();

        let response = self.inner.http.execute(request).await?;

        let status = response.status();
        let resp_str = response.text().await?;

        let create_report = |level: Level, error: Option<String>, msgs: Vec<String>| {
            if let Some(reporter) = &self.inner.reporter {
                let report = Report {
                    package: "rustypipe".to_owned(),
                    version: "0.1.0".to_owned(),
                    date: chrono::Local::now(),
                    level,
                    operation: operation.to_owned(),
                    error,
                    msgs,
                    http_request: crate::report::HTTPRequest {
                        url: request_url,
                        method: method.to_string(),
                        req_header: request_headers
                            .iter()
                            .map(|(k, v)| {
                                (k.to_string(), v.to_str().unwrap_or_default().to_owned())
                            })
                            .collect(),
                        req_body: serde_json::to_string(body).unwrap_or_default(),
                        status: status.into(),
                        resp_body: resp_str.to_owned(),
                    },
                };

                reporter.report(&report);
            }
        };

        if status.is_client_error() || status.is_server_error() {
            let e = anyhow!("Server responded with error code {}", status);
            create_report(Level::ERR, Some(e.to_string()), vec![]);
            return Err(e);
        }

        match serde_json::from_str::<R>(&resp_str) {
            Ok(deserialized) => match deserialized.map_response(id, self.opts.lang, deobf) {
                Ok(mapres) => {
                    if !mapres.warnings.is_empty() {
                        create_report(
                            Level::WRN,
                            Some("Warnings during deserialization/mapping".to_owned()),
                            mapres.warnings,
                        );
                    } else if self.opts.report {
                        create_report(Level::DBG, None, vec![]);
                    }
                    Ok(mapres.c)
                }
                Err(e) => {
                    let emsg = "Could not map reponse";
                    create_report(Level::ERR, Some(emsg.to_owned()), vec![e.to_string()]);
                    Err(e).context(emsg)
                }
            },
            Err(e) => {
                let emsg = "Could not deserialize response";
                create_report(Level::ERR, Some(emsg.to_owned()), vec![e.to_string()]);
                Err(e).context(emsg)
            }
        }
    }
}

trait MapResponse<T> {
    fn map_response(
        self,
        id: &str,
        lang: Language,
        deobf: Option<&Deobfuscator>,
    ) -> Result<MapResult<T>>;
}

#[derive(Clone)]
pub struct MapResult<T> {
    pub c: T,
    pub warnings: Vec<String>,
}

impl<T> Debug for MapResult<T>
where
    T: Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.c.fmt(f)
    }
}

/*
#[cfg(test)]
mod tests {
    use super::*;
}
*/
