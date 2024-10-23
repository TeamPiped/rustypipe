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

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};
use std::{borrow::Cow, fmt::Debug, time::Duration};

use once_cell::sync::Lazy;
use regex::Regex;
use reqwest::{header, Client, ClientBuilder, Request, RequestBuilder, Response, StatusCode};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use time::OffsetDateTime;
use tokio::sync::RwLock as AsyncRwLock;

use crate::error::AuthError;
use crate::{
    cache::{CacheStorage, FileStorage, DEFAULT_CACHE_FILE},
    deobfuscate::DeobfData,
    error::{Error, ExtractionError},
    model::ArtistId,
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
    /// - can access YTM-specific data
    /// - cannot access non-music content
    DesktopMusic,
    /// Client used by m.youtube.com
    ///
    /// - includes lower resolution audio streams
    /// - does not return audio tracks in different languages
    Mobile,
    /// Client used by youtube.com/tv
    ///
    /// - Does not return video metadata when fetching the player
    Tv,
    /// Client used by the Android app
    ///
    /// - no obfuscated stream URLs
    /// - includes lower resolution audio streams
    Android,
    /// Client used by the iOS app
    ///
    /// - no obfuscated stream URLs
    /// - does not include opus audio streams
    Ios,
}

impl ClientType {
    fn is_web(self) -> bool {
        match self {
            ClientType::Desktop
            | ClientType::DesktopMusic
            | ClientType::Mobile
            | ClientType::Tv => true,
            ClientType::Android | ClientType::Ios => false,
        }
    }
}

/// YouTube context request parameter
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct YTContext<'a> {
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
    visitor_data: &'a str,
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
            visitor_data: "",
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
struct QBody<'a, T> {
    context: YTContext<'a>,
    #[serde(flatten)]
    body: T,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct QBrowse<'a> {
    browse_id: &'a str,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct QBrowseParams<'a> {
    browse_id: &'a str,
    params: &'a str,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct QContinuation<'a> {
    continuation: &'a str,
}

#[derive(Debug, Serialize)]
struct OauthCodeRequest {
    client_id: &'static str,
    device_id: String,
    device_model: &'static str,
    scope: &'static str,
}

/// Device code used for logging a user into YouTube
///
/// The login process works as follows:
/// 1. Obtain a user code and show it to the user
/// 2. The user opens the login page under <https://google.com/device>, enters the code and logs in with his account
/// 3. The application has to check periodically if the login has succeeded using [`RustyPipe::oauth_login`] or [`RustyPipe::oauth_wait_for_login`]
/// 4. If the login is successful, the application receives a valid access/refresh token pair which can be used to access YouTube
#[derive(Debug, Deserialize)]
pub struct OauthDeviceCode {
    device_code: String,
    /// Code to be shown to the user to log himself in
    pub user_code: String,
    /// Time in seconds until the code expires
    pub expires_in: u32,
    /// Interval in seconds for checking if the login was completed
    pub interval: u32,
    /// URL to the login page (<https://google.com/device>)
    pub verification_url: String,
}

#[derive(Debug, Serialize)]
struct OauthTokenRequest<'a> {
    client_id: &'static str,
    client_secret: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    code: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    refresh_token: Option<&'a str>,
    grant_type: &'static str,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum OauthTokenResponse {
    Ok(OauthTokenResponseInner),
    Error {
        error: String,
        #[serde(default)]
        error_description: String,
    },
}

#[derive(Debug, Deserialize)]
struct OauthTokenResponseInner {
    access_token: String,
    refresh_token: Option<String>,
    expires_in: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct OauthToken {
    access_token: String,
    refresh_token: String,
    #[serde(with = "time::serde::rfc3339")]
    expires_at: OffsetDateTime,
}

impl OauthToken {
    fn from_response(
        value: OauthTokenResponseInner,
        refresh_token: Option<String>,
    ) -> Result<Self, Error> {
        Ok(Self {
            access_token: value.access_token,
            refresh_token: value
                .refresh_token
                .or(refresh_token)
                .ok_or(Error::Other("missing refresh token".into()))?,
            expires_at: util::now_sec() + Duration::from_secs(value.expires_in.into()),
        })
    }
}

const DEFAULT_UA: &str = "Mozilla/5.0 (X11; Linux x86_64; rv:109.0) Gecko/20100101 Firefox/115.0";
const MOBILE_UA: &str = "Mozilla/5.0 (Android 14; Mobile; rv:129.0) Gecko/129.0 Firefox/129.0";
const TV_UA: &str = "Mozilla/5.0 (SMART-TV; Linux; Tizen 5.0) AppleWebKit/538.1 (KHTML, like Gecko) Version/5.0 NativeTVAds Safari/538.1";

const CONSENT_COOKIE: &str = "SOCS=CAISAiAD";

const YOUTUBEI_V1_URL: &str = "https://www.youtube.com/youtubei/v1/";
const YOUTUBEI_V1_GAPIS_URL: &str = "https://youtubei.googleapis.com/youtubei/v1/";
const YOUTUBE_MUSIC_V1_URL: &str = "https://music.youtube.com/youtubei/v1/";
const YOUTUBEI_MOBILE_V1_URL: &str = "https://m.youtube.com/youtubei/v1/";
const YOUTUBE_HOME_URL: &str = "https://www.youtube.com/";
const YOUTUBE_MUSIC_HOME_URL: &str = "https://music.youtube.com/";
const YOUTUBE_MOBILE_HOME_URL: &str = "https://m.youtube.com/";
const YOUTUBE_TV_URL: &str = "https://www.youtube.com/tv";

const DISABLE_PRETTY_PRINT_PARAMETER: &str = "prettyPrint=false";

// Web client
const DESKTOP_CLIENT_VERSION: &str = "2.20241010.09.00";
const DESKTOP_MUSIC_CLIENT_VERSION: &str = "1.20241007.00.00";
const MOBILE_CLIENT_VERSION: &str = "2.20241011.01.00";
const TV_CLIENT_VERSION: &str = "7.20241008.14.02";

// Mobile app client
const APP_CLIENT_VERSION: &str = "18.03.33";
const IOS_DEVICE_MODEL: &str = "iPhone14,5";

const OAUTH_CLIENT_ID: &str =
    "861556708454-d6dlm3lh05idd8npek18k6be8ba3oc68.apps.googleusercontent.com";
const OAUTH_CLIENT_SECRET: &str = "SboVhoG9s0rNafixCSGGKXAT";
const OAUTH_SCOPES: &str = "http://gdata.youtube.com https://www.googleapis.com/auth/youtube";

static CLIENT_VERSION_REGEX: Lazy<Regex> =
    Lazy::new(|| Regex::new(r#""INNERTUBE_CONTEXT_CLIENT_VERSION":"([\w\d\._-]+?)""#).unwrap());
static VISITOR_DATA_REGEX: Lazy<Regex> =
    Lazy::new(|| Regex::new(r#""visitorData":"([\w\d_\-%]+?)""#).unwrap());

/// Default order of client types when fetching player data
///
/// The order may change in the future in case YouTube applies changes to their
/// platform that disable a client or make it less reliable.
pub const DEFAULT_PLAYER_CLIENT_ORDER: &[ClientType] =
    &[ClientType::Tv, ClientType::Android, ClientType::Ios];

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
    cache: CacheHolder,
    default_opts: RustyPipeOpts,
    user_agent: Cow<'static, str>,
}

#[derive(Clone)]
struct RustyPipeOpts {
    lang: Language,
    country: Country,
    report: bool,
    strict: bool,
    auth: Option<bool>,
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
            auth: None,
            visitor_data: None,
        }
    }
}

#[derive(Default, Debug)]
struct CacheHolder {
    clients: HashMap<ClientType, AsyncRwLock<CacheEntry<ClientData>>>,
    deobf: AsyncRwLock<CacheEntry<DeobfData>>,
    oauth_token: RwLock<Option<OauthToken>>,
}

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
struct CacheData {
    clients: HashMap<ClientType, CacheEntry<ClientData>>,
    #[serde(skip_serializing_if = "CacheEntry::is_none")]
    deobf: CacheEntry<DeobfData>,
    #[serde(skip_serializing_if = "Option::is_none")]
    oauth_token: Option<OauthToken>,
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

    fn is_none(&self) -> bool {
        matches!(self, Self::None)
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
    /// Create a new [`RustyPipeBuilder`].
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

    /// Create a new, configured [`RustyPipe`] instance.
    pub fn build(self) -> Result<RustyPipe, Error> {
        self.build_with_client(ClientBuilder::new())
    }

    /// Create a new, configured RustyPipe instance using a Reqwest [`ClientBuilder`].
    pub fn build_with_client(self, mut client_builder: ClientBuilder) -> Result<RustyPipe, Error> {
        let user_agent = self
            .user_agent
            .map(Cow::Owned)
            .unwrap_or(Cow::Borrowed(DEFAULT_UA));

        client_builder = client_builder
            .user_agent(user_agent.as_ref())
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

        let mut cdata = if let Some(data) = storage.as_ref().and_then(|storage| storage.read()) {
            match serde_json::from_str::<CacheData>(&data) {
                Ok(data) => data,
                Err(e) => {
                    tracing::error!("Could not deserialize cache. Error: {}", e);
                    CacheData::default()
                }
            }
        } else {
            CacheData::default()
        };

        let cache_clients = [
            ClientType::Desktop,
            ClientType::DesktopMusic,
            ClientType::Mobile,
            ClientType::Tv,
        ]
        .into_iter()
        .map(|c| {
            (
                c,
                AsyncRwLock::new(cdata.clients.remove(&c).unwrap_or_default()),
            )
        })
        .collect::<HashMap<_, _>>();

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
                cache: CacheHolder {
                    clients: cache_clients,
                    deobf: AsyncRwLock::new(cdata.deobf),
                    oauth_token: RwLock::new(cdata.oauth_token),
                },
                default_opts: self.default_opts,
                user_agent,
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

    /// Enable authentication for all requests
    #[must_use]
    pub fn authenticated(mut self) -> Self {
        self.default_opts.auth = Some(true);
        self
    }

    /// Disable authentication for all requests
    #[must_use]
    pub fn unauthenticated(mut self) -> Self {
        self.default_opts.auth = Some(false);
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
            let resp = self.inner.http.execute(request.try_clone().unwrap()).await;

            let err = match resp {
                Ok(resp) => {
                    let status = resp.status();
                    // Immediately return in case of success or unrecoverable status code
                    if status.is_success()
                        || (!status.is_server_error() && status != StatusCode::TOO_MANY_REQUESTS)
                    {
                        return Ok(resp);
                    }
                    last_resp = Some(Ok(resp));
                    status.to_string()
                }
                Err(e) => {
                    // Retry in case of a timeout error
                    if !e.is_timeout() {
                        return Err(e);
                    }
                    last_resp = Some(Err(e));
                    "timeout".to_string()
                }
            };

            // Retry in case of a recoverable status code (server err, too many requests)
            if n != self.inner.n_http_retries {
                let ms = util::retry_delay(n, 1000, 60000, 3);
                tracing::warn!(
                    "Retry attempt #{}. Error: {}. Waiting {} ms",
                    n + 1,
                    err,
                    ms
                );
                tokio::time::sleep(Duration::from_millis(ms.into())).await;
            }
        }
        last_resp.unwrap()
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

    async fn extract_client_version(&self, client_type: ClientType) -> Result<String, Error> {
        let (sw_url, html_url, origin, ua) = match client_type {
            ClientType::Desktop => (
                Some("https://www.youtube.com/sw.js"),
                "https://www.youtube.com/results?search_query=",
                YOUTUBE_HOME_URL,
                None,
            ),
            ClientType::DesktopMusic => (
                Some("https://music.youtube.com/sw.js"),
                YOUTUBE_MUSIC_HOME_URL,
                YOUTUBE_MUSIC_HOME_URL,
                None,
            ),
            ClientType::Mobile => (
                Some("https://m.youtube.com/sw.js"),
                "https://m.youtube.com/results?search_query=",
                YOUTUBE_MUSIC_HOME_URL,
                Some(MOBILE_UA),
            ),
            ClientType::Tv => (None, YOUTUBE_TV_URL, YOUTUBE_TV_URL, Some(TV_UA)),
            _ => panic!("cannot extract client version for {client_type:?}"),
        };

        let from_swjs = sw_url.map(|sw_url| async move {
            let swjs = self
                .http_request_txt(
                    &self
                        .inner
                        .http
                        .get(sw_url)
                        .header(header::ORIGIN, origin)
                        .header(header::REFERER, origin)
                        .header(header::COOKIE, CONSENT_COOKIE)
                        .build()
                        .unwrap(),
                )
                .await?;

            util::get_cg_from_regex(&CLIENT_VERSION_REGEX, &swjs, 1).ok_or(Error::Extraction(
                ExtractionError::InvalidData("Could not find client version in sw.js".into()),
            ))
        });

        let from_html = async {
            let mut builder = self.inner.http.get(html_url);
            if let Some(ua) = ua {
                builder = builder.header(header::USER_AGENT, ua);
            }

            let html = self.http_request_txt(&builder.build().unwrap()).await?;

            util::get_cg_from_regex(&CLIENT_VERSION_REGEX, &html, 1).ok_or(Error::Extraction(
                ExtractionError::InvalidData("Could not find client version on html page".into()),
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

    async fn get_client_version(&self, client_type: ClientType) -> Cow<'static, str> {
        // Write lock here to prevent concurrent tasks from fetching the same data
        let mut client = self.inner.cache.clients[&client_type].write().await;

        match client.get() {
            Some(cdata) => cdata.version.clone().into(),
            None => {
                tracing::debug!("getting {client_type:?} client version");
                match self.extract_client_version(client_type).await {
                    Ok(version) => {
                        *client = CacheEntry::from(ClientData {
                            version: version.clone(),
                        });
                        drop(client);
                        self.store_cache().await;
                        version.into()
                    }
                    Err(e) => {
                        tracing::warn!(
                            "{e}, falling back to hardcoded {client_type:?} client version"
                        );
                        match client_type {
                            ClientType::Desktop => DESKTOP_CLIENT_VERSION,
                            ClientType::DesktopMusic => DESKTOP_MUSIC_CLIENT_VERSION,
                            ClientType::Mobile => MOBILE_CLIENT_VERSION,
                            ClientType::Tv => TV_CLIENT_VERSION,
                            _ => unreachable!(),
                        }
                        .into()
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
                tracing::debug!("getting deobf data");

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
                                tracing::warn!("could not get new deobf data ({e}), falling back to expired cache");
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
        let mut cache_clients = HashMap::new();
        for (c, lk) in &self.inner.cache.clients {
            let v = lk.read().await.clone();
            if !v.is_none() {
                cache_clients.insert(*c, v);
            }
        }

        if let Some(storage) = &self.inner.storage {
            let cdata = CacheData {
                clients: cache_clients,
                deobf: self.inner.cache.deobf.read().await.clone(),
                oauth_token: self.inner.cache.oauth_token.read().unwrap().clone(),
            };

            match serde_json::to_string(&cdata) {
                Ok(data) => storage.write(&data),
                Err(e) => tracing::error!("Could not serialize cache. Error: {}", e),
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
        tracing::debug!("getting YT visitor data");
        let resp = self
            .inner
            .http
            .get(YOUTUBE_MUSIC_HOME_URL)
            .header(header::ORIGIN, YOUTUBE_MUSIC_HOME_URL)
            .header(header::REFERER, YOUTUBE_MUSIC_HOME_URL)
            .send()
            .await?;

        let vdata = resp
            .headers()
            .get_all(header::SET_COOKIE)
            .iter()
            .find_map(|c| {
                if let Ok(cookie) = c.to_str() {
                    if let Some(after) = cookie.strip_prefix("__Secure-YEC=") {
                        return after
                            .split_once(';')
                            .map(|s| s.0.to_owned())
                            .filter(|s| !s.is_empty());
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
                        ExtractionError::InvalidData(
                            "Could not find visitor data on html page".into(),
                        ),
                    ))
                } else {
                    Err(Error::Extraction(ExtractionError::InvalidData(
                        format!("Could not get visitor data, status: {}", resp.status()).into(),
                    )))
                }
            }
        }
    }

    /// Get a new device code for logging into YouTube
    pub async fn user_auth_get_code(&self) -> Result<OauthDeviceCode, Error> {
        tracing::debug!("getting OAuth user code");

        let code_request = OauthCodeRequest {
            client_id: OAUTH_CLIENT_ID,
            device_id: util::random_uuid(),
            device_model: "ytlr:samsung:smarttv",
            scope: OAUTH_SCOPES,
        };

        self.inner
            .http
            .post("https://www.youtube.com/o/oauth2/device/code")
            .header(header::USER_AGENT, TV_UA)
            .header(header::ORIGIN, YOUTUBE_HOME_URL)
            .header(header::REFERER, YOUTUBE_TV_URL)
            .json(&code_request)
            .send()
            .await?
            .error_for_status()?
            .json::<OauthDeviceCode>()
            .await
            .map_err(Error::from)
    }

    /// Attempt to log in the user using the given device code
    ///
    /// Returns `true` if the user has successfully logged in using the code.
    ///
    /// Returns `false` if the user has not logged in yet, in this case repeat
    /// the login attempt after a few seconds.
    /// The function [`RustyPipe::oauth_wait_for_login`] does this automatically.
    pub async fn user_auth_login(&self, code: &OauthDeviceCode) -> Result<bool, Error> {
        tracing::debug!("OAuth login attempt (user_code: {})", code.user_code);

        let token_request = OauthTokenRequest {
            client_id: OAUTH_CLIENT_ID,
            client_secret: OAUTH_CLIENT_SECRET,
            code: Some(&code.device_code),
            refresh_token: None,
            grant_type: "http://oauth.net/grant_type/device/1.0",
        };

        let token_response = self
            .inner
            .http
            .post("https://www.youtube.com/o/oauth2/token")
            .header(header::USER_AGENT, TV_UA)
            .header(header::ORIGIN, YOUTUBE_HOME_URL)
            .header(header::REFERER, YOUTUBE_TV_URL)
            .json(&token_request)
            .send()
            .await?
            .error_for_status()?
            .json::<OauthTokenResponse>()
            .await?;

        match token_response {
            OauthTokenResponse::Ok(token) => {
                let token = OauthToken::from_response(token, None)?;
                {
                    let mut cache_token = self.inner.cache.oauth_token.write().unwrap();
                    *cache_token = Some(token);
                }
                self.store_cache().await;
                Ok(true)
            }
            OauthTokenResponse::Error {
                error,
                error_description,
            } => match error.as_str() {
                "authorization_pending" => Ok(false),
                "expired_token" => Err(Error::Auth(AuthError::DeviceCodeExpired)),
                _ => Err(Error::Auth(AuthError::Other(format!(
                    "{error}: {error_description}"
                )))),
            },
        }
    }

    /// Attempt to refresh the OAuth access token to check if the user is successfully logged in
    /// and the session is still valid.
    pub async fn user_auth_check_login(&self) -> Result<(), Error> {
        let cache_token = self.inner.cache.oauth_token.read().unwrap().clone();
        if let Some(token) = cache_token {
            let token = self.user_auth_refresh_token(&token.refresh_token).await?;
            {
                let mut cache_token = self.inner.cache.oauth_token.write().unwrap();
                *cache_token = Some(token.clone());
            }
            self.store_cache().await;
            Ok(())
        } else {
            Err(Error::Auth(AuthError::NoLogin))
        }
    }

    /// Attempt to log in the user using the given device code.
    ///
    /// This function waits until the login was successful or an error occurred.
    pub async fn user_auth_wait_for_login(&self, code: &OauthDeviceCode) -> Result<(), Error> {
        while !self.user_auth_login(code).await? {
            tokio::time::sleep(Duration::from_secs(code.interval.into())).await;
        }
        Ok(())
    }

    /// Log out the user and remove the OAuth token from the cache
    pub async fn user_auth_logout(&self) -> Result<(), Error> {
        #[derive(Serialize)]
        struct RevokeRequest<'a> {
            token: &'a str,
        }

        let cache_token = self
            .inner
            .cache
            .oauth_token
            .read()
            .unwrap()
            .clone()
            .ok_or(Error::Auth(AuthError::NoLogin))?;
        let revoke_request = RevokeRequest {
            token: &cache_token.refresh_token,
        };

        let resp = self
            .inner
            .http
            .post("https://www.youtube.com/o/oauth2/revoke")
            .header(header::USER_AGENT, TV_UA)
            .header(header::ORIGIN, YOUTUBE_HOME_URL)
            .header(header::REFERER, YOUTUBE_TV_URL)
            .json(&revoke_request)
            .send()
            .await?;

        if let Err(estatus) = resp.error_for_status_ref().map(|_| ()) {
            if let Ok(OauthTokenResponse::Error {
                error,
                error_description,
            }) = resp.json::<OauthTokenResponse>().await
            {
                // User is already logged out
                if error == "invalid_token" {
                    tracing::info!("user already logged out ({error}: {error_description})");
                } else {
                    return Err(Error::Other(format!("{error}: {error_description}").into()));
                }
            } else {
                return Err(estatus.into());
            }
        }
        self.user_auth_remove_token().await;
        Ok(())
    }

    /// Remove the stored OAuth token from the cache
    async fn user_auth_remove_token(&self) {
        {
            let mut cache_token = self.inner.cache.oauth_token.write().unwrap();
            *cache_token = None;
        }
        self.store_cache().await;
    }

    /// Obtain a new OAuth token using the given refresh token
    async fn user_auth_refresh_token(&self, refresh_token: &str) -> Result<OauthToken, Error> {
        tracing::debug!("refreshing OAuth token");

        let token_request = OauthTokenRequest {
            client_id: OAUTH_CLIENT_ID,
            client_secret: OAUTH_CLIENT_SECRET,
            code: None,
            refresh_token: Some(refresh_token),
            grant_type: "refresh_token",
        };

        let token_response = self
            .inner
            .http
            .post("https://www.youtube.com/o/oauth2/token")
            .header(header::USER_AGENT, TV_UA)
            .header(header::ORIGIN, YOUTUBE_HOME_URL)
            .header(header::REFERER, YOUTUBE_TV_URL)
            .json(&token_request)
            .send()
            .await?
            .error_for_status()?
            .json::<OauthTokenResponse>()
            .await?;

        match token_response {
            OauthTokenResponse::Ok(token) => {
                OauthToken::from_response(token, Some(refresh_token.to_owned()))
            }
            OauthTokenResponse::Error {
                error,
                error_description,
            } => {
                // If the token is expired or revoked, remove it from the client
                if error == "invalid_grant" {
                    self.user_auth_remove_token().await;
                }
                Err(Error::Auth(AuthError::Refresh(format!(
                    "{error}: {error_description}"
                ))))
            }
        }
    }

    /// Get the OAuth access token for accessing YouTube as an authenticated user
    pub async fn user_auth_access_token(&self) -> Result<String, Error> {
        let cache_token = self.inner.cache.oauth_token.read().unwrap().clone();
        if let Some(token) = cache_token {
            if token.expires_at < (OffsetDateTime::now_utc() + Duration::from_secs(60)) {
                let token = self.user_auth_refresh_token(&token.refresh_token).await?;
                let access_token = token.access_token.to_owned();

                {
                    let mut cache_token = self.inner.cache.oauth_token.write().unwrap();
                    *cache_token = Some(token.clone());
                }
                self.store_cache().await;

                Ok(access_token)
            } else {
                Ok(token.access_token.to_owned())
            }
        } else {
            Err(Error::Auth(AuthError::NoLogin))
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

    /// Enable authentication for this request
    #[must_use]
    pub fn authenticated(mut self) -> Self {
        self.opts.auth = Some(true);
        self
    }

    /// Disable authentication for this request
    #[must_use]
    pub fn unauthenticated(mut self) -> Self {
        self.opts.auth = Some(false);
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

    /// Get the user agent for the given client type
    ///
    /// This can be used for additional HTTP requests (e.g. downloading/streaming)
    pub fn user_agent(&self, ctype: ClientType) -> Cow<'_, str> {
        match ctype {
            ClientType::Desktop | ClientType::DesktopMusic => {
                Cow::Borrowed(&self.client.inner.user_agent)
            }
            ClientType::Mobile => MOBILE_UA.into(),
            ClientType::Tv => TV_UA.into(),
            ClientType::Android => format!(
                "com.google.android.youtube/{} (Linux; U; Android 12; {}) gzip",
                APP_CLIENT_VERSION, self.opts.country
            )
            .into(),
            ClientType::Ios => format!(
                "com.google.ios.youtube/{} ({}; U; CPU iOS 15_4 like Mac OS X; {})",
                APP_CLIENT_VERSION, IOS_DEVICE_MODEL, self.opts.country
            )
            .into(),
        }
    }

    /// Return `true` if the client has stored login credentials and authentication has not been disabled
    pub fn auth_enabled(&self) -> bool {
        if self.opts.auth == Some(false) {
            return false;
        }
        let cache_token = self.client.inner.cache.oauth_token.read().unwrap();
        cache_token.is_some()
    }

    /// Create a new context object, which is included in every request to
    /// the YouTube API and contains language, country and device parameters.
    ///
    /// # Parameters
    /// - `ctype`: Client type (`Desktop`, `DesktopMusic`, `Android`, ...)
    /// - `localized`: Whether to include the configured language and country
    async fn get_context<'a>(
        &'a self,
        ctype: ClientType,
        localized: bool,
        visitor_data: &'a str,
    ) -> YTContext {
        let (hl, gl) = if localized {
            (self.opts.lang, self.opts.country)
        } else {
            (Language::En, Country::Us)
        };

        match ctype {
            ClientType::Desktop => YTContext {
                client: ClientInfo {
                    client_name: "WEB",
                    client_version: self.client.get_client_version(ctype).await,
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
                    client_version: self.client.get_client_version(ctype).await,
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
            ClientType::Mobile => YTContext {
                client: ClientInfo {
                    client_name: "MWEB",
                    client_version: self.client.get_client_version(ctype).await,
                    platform: "MOBILE",
                    original_url: Some(YOUTUBE_MOBILE_HOME_URL),
                    visitor_data,
                    hl,
                    gl,
                    ..Default::default()
                },
                request: Some(RequestYT::default()),
                user: User::default(),
                third_party: None,
            },
            ClientType::Tv => YTContext {
                client: ClientInfo {
                    client_name: "TVHTML5",
                    client_version: self.client.get_client_version(ctype).await,
                    client_screen: Some("WATCH"),
                    platform: "TV",
                    device_model: Some("SmartTV"),
                    visitor_data,
                    hl,
                    gl,
                    ..Default::default()
                },
                request: Some(RequestYT::default()),
                user: User::default(),
                third_party: Some(ThirdParty {
                    embed_url: YOUTUBE_TV_URL,
                }),
            },
            ClientType::Android => YTContext {
                client: ClientInfo {
                    client_name: "ANDROID",
                    client_version: APP_CLIENT_VERSION.into(),
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
                    client_version: APP_CLIENT_VERSION.into(),
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
    /// - `visitor_data`: YouTube visitor data cookie
    async fn request_builder(
        &self,
        ctype: ClientType,
        endpoint: &str,
        visitor_data: Option<&str>,
    ) -> RequestBuilder {
        let mut r = match ctype {
            ClientType::Desktop => self
                .client
                .inner
                .http
                .post(format!(
                    "{YOUTUBEI_V1_URL}{endpoint}?{DISABLE_PRETTY_PRINT_PARAMETER}"
                ))
                .header(header::ORIGIN, YOUTUBE_HOME_URL)
                .header(header::REFERER, YOUTUBE_HOME_URL)
                .header(header::COOKIE, CONSENT_COOKIE)
                .header("X-YouTube-Client-Name", "1")
                .header(
                    "X-YouTube-Client-Version",
                    self.client.get_client_version(ctype).await.into_owned(),
                ),
            ClientType::DesktopMusic => self
                .client
                .inner
                .http
                .post(format!(
                    "{YOUTUBE_MUSIC_V1_URL}{endpoint}?{DISABLE_PRETTY_PRINT_PARAMETER}"
                ))
                .header(header::ORIGIN, YOUTUBE_MUSIC_HOME_URL)
                .header(header::REFERER, YOUTUBE_MUSIC_HOME_URL)
                .header(header::COOKIE, CONSENT_COOKIE)
                .header("X-YouTube-Client-Name", "67")
                .header(
                    "X-YouTube-Client-Version",
                    self.client.get_client_version(ctype).await.into_owned(),
                ),
            ClientType::Mobile => self
                .client
                .inner
                .http
                .post(format!(
                    "{YOUTUBEI_MOBILE_V1_URL}{endpoint}?{DISABLE_PRETTY_PRINT_PARAMETER}"
                ))
                .header(header::ORIGIN, YOUTUBE_MUSIC_HOME_URL)
                .header(header::REFERER, YOUTUBE_MUSIC_HOME_URL)
                .header(header::COOKIE, CONSENT_COOKIE)
                .header("X-YouTube-Client-Name", "2")
                .header(
                    "X-YouTube-Client-Version",
                    self.client.get_client_version(ctype).await.into_owned(),
                ),
            ClientType::Tv => self
                .client
                .inner
                .http
                .post(format!(
                    "{YOUTUBEI_V1_URL}{endpoint}?{DISABLE_PRETTY_PRINT_PARAMETER}"
                ))
                .header(header::ORIGIN, YOUTUBE_HOME_URL)
                .header(header::REFERER, YOUTUBE_TV_URL)
                .header("X-YouTube-Client-Name", "7")
                .header(
                    "X-YouTube-Client-Version",
                    self.client.get_client_version(ctype).await.into_owned(),
                ),
            ClientType::Android => self
                .client
                .inner
                .http
                .post(format!(
                    "{YOUTUBEI_V1_GAPIS_URL}{endpoint}?{DISABLE_PRETTY_PRINT_PARAMETER}"
                ))
                .header("X-Goog-Api-Format-Version", "2"),
            ClientType::Ios => self
                .client
                .inner
                .http
                .post(format!(
                    "{YOUTUBEI_V1_GAPIS_URL}{endpoint}?{DISABLE_PRETTY_PRINT_PARAMETER}"
                ))
                .header("X-Goog-Api-Format-Version", "2"),
        };
        r = r.header(header::USER_AGENT, self.user_agent(ctype).as_ref());
        if let Some(vdata) = self.opts.visitor_data.as_deref().or(visitor_data) {
            r = r.header("X-Goog-EOM-Visitor-Id", vdata);
        }
        r
    }

    /// Get a YouTube visitor data cookie, which is necessary for certain requests
    pub async fn get_visitor_data(&self) -> Result<String, Error> {
        match &self.opts.visitor_data {
            Some(vd) => Ok(vd.clone()),
            None => self.client.get_visitor_data().await,
        }
    }

    async fn yt_request_attempt<R: DeserializeOwned + MapResponse<M> + Debug, M>(
        &self,
        request: &Request,
        ctx: &MapRespCtx<'_>,
    ) -> Result<RequestResult<M>, Error> {
        let response = self
            .client
            .inner
            .http
            .execute(request.try_clone().unwrap())
            .await?;

        let status = response.status();
        let body = response.text().await?;
        tracing::debug!("fetched {} bytes from YT", body.len());

        let res = if status.is_client_error() || status.is_server_error() {
            let error_msg = serde_json::from_str::<response::ErrorResponse>(&body)
                .map(|r| Cow::from(r.error.message));

            Err(match status {
                StatusCode::NOT_FOUND => Error::Extraction(ExtractionError::NotFound {
                    id: ctx.id.to_owned(),
                    msg: error_msg.unwrap_or("404".into()),
                }),
                StatusCode::BAD_REQUEST => {
                    Error::Extraction(ExtractionError::BadRequest(error_msg.unwrap_or_default()))
                }
                _ => Error::HttpStatus(status.as_u16(), error_msg.unwrap_or_default()),
            })
        } else {
            match serde_json::from_str::<R>(&body) {
                Ok(deserialized) => match deserialized.map_response(ctx) {
                    Ok(mapres) => Ok(mapres),
                    Err(e) => Err(e.into()),
                },
                Err(e) => Err(Error::from(ExtractionError::from(e))),
            }
        };

        tracing::trace!("mapped response");
        Ok(RequestResult { res, status, body })
    }

    #[tracing::instrument(skip_all)]
    async fn yt_request<R: DeserializeOwned + MapResponse<M> + Debug, M>(
        &self,
        request: &Request,
        ctx: &MapRespCtx<'_>,
    ) -> Result<RequestResult<M>, Error> {
        let mut last_resp = None;
        for n in 0..=self.client.inner.n_http_retries {
            let resp = self.yt_request_attempt::<R, M>(request, ctx).await?;

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
                tracing::warn!(
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
    /// - `ctx_src`: Context source (additional parameters for fetching and mapping, used to build the MapRespCtx)
    #[allow(clippy::too_many_arguments)]
    async fn execute_request_ctx<
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
        ctx_src: MapRespCtxSource<'_>,
    ) -> Result<M, Error> {
        tracing::debug!("getting {}({})", operation, id);

        let visitor_data = ctx_src
            .visitor_data
            .or(self.opts.visitor_data.as_deref())
            .map(Cow::Borrowed)
            .unwrap_or_else(|| util::random_visitor_data(self.opts.country).into());

        let context = self
            .get_context(ctype, !ctx_src.unlocalized, &visitor_data)
            .await;
        let req_body = QBody { context, body };

        let ctx = MapRespCtx {
            id,
            lang: self.opts.lang,
            deobf: ctx_src.deobf,
            visitor_data: Some(&visitor_data),
            client_type: ctype,
            artist: ctx_src.artist,
        };

        let mut r = self
            .request_builder(ctype, endpoint, ctx.visitor_data)
            .await;

        if self.opts.auth == Some(true) {
            let access_token = self.client.user_auth_access_token().await?;
            r = r.header(header::AUTHORIZATION, format!("Bearer {}", access_token));
        }
        let request = r.json(&req_body).build()?;

        let req_res = self.yt_request::<R, M>(&request, &ctx).await?;

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
                    info: RustyPipeInfo::new(Some(self.opts.lang)),
                    level,
                    operation: &format!("{operation}({id})"),
                    error,
                    msgs,
                    deobf_data: ctx.deobf.cloned(),
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
        self.execute_request_ctx::<R, M, B>(
            ctype,
            operation,
            id,
            endpoint,
            body,
            MapRespCtxSource::default(),
        )
        .await
    }

    /// Execute a request to the YouTube API and return the response string
    ///
    /// # Parameters
    /// - `ctype`: Client type (`Desktop`, `DesktopMusic`, `Android`, ...)
    /// - `endpoint`: YouTube API endpoint (`https://www.youtube.com/youtubei/v1/<XYZ>?key=...`)
    /// - `body`: Serializable request body to be sent in json format
    pub async fn raw<B: Serialize + ?Sized>(
        &self,
        ctype: ClientType,
        endpoint: &str,
        body: &B,
    ) -> Result<String, Error> {
        let visitor_data = self
            .opts
            .visitor_data
            .as_deref()
            .map(Cow::Borrowed)
            .unwrap_or_else(|| util::random_visitor_data(self.opts.country).into());

        let context = self.get_context(ctype, true, &visitor_data).await;
        let req_body = QBody { context, body };

        let request = self
            .request_builder(ctype, endpoint, None)
            .await
            .json(&req_body)
            .build()?;

        self.client.http_request_txt(&request).await
    }
}

impl AsRef<RustyPipeQuery> for RustyPipeQuery {
    fn as_ref(&self) -> &RustyPipeQuery {
        self
    }
}

struct MapRespCtx<'a> {
    id: &'a str,
    lang: Language,
    deobf: Option<&'a DeobfData>,
    visitor_data: Option<&'a str>,
    client_type: ClientType,
    artist: Option<ArtistId>,
}

#[derive(Default)]
struct MapRespCtxSource<'a> {
    visitor_data: Option<&'a str>,
    deobf: Option<&'a DeobfData>,
    artist: Option<ArtistId>,
    unlocalized: bool,
}

impl<'a> MapRespCtx<'a> {
    /// Create a [`MapRespCtx`] for testing
    #[cfg(test)]
    fn test(id: &'a str) -> Self {
        Self {
            id,
            lang: Language::En,
            deobf: None,
            visitor_data: None,
            client_type: ClientType::Desktop,
            artist: None,
        }
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
    /// - `visitor_data`: Visitor data option of the client
    fn map_response(self, ctx: &MapRespCtx<'_>) -> Result<MapResult<T>, ExtractionError>;
}

fn validate_country(country: Country) -> Country {
    if country == Country::Zz {
        tracing::warn!("Country:Zz (Global) can only be used for fetching music charts, falling back to Country:Us");
        Country::Us
    } else {
        country
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use rstest::rstest;

    // 1.20240506.01.00-canary_control_1.20240508.01.01
    // 1.20240508.01.01-canary_experiment_1.20240506.01.00
    fn get_major_version(version: &str) -> u32 {
        let parts = version.split('.').collect::<Vec<_>>();
        assert!(parts.len() >= 4, "version: {version}");
        parts[0].parse().unwrap()
    }

    #[rstest]
    #[case(ClientType::Desktop, 2)]
    #[case(ClientType::DesktopMusic, 1)]
    #[case(ClientType::Mobile, 2)]
    #[case(ClientType::Tv, 1)]
    #[tokio::test]
    async fn extract_desktop_client_version(#[case] client_type: ClientType, #[case] major: u32) {
        let rp = RustyPipe::new();
        let version = rp.extract_client_version(client_type).await.unwrap();
        assert!(get_major_version(&version) >= major);
    }

    #[tokio::test]
    async fn get_visitor_data() {
        let rp = RustyPipe::new();
        let visitor_data = rp.get_visitor_data().await.unwrap();

        assert!(
            visitor_data.starts_with("Cg") && visitor_data.len() > 23,
            "invalid visitor data: {visitor_data}"
        );
    }
}
