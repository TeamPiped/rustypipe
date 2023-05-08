//! YouTube API Client

pub(crate) mod response;

mod channel;
mod music_artist;
mod music_charts;
mod music_details;
mod music_genres;
mod music_new;
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

use once_cell::sync::Lazy;
use rand::Rng;
use regex::Regex;
use reqwest::{header, Client, ClientBuilder, Request, RequestBuilder, Response, StatusCode};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use time::{Duration, OffsetDateTime};
use tokio::sync::RwLock;

use crate::{
    cache::{CacheStorage, FileStorage},
    deobfuscate::DeobfData,
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
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum ClientType {
    /// Client used by youtube.com
    Desktop,
    /// Client used by music.youtube.com
    ///
    /// can access YTM-specific data, cannot access non-music content
    DesktopMusic,
    /// Client used by the embedded player for Smart TVs
    ///
    /// can access age-restricted videos, cannot access non-embeddable videos
    TvHtml5Embed,
    /// Client used by the Android app
    ///
    /// no obfuscated stream URLs, includes lower resolution audio streams
    Android,
    /// Client used by the iOS app
    ///
    /// no obfuscated stream URLs
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

/// YouTube context request parameter
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
    time_zone: &'a str,
    utc_offset_minutes: i16,
}

impl Default for ClientInfo<'_> {
    fn default() -> Self {
        Self {
            client_name: Default::default(),
            client_version: Default::default(),
            client_screen: None,
            device_model: None,
            platform: Default::default(),
            original_url: None,
            visitor_data: None,
            hl: Language::En,
            gl: Country::Us,
            time_zone: "UTC",
            utc_offset_minutes: 0,
        }
    }
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
    browse_id: &'a str,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct QBrowseParams<'a> {
    context: YTContext<'a>,
    browse_id: &'a str,
    params: &'a str,
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

// Desktop client
const DESKTOP_CLIENT_VERSION: &str = "2.20230126.00.00";
const DESKTOP_API_KEY: &str = "AIzaSyAO_FJ2SlqU8Q4STEHLGCilw_Y9_11qcW8";
const TVHTML5_CLIENT_VERSION: &str = "2.0";
const DESKTOP_MUSIC_API_KEY: &str = "AIzaSyC9XL3ZjWddXya6X74dJoCTL-WEYFDNX30";
const DESKTOP_MUSIC_CLIENT_VERSION: &str = "1.20230123.01.01";

// Mobile client
const MOBILE_CLIENT_VERSION: &str = "18.03.33";
const ANDROID_API_KEY: &str = "AIzaSyA8eiZmM1FaDVjRy-df2KTyQ_vz_yYM39w";
const IOS_API_KEY: &str = "AIzaSyB-63vPrdThhKuerbB2N_l7Kwwcxj6yUAc";
const IOS_DEVICE_MODEL: &str = "iPhone14,5";

static CLIENT_VERSION_REGEXES: Lazy<[Regex; 1]> =
    Lazy::new(|| [Regex::new(r#"INNERTUBE_CONTEXT_CLIENT_VERSION":"([\w\d\._-]+?)""#).unwrap()]);

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

/// Builder to construct a new RustyPipe client
pub struct RustyPipeBuilder {
    storage: Option<Box<dyn CacheStorage>>,
    no_storage: bool,
    reporter: Option<Box<dyn Reporter>>,
    no_reporter: bool,
    n_http_retries: u32,
    user_agent: Option<String>,
    default_opts: RustyPipeOpts,
}

/// RustyPipe query object
///
/// Contains a reference to the RustyPipe client as well as query-specific
/// options (e.g. language preference).
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
#[serde(default)]
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
struct ClientData {
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
            storage: None,
            no_storage: false,
            reporter: None,
            no_reporter: false,
            n_http_retries: 2,
            user_agent: None,
        }
    }

    /// Returns a new, configured RustyPipe instance.
    pub fn build(self) -> RustyPipe {
        let http = ClientBuilder::new()
            .user_agent(self.user_agent.unwrap_or_else(|| DEFAULT_UA.to_owned()))
            .gzip(true)
            .brotli(true)
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .unwrap();

        let cdata = if let Some(storage) = &self.storage {
            if let Some(data) = storage.read() {
                match serde_json::from_str::<CacheData>(&data) {
                    Ok(data) => data,
                    Err(e) => {
                        log::error!("Could not deserialize cache. Error: {}", e);
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
                storage: if self.no_storage {
                    None
                } else {
                    Some(
                        self.storage
                            .unwrap_or_else(|| Box::<FileStorage>::default()),
                    )
                },
                reporter: if self.no_reporter {
                    None
                } else {
                    Some(
                        self.reporter
                            .unwrap_or_else(|| Box::<FileReporter>::default()),
                    )
                },
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

    /// Add a [`CacheStorage`] backend for persisting cached information
    /// (YouTube client versions, deobfuscation code) between
    /// program executions.
    ///
    /// **Default value**: [`FileStorage`] in `rustypipe_cache.json`
    pub fn storage(mut self, storage: Box<dyn CacheStorage>) -> Self {
        self.storage = Some(storage);
        self.no_storage = false;
        self
    }

    /// Disable cache storage
    pub fn no_storage(mut self) -> Self {
        self.storage = None;
        self.no_storage = true;
        self
    }

    /// Add a `Reporter` to collect error details
    ///
    ///  **Default value**: [`FileReporter`] creating reports in `./rustypipe_reports`
    pub fn reporter(mut self, reporter: Box<dyn Reporter>) -> Self {
        self.reporter = Some(reporter);
        self.no_reporter = false;
        self
    }

    /// Disable the creation of report files in case of errors and warnings.
    pub fn no_reporter(mut self) -> Self {
        self.reporter = None;
        self.no_reporter = true;
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
    pub fn user_agent<S: Into<String>>(mut self, user_agent: S) -> Self {
        self.user_agent = Some(user_agent.into());
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
        self.default_opts.country = validate_country(country);
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
    pub fn visitor_data<S: Into<String>>(mut self, visitor_data: S) -> Self {
        self.default_opts.visitor_data = Some(visitor_data.into());
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
                    if status.is_success() || (!status.is_server_error() && status != 429) {
                        return res;
                    }
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
            log::warn!("Retry attempt #{}. Error: {}. Waiting {} ms", n, emsg, ms);
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
            Err(Error::HttpStatus(status.into(), "none".into()))
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
        self.extract_client_version(
            Some("https://www.youtube.com/sw.js"),
            "https://www.youtube.com/results?search_query=",
            "https://www.youtube.com",
            None,
        )
        .await
    }

    /// Extract the current version of the YouTube Music desktop client from the website.
    async fn extract_music_client_version(&self) -> Result<String, Error> {
        self.extract_client_version(
            Some("https://music.youtube.com/sw.js"),
            "https://music.youtube.com",
            "https://music.youtube.com",
            None,
        )
        .await
    }

    async fn extract_client_version(
        &self,
        sw_url: Option<&str>,
        html_url: &str,
        origin: &str,
        ua: Option<&str>,
    ) -> Result<String, Error> {
        let from_swjs = sw_url.map(|sw_url| async move {
            let swjs = self
                .http_request_txt(
                    self.inner
                        .http
                        .get(sw_url)
                        .header(header::ORIGIN, origin)
                        .header(header::REFERER, origin)
                        .header(header::COOKIE, self.inner.consent_cookie.to_owned())
                        .build()
                        .unwrap(),
                )
                .await?;

            util::get_cg_from_regexes(CLIENT_VERSION_REGEXES.iter(), &swjs, 1).ok_or(
                Error::Extraction(ExtractionError::InvalidData(Cow::Borrowed(
                    "Could not find client version in sw.js",
                ))),
            )
        });

        let from_html = async {
            let mut builder = self.inner.http.get(html_url);
            if let Some(ua) = ua {
                builder = builder.header(header::USER_AGENT, ua);
            }

            let html = self.http_request_txt(builder.build().unwrap()).await?;

            util::get_cg_from_regexes(CLIENT_VERSION_REGEXES.iter(), &html, 1).ok_or(
                Error::Extraction(ExtractionError::InvalidData(Cow::Borrowed(
                    "Could not find client version on html page",
                ))),
            )
        };

        if let Some(from_swjs) = from_swjs {
            match from_swjs.await {
                Ok(client_version) => Ok(client_version),
                Err(_) => from_html.await,
            }
        } else {
            from_html.await
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
                log::debug!("getting desktop client version");
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
                        log::warn!("{}, falling back to hardcoded desktop client version", e);
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
                log::debug!("getting music client version");
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
                        log::warn!("{}, falling back to hardcoded music client version", e);
                        DESKTOP_MUSIC_CLIENT_VERSION.to_owned()
                    }
                }
            }
        }
    }

    /// Instantiate a new deobfuscator from either cached or extracted YouTube JavaScript code.
    async fn get_deobf_data(&self) -> Result<DeobfData, Error> {
        // Write lock here to prevent concurrent tasks from fetching the same data
        let mut deobf_data = self.inner.cache.deobf.write().await;

        match deobf_data.get() {
            Some(deobf_data) => Ok(deobf_data.clone()),
            None => {
                log::debug!("getting deobfuscator");
                let new_data = DeobfData::download(self.inner.http.clone()).await?;
                *deobf_data = CacheEntry::from(new_data.clone());
                drop(deobf_data);
                self.store_cache().await;
                Ok(new_data)
            }
        }
    }

    /// Write the current cache data to the storage backend.
    async fn store_cache(&self) {
        if let Some(storage) = &self.inner.storage {
            let cdata = CacheData {
                desktop_client: self.inner.cache.desktop_client.read().await.clone(),
                music_client: self.inner.cache.music_client.read().await.clone(),
                deobf: self.inner.cache.deobf.read().await.clone(),
            };

            match serde_json::to_string(&cdata) {
                Ok(data) => storage.write(&data),
                Err(e) => log::error!("Could not serialize cache. Error: {}", e),
            }
        }
    }

    async fn get_ytm_visitor_data(&self) -> Result<String, Error> {
        log::debug!("getting YTM visitor data");
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
        self.opts.country = validate_country(country);
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
    pub fn visitor_data<S: Into<String>>(mut self, visitor_data: S) -> Self {
        self.opts.visitor_data = Some(visitor_data.into());
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
                    platform: "DESKTOP",
                    original_url: Some("https://www.youtube.com/"),
                    visitor_data,
                    hl,
                    gl,
                    ..Default::default()
                },
                request: Some(RequestYT::default()),
                user: User::default(),
                third_party: None,
            },
            ClientType::DesktopMusic => YTContext {
                client: ClientInfo {
                    client_name: "WEB_REMIX",
                    client_version: Cow::Owned(self.client.get_music_client_version().await),
                    platform: "DESKTOP",
                    original_url: Some("https://music.youtube.com/"),
                    visitor_data,
                    hl,
                    gl,
                    ..Default::default()
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
                    platform: "TV",
                    visitor_data,
                    hl,
                    gl,
                    ..Default::default()
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
                    platform: "MOBILE",
                    visitor_data,
                    hl,
                    gl,
                    ..Default::default()
                },
                request: None,
                user: User::default(),
                third_party: None,
            },
            ClientType::Ios => YTContext {
                client: ClientInfo {
                    client_name: "IOS",
                    client_version: Cow::Borrowed(MOBILE_CLIENT_VERSION),
                    device_model: Some(IOS_DEVICE_MODEL),
                    platform: "MOBILE",
                    visitor_data,
                    hl,
                    gl,
                    ..Default::default()
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
                    "{YOUTUBEI_V1_URL}{endpoint}?key={DESKTOP_API_KEY}{DISABLE_PRETTY_PRINT_PARAMETER}"
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
                    "{YOUTUBE_MUSIC_V1_URL}{endpoint}?key={DESKTOP_MUSIC_API_KEY}{DISABLE_PRETTY_PRINT_PARAMETER}"
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
                    "{YOUTUBEI_V1_URL}{endpoint}?key={DESKTOP_API_KEY}{DISABLE_PRETTY_PRINT_PARAMETER}"
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
                    "{YOUTUBEI_V1_GAPIS_URL}{endpoint}?key={ANDROID_API_KEY}{DISABLE_PRETTY_PRINT_PARAMETER}"
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
                    "{YOUTUBEI_V1_GAPIS_URL}{endpoint}?key={IOS_API_KEY}{DISABLE_PRETTY_PRINT_PARAMETER}"
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

    /// Get a YouTube Music visitor data cookie, which is necessary for certain requests
    async fn get_ytm_visitor_data(&self) -> Result<String, Error> {
        match &self.opts.visitor_data {
            Some(vd) => Ok(vd.to_owned()),
            None => self.client.get_ytm_visitor_data().await,
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
        deobf: Option<&DeobfData>,
    ) -> Result<M, Error> {
        log::debug!("getting {}({})", operation, id);

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
                    operation: format!("{operation}({id})"),
                    error,
                    msgs,
                    deobf_data: deobf.cloned(),
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
            let error_msg = serde_json::from_str::<response::ErrorResponse>(&resp_str)
                .map(|r| r.error.message)
                .unwrap_or_default();

            return match status {
                StatusCode::NOT_FOUND => Err(Error::Extraction(
                    ExtractionError::ContentUnavailable(error_msg.into()),
                )),
                StatusCode::BAD_REQUEST => Err(Error::Extraction(ExtractionError::BadRequest(
                    error_msg.into(),
                ))),
                _ => Err(Error::HttpStatus(status.as_u16(), error_msg.into())),
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
                    if e.should_report() || self.opts.report {
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

impl AsRef<RustyPipeQuery> for RustyPipeQuery {
    fn as_ref(&self) -> &RustyPipeQuery {
        self
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
        deobf: Option<&DeobfData>,
    ) -> Result<MapResult<T>, ExtractionError>;
}

fn validate_country(country: Country) -> Country {
    if country == Country::Zz {
        log::warn!("Country:Zz (Global) can only be used for fetching music charts, falling back to Country:Us");
        Country::Us
    } else {
        country
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn get_major_version(version: &str) -> u32 {
        let parts = version.split('.').collect::<Vec<_>>();
        assert_eq!(parts.len(), 4);
        parts[0].parse().unwrap()
    }

    #[test]
    fn t_extract_desktop_client_version() {
        let rp = RustyPipe::new();
        let version = tokio_test::block_on(rp.extract_desktop_client_version()).unwrap();
        assert!(get_major_version(&version) >= 2);
    }

    #[test]
    fn t_extract_music_client_version() {
        let rp = RustyPipe::new();
        let version = tokio_test::block_on(rp.extract_music_client_version()).unwrap();
        assert!(get_major_version(&version) >= 1);
    }

    #[test]
    fn t_get_ytm_visitor_data() {
        let rp = RustyPipe::new();
        let visitor_data = tokio_test::block_on(rp.get_ytm_visitor_data()).unwrap();
        assert!(visitor_data.ends_with("%3D"));
        assert_eq!(visitor_data.len(), 32)
    }
}
