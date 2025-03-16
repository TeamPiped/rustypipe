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

#[cfg(feature = "userdata")]
#[cfg_attr(docsrs, doc(cfg(feature = "userdata")))]
mod music_userdata;
#[cfg(feature = "userdata")]
#[cfg_attr(docsrs, doc(cfg(feature = "userdata")))]
mod userdata;

#[cfg(feature = "rss")]
#[cfg_attr(docsrs, doc(cfg(feature = "rss")))]
mod channel_rss;

use std::collections::HashMap;
use std::ffi::OsString;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};
use std::{borrow::Cow, fmt::Debug, time::Duration};

use once_cell::sync::Lazy;
use regex::Regex;
use reqwest::{header, Client, ClientBuilder, Request, RequestBuilder, Response, StatusCode};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use sha1::{Digest, Sha1};
use time::{OffsetDateTime, UtcOffset};
use tokio::sync::RwLock as AsyncRwLock;

use crate::error::AuthError;
use crate::util::VisitorDataCache;
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
    Ios,
}

impl ClientType {
    fn is_web(self) -> bool {
        matches!(
            self,
            ClientType::Desktop | ClientType::DesktopMusic | ClientType::Mobile
        )
    }

    fn needs_deobf(self) -> bool {
        !matches!(self, ClientType::Ios)
    }

    fn needs_po_token(self) -> bool {
        matches!(
            self,
            ClientType::Desktop | ClientType::DesktopMusic | ClientType::Mobile
        )
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
    #[serde(skip_serializing_if = "str::is_empty")]
    client_screen: &'a str,
    #[serde(skip_serializing_if = "str::is_empty")]
    device_model: &'a str,
    #[serde(skip_serializing_if = "str::is_empty")]
    os_name: &'a str,
    #[serde(skip_serializing_if = "str::is_empty")]
    os_version: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    android_sdk_version: Option<u8>,
    platform: &'a str,
    #[serde(skip_serializing_if = "str::is_empty")]
    original_url: &'a str,
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
            client_screen: "",
            device_model: "",
            os_name: "",
            os_version: "",
            android_sdk_version: None,
            platform: "",
            original_url: "",
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
/// 3. The application has to check periodically if the login has succeeded using [`RustyPipe::user_auth_login`] or [`RustyPipe::user_auth_wait_for_login`]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
struct AuthCookie {
    cookie: String,
    #[serde(alias = "account_syncid", skip_serializing_if = "Option::is_none")]
    channel_syncid: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    user_syncid: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    session_index: Option<String>,
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

impl AuthCookie {
    fn new(cookie: String) -> Self {
        Self {
            cookie,
            channel_syncid: None,
            session_index: None,
            user_syncid: None,
        }
    }
}

pub(crate) const DEFAULT_UA: &str =
    "Mozilla/5.0 (X11; Linux x86_64; rv:128.0) Gecko/20100101 Firefox/128.0";
pub(crate) const MOBILE_UA: &str = "Mozilla/5.0 (Linux; Android 10; K) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.6778.135 Mobile Safari/537.36";
pub(crate) const TV_UA: &str = "Mozilla/5.0 (SMART-TV; Linux; Tizen 5.0) AppleWebKit/538.1 (KHTML, like Gecko) Version/5.0 NativeTVAds Safari/538.1";

pub(crate) const CONSENT_COOKIE: &str = "SOCS=CAISAiAD";

const YOUTUBEI_V1_URL: &str = "https://www.youtube.com/youtubei/v1/";
const YOUTUBEI_V1_GAPIS_URL: &str = "https://youtubei.googleapis.com/youtubei/v1/";
const YOUTUBE_MUSIC_V1_URL: &str = "https://music.youtube.com/youtubei/v1/";
const YOUTUBEI_MOBILE_V1_URL: &str = "https://m.youtube.com/youtubei/v1/";
const YOUTUBE_HOME_URL: &str = "https://www.youtube.com";
pub(crate) const YOUTUBE_MUSIC_HOME_URL: &str = "https://music.youtube.com";
const YOUTUBE_MOBILE_HOME_URL: &str = "https://m.youtube.com";
const YOUTUBE_TV_URL: &str = "https://www.youtube.com/tv";

const DISABLE_PRETTY_PRINT_PARAMETER: &str = "prettyPrint=false";

// Web client
const DESKTOP_CLIENT_VERSION: &str = "2.20241216.05.00";
const DESKTOP_MUSIC_CLIENT_VERSION: &str = "1.20241216.01.00";
const MOBILE_CLIENT_VERSION: &str = "2.20241217.07.00";
const TV_CLIENT_VERSION: &str = "7.20241211.14.00";

// Mobile app client
const IOS_CLIENT_VERSION: &str = "20.03.02";
const IOS_VERSION: &str = "18_2_1";
const IOS_VERSION_BUILD: &str = "18.2.1.22C161";
const IOS_DEVICE_MODEL: &str = "iPhone16,2";
const ANDROID_CLIENT_VERSION: &str = "19.44.38";
const ANDROID_VERSION: &str = "11";

const OAUTH_CLIENT_ID: &str =
    "861556708454-d6dlm3lh05idd8npek18k6be8ba3oc68.apps.googleusercontent.com";
const OAUTH_CLIENT_SECRET: &str = "SboVhoG9s0rNafixCSGGKXAT";
const OAUTH_SCOPES: &str = "http://gdata.youtube.com https://www.googleapis.com/auth/youtube";

const BOTGUARD_API_VERSION: &str = "1";

static CLIENT_VERSION_REGEX: Lazy<Regex> =
    Lazy::new(|| Regex::new(r#""INNERTUBE_CONTEXT_CLIENT_VERSION":"([\w\d\._-]+?)""#).unwrap());

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
    visitor_data_cache: VisitorDataCache,
    botguard: Option<BotguardCfg>,
}

#[derive(Clone)]
struct RustyPipeOpts {
    lang: Language,
    country: Country,
    timezone: Option<String>,
    utc_offset_minutes: i16,
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
    botguard_bin: DefaultOpt<OsString>,
    snapshot_file: Option<PathBuf>,
    po_token_cache: bool,
}

struct BotguardCfg {
    program: OsString,
    version: String,
    snapshot_file: PathBuf,
    po_token_cache: bool,
}

/// Proof-of-origin token
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PoToken {
    /// PO token value
    pub po_token: String,
    /// Date until which the token is valid
    pub valid_until: OffsetDateTime,
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
/// ### User data (🔒 Feature `userdata`)
///
/// - **Playback history**
///   - [`history`](RustyPipeQuery::history)
///   - [`history_search`](RustyPipeQuery::history_search)
///   - [`music_history`](RustyPipeQuery::music_history)
/// - **YouTube library**
///   - [`liked_videos`](RustyPipeQuery::liked_videos)
///   - [`watch_later`](RustyPipeQuery::watch_later)
///   - [`saved_playlists`](RustyPipeQuery::saved_playlists)
/// - **Music library**
///   - [`music_saved_artists`](RustyPipeQuery::music_saved_artists)
///   - [`music_saved_albums`](RustyPipeQuery::music_saved_albums)
///   - [`music_saved_tracks`](RustyPipeQuery::music_saved_tracks)
///   - [`music_saved_playlists`](RustyPipeQuery::music_saved_playlists)
///   - [`music_liked_tracks`](RustyPipeQuery::music_liked_tracks)
/// - **Subscriptions**
///   - [`subscriptions`](RustyPipeQuery::subscriptions)
///   - [`subscription_feed`](RustyPipeQuery::subscription_feed)
///
/// ## Options
///
/// You can set the language, country and visitor data ID for individual requests.
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
            timezone: None,
            utc_offset_minutes: 0,
            report: false,
            strict: false,
            auth: None,
            visitor_data: None,
        }
    }
}

#[derive(Debug)]
struct CacheHolder {
    clients: HashMap<ClientType, AsyncRwLock<CacheEntry<ClientData>>>,
    deobf: AsyncRwLock<CacheEntry<DeobfData>>,
    oauth_token: RwLock<Option<OauthToken>>,
    auth_cookie: RwLock<Option<AuthCookie>>,
}

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
struct CacheData {
    clients: HashMap<ClientType, CacheEntry<ClientData>>,
    deobf: CacheEntry<DeobfData>,
    #[serde(skip_serializing_if = "Option::is_none")]
    oauth_token: Option<OauthToken>,
    #[serde(skip_serializing_if = "Option::is_none")]
    auth_cookie: Option<AuthCookie>,
}

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
struct CacheEntry<T> {
    #[serde(
        with = "time::serde::rfc3339::option",
        skip_serializing_if = "Option::is_none"
    )]
    last_update: Option<OffsetDateTime>,
    /// If the entry failed to update, wait until this time before retrying
    #[serde(
        with = "time::serde::rfc3339::option",
        skip_serializing_if = "Option::is_none"
    )]
    retry_at: Option<OffsetDateTime>,
    /// RustyPipe version that failed to updated the entry
    #[serde(skip_serializing_if = "Option::is_none")]
    failed_version: Option<String>,
    data: Option<T>,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct ClientData {
    pub version: String,
}

/// Result of a YouTube HTTP request
struct RequestResult<T> {
    /// Result of the deserialiation/mapping
    res: Result<MapResult<T>, Error>,
    status: StatusCode,
    body: String,
    visitor_data: String,
    request: Request,
}

impl<T> CacheEntry<T> {
    /// Get the content of the cache if it is still fresh
    fn get(&self) -> Option<&T> {
        self.data.as_ref().filter(|_| {
            self.last_update.unwrap_or(OffsetDateTime::UNIX_EPOCH)
                > (OffsetDateTime::now_utc() - time::Duration::days(1))
        })
    }

    /// Get the content of the cache, even if it is expired
    fn get_expired(&self) -> Option<&T> {
        self.data.as_ref()
    }

    fn is_none(&self) -> bool {
        self.data.is_none()
    }

    /// Retry updating a cache entry only after a delay or a RustyPipe update
    fn should_retry(&self) -> bool {
        self.retry_at
            .map(|d| OffsetDateTime::now_utc() > d)
            .unwrap_or(true)
            || self
                .failed_version
                .as_deref()
                .map(|v| crate::VERSION != v)
                .unwrap_or(true)
    }

    fn retry_later(&mut self, delay_h: i64) {
        self.retry_at = Some(util::now_sec() + time::Duration::hours(delay_h));
        self.failed_version = Some(crate::VERSION.to_owned());
    }
}

impl<T> From<T> for CacheEntry<T> {
    fn from(f: T) -> Self {
        Self {
            last_update: Some(util::now_sec()),
            retry_at: None,
            failed_version: None,
            data: Some(f),
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
            botguard_bin: DefaultOpt::Default,
            snapshot_file: None,
            po_token_cache: false,
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

        let visitor_data_cache = VisitorDataCache::new(http.clone(), 50, 20);

        let botguard = match self.botguard_bin {
            DefaultOpt::Some(botguard_bin) => Some(detect_botguard_bin(botguard_bin)?),
            DefaultOpt::None => None,
            DefaultOpt::Default => detect_botguard_bin("./rustypipe-botguard".into())
                .or_else(|_| detect_botguard_bin("rustypipe-botguard".into()))
                .map_err(|e| tracing::debug!("could not detect rustypipe-botguard: {e}"))
                .ok(),
        }
        .map(|(program, version)| {
            tracing::debug!(
                "rustypipe-botguard: using {} at {}",
                version,
                program.to_string_lossy()
            );

            BotguardCfg {
                program: program.to_owned(),
                version,
                snapshot_file: self.snapshot_file.unwrap_or_else(|| {
                    let mut snapshot_file = storage_dir.clone();
                    snapshot_file.push("bg_snapshot.bin");
                    snapshot_file
                }),
                po_token_cache: self.po_token_cache,
            }
        });

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
                    auth_cookie: RwLock::new(cdata.auth_cookie),
                },
                default_opts: self.default_opts,
                user_agent,
                visitor_data_cache,
                botguard,
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

    /// Set the maximum number of retries for YouTube requests.
    ///
    /// If a request fails because of a serverside error and retries are enabled,
    /// RustyPipe waits 1 second before the next attempt.
    ///
    /// The wait time is doubled for subsequent attempts (including a bit of
    /// random jitter to be less predictable).
    ///
    /// **Default value**: 2
    #[must_use]
    pub fn n_http_retries(mut self, n_retries: u32) -> Self {
        self.n_http_retries = n_retries.max(1);
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

    /// Set the timezone and its associated UTC offset in minutes used
    /// when accessing the YouTube API.
    ///
    /// **Default value**: `0` (UTC)
    ///
    /// **Info**: you can set this option for individual queries, too
    #[must_use]
    pub fn timezone<S: Into<String>>(mut self, timezone: S, utc_offset_minutes: i16) -> Self {
        self.default_opts.timezone = Some(timezone.into());
        self.default_opts.utc_offset_minutes = utc_offset_minutes;
        self
    }

    /// Access the YouTube API using the local system timezone
    ///
    /// If the local timezone could not be determined, an error is logged and RustyPipe falls
    /// back to UTC.
    #[must_use]
    pub fn timezone_local(self) -> Self {
        let (timezone, utc_offset_minutes) = local_tz_offset();
        self.timezone(timezone, utc_offset_minutes)
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
    ///
    /// Depending on the client type RustyPipe uses either the authentication cookie or the
    /// OAuth token to authenticate requests.
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

    /// Set the YouTube visitor data ID
    ///
    /// YouTube assigns a session cookie to each user which is used for personalized
    /// recommendations. By default, RustyPipe does not send this cookie to preserve
    /// user privacy. For requests that mandatate the cookie, a new one is requested
    /// for every query.
    ///
    /// This option allows you to manually set the visitor data ID of your client,
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

    /// Set the YouTube visitor data ID to an optional value
    ///
    /// see also [`RustyPipeBuilder::visitor_data`]
    ///
    /// **Info**: you can set this option for individual queries, too
    #[must_use]
    pub fn visitor_data_opt<S: Into<String>>(mut self, visitor_data: Option<S>) -> Self {
        self.default_opts.visitor_data = visitor_data.map(S::into);
        self
    }

    /// Disable RustyPipe Botguard
    ///
    /// By default, RustyPipe uses the `rustypipe-botguard` binary if it is available. If you want to
    /// use RustyPipe without Botguard, you can disable it.
    #[must_use]
    pub fn no_botguard(mut self) -> Self {
        self.botguard_bin = DefaultOpt::None;
        self
    }

    /// Enable RustyPipe Botguard using the given binary
    ///
    /// Botguard is required to generate PO tokens for accessing streams on browser-based clients.
    /// By default, RustyPipe uses the `rustypipe-botguard` binary if it is available.
    ///
    /// More information: <https://codeberg.org/ThetaDev/rustypipe-botguard>
    #[must_use]
    pub fn botguard_bin<S: Into<OsString>>(mut self, botguard_bin: S) -> Self {
        self.botguard_bin = DefaultOpt::Some(botguard_bin.into());
        self
    }

    /// Set the path where the rustypipe-botguard snapshot file is stored
    ///
    /// After solving a Botguard challenge, rustypipe-botguard stores its
    /// JavaScript environment in a snapshot file, so it can quickly generate additional tokens.
    ///
    /// By default the snapshot is stored in the storage_dir (Filename: bg_snapshot.bin).
    #[must_use]
    pub fn botguard_snapshot_file<P: Into<PathBuf>>(mut self, snapshot_file: P) -> Self {
        self.snapshot_file = Some(snapshot_file.into());
        self
    }

    /// Enable caching for session-bound PO tokens
    ///
    /// By default, RustyPipe calls Botguard for every player request to fetch both a
    /// content-bound and a session-bound PO token.
    ///
    /// With caching enabled, the session-bound PO tokens are stored and reused.
    /// Content-bound PO tokens are not used (they are not mandatory at the moment).
    #[must_use]
    pub fn po_token_cache(mut self) -> Self {
        self.po_token_cache = true;
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
            let error_msg = if let Ok(body) = res.text().await {
                serde_json::from_str::<response::ErrorResponse>(&body)
                    .map(|r| Cow::from(r.error.message))
                    .ok()
            } else {
                None
            }
            .unwrap_or_default();
            Err(Error::HttpStatus(status.into(), error_msg))
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
                if client.should_retry() {
                    tracing::debug!("getting {client_type:?} client version");
                    match self.extract_client_version(client_type).await {
                        Ok(version) => {
                            *client = CacheEntry::from(ClientData {
                                version: version.clone(),
                            });
                            drop(client);
                            self.store_cache().await;
                            return version.into();
                        }
                        Err(e) => {
                            client.retry_later(1);
                            drop(client);
                            self.store_cache().await;
                            tracing::warn!(
                                "{e}, falling back to hardcoded {client_type:?} client version"
                            );
                        }
                    }
                } else {
                    tracing::warn!("falling back to hardcoded {client_type:?} client version")
                }

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

    /// Get deobfuscation data (either from cache or extracted from YouTube's JavaScript code)
    async fn get_deobf_data(&self) -> Result<DeobfData, Error> {
        // Write lock here to prevent concurrent tasks from fetching the same data
        let mut deobf_data = self.inner.cache.deobf.write().await;

        match deobf_data.get() {
            Some(deobf_data) => Ok(deobf_data.clone()),
            None => {
                // Only attempt to fetch deobf data every 24 hours to avoid a flood of error reports
                // if the client JS cannot be parsed
                if deobf_data.should_retry() {
                    tracing::debug!("getting deobf data");

                    match DeobfData::extract(&self.inner.http, self.inner.reporter.as_deref()).await
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
                            deobf_data.retry_later(24);
                            let res = match deobf_data.get_expired() {
                                Some(d) => {
                                    tracing::warn!("could not get new deobf data ({e}), falling back to expired cache");
                                    Ok(d.clone())
                                }
                                None => Err(e),
                            };
                            drop(deobf_data);
                            self.store_cache().await;
                            res
                        }
                    }
                } else {
                    match deobf_data.get_expired() {
                        Some(d) => {
                            tracing::warn!(
                                "could not get new deobf data, falling back to expired cache"
                            );
                            Ok(d.clone())
                        }
                        None => Err(Error::Extraction(ExtractionError::Deobfuscation(
                            "could not get deobf data".into(),
                        ))),
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
                auth_cookie: self.inner.cache.auth_cookie.read().unwrap().clone(),
            };

            match serde_json::to_string(&cdata) {
                Ok(data) => storage.write(&data),
                Err(e) => tracing::error!("Could not serialize cache. Error: {}", e),
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
    /// The function [`RustyPipe::user_auth_wait_for_login`] does this automatically.
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

    /// Get a copy of the authentication cookie from the cache
    fn user_auth_cookie(&self) -> Result<AuthCookie, Error> {
        self.inner
            .cache
            .auth_cookie
            .read()
            .unwrap()
            .clone()
            .ok_or(Error::Auth(AuthError::NoLogin))
    }

    fn user_auth_datasync_id(&self) -> Result<String, Error> {
        self.inner
            .cache
            .auth_cookie
            .read()
            .unwrap()
            .as_ref()
            .and_then(|c| c.user_syncid.as_ref().map(|id| id.to_owned()))
            .ok_or(Error::Auth(AuthError::NoLogin))
    }

    /// Set the user authentication cookie
    ///
    /// The cookie is used for authenticated requests with browser-based clients
    /// (Desktop, DesktopMusic, Mobile).
    ///
    /// **Note:** YouTube rotates cookies every few minutes when using the web application.
    /// Do not use the session you obtained cookies from afterwards or it will
    /// become invalid.
    ///
    /// I recommend to log in using Incognito mode, get the cookies from the devtools
    /// and then close the page.
    pub async fn user_auth_set_cookie<S: Into<String>>(&self, cookie: S) -> Result<(), Error> {
        let cookie = cookie.into();
        if cookie.is_empty() {
            return Err(Error::Auth(AuthError::NoLogin));
        }
        let mut auth_cookie = AuthCookie::new(cookie);
        self.extract_session_headers(&mut auth_cookie).await?;
        {
            let mut c = self.inner.cache.auth_cookie.write().unwrap();
            *c = Some(auth_cookie);
        }
        self.store_cache().await;
        Ok(())
    }

    /// Parse the user authentication cookie from a Netscape HTTP Cookie File
    ///
    /// The cookie is used for authenticated requests with browser-based clients
    /// (Desktop, DesktopMusic, Mobile).
    ///
    /// cookie.txt files can be extracted using browser plugins like
    /// "Get cookies.txt LOCALLY" ([Firefox](https://addons.mozilla.org/de/firefox/addon/get-cookies-txt-locally/))
    /// ([Chromium](https://chromewebstore.google.com/detail/get-cookiestxt-locally/cclelndahbckbenkjhflpdbgdldlbecc)).
    ///
    /// **Note:** YouTube rotates cookies every few minutes when using the web application.
    /// Do not use the session you obtained cookies from afterwards or it will
    /// become invalid.
    ///
    /// I recommend to log in using Incognito mode, obtain the cookies and then close the page.
    pub async fn user_auth_set_cookie_txt(&self, cookies: &str) -> Result<(), Error> {
        let cookie = util::parse_netscape_cookies(cookies, ".youtube.com")?;
        self.user_auth_set_cookie(cookie).await
    }

    /// Remove the user authentication cookie from cache storage
    pub async fn user_auth_remove_cookie(&self) -> Result<(), Error> {
        {
            let mut cookie = self.inner.cache.auth_cookie.write().unwrap();
            if cookie.is_none() {
                return Err(Error::Auth(AuthError::NoLogin));
            }
            *cookie = None;
        }
        self.store_cache().await;
        Ok(())
    }

    /// Attempt to fetch the YouTube website with login cookies to check if the user is successfully logged in
    /// and the session is still valid.
    pub async fn user_auth_check_cookie(&self) -> Result<(), Error> {
        let mut cookie = self.user_auth_cookie()?;
        self.extract_session_headers(&mut cookie).await?;
        Ok(())
    }

    /// Since YouTube allows multiple channels/profiles per account, cookie-authenticated requests must include
    /// the X-Goog-AuthUser and X-Goog-PageId headers to specify which account should be used.
    ///
    /// The header values are included in the ytcfg object which is embedded in the html code.
    async fn extract_session_headers(&self, auth_cookie: &mut AuthCookie) -> Result<(), Error> {
        let re_session_id = Regex::new(r#""USER_SESSION_ID":"[\d]+?""#).unwrap();
        let re_sync_id = Regex::new(r#""datasyncId":"([\w|]+?)""#).unwrap();
        let re_session_index = Regex::new(r#""SESSION_INDEX":"([\d]+?)""#).unwrap();

        let req = self
            .inner
            .http
            .get("https://www.youtube.com/results?search_query=")
            .header(header::COOKIE, &auth_cookie.cookie)
            .build()?;
        let html = self.http_request_txt(&req).await?;

        if !re_session_id.is_match(&html) {
            tracing::debug!("session check failed: USER_SESSION_ID not found in reponse");
            return Err(Error::Auth(AuthError::NoLogin));
        }

        let datasync_id =
            util::get_cg_from_regex(&re_sync_id, &html, 1).ok_or(Error::Extraction(
                ExtractionError::InvalidData("could not find datasyncId on html page".into()),
            ))?;

        // datasyncid is of the form "channel_syncid||user_syncid" for secondary channel
        // and just "user_syncid||" for primary channel.
        let (p1, p2) =
            datasync_id
                .split_once("||")
                .ok_or(Error::Extraction(ExtractionError::InvalidData(
                    "datasyncId does not contain || seperator".into(),
                )))?;
        (auth_cookie.channel_syncid, auth_cookie.user_syncid) = if p2.is_empty() {
            (None, Some(p1.to_owned()))
        } else {
            (Some(p1.to_owned()), Some(p2.to_owned()))
        };

        auth_cookie.session_index = Some(
            util::get_cg_from_regex(&re_session_index, &html, 1).ok_or(Error::Extraction(
                ExtractionError::InvalidData("could not find SESSION_INDEX on html page".into()),
            ))?,
        );
        Ok(())
    }

    /// Get the version string (e.g. `rustypipe-botguard 0.1.1`) of the used botguard binary
    pub async fn version_botguard(&self) -> Option<String> {
        self.inner.botguard.as_ref().map(|bg| bg.version.to_owned())
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

    /// Set the timezone and its associated UTC offset in minutes used
    /// when accessing the YouTube API.
    #[must_use]
    pub fn timezone<S: Into<String>>(mut self, timezone: S, utc_offset_minutes: i16) -> Self {
        self.opts.timezone = Some(timezone.into());
        self.opts.utc_offset_minutes = utc_offset_minutes;
        self
    }

    /// Access the YouTube API using the local system timezone
    #[must_use]
    pub fn timezone_local(self) -> Self {
        let (timezone, utc_offset_minutes) = local_tz_offset();
        self.timezone(timezone, utc_offset_minutes)
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
    ///
    /// Depending on the client type RustyPipe uses either the authentication cookie or the
    /// OAuth token to authenticate requests.
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

    /// Set the YouTube visitor data ID
    ///
    /// YouTube assigns a session cookie to each user which is used for personalized
    /// recommendations. By default, RustyPipe does not send this cookie to preserve
    /// user privacy. For requests that mandatate the cookie, a new one is requested
    /// for every query.
    ///
    /// This option allows you to manually set the visitor data ID of your query,
    /// allowing you to get personalized recommendations or reproduce A/B tests.
    ///
    /// Note that YouTube has a rate limit on the number of requests from a single
    /// visitor, so you should not use the same vistor data cookie for batch operations.
    #[must_use]
    pub fn visitor_data<S: Into<String>>(mut self, visitor_data: S) -> Self {
        self.opts.visitor_data = Some(visitor_data.into());
        self
    }

    /// Set the YouTube visitor data ID to an optional value
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
                "com.google.android.youtube/{} (Linux; U; Android {}) gzip",
                ANDROID_CLIENT_VERSION, ANDROID_VERSION
            )
            .into(),
            ClientType::Ios => format!(
                "com.google.ios.youtube/{} ({}; U; CPU iOS {} like Mac OS X)",
                IOS_CLIENT_VERSION, IOS_DEVICE_MODEL, IOS_VERSION
            )
            .into(),
        }
    }

    /// Return `true` if the client has stored login credentials for the given client type
    /// and authentication has not been disabled
    pub fn auth_enabled(&self, ctype: ClientType) -> bool {
        if self.opts.auth == Some(false) {
            return false;
        }
        if ctype.is_web() {
            let auth_cookie = self.client.inner.cache.auth_cookie.read().unwrap();
            auth_cookie.is_some()
        } else if ctype == ClientType::Tv {
            let cache_token = self.client.inner.cache.oauth_token.read().unwrap();
            cache_token.is_some()
        } else {
            false
        }
    }

    /// Filter the given list of client types and iterate over those which have login credentials available.
    pub fn auth_enabled_clients<'a>(
        &self,
        clients: &'a [ClientType],
    ) -> impl Iterator<Item = ClientType> + 'a {
        let (has_cookie, has_token) = if self.opts.auth == Some(false) {
            (false, false)
        } else {
            let auth_cookie = self.client.inner.cache.auth_cookie.read().unwrap();
            let oauth_token = self.client.inner.cache.oauth_token.read().unwrap();
            (auth_cookie.is_some(), oauth_token.is_some())
        };

        clients
            .iter()
            .filter(move |c| {
                if c.is_web() {
                    has_cookie
                } else if **c == ClientType::Tv {
                    has_token
                } else {
                    false
                }
            })
            .copied()
    }

    /// Return the first client type from the given list which has login credentials available.
    ///
    /// Returns [`None`] if authentication has been disabled or there are no available client types.
    pub fn auth_enabled_client(&self, clients: &[ClientType]) -> Option<ClientType> {
        self.auth_enabled_clients(clients).next()
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
    ) -> YTContext<'a> {
        let (hl, gl) = if localized {
            (self.opts.lang, self.opts.country)
        } else {
            (Language::En, Country::Us)
        };
        let utc_offset_minutes = self.opts.utc_offset_minutes;
        let time_zone = self.opts.timezone.as_deref().unwrap_or("UTC");

        match ctype {
            ClientType::Desktop => YTContext {
                client: ClientInfo {
                    client_name: "WEB",
                    client_version: self.client.get_client_version(ctype).await,
                    platform: "DESKTOP",
                    original_url: YOUTUBE_HOME_URL,
                    visitor_data,
                    hl,
                    gl,
                    time_zone,
                    utc_offset_minutes,
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
                    original_url: YOUTUBE_MUSIC_HOME_URL,
                    visitor_data,
                    hl,
                    gl,
                    time_zone,
                    utc_offset_minutes,
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
                    original_url: YOUTUBE_MOBILE_HOME_URL,
                    visitor_data,
                    hl,
                    gl,
                    time_zone,
                    utc_offset_minutes,
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
                    client_screen: "WATCH",
                    platform: "TV",
                    device_model: "SmartTV",
                    visitor_data,
                    hl,
                    gl,
                    time_zone,
                    utc_offset_minutes,
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
                    client_version: ANDROID_CLIENT_VERSION.into(),
                    os_name: "Android",
                    os_version: ANDROID_VERSION,
                    android_sdk_version: Some(30),
                    platform: "MOBILE",
                    visitor_data,
                    hl,
                    gl,
                    time_zone,
                    utc_offset_minutes,
                    ..Default::default()
                },
                request: None,
                user: User::default(),
                third_party: None,
            },
            ClientType::Ios => YTContext {
                client: ClientInfo {
                    client_name: "IOS",
                    client_version: IOS_CLIENT_VERSION.into(),
                    device_model: IOS_DEVICE_MODEL,
                    os_name: "iPhone",
                    os_version: IOS_VERSION_BUILD,
                    platform: "MOBILE",
                    visitor_data,
                    hl,
                    gl,
                    time_zone,
                    utc_offset_minutes,
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
    /// - `visitor_data`: YouTube visitor data ID
    async fn request_builder(
        &self,
        ctype: ClientType,
        endpoint: &str,
        visitor_data: Option<&str>,
    ) -> Result<RequestBuilder, Error> {
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
                .header("X-YouTube-Client-Name", "3")
                .header("X-Goog-Api-Format-Version", "2"),
            ClientType::Ios => self
                .client
                .inner
                .http
                .post(format!(
                    "{YOUTUBEI_V1_GAPIS_URL}{endpoint}?{DISABLE_PRETTY_PRINT_PARAMETER}"
                ))
                .header("X-YouTube-Client-Name", "5")
                .header("X-Goog-Api-Format-Version", "2"),
        };
        r = r
            .header(header::CONTENT_TYPE, "application/json")
            .header(header::USER_AGENT, self.user_agent(ctype).as_ref());
        if let Some(vdata) = self.opts.visitor_data.as_deref().or(visitor_data) {
            r = r.header("X-Goog-EOM-Visitor-Id", vdata);
        }

        let mut cookie = None;

        if self.opts.auth == Some(true) {
            if ctype.is_web() {
                let auth_cookie = self.client.user_auth_cookie()?;

                if let Some(auth_header) = Self::sapisidhash_header(&auth_cookie.cookie, ctype) {
                    r = r.header(header::AUTHORIZATION, auth_header);
                }
                if let Some(session_index) = auth_cookie.session_index {
                    r = r.header("X-Goog-AuthUser", session_index);
                }
                if let Some(account_syncid) = auth_cookie.channel_syncid {
                    r = r.header("X-Goog-PageId", account_syncid);
                }
                cookie = Some(auth_cookie.cookie);
            } else if ctype == ClientType::Tv {
                let access_token = self.client.user_auth_access_token().await?;
                r = r.header(header::AUTHORIZATION, format!("Bearer {}", access_token));
            }
        }

        if ctype.is_web() {
            r = r.header(header::COOKIE, cookie.as_deref().unwrap_or(CONSENT_COOKIE));
        }

        Ok(r)
    }

    fn sapisidhash_header(cookie: &str, ctype: ClientType) -> Option<String> {
        let sapisid = cookie
            .split(';')
            .find_map(|c| c.trim().strip_prefix("SAPISID="))?;
        let time_now = OffsetDateTime::now_utc().unix_timestamp();
        let mut sapisidhash = Sha1::new();
        sapisidhash.update(time_now.to_string());
        sapisidhash.update(" ");
        sapisidhash.update(sapisid);
        sapisidhash.update(" ");
        sapisidhash.update(match ctype {
            ClientType::DesktopMusic => YOUTUBE_MUSIC_HOME_URL,
            ClientType::Mobile => YOUTUBE_MOBILE_HOME_URL,
            _ => YOUTUBE_HOME_URL,
        });

        let sapisidhash_hex = data_encoding::HEXLOWER.encode(&sapisidhash.finalize());
        Some(format!("SAPISIDHASH {time_now}_{sapisidhash_hex}"))
    }

    /// Get a YouTube visitor data ID, which is necessary for certain requests
    pub async fn get_visitor_data(&self, force_new: bool) -> Result<String, Error> {
        if force_new {
            return self
                .client
                .inner
                .visitor_data_cache
                .new_visitor_data()
                .await;
        }

        match &self.opts.visitor_data {
            Some(vd) => Ok(vd.clone()),
            None => self.client.inner.visitor_data_cache.get().await,
        }
    }

    /// Remove a YouTube visitor data ID from the cache so it is not used again
    pub fn remove_visitor_data(&self, visitor_data: &str) {
        self.client.inner.visitor_data_cache.remove(visitor_data);
    }

    /// Generate PO tokens
    async fn get_po_tokens(&self, idents: &[&str]) -> Result<(Vec<String>, OffsetDateTime), Error> {
        let bg = self
            .client
            .inner
            .botguard
            .as_ref()
            .ok_or(ExtractionError::Botguard("not enabled".into()))?;

        let start = std::time::Instant::now();
        let cmd = tokio::process::Command::new(&bg.program)
            .arg("--snapshot-file")
            .arg(&bg.snapshot_file)
            .arg("--")
            .args(idents)
            .output()
            .await
            .map_err(|e| Error::Extraction(ExtractionError::Botguard(e.to_string().into())))?;
        if !cmd.status.success() {
            return Err(Error::Extraction(ExtractionError::Botguard(
                String::from_utf8_lossy(&cmd.stderr).into_owned().into(),
            )));
        }

        let output = String::from_utf8(cmd.stdout)
            .map_err(|e| Error::Extraction(ExtractionError::Botguard(e.to_string().into())))?;

        let mut words = output.split_whitespace();
        let mut tokens = Vec::with_capacity(idents.len());
        for _ in 0..idents.len() {
            tokens.push(
                words
                    .next()
                    .ok_or(ExtractionError::Botguard("too few tokens returned".into()))?
                    .to_owned(),
            );
        }

        let mut valid_until = None;
        let mut from_snapshot = false;
        for word in words {
            if let Some((k, v)) = word.split_once('=') {
                match k {
                    "valid_until" => {
                        valid_until = Some(
                            v.parse::<i64>()
                                .ok()
                                .and_then(|x| OffsetDateTime::from_unix_timestamp(x).ok())
                                .ok_or(ExtractionError::Botguard(
                                    format!("invalid validity date: {v}").into(),
                                ))?,
                        );
                    }
                    "from_snapshot" => {
                        from_snapshot = v.eq_ignore_ascii_case("true") || v == "1";
                    }
                    _ => {}
                }
            }
        }

        let valid_until =
            valid_until.unwrap_or_else(|| OffsetDateTime::now_utc() + time::Duration::hours(12));

        tracing::debug!(
            "generated PO token (valid_until {}, from_snapshot={}, took {}ms)",
            valid_until,
            from_snapshot,
            start.elapsed().as_millis()
        );
        Ok((tokens, valid_until))
    }

    /// Get a session-bound PO token (either from cache or newly generated)
    async fn get_session_po_token(&self, visitor_data: &str) -> Result<PoToken, Error> {
        if let Some(po_token) = self.client.inner.visitor_data_cache.get_pot(visitor_data) {
            return Ok(po_token);
        }

        let po_token = self.get_po_token(visitor_data).await?;
        self.client
            .inner
            .visitor_data_cache
            .store_pot(visitor_data, po_token.clone());
        Ok(po_token)
    }

    /// Get a PO token (Proof-of-origin token)
    ///
    /// PO tokens are used by the web-based YouTube clients for requesting player data and video streams.
    ///
    /// See <https://codeberg.org/ThetaDev/rustypipe-botguard> for more information
    pub async fn get_po_token<S: AsRef<str>>(&self, ident: S) -> Result<PoToken, Error> {
        let (tokens, valid_until) = self.get_po_tokens(&[ident.as_ref()]).await?;

        Ok(PoToken {
            po_token: tokens.into_iter().next().unwrap(),
            valid_until,
        })
    }

    /// Get a new RustyPipeInfo object for reports
    fn rp_info(&self) -> RustyPipeInfo<'_> {
        RustyPipeInfo::new(
            Some(self.opts.lang),
            self.client
                .inner
                .botguard
                .as_ref()
                .map(|bg| bg.version.as_str()),
        )
    }

    /// Execute a request to the YouTube API, then deobfuscate and map the response.
    ///
    /// Runs a single attempt, returns Ok with a erroneous RequestResult in case of a
    /// HTTP or mapping error so it can be retried/reported.
    async fn execute_request_attempt<
        R: DeserializeOwned + MapResponse<M> + Debug,
        M,
        B: Serialize + ?Sized,
    >(
        &self,
        ctype: ClientType,
        id: &str,
        endpoint: &str,
        body: &B,
        ctx_src: &MapRespOptions<'_>,
    ) -> Result<RequestResult<M>, Error> {
        let visitor_data = match ctx_src
            .visitor_data
            .or(self.opts.visitor_data.as_deref())
            .map(Cow::Borrowed)
        {
            Some(vd) => vd,
            None => self.client.inner.visitor_data_cache.get().await?.into(),
        };

        let context = self
            .get_context(ctype, !ctx_src.unlocalized, &visitor_data)
            .await;
        let req_body = QBody { context, body };

        let ctx = MapRespCtx {
            id,
            lang: self.opts.lang,
            utc_offset: UtcOffset::from_whole_seconds(i32::from(self.opts.utc_offset_minutes) * 60)
                .map_err(|_| Error::Other("utc_offset overflow".into()))?,
            deobf: ctx_src.deobf,
            visitor_data: Some(&visitor_data),
            client_type: ctype,
            artist: ctx_src.artist.clone(),
            authenticated: self.opts.auth.unwrap_or_default(),
            session_po_token: ctx_src.session_po_token.clone(),
        };

        let request = self
            .request_builder(ctype, endpoint, ctx.visitor_data)
            .await?
            .json(&req_body)
            .build()?;

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
                StatusCode::UNAUTHORIZED => Error::Auth(AuthError::NoLogin),
                _ => Error::HttpStatus(status.as_u16(), error_msg.unwrap_or_default()),
            })
        } else {
            match serde_json::from_str::<R>(&body) {
                Ok(deserialized) => match deserialized.map_response(&ctx) {
                    Ok(mapres) => Ok(mapres),
                    Err(e) => Err(e.into()),
                },
                Err(e) => Err(Error::from(ExtractionError::from(e))),
            }
        };

        tracing::trace!("mapped response");
        Ok(RequestResult {
            res,
            status,
            body,
            request,
            visitor_data: visitor_data.into_owned(),
        })
    }

    /// Execute a request to the YouTube API, then deobfuscate and map the response.
    ///
    /// Runs up to n_request_attempts, returns Ok with a erroneous RequestResult in case of a
    /// HTTP or mapping error so it can be reported.
    async fn execute_request_inner<
        R: DeserializeOwned + MapResponse<M> + Debug,
        M,
        B: Serialize + ?Sized,
    >(
        &self,
        ctype: ClientType,
        id: &str,
        endpoint: &str,
        body: &B,
        ctx_src: &MapRespOptions<'_>,
    ) -> Result<RequestResult<M>, Error> {
        let mut last_resp = None;
        for n in 0..=self.client.inner.n_http_retries {
            let resp = self
                .execute_request_attempt::<R, M, B>(ctype, id, endpoint, body, ctx_src)
                .await?;

            let err = match &resp.res {
                Ok(_) => return Ok(resp),
                Err(e) => {
                    if !e.should_retry() {
                        return Ok(resp);
                    }
                    e
                }
            };

            // Remove the used visitor data from cache if the request resulted in a recoverable error
            self.remove_visitor_data(&resp.visitor_data);

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
        ctx_src: MapRespOptions<'_>,
    ) -> Result<M, Error> {
        tracing::debug!("getting {}({})", operation, id);

        let req_res = self
            .execute_request_inner::<R, M, B>(ctype, id, endpoint, body, &ctx_src)
            .await?;
        let request = req_res.request;

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
                    info: self.rp_info(),
                    level,
                    operation: &format!("{operation}({id})"),
                    error,
                    msgs,
                    deobf_data: ctx_src.deobf.cloned(),
                    http_request: crate::report::HTTPRequest {
                        url: request.url().as_str(),
                        method: request.method().as_str(),
                        req_header: Some(
                            request
                                .headers()
                                .iter()
                                .filter(|(k, _)| k != &header::COOKIE)
                                .map(|(k, v)| {
                                    let vstr = if k == header::AUTHORIZATION {
                                        "[redacted]"
                                    } else {
                                        v.to_str().unwrap_or_default()
                                    };
                                    (k.as_str(), vstr.to_owned())
                                })
                                .collect(),
                        ),
                        req_body: request
                            .body()
                            .as_ref()
                            .and_then(|b| b.as_bytes())
                            .map(|b| String::from_utf8_lossy(b).into_owned()),
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
            MapRespOptions::default(),
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
        let visitor_data = match self.opts.visitor_data.as_deref().map(Cow::Borrowed) {
            Some(vd) => vd,
            None => self.client.inner.visitor_data_cache.get().await?.into(),
        };

        let context = self.get_context(ctype, true, &visitor_data).await;
        let req_body = QBody { context, body };

        let request = self
            .request_builder(ctype, endpoint, None)
            .await?
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

/// Additional data needed for mapping YouTube responses
struct MapRespCtx<'a> {
    id: &'a str,
    lang: Language,
    utc_offset: UtcOffset,
    deobf: Option<&'a DeobfData>,
    visitor_data: Option<&'a str>,
    client_type: ClientType,
    artist: Option<ArtistId>,
    authenticated: bool,
    session_po_token: Option<PoToken>,
}

/// Options to give to the mapper when making requests;
/// used to construct the [`MapRespCtx`]
#[derive(Default)]
struct MapRespOptions<'a> {
    visitor_data: Option<&'a str>,
    deobf: Option<&'a DeobfData>,
    artist: Option<ArtistId>,
    unlocalized: bool,
    session_po_token: Option<PoToken>,
}

#[allow(clippy::needless_lifetimes)]
impl<'a> MapRespCtx<'a> {
    /// Create a [`MapRespCtx`] for testing
    #[cfg(test)]
    fn test(id: &'a str) -> Self {
        Self {
            id,
            lang: Language::En,
            utc_offset: UtcOffset::UTC,
            deobf: None,
            visitor_data: None,
            client_type: ClientType::Desktop,
            artist: None,
            authenticated: false,
            session_po_token: None,
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

fn local_tz_offset() -> (String, i16) {
    match (
        localzone::get_local_zone().ok_or(Error::Other("could not get local timezone".into())),
        UtcOffset::current_local_offset().map_err(|_| Error::Other("indeterminate offset".into())),
    ) {
        (Ok(timezone), Ok(offset)) => (timezone, offset.whole_minutes()),
        (Err(e), _) | (_, Err(e)) => {
            tracing::error!("{e}");
            ("UTC".to_owned(), 0)
        }
    }
}

/// Check if a valid Botguard binary is available at the given location
fn detect_botguard_bin(program: OsString) -> Result<(OsString, String), Error> {
    let out = std::process::Command::new(&program)
        .arg("--version")
        .output()
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                Error::Other("rustypipe-botguard binary not found".into())
            } else {
                Error::Other(format!("error calling rustypipe-botguard {e}").into())
            }
        })?;
    if !out.status.success() {
        return Err(Error::Extraction(ExtractionError::Botguard(
            format!("version check failed with status {}", out.status).into(),
        )));
    }
    let output = String::from_utf8_lossy(&out.stdout);
    let pat = "rustypipe-botguard-api ";
    let pos = output.find(pat).ok_or(Error::Other(
        "no rustypipe-botguard-api version returned".into(),
    ))? + pat.len();
    let pos_end = output[pos..]
        .char_indices()
        .find(|(_, c)| !c.is_ascii_digit())
        .map(|(p, _)| p + pos)
        .unwrap_or(output.len());
    let api_version = &output[pos..pos_end];
    if api_version != BOTGUARD_API_VERSION {
        return Err(Error::Other(
            format!(
                "incompatible rustypipe-botguard-api version {api_version}, expected {BOTGUARD_API_VERSION}"
            )
            .into(),
        ));
    }
    let version = output[..pos].lines().next().unwrap_or_default().to_owned();
    Ok((program, version))
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
        let visitor_data = rp.query().get_visitor_data(true).await.unwrap();

        assert!(
            visitor_data.starts_with("Cg") && visitor_data.len() > 23,
            "invalid visitor data: {visitor_data}"
        );
    }

    #[tokio::test]
    async fn get_po_token() {
        let rp = RustyPipe::builder().build().unwrap();
        let ident = "Cgt4eDYyVVJveGQtbyiLyvu8BjIKCgJERRIEEgAgKw==";
        let po_token = rp.query().get_po_token(ident).await.unwrap();

        let token_bts = data_encoding::BASE64URL
            .decode(po_token.po_token.as_bytes())
            .unwrap();
        assert_eq!(token_bts.len(), ident.len() + 74);
        assert!(
            po_token.valid_until > OffsetDateTime::now_utc() + time::Duration::minutes(30),
            "valid until {}",
            po_token.valid_until
        )
    }
}
