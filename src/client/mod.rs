//! YouTube API Client

pub(crate) mod response;

mod channel;
mod music_artist;
mod music_playlist;
mod music_search;
mod pagination;
mod player;
mod playlist;
mod search;
mod trends;
mod url_resolver;
mod video_details;

#[cfg(feature = "rss")]
#[cfg_attr(docsrs, doc(cfg(feature = "rss")))]
mod channel_rss;

use std::sync::Arc;
use std::{borrow::Cow, fmt::Debug};

use fancy_regex::Regex;
use log::{debug, error, warn};
use once_cell::sync::Lazy;
use rand::Rng;
use reqwest::{header, Client, ClientBuilder, Request, RequestBuilder, Response};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use time::{Duration, OffsetDateTime};
use tokio::sync::RwLock;

use crate::{
    cache::{CacheStorage, FileStorage},
    deobfuscate::{DeobfData, Deobfuscator},
    error::{Error, ExtractionError},
    param::{Country, Language},
    report::{FileReporter, Level, Report, Reporter},
    serializer::MapResult,
    util,
};

/// Client types for accessing the YouTube API.
///
/// There are multiple clients for accessing the YouTube API which have
/// slightly different features
///
/// - **Desktop**: used by youtube.com
/// - **DesktopMusic**: used by music.youtube.com, can access special music data,
///   cannot access non-music content
/// - **TvHtml5Embed**: used by Smart TVs, can access age-restricted videos
/// - **Android**: used by the Android app, no obfuscated URLs, includes lower resolution audio streams
/// - **Ios**: used by the iOS app, no obfuscated URLs
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum ClientType {
    Desktop,
    DesktopMusic,
    TvHtml5Embed,
    Android,
    Ios,
}

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
pub struct YTContext<'a> {
    client: ClientInfo<'a>,
    /// only used on desktop
    #[serde(skip_serializing_if = "Option::is_none")]
    request: Option<RequestYT>,
    user: User,
    /// only used for the embedded player
    #[serde(skip_serializing_if = "Option::is_none")]
    third_party: Option<ThirdParty<'a>>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ClientInfo<'a> {
    client_name: &'a str,
    client_version: Cow<'a, str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    client_screen: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    device_model: Option<&'a str>,
    platform: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    original_url: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    visitor_data: Option<&'a str>,
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
    // TODO: provide a way to enable restricted mode with:
    // "enableSafetyMode": true
    locked_safety_mode: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ThirdParty<'a> {
    embed_url: &'a str,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct QBrowse<'a> {
    context: YTContext<'a>,
    browse_id: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct QContinuation<'a> {
    context: YTContext<'a>,
    continuation: &'a str,
}

const DEFAULT_UA: &str = "Mozilla/5.0 (X11; Linux x86_64; rv:102.0) Gecko/20100101 Firefox/102.0";

const CONSENT_COOKIE: &str = "CONSENT";
const CONSENT_COOKIE_YES: &str = "YES+yt.462272069.de+FX+";

const YOUTUBEI_V1_URL: &str = "https://www.youtube.com/youtubei/v1/";
const YOUTUBEI_V1_GAPIS_URL: &str = "https://youtubei.googleapis.com/youtubei/v1/";
const YOUTUBE_MUSIC_V1_URL: &str = "https://music.youtube.com/youtubei/v1/";
const YOUTUBE_MUSIC_HOME_URL: &str = "https://music.youtube.com/";

const DISABLE_PRETTY_PRINT_PARAMETER: &str = "&prettyPrint=false";

const DESKTOP_CLIENT_VERSION: &str = "2.20221011.00.00";
const DESKTOP_API_KEY: &str = "AIzaSyAO_FJ2SlqU8Q4STEHLGCilw_Y9_11qcW8";
const TVHTML5_CLIENT_VERSION: &str = "2.0";
const DESKTOP_MUSIC_API_KEY: &str = "AIzaSyC9XL3ZjWddXya6X74dJoCTL-WEYFDNX30";
const DESKTOP_MUSIC_CLIENT_VERSION: &str = "1.20221005.01.00";

const MOBILE_CLIENT_VERSION: &str = "17.39.35";
const ANDROID_API_KEY: &str = "AIzaSyA8eiZmM1FaDVjRy-df2KTyQ_vz_yYM39w";
const IOS_API_KEY: &str = "AIzaSyB-63vPrdThhKuerbB2N_l7Kwwcxj6yUAc";
const IOS_DEVICE_MODEL: &str = "iPhone14,5";

static CLIENT_VERSION_REGEXES: Lazy<[Regex; 1]> =
    Lazy::new(|| [Regex::new("INNERTUBE_CONTEXT_CLIENT_VERSION\":\"([0-9\\.]+?)\"").unwrap()]);

/// The RustyPipe client used to access YouTube's API
///
/// RustyPipe includes an `Arc` internally, so if you are using the client
/// at multiple locations, you can just clone it. Note that options (lang/country/report)
/// are not shared between clones.
#[derive(Clone)]
pub struct RustyPipe {
    inner: Arc<RustyPipeRef>,
}

struct RustyPipeRef {
    http: Client,
    storage: Option<Box<dyn CacheStorage>>,
    reporter: Option<Box<dyn Reporter>>,
    n_http_retries: u32,
    consent_cookie: String,
    cache: CacheHolder,
    default_opts: RustyPipeOpts,
}

#[derive(Clone)]
struct RustyPipeOpts {
    lang: Language,
    country: Country,
    report: bool,
    strict: bool,
    visitor_data: Option<String>,
}

pub struct RustyPipeBuilder {
    storage: Option<Box<dyn CacheStorage>>,
    reporter: Option<Box<dyn Reporter>>,
    n_http_retries: u32,
    user_agent: String,
    default_opts: RustyPipeOpts,
}

#[derive(Clone)]
pub struct RustyPipeQuery {
    client: RustyPipe,
    opts: RustyPipeOpts,
}

impl Default for RustyPipeOpts {
    fn default() -> Self {
        Self {
            lang: Language::En,
            country: Country::Us,
            report: false,
            strict: false,
            visitor_data: None,
        }
    }
}

#[derive(Default, Debug)]
struct CacheHolder {
    desktop_client: RwLock<CacheEntry<ClientData>>,
    music_client: RwLock<CacheEntry<ClientData>>,
    deobf: RwLock<CacheEntry<DeobfData>>,
}

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
struct CacheData {
    desktop_client: CacheEntry<ClientData>,
    music_client: CacheEntry<ClientData>,
    deobf: CacheEntry<DeobfData>,
}

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
enum CacheEntry<T> {
    #[default]
    None,
    Some {
        #[serde(with = "time::serde::rfc3339")]
        last_update: OffsetDateTime,
        data: T,
    },
}

#[derive(Debug, Default, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ClientData {
    pub version: String,
}

impl<T> CacheEntry<T> {
    fn get(&self) -> Option<&T> {
        match self {
            CacheEntry::Some { last_update, data } => {
                if last_update < &(OffsetDateTime::now_utc() - Duration::hours(24)) {
                    None
                } else {
                    Some(data)
                }
            }
            CacheEntry::None => None,
        }
    }
}

impl<T> From<T> for CacheEntry<T> {
    fn from(f: T) -> Self {
        Self::Some {
            last_update: util::now_sec(),
            data: f,
        }
    }
}

impl Default for RustyPipeBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl RustyPipeBuilder {
    /// Constructs a new `RustyPipeBuilder`.
    ///
    /// This is the same as `RustyPipe::builder()`
    pub fn new() -> Self {
        RustyPipeBuilder {
            default_opts: RustyPipeOpts::default(),
            storage: Some(Box::new(FileStorage::default())),
            reporter: Some(Box::new(FileReporter::default())),
            n_http_retries: 2,
            user_agent: DEFAULT_UA.to_owned(),
        }
    }

    /// Returns a new, configured RustyPipe instance.
    pub fn build(self) -> RustyPipe {
        let http = ClientBuilder::new()
            .user_agent(self.user_agent)
            .gzip(true)
            .brotli(true)
            .build()
            .unwrap();

        let cdata = if let Some(storage) = &self.storage {
            if let Some(data) = storage.read() {
                match serde_json::from_str::<CacheData>(&data) {
                    Ok(data) => data,
                    Err(e) => {
                        error!("Could not deserialize cache. Error: {}", e);
                        CacheData::default()
                    }
                }
            } else {
                CacheData::default()
            }
        } else {
            CacheData::default()
        };

        RustyPipe {
            inner: Arc::new(RustyPipeRef {
                http,
                storage: self.storage,
                reporter: self.reporter,
                n_http_retries: self.n_http_retries,
                consent_cookie: format!(
                    "{}={}{}",
                    CONSENT_COOKIE,
                    CONSENT_COOKIE_YES,
                    rand::thread_rng().gen_range(100..1000)
                ),
                cache: CacheHolder {
                    desktop_client: RwLock::new(cdata.desktop_client),
                    music_client: RwLock::new(cdata.music_client),
                    deobf: RwLock::new(cdata.deobf),
                },
                default_opts: self.default_opts,
            }),
        }
    }

    /// Add a `CacheStorage` backend for persisting cached information
    /// (YouTube client versions, deobfuscation code) between
    /// program executions.
    ///
    /// **Default value**: `FileStorage` in `rustypipe_cache.json`
    pub fn storage(mut self, storage: Box<dyn CacheStorage>) -> Self {
        self.storage = Some(storage);
        self
    }

    /// Disable cache storage
    pub fn no_storage(mut self) -> Self {
        self.storage = None;
        self
    }

    /// Add a `Reporter` to collect error details
    ///
    ///  **Default value**: `FileReporter` creating reports in `./rustypipe_reports`
    pub fn reporter(mut self, reporter: Box<dyn Reporter>) -> Self {
        self.reporter = Some(reporter);
        self
    }

    /// Disable the creation of report files in case of errors and warnings.
    pub fn no_reporter(mut self) -> Self {
        self.reporter = None;
        self
    }

    /// Set the number of retries for HTTP requests.
    ///
    /// If a HTTP requests fails and retries are enabled,
    /// RustyPipe waits 1 second before the next attempt.
    /// The waiting time is doubled for subsequent attempts (including a bit of
    /// random jitter to be less predictable).
    ///
    /// **Default value**: 2
    pub fn n_http_retries(mut self, n_retries: u32) -> Self {
        self.n_http_retries = n_retries;
        self
    }

    /// Set the user agent used for making requests to the web API.
    ///
    /// **Default value**: `Mozilla/5.0 (X11; Linux x86_64; rv:102.0) Gecko/20100101 Firefox/102.0`
    /// (Firefox ESR on Debian)
    pub fn user_agent(mut self, user_agent: &str) -> Self {
        self.user_agent = user_agent.to_owned();
        self
    }

    /// Set the language parameter used when accessing the YouTube API.
    /// This will change multilanguage video titles, descriptions and textual dates
    ///
    /// **Default value**: `Language::En` (English)
    ///
    /// **Info**: you can set this option for individual queries, too
    pub fn lang(mut self, lang: Language) -> Self {
        self.default_opts.lang = lang;
        self
    }

    /// Set the country parameter used when accessing the YouTube API.
    /// This will change trends and recommended content.
    ///
    /// **Default value**: `Country::Us` (USA)
    ///
    /// **Info**: you can set this option for individual queries, too
    pub fn country(mut self, country: Country) -> Self {
        self.default_opts.country = country;
        self
    }

    /// Generate a report on every operation.
    /// This should only be used for debugging.
    ///
    /// **Info**: you can set this option for individual queries, too
    pub fn report(mut self) -> Self {
        self.default_opts.report = true;
        self
    }

    /// Enable strict mode, causing operations to fail if there
    /// are warnings during deserialization (e.g. invalid items).
    /// This should only be used for testing.
    ///
    /// **Info**: you can set this option for individual queries, too
    pub fn strict(mut self) -> Self {
        self.default_opts.strict = true;
        self
    }

    /// Set the default YouTube visitor data cookie
    pub fn visitor_data(mut self, visitor_data: &str) -> Self {
        self.default_opts.visitor_data = Some(visitor_data.to_owned());
        self
    }

    /// Set the default YouTube visitor data cookie to an optional value
    pub fn visitor_data_opt(mut self, visitor_data: Option<String>) -> Self {
        self.default_opts.visitor_data = visitor_data;
        self
    }
}

impl Default for RustyPipe {
    fn default() -> Self {
        Self::new()
    }
}

impl RustyPipe {
    /// Create a new RustyPipe instance with default settings.
    ///
    /// To create an instance with custom options, use `RustyPipeBuilder` instead.
    pub fn new() -> Self {
        RustyPipeBuilder::new().build()
    }

    /// Constructs a new `RustyPipeBuilder`.
    ///
    /// This is the same as `RustyPipeBuilder::new()`
    pub fn builder() -> RustyPipeBuilder {
        RustyPipeBuilder::new()
    }

    /// Constructs a new `RustyPipeQuery`.
    pub fn query(&self) -> RustyPipeQuery {
        RustyPipeQuery {
            client: self.clone(),
            opts: self.inner.default_opts.clone(),
        }
    }

    /// Execute the given http request.
    async fn http_request(&self, request: Request) -> Result<Response, reqwest::Error> {
        let mut last_res = None;
        for n in 0..=self.inner.n_http_retries {
            let res = self.inner.http.execute(request.try_clone().unwrap()).await;
            let emsg = match &res {
                Ok(response) => {
                    let status = response.status();
                    // Immediately return in case of success or unrecoverable status code
                    if status.is_success() || !status.is_server_error() {
                        return res;
                    }
                    // TODO: handle 429 (captcha)
                    status.to_string()
                }
                Err(e) => {
                    // Immediately return in case of unrecoverable error
                    if !e.is_timeout() && !e.is_connect() {
                        return res;
                    }
                    e.to_string()
                }
            };

            let ms = util::retry_delay(n, 1000, 60000, 3);
            warn!("Retry attempt #{}. Error: {}. Waiting {} ms", n, emsg, ms);
            tokio::time::sleep(std::time::Duration::from_millis(ms.into())).await;

            last_res = Some(res);
        }
        last_res.unwrap()
    }

    /// Execute the given http request, returning an error in case of a
    /// non-successful status code.
    async fn http_request_estatus(&self, request: Request) -> Result<Response, Error> {
        let res = self.http_request(request).await?;
        let status = res.status();

        if status.is_client_error() || status.is_server_error() {
            Err(Error::HttpStatus(status.into()))
        } else {
            Ok(res)
        }
    }

    /// Execute the given http request, returning the response body as a string.
    async fn http_request_txt(&self, request: Request) -> Result<String, Error> {
        Ok(self.http_request_estatus(request).await?.text().await?)
    }

    /// Extract the current version of the YouTube desktop client from the website.
    async fn extract_desktop_client_version(&self) -> Result<String, Error> {
        let from_swjs = async {
            let swjs = self
                .http_request_txt(
                    self.inner
                        .http
                        .get("https://www.youtube.com/sw.js")
                        .header(header::ORIGIN, "https://www.youtube.com")
                        .header(header::REFERER, "https://www.youtube.com")
                        .header(header::COOKIE, self.inner.consent_cookie.to_owned())
                        .build()
                        .unwrap(),
                )
                .await?;

            util::get_cg_from_regexes(CLIENT_VERSION_REGEXES.iter(), &swjs, 1).ok_or(
                Error::Extraction(ExtractionError::InvalidData(Cow::Borrowed(
                    "Could not find desktop client version in sw.js",
                ))),
            )
        };

        let from_html = async {
            let html = self
                .http_request_txt(
                    self.inner
                        .http
                        .get("https://www.youtube.com/results?search_query=")
                        .build()
                        .unwrap(),
                )
                .await?;

            util::get_cg_from_regexes(CLIENT_VERSION_REGEXES.iter(), &html, 1).ok_or(
                Error::Extraction(ExtractionError::InvalidData(Cow::Borrowed(
                    "Could not find desktop client version on html page",
                ))),
            )
        };

        match from_swjs.await {
            Ok(client_version) => Ok(client_version),
            Err(_) => from_html.await,
        }
    }

    /// Extract the current version of the YouTube Music desktop client from the website.
    async fn extract_music_client_version(&self) -> Result<String, Error> {
        let from_swjs = async {
            let swjs = self
                .http_request_txt(
                    self.inner
                        .http
                        .get("https://music.youtube.com/sw.js")
                        .header(header::ORIGIN, "https://music.youtube.com")
                        .header(header::REFERER, "https://music.youtube.com")
                        .header(header::COOKIE, self.inner.consent_cookie.to_owned())
                        .build()
                        .unwrap(),
                )
                .await?;

            util::get_cg_from_regexes(CLIENT_VERSION_REGEXES.iter(), &swjs, 1).ok_or(
                Error::Extraction(ExtractionError::InvalidData(Cow::Borrowed(
                    "Could not find music client version in sw.js",
                ))),
            )
        };

        let from_html = async {
            let html = self
                .http_request_txt(
                    self.inner
                        .http
                        .get("https://music.youtube.com")
                        .build()
                        .unwrap(),
                )
                .await?;

            util::get_cg_from_regexes(CLIENT_VERSION_REGEXES.iter(), &html, 1).ok_or(
                Error::Extraction(ExtractionError::InvalidData(Cow::Borrowed(
                    "Could not find music client version on html page",
                ))),
            )
        };

        match from_swjs.await {
            Ok(client_version) => Ok(client_version),
            Err(_) => from_html.await,
        }
    }

    /// Get the current version of the YouTube web client from the following sources
    ///
    /// 1. from cache
    /// 2. from YouTube's service worker script (`sw.js`)
    /// 3. from the YouTube website
    /// 4. fall back to the hardcoded version
    async fn get_desktop_client_version(&self) -> String {
        // Write lock here to prevent concurrent tasks from fetching the same data
        let mut desktop_client = self.inner.cache.desktop_client.write().await;

        match desktop_client.get() {
            Some(cdata) => cdata.version.to_owned(),
            None => {
                debug!("getting desktop client version");
                match self.extract_desktop_client_version().await {
                    Ok(version) => {
                        *desktop_client = CacheEntry::from(ClientData {
                            version: version.to_owned(),
                        });
                        drop(desktop_client);
                        self.store_cache().await;
                        version
                    }
                    Err(e) => {
                        warn!("{}, falling back to hardcoded version", e);
                        DESKTOP_CLIENT_VERSION.to_owned()
                    }
                }
            }
        }
    }

    /// Get the current version of the YouTube Music web client from the following sources
    ///
    /// 1. from cache
    /// 2. from YouTube Music's service worker script (`sw.js`)
    /// 3. from the YouTube Music website
    /// 4. fall back to the hardcoded version
    async fn get_music_client_version(&self) -> String {
        // Write lock here to prevent concurrent tasks from fetching the same data
        let mut music_client = self.inner.cache.music_client.write().await;

        match music_client.get() {
            Some(cdata) => cdata.version.to_owned(),
            None => {
                debug!("getting music client version");
                match self.extract_music_client_version().await {
                    Ok(version) => {
                        *music_client = CacheEntry::from(ClientData {
                            version: version.to_owned(),
                        });
                        drop(music_client);
                        self.store_cache().await;
                        version
                    }
                    Err(e) => {
                        warn!("{}, falling back to hardcoded version", e);
                        DESKTOP_MUSIC_CLIENT_VERSION.to_owned()
                    }
                }
            }
        }
    }

    /// Instantiate a new deobfuscator from either cached or extracted YouTube JavaScript code.
    async fn get_deobf(&self) -> Result<Deobfuscator, Error> {
        // Write lock here to prevent concurrent tasks from fetching the same data
        let mut deobf = self.inner.cache.deobf.write().await;

        match deobf.get() {
            Some(deobf) => Ok(Deobfuscator::from(deobf.to_owned())),
            None => {
                debug!("getting deobfuscator");
                let new_deobf = Deobfuscator::new(self.inner.http.clone()).await?;
                *deobf = CacheEntry::from(new_deobf.get_data());
                drop(deobf);
                self.store_cache().await;
                Ok(new_deobf)
            }
        }
    }

    /// Write the given cache data to the storage backend.
    async fn store_cache(&self) {
        if let Some(storage) = &self.inner.storage {
            let cdata = CacheData {
                desktop_client: self.inner.cache.desktop_client.read().await.clone(),
                music_client: self.inner.cache.music_client.read().await.clone(),
                deobf: self.inner.cache.deobf.read().await.clone(),
            };

            match serde_json::to_string(&cdata) {
                Ok(data) => storage.write(&data),
                Err(e) => error!("Could not serialize cache. Error: {}", e),
            }
        }
    }

    async fn get_ytm_visitor_data(&self) -> Result<String, Error> {
        let resp = self.inner.http.get(YOUTUBE_MUSIC_HOME_URL).send().await?;

        resp.headers()
            .get_all(header::SET_COOKIE)
            .iter()
            .find_map(|c| {
                if let Ok(cookie) = c.to_str() {
                    if let Some(after) = cookie.strip_prefix("__Secure-YEC=") {
                        return after.split_once(';').map(|s| s.0.to_owned());
                    }
                }
                None
            })
            .ok_or(Error::Extraction(ExtractionError::InvalidData(
                Cow::Borrowed("could not get YTM cookies"),
            )))
    }
}

impl RustyPipeQuery {
    /// Set the language parameter used when accessing the YouTube API
    /// This will change multilanguage video titles, descriptions and textual dates
    pub fn lang(mut self, lang: Language) -> Self {
        self.opts.lang = lang;
        self
    }

    /// Set the country parameter used when accessing the YouTube API.
    /// This will change trends and recommended content.
    pub fn country(mut self, country: Country) -> Self {
        self.opts.country = country;
        self
    }

    /// Generate a report on every operation.
    /// This should only be used for debugging.
    pub fn report(mut self) -> Self {
        self.opts.report = true;
        self
    }

    /// Enable strict mode, causing operations to fail if there
    /// are warnings during deserialization (e.g. invalid items).
    /// This should only be used for testing.
    pub fn strict(mut self) -> Self {
        self.opts.strict = true;
        self
    }

    /// Set the YouTube visitor data cookie
    pub fn visitor_data(mut self, visitor_data: &str) -> Self {
        self.opts.visitor_data = Some(visitor_data.to_owned());
        self
    }

    /// Set the YouTube visitor data cookie to an optional value
    pub fn visitor_data_opt(mut self, visitor_data: Option<String>) -> Self {
        self.opts.visitor_data = visitor_data;
        self
    }

    /// Create a new context object, which is included in every request to
    /// the YouTube API and contains language, country and device parameters.
    ///
    /// # Parameters
    /// - `ctype`: Client type (`Desktop`, `DesktopMusic`, `Android`, ...)
    /// - `localized`: Whether to include the configured language and country
    pub async fn get_context<'a>(
        &'a self,
        ctype: ClientType,
        localized: bool,
        visitor_data: Option<&'a str>,
    ) -> YTContext {
        let hl = match localized {
            true => self.opts.lang,
            false => Language::En,
        };
        let gl = match localized {
            true => self.opts.country,
            false => Country::Us,
        };
        let visitor_data = self.opts.visitor_data.as_deref().or(visitor_data);

        match ctype {
            ClientType::Desktop => YTContext {
                client: ClientInfo {
                    client_name: "WEB",
                    client_version: Cow::Owned(self.client.get_desktop_client_version().await),
                    client_screen: None,
                    device_model: None,
                    platform: "DESKTOP",
                    original_url: Some("https://www.youtube.com/"),
                    visitor_data,
                    hl,
                    gl,
                },
                request: Some(RequestYT::default()),
                user: User::default(),
                third_party: None,
            },
            ClientType::DesktopMusic => YTContext {
                client: ClientInfo {
                    client_name: "WEB_REMIX",
                    client_version: Cow::Owned(self.client.get_music_client_version().await),
                    client_screen: None,
                    device_model: None,
                    platform: "DESKTOP",
                    original_url: Some("https://music.youtube.com/"),
                    visitor_data,
                    hl,
                    gl,
                },
                request: Some(RequestYT::default()),
                user: User::default(),
                third_party: None,
            },
            ClientType::TvHtml5Embed => YTContext {
                client: ClientInfo {
                    client_name: "TVHTML5_SIMPLY_EMBEDDED_PLAYER",
                    client_version: Cow::Borrowed(TVHTML5_CLIENT_VERSION),
                    client_screen: Some("EMBED"),
                    device_model: None,
                    platform: "TV",
                    original_url: None,
                    visitor_data,
                    hl,
                    gl,
                },
                request: Some(RequestYT::default()),
                user: User::default(),
                third_party: Some(ThirdParty {
                    embed_url: "https://www.youtube.com/",
                }),
            },
            ClientType::Android => YTContext {
                client: ClientInfo {
                    client_name: "ANDROID",
                    client_version: Cow::Borrowed(MOBILE_CLIENT_VERSION),
                    client_screen: None,
                    device_model: None,
                    platform: "MOBILE",
                    original_url: None,
                    visitor_data,
                    hl,
                    gl,
                },
                request: None,
                user: User::default(),
                third_party: None,
            },
            ClientType::Ios => YTContext {
                client: ClientInfo {
                    client_name: "IOS",
                    client_version: Cow::Borrowed(MOBILE_CLIENT_VERSION),
                    client_screen: None,
                    device_model: Some(IOS_DEVICE_MODEL),
                    platform: "MOBILE",
                    original_url: None,
                    visitor_data,
                    hl,
                    gl,
                },
                request: None,
                user: User::default(),
                third_party: None,
            },
        }
    }

    /// Create a new Reqwest HTTP request builder with the URL and headers required
    /// for accessing the YouTube API
    ///
    /// # Parameters
    /// - `ctype`: Client type (`Desktop`, `DesktopMusic`, `Android`, ...)
    /// - `method`: HTTP method
    /// - `endpoint`: YouTube API endpoint (`https://www.youtube.com/youtubei/v1/<XYZ>?key=...`)
    async fn request_builder(&self, ctype: ClientType, endpoint: &str) -> RequestBuilder {
        match ctype {
            ClientType::Desktop => self
                .client
                .inner
                .http
                .post(format!(
                    "{}{}?key={}{}",
                    YOUTUBEI_V1_URL, endpoint, DESKTOP_API_KEY, DISABLE_PRETTY_PRINT_PARAMETER
                ))
                .header(header::ORIGIN, "https://www.youtube.com")
                .header(header::REFERER, "https://www.youtube.com")
                .header(header::COOKIE, self.client.inner.consent_cookie.to_owned())
                .header("X-YouTube-Client-Name", "1")
                .header(
                    "X-YouTube-Client-Version",
                    self.client.get_desktop_client_version().await,
                ),
            ClientType::DesktopMusic => self
                .client
                .inner
                .http
                .post(format!(
                    "{}{}?key={}{}",
                    YOUTUBE_MUSIC_V1_URL,
                    endpoint,
                    DESKTOP_MUSIC_API_KEY,
                    DISABLE_PRETTY_PRINT_PARAMETER
                ))
                .header(header::ORIGIN, "https://music.youtube.com")
                .header(header::REFERER, "https://music.youtube.com")
                .header(header::COOKIE, self.client.inner.consent_cookie.to_owned())
                .header("X-YouTube-Client-Name", "67")
                .header(
                    "X-YouTube-Client-Version",
                    self.client.get_music_client_version().await,
                ),
            ClientType::TvHtml5Embed => self
                .client
                .inner
                .http
                .post(format!(
                    "{}{}?key={}{}",
                    YOUTUBEI_V1_URL, endpoint, DESKTOP_API_KEY, DISABLE_PRETTY_PRINT_PARAMETER
                ))
                .header(header::ORIGIN, "https://www.youtube.com")
                .header(header::REFERER, "https://www.youtube.com")
                .header("X-YouTube-Client-Name", "1")
                .header("X-YouTube-Client-Version", TVHTML5_CLIENT_VERSION),
            ClientType::Android => self
                .client
                .inner
                .http
                .post(format!(
                    "{}{}?key={}{}",
                    YOUTUBEI_V1_GAPIS_URL,
                    endpoint,
                    ANDROID_API_KEY,
                    DISABLE_PRETTY_PRINT_PARAMETER
                ))
                .header(
                    header::USER_AGENT,
                    format!(
                        "com.google.android.youtube/{} (Linux; U; Android 12; {}) gzip",
                        MOBILE_CLIENT_VERSION, self.opts.country
                    ),
                )
                .header("X-Goog-Api-Format-Version", "2"),
            ClientType::Ios => self
                .client
                .inner
                .http
                .post(format!(
                    "{}{}?key={}{}",
                    YOUTUBEI_V1_GAPIS_URL, endpoint, IOS_API_KEY, DISABLE_PRETTY_PRINT_PARAMETER
                ))
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

    /// Execute a request to the YouTube API, then deobfuscate and map the response.
    ///
    /// Creates a report in case of failure for easy debugging.
    ///
    /// # Parameters
    /// - `ctype`: Client type (`Desktop`, `DesktopMusic`, `Android`, ...)
    /// - `operation`: Name of the RustyPipe operation (only for reporting, e.g. `get_player`)
    /// - `id`: ID of the requested entity (Video ID, Channel ID, ...).
    ///   The ID is included in reports and is also passed to the mapper for validating the response.
    ///   Set it to an empty string if you are not requesting an entity with an ID.
    /// - `method`: HTTP method
    /// - `endpoint`: YouTube API endpoint (`https://www.youtube.com/youtubei/v1/<XYZ>?key=...`)
    /// - `body`: Serializable request body to be sent in json format
    /// - `deobf`: Deobfuscator (is passed to the mapper to deobfuscate stream URLs).
    async fn execute_request_deobf<
        R: DeserializeOwned + MapResponse<M> + Debug,
        M,
        B: Serialize + ?Sized,
    >(
        &self,
        ctype: ClientType,
        operation: &str,
        id: &str,
        endpoint: &str,
        body: &B,
        deobf: Option<&Deobfuscator>,
    ) -> Result<M, Error> {
        let request = self
            .request_builder(ctype, endpoint)
            .await
            .json(body)
            .build()?;

        let request_url = request.url().to_string();
        let request_headers = request.headers().to_owned();

        let response = self.client.http_request(request).await?;

        let status = response.status();
        let resp_str = response.text().await?;

        // Uncomment to debug response text
        // println!("{}", &resp_str);

        let create_report = |level: Level, error: Option<String>, msgs: Vec<String>| {
            if let Some(reporter) = &self.client.inner.reporter {
                let report = Report {
                    info: Default::default(),
                    level,
                    operation: format!("{}({})", operation, id),
                    error,
                    msgs,
                    deobf_data: deobf.map(Deobfuscator::get_data),
                    http_request: crate::report::HTTPRequest {
                        url: request_url,
                        method: "POST".to_string(),
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
            let status_code = status.as_u16();
            return if status_code == 404 {
                Err(Error::Extraction(ExtractionError::ContentUnavailable(
                    "Not found".into(),
                )))
            } else {
                let e = Error::HttpStatus(status_code);
                create_report(Level::ERR, Some(e.to_string()), vec![]);
                Err(e)
            };
        }

        match serde_json::from_str::<R>(&resp_str) {
            Ok(deserialized) => match deserialized.map_response(id, self.opts.lang, deobf) {
                Ok(mapres) => {
                    if !mapres.warnings.is_empty() {
                        create_report(
                            Level::WRN,
                            Some(ExtractionError::DeserializationWarnings.to_string()),
                            mapres.warnings,
                        );

                        if self.opts.strict {
                            return Err(Error::Extraction(
                                ExtractionError::DeserializationWarnings,
                            ));
                        }
                    } else if self.opts.report {
                        create_report(Level::DBG, None, vec![]);
                    }
                    Ok(mapres.c)
                }
                Err(e) => {
                    if e.should_report() {
                        create_report(Level::ERR, Some(e.to_string()), Vec::new());
                    }
                    Err(e.into())
                }
            },
            Err(e) => {
                create_report(Level::ERR, Some(e.to_string()), Vec::new());
                Err(Error::from(ExtractionError::from(e)))
            }
        }
    }

    /// Execute a request to the YouTube API, then map the response.
    ///
    /// Creates a report in case of failure for easy debugging.
    ///
    /// # Parameters
    /// - `ctype`: Client type (`Desktop`, `DesktopMusic`, `Android`, ...)
    /// - `operation`: Name of the RustyPipe operation (only for reporting, e.g. `get_player`)
    /// - `id`: ID of the requested entity (Video ID, Channel ID, ...).
    ///   The ID is included in reports and is also passed to the mapper for validating the response.
    ///   Set it to an empty string if you are not requesting an entity with an ID.
    /// - `method`: HTTP method
    /// - `endpoint`: YouTube API endpoint (`https://www.youtube.com/youtubei/v1/<XYZ>?key=...`)
    /// - `body`: Serializable request body to be sent in json format
    async fn execute_request<
        R: DeserializeOwned + MapResponse<M> + Debug,
        M,
        B: Serialize + ?Sized,
    >(
        &self,
        ctype: ClientType,
        operation: &str,
        id: &str,
        endpoint: &str,
        body: &B,
    ) -> Result<M, Error> {
        self.execute_request_deobf::<R, M, B>(ctype, operation, id, endpoint, body, None)
            .await
    }

    /// Execute a request to the YouTube API and return the response string
    pub async fn raw<B: Serialize + ?Sized>(
        &self,
        ctype: ClientType,
        endpoint: &str,
        body: &B,
    ) -> Result<String, Error> {
        let request = self
            .request_builder(ctype, endpoint)
            .await
            .json(body)
            .build()?;

        self.client.http_request_txt(request).await
    }
}

/// Implement this for YouTube API response structs that need to be mapped to
/// RustyPipe models.
trait MapResponse<T> {
    /// Map the YouTube API response structs to a RustyPipe model.
    ///
    /// Returns an error if crucial data required for the model could not be extracted.
    ///
    /// Returns a `MapResult` with warnings if there were issues with the deserializing/mapping,
    /// but the resulting data is still usable.
    ///
    /// # Parameters
    /// - `id`: The ID of the requested entity (Video ID, Channel ID, ...). If possible, assert
    ///   that the returned entity matches this ID and return an error instead.
    /// - `lang`: Language of the request. Used for mapping localized information like dates.
    /// - `deobf`: Deobfuscator (if passed to the `execute_request_deobf` method)
    fn map_response(
        self,
        id: &str,
        lang: Language,
        deobf: Option<&Deobfuscator>,
    ) -> Result<MapResult<T>, ExtractionError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn t_get_ytm_visitor_data() {
        let rp = RustyPipe::new();
        let visitor_data = rp.get_ytm_visitor_data().await.unwrap();
        assert!(visitor_data.ends_with("%3D"));
        assert_eq!(visitor_data.len(), 32)
    }
}
