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

use std::path::PathBuf;
use std::sync::Arc;
use std::{borrow::Cow, fmt::Debug, time::Duration};

use once_cell::sync::Lazy;
use rand::Rng;
use regex::Regex;
use reqwest::{header, Client, ClientBuilder, Request, RequestBuilder, Response, StatusCode};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use time::OffsetDateTime;
use tokio::sync::RwLock;

use crate::{
    cache::{CacheStorage, FileStorage, DEFAULT_CACHE_FILE},
    deobfuscate::DeobfData,
    error::{Error, ExtractionError},
    param::{Country, Language},
    report::{FileReporter, Level, Report, Reporter, RustyPipeInfo, DEFAULT_REPORT_DIR},
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
    fn is_web(self) -> bool {
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
            client_name: "",
            client_version: Cow::default(),
            client_screen: None,
            device_model: None,
            platform: "",
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
const YOUTUBE_HOME_URL: &str = "https://www.youtube.com/";
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

static CLIENT_VERSION_REGEX: Lazy<Regex> =
    Lazy::new(|| Regex::new(r#""INNERTUBE_CONTEXT_CLIENT_VERSION":"([\w\d\._-]+?)""#).unwrap());
static VISITOR_DATA_REGEX: Lazy<Regex> =
    Lazy::new(|| Regex::new(r#""visitorData":"([\w\d_\-%]+?)""#).unwrap());

/// The RustyPipe client used to access YouTube's API
///
/// RustyPipe uses an [`Arc`] internally, so if you are using the client
/// at multiple locations, you can just clone it. Note that query options
/// (lang/country/report/visitor data) are not shared between clones.
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
    storage: DefaultOpt<Box<dyn CacheStorage>>,
    reporter: DefaultOpt<Box<dyn Reporter>>,
    n_http_retries: u32,
    timeout: DefaultOpt<Duration>,
    user_agent: Option<String>,
    default_opts: RustyPipeOpts,
    storage_dir: Option<PathBuf>,
}

enum DefaultOpt<T> {
    Some(T),
    None,
    Default,
}

impl<T> DefaultOpt<T> {
    fn or_default<F: FnOnce() -> T>(self, f: F) -> Option<T> {
        match self {
            DefaultOpt::Some(x) => Some(x),
            DefaultOpt::None => None,
            DefaultOpt::Default => Some(f()),
        }
    }
}

/// # RustyPipe query
///
/// ## Queries
///
/// ### YouTube
///
/// - **Video**
///   - [`player`](RustyPipeQuery::player)
///   - [`video_details`](RustyPipeQuery::video_details)
///   - [`video_comments`](RustyPipeQuery::video_comments)
/// - **Channel**
///   - [`channel_videos`](RustyPipeQuery::channel_videos)
///   - [`channel_videos_order`](RustyPipeQuery::channel_videos_order)
///   - [`channel_videos_tab`](RustyPipeQuery::channel_videos_tab)
///   - [`channel_videos_tab_order`](RustyPipeQuery::channel_videos_tab_order)
///   - [`channel_playlists`](RustyPipeQuery::channel_playlists)
///   - [`channel_search`](RustyPipeQuery::channel_search)
///   - [`channel_info`](RustyPipeQuery::channel_info)
///   - [`channel_rss`](RustyPipeQuery::channel_rss) (🔒 Feature `rss`)
/// - **Playlist** [`playlist`](RustyPipeQuery::playlist)
/// - **Search**
///   - [`search`](RustyPipeQuery::search)
///   - [`search_filter`](RustyPipeQuery::search_filter)
///   - [`search_suggestion`](RustyPipeQuery::search_suggestion)
/// - **Trending** [`trending`](RustyPipeQuery::trending)
/// - **Resolver** (convert URLs and strings to YouTube IDs)
///   - [`resolve_url`](RustyPipeQuery::resolve_url)
///   - [`resolve_string`](RustyPipeQuery::resolve_string)
///
/// ### YouTube Music
///
/// - **Playlist** [`music_playlist`](RustyPipeQuery::music_playlist)
/// - **Album** [`music_album`](RustyPipeQuery::music_album)
/// - **Artist** [`music_artist`](RustyPipeQuery::music_artist)
/// - **Search**
///   - [`music_search`](RustyPipeQuery::music_search)
///   - [`music_search_tracks`](RustyPipeQuery::music_search_tracks)
///   - [`music_search_videos`](RustyPipeQuery::music_search_videos)
///   - [`music_search_albums`](RustyPipeQuery::music_search_albums)
///   - [`music_search_artists`](RustyPipeQuery::music_search_artists)
///   - [`music_search_playlists`](RustyPipeQuery::music_search_playlists)
///   - [`music_search_playlists_filter`](RustyPipeQuery::music_search_playlists_filter)
///   - [`music_search_suggestion`](RustyPipeQuery::music_search_suggestion)
/// - **Radio**
///   - [`music_radio`](RustyPipeQuery::music_radio)
///   - [`music_radio_playlist`](RustyPipeQuery::music_radio_playlist)
///   - [`music_radio_track`](RustyPipeQuery::music_radio_track)
/// - **Track details**
///   - [`music_details`](RustyPipeQuery::music_details)
///   - [`music_lyrics`](RustyPipeQuery::music_lyrics)
///   - [`music_related`](RustyPipeQuery::music_related)
/// - **Moods/Genres**
///   - [`music_genres`](RustyPipeQuery::music_genres)
///   - [`music_genre`](RustyPipeQuery::music_genre)
/// - **Charts** [`music_charts`](RustyPipeQuery::music_charts)
/// - **New**
///   - [`music_new_albums`](RustyPipeQuery::music_new_albums)
///   - [`music_new_videos`](RustyPipeQuery::music_new_videos)
///
/// ## Options
///
/// You can set the language, country and visitor data cookie for individual requests.
///
/// ```
/// # use rustypipe::client::RustyPipe;
/// let rp = RustyPipe::new();
/// rp.query()
///     .country(rustypipe::param::Country::De)
///     .lang(rustypipe::param::Language::De)
///     .visitor_data("CgthZVRCd1dkbTlRWSj3v_miBg%3D%3D")
///     .player("ZeerrnuLi5E");
/// ```
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

/// Result of a successful HTTP request
struct RequestResult<T> {
    /// Result of the deserialiation/mapping
    res: Result<MapResult<T>, Error>,
    status: StatusCode,
    body: String,
}

impl<T> CacheEntry<T> {
    /// Get the content of the cache if it is still fresh
    fn get(&self) -> Option<&T> {
        match self {
            CacheEntry::Some { last_update, data } => {
                if last_update < &(OffsetDateTime::now_utc() - time::Duration::hours(24)) {
                    None
                } else {
                    Some(data)
                }
            }
            CacheEntry::None => None,
        }
    }

    /// Get the content of the cache, even if it is expired
    fn get_expired(&self) -> Option<&T> {
        match self {
            CacheEntry::Some { data, .. } => Some(data),
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
    /// Return a new `RustyPipeBuilder`.
    ///
    /// This is the same as [`RustyPipe::builder`]
    #[must_use]
    pub fn new() -> Self {
        RustyPipeBuilder {
            default_opts: RustyPipeOpts::default(),
            storage: DefaultOpt::Default,
            reporter: DefaultOpt::Default,
            timeout: DefaultOpt::Default,
            n_http_retries: 2,
            user_agent: None,
            storage_dir: None,
        }
    }

    /// Return a new, configured RustyPipe instance.
    pub fn build(self) -> Result<RustyPipe, Error> {
        self.build_with_client(ClientBuilder::new())
    }

    /// Return a new, configured RustyPipe instance using a Reqwest client builder.
    pub fn build_with_client(self, mut client_builder: ClientBuilder) -> Result<RustyPipe, Error> {
        client_builder = client_builder
            .user_agent(self.user_agent.unwrap_or_else(|| DEFAULT_UA.to_owned()))
            .gzip(true)
            .brotli(true)
            .redirect(reqwest::redirect::Policy::none());

        if let Some(timeout) = self.timeout.or_default(|| Duration::from_secs(20)) {
            client_builder = client_builder.timeout(timeout);
        }

        let http = client_builder.build()?;

        let storage_dir = self.storage_dir.unwrap_or_default();

        let storage = self.storage.or_default(|| {
            let mut cache_file = storage_dir.clone();
            cache_file.push(DEFAULT_CACHE_FILE);
            Box::new(FileStorage::new(cache_file))
        });

        let cdata = storage
            .as_ref()
            .and_then(|storage| storage.read())
            .and_then(|data| match serde_json::from_str::<CacheData>(&data) {
                Ok(data) => Some(data),
                Err(e) => {
                    log::error!("Could not deserialize cache. Error: {}", e);
                    None
                }
            })
            .unwrap_or_default();

        Ok(RustyPipe {
            inner: Arc::new(RustyPipeRef {
                http,
                storage,
                reporter: self.reporter.or_default(|| {
                    let mut report_dir = storage_dir;
                    report_dir.push(DEFAULT_REPORT_DIR);
                    Box::new(FileReporter::new(report_dir))
                }),
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
        })
    }

    /// Set the default directory to store the cachefile and reports.
    ///
    /// This option has no effect if the storage backend or reporter are manually set or disabled.
    ///
    /// **Default value**: current working directory
    #[must_use]
    pub fn storage_dir<P: Into<PathBuf>>(mut self, path: P) -> Self {
        self.storage_dir = Some(path.into());
        self
    }

    /// Add a [`CacheStorage`] backend for persisting cached information
    /// (YouTube client versions, deobfuscation code) between
    /// program executions.
    ///
    /// **Default value**: [`FileStorage`] in `rustypipe_cache.json`
    #[must_use]
    pub fn storage(mut self, storage: Box<dyn CacheStorage>) -> Self {
        self.storage = DefaultOpt::Some(storage);
        self
    }

    /// Disable cache storage
    #[must_use]
    pub fn no_storage(mut self) -> Self {
        self.storage = DefaultOpt::None;
        self
    }

    /// Add a `Reporter` to collect error details
    ///
    ///  **Default value**: [`FileReporter`] creating reports in `./rustypipe_reports`
    #[must_use]
    pub fn reporter(mut self, reporter: Box<dyn Reporter>) -> Self {
        self.reporter = DefaultOpt::Some(reporter);
        self
    }

    /// Disable the creation of report files in case of errors and warnings.
    #[must_use]
    pub fn no_reporter(mut self) -> Self {
        self.reporter = DefaultOpt::None;
        self
    }

    /// Enable a HTTP request timeout
    ///
    /// The timeout is applied from when the request starts connecting until the
    /// response body has finished.
    ///
    ///  **Default value**: 20s
    #[must_use]
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = DefaultOpt::Some(timeout);
        self
    }

    /// Disable the HTTP request timeout.
    #[must_use]
    pub fn no_timeout(mut self) -> Self {
        self.timeout = DefaultOpt::None;
        self
    }

    /// Set the number of retries for HTTP requests.
    ///
    /// If a HTTP requests fails because of a serverside error and retries are enabled,
    /// RustyPipe waits 1 second before the next attempt.
    ///
    /// The waiting time is doubled for subsequent attempts (including a bit of
    /// random jitter to be less predictable).
    ///
    /// **Default value**: 2
    #[must_use]
    pub fn n_http_retries(mut self, n_retries: u32) -> Self {
        self.n_http_retries = n_retries;
        self
    }

    /// Set the user agent used for making requests to the web API.
    ///
    /// **Default value**: `Mozilla/5.0 (X11; Linux x86_64; rv:102.0) Gecko/20100101 Firefox/102.0`
    /// (Firefox ESR on Debian)
    #[must_use]
    pub fn user_agent<S: Into<String>>(mut self, user_agent: S) -> Self {
        self.user_agent = Some(user_agent.into());
        self
    }

    /// Set the language parameter used when accessing the YouTube API.
    ///
    /// This will change multilanguage video titles, descriptions and textual dates
    ///
    /// **Default value**: `Language::En` (English)
    ///
    /// **Info**: you can set this option for individual queries, too
    #[must_use]
    pub fn lang(mut self, lang: Language) -> Self {
        self.default_opts.lang = lang;
        self
    }

    /// Set the country parameter used when accessing the YouTube API.
    ///
    /// This will change trends and recommended content.
    ///
    /// **Default value**: `Country::Us` (USA)
    ///
    /// **Info**: you can set this option for individual queries, too
    #[must_use]
    pub fn country(mut self, country: Country) -> Self {
        self.default_opts.country = validate_country(country);
        self
    }

    /// Generate a report on every operation.
    ///
    /// This should only be used for debugging.
    ///
    /// **Info**: you can set this option for individual queries, too
    #[must_use]
    pub fn report(mut self) -> Self {
        self.default_opts.report = true;
        self
    }

    /// Enable strict mode, causing operations to fail if there
    /// are warnings during deserialization (e.g. invalid items).
    ///
    /// This should only be used for testing.
    ///
    /// **Info**: you can set this option for individual queries, too
    #[must_use]
    pub fn strict(mut self) -> Self {
        self.default_opts.strict = true;
        self
    }

    /// Set the YouTube visitor data cookie
    ///
    /// YouTube assigns a session cookie to each user which is used for personalized
    /// recommendations. By default, RustyPipe does not send this cookie to preserve
    /// user privacy. For requests that mandatate the cookie, a new one is requested
    /// for every query.
    ///
    /// This option allows you to manually set the visitor data cookie of your client,
    /// allowing you to get personalized recommendations or reproduce A/B tests.
    ///
    /// Note that YouTube has a rate limit on the number of requests from a single
    /// visitor, so you should not use the same vistor data cookie for batch operations.
    ///
    /// **Info**: you can set this option for individual queries, too
    #[must_use]
    pub fn visitor_data<S: Into<String>>(mut self, visitor_data: S) -> Self {
        self.default_opts.visitor_data = Some(visitor_data.into());
        self
    }

    /// Set the YouTube visitor data cookie to an optional value
    ///
    /// see also [`RustyPipeBuilder::visitor_data`]
    ///
    /// **Info**: you can set this option for individual queries, too
    #[must_use]
    pub fn visitor_data_opt<S: Into<String>>(mut self, visitor_data: Option<S>) -> Self {
        self.default_opts.visitor_data = visitor_data.map(S::into);
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
    /// To create an instance with custom options, use [`RustyPipeBuilder`] instead.
    #[must_use]
    #[allow(clippy::missing_panics_doc)]
    pub fn new() -> Self {
        RustyPipeBuilder::new().build().unwrap()
    }

    /// Create a new [`RustyPipeBuilder`]
    ///
    /// This is the same as [`RustyPipeBuilder::new`]
    #[must_use]
    pub fn builder() -> RustyPipeBuilder {
        RustyPipeBuilder::new()
    }

    /// Create a new [`RustyPipeQuery`] to run an API request
    #[must_use]
    pub fn query(&self) -> RustyPipeQuery {
        RustyPipeQuery {
            client: self.clone(),
            opts: self.inner.default_opts.clone(),
        }
    }

    /// Execute the given http request.
    async fn http_request(&self, request: &Request) -> Result<Response, reqwest::Error> {
        let mut last_resp = None;
        for n in 0..=self.inner.n_http_retries {
            let resp = self
                .inner
                .http
                .execute(request.try_clone().unwrap())
                .await?;

            let status = resp.status();
            // Immediately return in case of success or unrecoverable status code
            if status.is_success()
                || (!status.is_server_error() && status != StatusCode::TOO_MANY_REQUESTS)
            {
                return Ok(resp);
            }

            // Retry in case of a recoverable status code (server err, too many requests)
            if n != self.inner.n_http_retries {
                let ms = util::retry_delay(n, 1000, 60000, 3);
                log::warn!(
                    "Retry attempt #{}. Error: {}. Waiting {} ms",
                    n + 1,
                    status,
                    ms
                );
                tokio::time::sleep(Duration::from_millis(ms.into())).await;
            }

            last_resp = Some(resp);
        }
        Ok(last_resp.unwrap())
    }

    /// Execute the given http request, returning an error in case of a
    /// non-successful status code.
    async fn http_request_estatus(&self, request: &Request) -> Result<Response, Error> {
        let res = self.http_request(request).await?;
        let status = res.status();

        if status.is_client_error() || status.is_server_error() {
            Err(Error::HttpStatus(status.into(), "none".into()))
        } else {
            Ok(res)
        }
    }

    /// Execute the given http request, returning the response body as a string.
    async fn http_request_txt(&self, request: &Request) -> Result<String, Error> {
        Ok(self.http_request_estatus(request).await?.text().await?)
    }

    /// Extract the current version of the YouTube desktop client from the website.
    async fn extract_desktop_client_version(&self) -> Result<String, Error> {
        self.extract_client_version(
            Some("https://www.youtube.com/sw.js"),
            "https://www.youtube.com/results?search_query=",
            YOUTUBE_HOME_URL,
            None,
        )
        .await
    }

    /// Extract the current version of the YouTube Music desktop client from the website.
    async fn extract_music_client_version(&self) -> Result<String, Error> {
        self.extract_client_version(
            Some("https://music.youtube.com/sw.js"),
            YOUTUBE_MUSIC_HOME_URL,
            YOUTUBE_MUSIC_HOME_URL,
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
                    &self
                        .inner
                        .http
                        .get(sw_url)
                        .header(header::ORIGIN, origin)
                        .header(header::REFERER, origin)
                        .header(header::COOKIE, self.inner.consent_cookie.clone())
                        .build()
                        .unwrap(),
                )
                .await?;

            util::get_cg_from_regex(&CLIENT_VERSION_REGEX, &swjs, 1).ok_or(Error::Extraction(
                ExtractionError::InvalidData(Cow::Borrowed(
                    "Could not find client version in sw.js",
                )),
            ))
        });

        let from_html = async {
            let mut builder = self.inner.http.get(html_url);
            if let Some(ua) = ua {
                builder = builder.header(header::USER_AGENT, ua);
            }

            let html = self.http_request_txt(&builder.build().unwrap()).await?;

            util::get_cg_from_regex(&CLIENT_VERSION_REGEX, &html, 1).ok_or(Error::Extraction(
                ExtractionError::InvalidData(Cow::Borrowed(
                    "Could not find client version on html page",
                )),
            ))
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
            Some(cdata) => cdata.version.clone(),
            None => {
                log::debug!("getting desktop client version");
                match self.extract_desktop_client_version().await {
                    Ok(version) => {
                        *desktop_client = CacheEntry::from(ClientData {
                            version: version.clone(),
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
            Some(cdata) => cdata.version.clone(),
            None => {
                log::debug!("getting music client version");
                match self.extract_music_client_version().await {
                    Ok(version) => {
                        *music_client = CacheEntry::from(ClientData {
                            version: version.clone(),
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

    /// Get deobfuscation data (either from cache or extracted from YouTube's JavaScript code)
    async fn get_deobf_data(&self) -> Result<DeobfData, Error> {
        // Write lock here to prevent concurrent tasks from fetching the same data
        let mut deobf_data = self.inner.cache.deobf.write().await;

        match deobf_data.get() {
            Some(deobf_data) => Ok(deobf_data.clone()),
            None => {
                log::debug!("getting deobf data");

                match DeobfData::extract(self.inner.http.clone(), self.inner.reporter.as_deref())
                    .await
                {
                    Ok(new_data) => {
                        // Write new data to the cache
                        *deobf_data = CacheEntry::from(new_data.clone());
                        drop(deobf_data);
                        self.store_cache().await;
                        Ok(new_data)
                    }
                    Err(e) => {
                        // Try to fall back to expired cache data if available, otherwise return error
                        match deobf_data.get_expired() {
                            Some(d) => {
                                log::warn!("could not get new deobf data ({e}), falling back to expired cache");
                                Ok(d.clone())
                            }
                            None => Err(e),
                        }
                    }
                }
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

    /// Request a new visitor data cookie from YouTube
    ///
    /// Since the cookie is shared between YT and YTM and the YTM page loads faster,
    /// we request that.
    ///
    /// Sometimes YouTube does not set the `__Secure-YEC` cookie. In this case, the
    /// visitor data is extracted from the html page.
    async fn get_visitor_data(&self) -> Result<String, Error> {
        log::debug!("getting YT visitor data");
        let resp = self.inner.http.get(YOUTUBE_MUSIC_HOME_URL).send().await?;

        let vdata = resp
            .headers()
            .get_all(header::SET_COOKIE)
            .iter()
            .find_map(|c| {
                if let Ok(cookie) = c.to_str() {
                    if let Some(after) = cookie.strip_prefix("__Secure-YEC=") {
                        return after.split_once(';').map(|s| s.0.to_owned());
                    }
                }
                None
            });

        match vdata {
            Some(vdata) => Ok(vdata),
            None => {
                if resp.status().is_success() {
                    // Extract visitor data from html
                    let html = resp.text().await?;

                    util::get_cg_from_regex(&VISITOR_DATA_REGEX, &html, 1).ok_or(Error::Extraction(
                        ExtractionError::InvalidData(Cow::Borrowed(
                            "Could not find visitor data on html page",
                        )),
                    ))
                } else {
                    Err(Error::Extraction(ExtractionError::InvalidData(
                        format!("Could not get visitor data, status: {}", resp.status()).into(),
                    )))
                }
            }
        }
    }
}

impl RustyPipeQuery {
    /// Set the language parameter used when accessing the YouTube API
    ///
    /// This will change multilanguage video titles, descriptions and textual dates
    #[must_use]
    pub fn lang(mut self, lang: Language) -> Self {
        self.opts.lang = lang;
        self
    }

    /// Set the country parameter used when accessing the YouTube API.
    ///
    /// This will change trends and recommended content.
    #[must_use]
    pub fn country(mut self, country: Country) -> Self {
        self.opts.country = validate_country(country);
        self
    }

    /// Generate a report on every operation.
    ///
    /// This should only be used for debugging.
    #[must_use]
    pub fn report(mut self) -> Self {
        self.opts.report = true;
        self
    }

    /// Enable strict mode, causing operations to fail if there
    /// are warnings during deserialization (e.g. invalid items).
    ///
    /// This should only be used for testing.
    #[must_use]
    pub fn strict(mut self) -> Self {
        self.opts.strict = true;
        self
    }

    /// Set the YouTube visitor data cookie
    ///
    /// YouTube assigns a session cookie to each user which is used for personalized
    /// recommendations. By default, RustyPipe does not send this cookie to preserve
    /// user privacy. For requests that mandatate the cookie, a new one is requested
    /// for every query.
    ///
    /// This option allows you to manually set the visitor data cookie of your query,
    /// allowing you to get personalized recommendations or reproduce A/B tests.
    ///
    /// Note that YouTube has a rate limit on the number of requests from a single
    /// visitor, so you should not use the same vistor data cookie for batch operations.
    #[must_use]
    pub fn visitor_data<S: Into<String>>(mut self, visitor_data: S) -> Self {
        self.opts.visitor_data = Some(visitor_data.into());
        self
    }

    /// Set the YouTube visitor data cookie to an optional value
    ///
    /// see also [`RustyPipeQuery::visitor_data`]
    #[must_use]
    pub fn visitor_data_opt<S: Into<String>>(mut self, visitor_data: Option<S>) -> Self {
        self.opts.visitor_data = visitor_data.map(S::into);
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
        let (hl, gl) = if localized {
            (self.opts.lang, self.opts.country)
        } else {
            (Language::En, Country::Us)
        };
        let visitor_data = self.opts.visitor_data.as_deref().or(visitor_data);

        match ctype {
            ClientType::Desktop => YTContext {
                client: ClientInfo {
                    client_name: "WEB",
                    client_version: Cow::Owned(self.client.get_desktop_client_version().await),
                    platform: "DESKTOP",
                    original_url: Some(YOUTUBE_HOME_URL),
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
                    original_url: Some(YOUTUBE_MUSIC_HOME_URL),
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
                    embed_url: YOUTUBE_HOME_URL,
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
                .header(header::ORIGIN, YOUTUBE_HOME_URL)
                .header(header::REFERER, YOUTUBE_HOME_URL)
                .header(header::COOKIE, self.client.inner.consent_cookie.clone())
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
                .header(header::ORIGIN, YOUTUBE_MUSIC_HOME_URL)
                .header(header::REFERER, YOUTUBE_MUSIC_HOME_URL)
                .header(header::COOKIE, self.client.inner.consent_cookie.clone())
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
                .header(header::ORIGIN, YOUTUBE_HOME_URL)
                .header(header::REFERER, YOUTUBE_HOME_URL)
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

    /// Get a YouTube visitor data cookie, which is necessary for certain requests
    async fn get_visitor_data(&self) -> Result<String, Error> {
        match &self.opts.visitor_data {
            Some(vd) => Ok(vd.clone()),
            None => self.client.get_visitor_data().await,
        }
    }

    async fn yt_request_attempt<R: DeserializeOwned + MapResponse<M> + Debug, M>(
        &self,
        request: &Request,
        id: &str,
        deobf: Option<&DeobfData>,
    ) -> Result<RequestResult<M>, Error> {
        let response = self
            .client
            .inner
            .http
            .execute(request.try_clone().unwrap())
            .await?;

        let status = response.status();
        let body = response.text().await?;

        let res = if status.is_client_error() || status.is_server_error() {
            let error_msg = serde_json::from_str::<response::ErrorResponse>(&body)
                .map(|r| Cow::from(r.error.message));

            Err(match status {
                StatusCode::NOT_FOUND => Error::Extraction(ExtractionError::NotFound {
                    id: id.to_owned(),
                    msg: error_msg.unwrap_or("404".into()),
                }),
                StatusCode::BAD_REQUEST => {
                    Error::Extraction(ExtractionError::BadRequest(error_msg.unwrap_or_default()))
                }
                _ => Error::HttpStatus(status.as_u16(), error_msg.unwrap_or_default()),
            })
        } else {
            match serde_json::from_str::<R>(&body) {
                Ok(deserialized) => match deserialized.map_response(
                    id,
                    self.opts.lang,
                    deobf,
                    self.opts.visitor_data.as_deref(),
                ) {
                    Ok(mapres) => Ok(mapres),
                    Err(e) => Err(e.into()),
                },
                Err(e) => Err(Error::from(ExtractionError::from(e))),
            }
        };

        Ok(RequestResult { res, status, body })
    }

    async fn yt_request<R: DeserializeOwned + MapResponse<M> + Debug, M>(
        &self,
        request: &Request,
        id: &str,
        deobf: Option<&DeobfData>,
    ) -> Result<RequestResult<M>, Error> {
        let mut last_resp = None;
        for n in 0..=self.client.inner.n_http_retries {
            let resp = self.yt_request_attempt::<R, M>(request, id, deobf).await?;

            let err = match &resp.res {
                Ok(_) => return Ok(resp),
                Err(e) => {
                    if !e.should_retry() {
                        return Ok(resp);
                    }
                    e
                }
            };

            if n != self.client.inner.n_http_retries {
                let ms = util::retry_delay(n, 1000, 60000, 3);
                log::warn!(
                    "Retry attempt #{}. Error: {}. Waiting {} ms",
                    n + 1,
                    err,
                    ms
                );
                tokio::time::sleep(Duration::from_millis(ms.into())).await;
            }

            last_resp = Some(resp);
        }
        Ok(last_resp.unwrap())
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

        let req_res = self.yt_request::<R, M>(&request, id, deobf).await?;

        // Uncomment to debug response text
        // println!("{}", &req_res.body);

        let (level, error, msgs, res) = match req_res.res {
            Ok(mapres) => {
                let level = if mapres.warnings.is_empty() {
                    Level::DBG
                } else {
                    Level::WRN
                };
                (level, None, mapres.warnings, Ok(mapres.c))
            }
            Err(e) => {
                let level = if e.should_report() {
                    Level::ERR
                } else {
                    Level::DBG
                };
                (level, Some(e.to_string()), Vec::new(), Err(e))
            }
        };

        if level > Level::DBG || self.opts.report {
            if let Some(reporter) = &self.client.inner.reporter {
                let report = Report {
                    info: RustyPipeInfo::default(),
                    level,
                    operation: &format!("{operation}({id})"),
                    error,
                    msgs,
                    deobf_data: deobf.cloned(),
                    http_request: crate::report::HTTPRequest {
                        url: request.url().as_str(),
                        method: request.method().as_str(),
                        req_header: Some(
                            request
                                .headers()
                                .iter()
                                .map(|(k, v)| {
                                    (k.as_str(), v.to_str().unwrap_or_default().to_owned())
                                })
                                .collect(),
                        ),
                        req_body: serde_json::to_string(body).ok(),
                        status: req_res.status.into(),
                        resp_body: req_res.body,
                    },
                };
                reporter.report(&report);
            }
        }

        if res.is_ok() && level > Level::DBG && self.opts.strict {
            return Err(Error::Extraction(ExtractionError::DeserializationWarnings));
        }

        res
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

        self.client.http_request_txt(&request).await
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
    /// - `vdata`: Visitor data option of the client
    fn map_response(
        self,
        id: &str,
        lang: Language,
        deobf: Option<&DeobfData>,
        vdata: Option<&str>,
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
    fn t_get_visitor_data() {
        let rp = RustyPipe::new();
        let visitor_data = tokio_test::block_on(rp.get_visitor_data()).unwrap();

        assert!(
            visitor_data.starts_with("Cg") && visitor_data.len() > 23,
            "invalid visitor data: {visitor_data}"
        )
    }
}
