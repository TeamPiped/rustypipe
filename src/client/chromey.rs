//! Real-browser PoToken provider.
//!
//! This module is the chromey counterpart to the
//! [`rustypipe-botguard`](https://codeberg.org/ThetaDev/rustypipe-botguard)
//! binary. Instead of running the BotGuard VM in a Deno+JSDOM
//! environment, it spawns a real headless Chrome via the
//! [`chromey`](https://github.com/spider-rs/chromey) CDP crate,
//! navigates to `https://www.youtube.com/`, and reuses the
//! botguard VM that YouTube's own page JS loads. Because the
//! snapshot is computed by YouTube's VM in YouTube's environment,
//! GVS trusts the resulting `integrityToken` (the rejection we
//! saw with a sandboxed background `bgutils-js` VM went away).
//!
//! The provider keeps a single `Browser` and `Page` alive for the
//! lifetime of the [`RustyPipe`](crate::client::RustyPipe) instance
//! and reuses them across all `mint` calls, mirroring the botguard
//! binary's snapshot reuse strategy.
//!
//! The provider is only compiled in when the `chromey-po-token`
//! cargo feature is enabled, so users that don't want a CDP dep
//! don't pay for it.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use chromiumoxide::browser::{Browser, BrowserConfig};
use chromiumoxide::cdp::browser_protocol::network::{
    EnableParams as NetworkEnableParams, EventRequestWillBeSent,
    GetRequestPostDataParams,
};
use chromiumoxide::cdp::browser_protocol::page::{
    AddScriptToEvaluateOnNewDocumentParams, EnableParams as PageEnableParams,
};
use chromiumoxide::cdp::js_protocol::runtime::{
    EnableParams as RuntimeEnableParams, EvaluateParams,
};
use chromiumoxide::page::Page;
use futures_util::stream::StreamExt;
use time::OffsetDateTime;
use tokio::sync::Mutex;
use tracing::debug;
use wreq::Client;

use crate::error::{Error, ExtractionError};

/// Default user agent reported to the BotGuard challenge and used
/// by the rustypipe HTTP client when the chromey provider is
/// enabled.
///
/// YouTube's GVS server cross-checks the SABR request's
/// `User-Agent` header against the environment the PoToken was
/// minted in. The PoToken's environment is bound by the Chrome
/// process's own `User-Agent` (which determines what the BotGuard
/// VM's `navigator.userAgent` reports), so the rustypipe client and
/// the Chrome browser **must** advertise the same UA.
///
/// The safe choice is "Chrome on Linux x86_64" — the chromey
/// browser runs on Linux x86_64 in headless mode, and BotGuard
/// fingerprints many signals (`navigator.platform`, screen,
/// plugins, ...) that all come from the host OS, not from the
/// `--user-agent` flag. Claiming a Windows or macOS UA while
/// running on Linux is a tell-tale bot signal that the server
/// uses to reject the token. Claiming a Linux UA matches the
/// actual environment and passes the cross-check.
pub(crate) const DEFAULT_UA: &str =
    "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/143.0.0.0 Safari/537.36";

/// Cap on a single BotGuard request.
const PER_REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

/// How long the BotGuard minter is considered valid. The server
/// reports a precise lifetime; this is the maximum we'll cache it
/// for as a safety cap.
const MAX_TOKEN_LIFETIME: Duration = Duration::from_secs(6 * 60 * 60);

/// Default lifetime if the server doesn't return one.
const DEFAULT_TOKEN_LIFETIME: Duration = Duration::from_secs(12 * 60 * 60);

/// YouTube API key (matches the one used by the botguard binary).
const GOOG_API_KEY: &str = "AIzaSyDyT5W0Jh49F30Pqqtyfdf7pDLFKLJoAnw";
/// X-User-Agent header for botguard endpoints.
const X_USER_AGENT: &str = "grpc-web-javascript/0.1";
/// Botguard content-binding key (session-bound Create/GenerateIT).
const REQUEST_KEY: &str = "O43z0dpjhgX20SCx4KAo";

/// JS that is registered on the main frame via
/// `Page.add_script_to_evaluate_on_new_document` and runs on the
/// first navigation. Defines `globalThis.runSnapshot(program,
/// globalName)`, `globalThis.newMinter(integrityToken,
/// webPoSignalOutput)`, and `globalThis.mint(identifier)` — all
/// reuse the botguard VM that YouTube's own page JS loads.
const RUNNER_JS: &str = include_str!("chromey_runner.js");

/// Real-browser PoToken provider.
///
/// Cloning the provider is cheap and shares the underlying browser
/// state. All clones mint from the same Chrome process.
#[derive(Clone)]
pub struct ChromeyProvider {
    inner: Arc<Inner>,
}

struct Inner {
    /// Path to the Chrome/Chromium binary (or `None` to auto-detect).
    chrome_path: Option<PathBuf>,
    user_agent: String,
    /// Run Chrome with a real window (requires Xvfb on headless hosts)
    /// instead of `--headless=new`. YouTube's botguard VM uses a few
    /// headless-only fingerprints (most importantly, the absence of
    /// a real `window` object on some CDP-attached pages), and a
    /// headful launch sidesteps the entire class.
    headful: bool,
    /// HTTP client used for the GenerateIT call. The browser
    /// context adds `sec-fetch-*`, `origin`, `referer` and other
    /// headers to cross-origin fetches that YouTube's
    /// botguard-validation endpoint rejects, so we run the
    /// GenerateIT call from Rust with a clean request instead.
    http: Client,
    /// When set, the provider listens for the first SABR
    /// `videoplayback` POST the watch page makes after init,
    /// extracts the `po_token` from the request body, and writes
    /// the raw bytes to this file. The rustypipe downloader
    /// reads the same file via the `RUSTYPIPE_SABR_PO_TOKEN_FILE`
    /// env-var override to send the *real browser's* PoToken
    /// instead of the one we mint from our own VM. The point is
    /// to compare GVS's behaviour against a PoToken we know was
    /// accepted (the browser's own player got a 200 with it) —
    /// if rustypipe's SABR stream still gets 403, the problem
    /// is in rustypipe's request envelope, not in the PoToken.
    intercept_file: Option<PathBuf>,
    state: Mutex<ProviderState>,
}

enum ProviderState {
    Uninit,
    /// The most recent mint produced these tokens. We
    /// re-use Chrome between calls but re-run the BotGuard
    /// flow each time, so we just keep the last set of
    /// tokens.
    Init {
        _browser: Option<Browser>,
        _handler_task: Option<tokio::task::JoinHandle<()>>,
        page: Option<Page>,
        _user_data_dir: Option<PathBuf>,
        cached_tokens: Vec<String>,
        valid_until: OffsetDateTime,
        /// `(contentBinding, signedTimestamp)` the cached
        /// minter was created with. YouTube's botguard
        /// `asyncSnapshotFunction` bakes those into the
        /// minter, so a new `(binding, sts)` requires a full
        /// re-init (new VM, new integrityToken, new
        /// minter). The Rust side compares on every
        /// `mint_with_binding` call and drops back to
        /// `Uninit` if they differ.
        binding: Option<(String, String)>,
        /// The video_id whose watch page the cached minter
        /// was built on. YouTube's botguard VM fingerprints
        /// the page's origin/referrer/navigation history
        /// and BgUtils builds the minter from the resulting
        /// signals. A minter built on the root
        /// `youtube.com` page is rejected by GVS when used
        /// to mint a token for a video-id identifier,
        /// because the watch page is the only place where
        /// the VM has the navigation context GVS expects.
        /// Tracking it lets the next call detect a
        /// video_id change and re-navigate to the new watch
        /// page before re-running the BotGuard flow.
        video_id: Option<String>,
    },
    /// BotGuard challenge failed; the next call rebuilds the
    /// browser.
    Failed(String),
}

impl ChromeyProvider {
    /// Create a new chromey provider. The Chrome binary is
    /// auto-detected at the first `mint` call; pass a `chrome_path`
    /// to override the auto-detect. Defaults to headless mode;
    /// switch to headful with [`Self::with_headful`].
    pub fn new(chrome_path: Option<PathBuf>) -> Self {
        let http = Client::builder()
            .user_agent(DEFAULT_UA)
            .build()
            .expect("building a wreq::Client should not fail");
        Self {
            inner: Arc::new(Inner {
                chrome_path,
                user_agent: DEFAULT_UA.to_owned(),
                headful: false,
                http,
                intercept_file: None,
                state: Mutex::new(ProviderState::Uninit),
            }),
        }
    }

    /// Switch the chromey provider to headful mode. The Chrome
    /// process will be launched with a real window (via `with_head`)
    /// rather than `--headless=new`.
    ///
    /// On a headless host you also need an X server — usually
    /// `xvfb-run` (`xvfb-run -a rustypipe --chromey --chromey-headful download ...`)
    /// or a persistent `Xvfb :99` + `DISPLAY=:99`. Headful mode
    /// bypasses the headless-only fingerprint checks that the
    /// BotGuard VM runs (most importantly, the `navigator.webdriver`
    /// and `window.chrome` shape checks), at the cost of needing a
    /// display.
    ///
    /// This is a no-op once the provider has been shared; if the
    /// `Arc<Inner>` already has multiple owners, the headful flag
    /// silently stays at whatever it was on the first call.
    #[must_use]
    pub fn with_headful(mut self, headful: bool) -> Self {
        if let Some(inner) = Arc::get_mut(&mut self.inner) {
            inner.headful = headful;
        }
        self
    }

    /// Override the user agent reported to the BotGuard challenge.
    /// The Chrome process is also launched with this UA.
    #[allow(dead_code)]
    pub fn with_user_agent(mut self, ua: impl Into<String>) -> Self {
        if let Some(inner) = Arc::get_mut(&mut self.inner) {
            inner.user_agent = ua.into();
        }
        self
    }

    /// Enable SABR `videoplayback` interception. When set, the
    /// provider navigates to the watch page, lets the page
    /// actually start playing, captures the first
    /// `googlevideo.com/videoplayback?…&sabr=1` POST the browser
    /// sends, extracts its `StreamerContext.po_token` (the same
    /// PoToken the browser just used to talk to GVS), and writes
    /// the raw PoToken bytes to `path`.
    ///
    /// The rustypipe downloader reads the same file via the
    /// `RUSTYPIPE_SABR_PO_TOKEN_FILE` env-var override to send
    /// that exact token in its own SABR requests. The point of
    /// the experiment is: "if I send the *real browser's* PoToken,
    /// does rustypipe's SABR stream work?" If yes, the problem is
    /// in our minted token. If no, the problem is in rustypipe's
    /// request envelope (the URL signature, the
    /// `ClientAbrState`, the headers, etc.) and the PoToken
    /// shape is fine.
    ///
    /// The interception adds ~3-15s of wall-clock to the first
    /// `mint` call (waiting for the page to play). It is a
    /// diagnostic mode, not a hot-path optimisation. Same caveats
    /// as `with_headful`: the player needs a user gesture to
    /// autoplay in headless mode, so the provider passes
    /// `--autoplay-policy=no-user-gesture-required` to Chrome
    /// and additionally clicks the play button via JS in case
    /// the autoplay flag is ignored.
    ///
    /// This is a no-op once the provider has been shared; if the
    /// `Arc<Inner>` already has multiple owners, the file path
    /// silently stays at whatever it was on the first call.
    #[must_use]
    pub fn with_intercept_file(mut self, path: impl Into<PathBuf>) -> Self {
        if let Some(inner) = Arc::get_mut(&mut self.inner) {
            inner.intercept_file = Some(path.into());
        }
        self
    }

    /// Mint a base64url-encoded PoToken for each given identifier.
    ///
    /// Returns one token per ident, in the same order. The
    /// returned `OffsetDateTime` is when the underlying BotGuard
    /// minter expires (server-reported `lifetime`); all tokens
    /// minted in the same call share it.
    ///
    /// `content_binding` and `signed_timestamp` are bound into
    /// the resulting minter by botguard's
    /// `asyncSnapshotFunction`. YouTube's `player.js` always
    /// passes `(visitor_data, deobf.sts)`; doing the same is
    /// what makes the resulting PoToken accepted by GVS
    /// without an `attestation_required` retry loop. Pass
    /// `None, None` to keep the historical unbound behaviour
    /// (each mint produces a token that GVS treats as a
    /// cold-start and asks for a fresh one immediately).
    ///
    /// `video_id` is the YouTube video id whose watch page the
    /// botguard VM should be loaded on. YouTube's player.js
    /// runs the VM inside the `watch?v=<VIDEO_ID>` page, and
    /// the resulting PoTokens are bound to that page's
    /// navigation context. Minting from a generic
    /// `youtube.com` page produces PoTokens GVS rejects with
    /// `attestation_required` after the first few segments,
    /// because the server cross-checks the VM environment
    /// against the page that issued the request. Pass
    /// `None` to fall back to the historical root-page
    /// navigation (used by callers that don't have a video id
    /// handy, e.g. the public `get_po_token` API).
    pub async fn mint(
        &self,
        idents: &[&str],
        content_binding: Option<&str>,
        signed_timestamp: Option<&str>,
        video_id: Option<&str>,
    ) -> Result<(Vec<String>, OffsetDateTime), Error> {
        if idents.is_empty() {
            return Ok((
                Vec::new(),
                OffsetDateTime::now_utc() + DEFAULT_TOKEN_LIFETIME,
            ));
        }
        let requested_binding = match (content_binding, signed_timestamp) {
            (Some(b), Some(s)) => Some((b.to_owned(), s.to_owned())),
            _ => None,
        };
        let requested_video_id = video_id.map(|v| v.to_owned());

        // We do a full init() (Create → snapshot → GenerateIT
        // → mint) on every call, because the minter cache
        // doesn't survive across CDP `evaluate_function`
        // isolated-world execution contexts. Chrome itself
        // is launched once and reused across calls; only
        // the BotGuard flow re-runs each time.
        let mut state = self.inner.state.lock().await;
        match &*state {
            ProviderState::Failed(msg) => {
                return Err(Error::Extraction(ExtractionError::Chromey(
                    msg.clone().into(),
                )));
            }
            ProviderState::Init {
                page: Some(page),
                binding,
                video_id: cached_video_id,
                ..
            } if binding == &requested_binding
                && cached_video_id == &requested_video_id =>
            {
                // Reuse the existing minter. The minter lives
                // on the page's `globalThis` (as
                // `__rustypipeMint`) and is bound to the
                // integrityToken from the previous init, so
                // every PoToken we mint from it is signed by
                // the SAME botguard VM instance. This is what
                // GVS expects: a SABR attestation refresh must
                // come from the same VM as the original
                // content token from the player request.
                //
                // We also require the binding and video_id to
                // match the one used to create the minter —
                // botguard's `asyncSnapshotFunction` baked the
                // binding in, and the page's navigation
                // context is what produced the VM instance.
                // Minting from a minter built on the root
                // `youtube.com` page produces PoTokens that
                // GVS rejects after a few segments because
                // the server cross-checks the VM environment
                // against the page that issued the request.
                // If either doesn't match, fall through to
                // the re-init path below.
                match Self::mint_with_existing_minter(page, idents).await {
                    Ok(tokens) => {
                        // Keep the existing valid_until; the
                        // minter is the same one as last time.
                        let valid_until = match &*state {
                            ProviderState::Init { valid_until, .. } => *valid_until,
                            _ => unreachable!(),
                        };
                        if let ProviderState::Init { cached_tokens, .. } = &mut *state {
                            *cached_tokens = tokens.clone();
                        }
                        return Ok((tokens, valid_until));
                    }
                    Err(e) => {
                        // Existing minter unusable (e.g. page
                        // died, or `__rustypipeMint` was never
                        // installed). Drop the page and re-init.
                        tracing::debug!(
                            "chromey: existing minter unusable ({}); re-initialising",
                            e
                        );
                        *state = ProviderState::Uninit;
                    }
                }
            }
            ProviderState::Init {
                binding,
                video_id: cached_video_id,
                ..
            } if binding != &requested_binding
                || cached_video_id != &requested_video_id =>
            {
                // The cached minter was built for a different
                // (binding, sts) pair or on a different page.
                // botguard baked the binding into the VM at
                // snapshot time, and the page's navigation
                // context is what produced the VM. We have to
                // tear down and rebuild before the next mint
                // can produce tokens GVS accepts.
                tracing::debug!(
                    "chromey: binding or video_id changed (binding {:?}->{:?}, video_id {:?}->{:?}); re-initialising",
                    binding,
                    requested_binding,
                    cached_video_id,
                    requested_video_id
                );
                *state = ProviderState::Uninit;
            }
            _ => {}
        }
        // Self::init writes the state itself, so we just
        // return the tokens. On error, set the state to
        // Failed so subsequent calls don't try to reuse the
        // (now-defunct) browser/page.
        let result = Self::init(
            &self.inner,
            &mut state,
            idents,
            content_binding,
            signed_timestamp,
            video_id,
        )
        .await;
        match result {
            Ok((tokens, valid_until)) => Ok((tokens, valid_until)),
            Err(e) => {
                let msg = e.to_string();
                *state = ProviderState::Failed(msg);
                Err(e)
            }
        }
    }

    /// Backwards-compatible mint without a binding. Equivalent
    /// to `mint(idents, None, None)`. Used by callers that
    /// don't have a `(visitor_data, deobf.sts)` pair handy
    /// (e.g. the public `get_po_token` API). The resulting
    /// token will be treated as a cold-start by GVS — fine
    /// for the initial player request, not fine for SABR
    /// attestation refreshes that need to come from a
    /// bound minter.
    pub async fn mint_unbound(
        &self,
        idents: &[&str],
    ) -> Result<(Vec<String>, OffsetDateTime), Error> {
        self.mint(idents, None, None, None).await
    }

    /// Return true if the provider's underlying browser is still
    /// alive. Used by the fallback logic in `get_po_tokens` to
    /// decide whether to retry chromey or fall through to the
    /// botguard binary.
    pub async fn is_healthy(&self) -> bool {
        let state = self.inner.state.lock().await;
        matches!(&*state, ProviderState::Init { .. })
    }

    async fn init(
        inner: &Inner,
        state: &mut ProviderState,
        idents: &[&str],
        content_binding: Option<&str>,
        signed_timestamp: Option<&str>,
        video_id: Option<&str>,
    ) -> Result<(Vec<String>, OffsetDateTime), Error> {
        let start = Instant::now();
        // The (binding, sts) pair that gets baked into the
        // minter at snapshot time. Stored in `Init` so the
        // outer `mint` can detect when the next call needs a
        // re-init.
        let bound_binding = match (content_binding, signed_timestamp) {
            (Some(b), Some(s)) => Some((b.to_owned(), s.to_owned())),
            _ => None,
        };
        // The video_id whose watch page the botguard VM is
        // loaded on. Stored in `Init` so the outer `mint`
        // can detect when the next call needs a re-init
        // (different video_id => different page =>
        // different VM environment).
        let bound_video_id = video_id.map(|v| v.to_owned());
        let chrome_path = match &inner.chrome_path {
            Some(p) => p.clone(),
            None => detect_chrome().map_err(|e| {
                Error::Extraction(ExtractionError::Chromey(
                    format!("chrome binary not found: {e}").into(),
                ))
            })?,
        };

        let user_data_dir = tempdir();
        let mut builder = BrowserConfig::builder()
            .chrome_executable(&chrome_path)
            .user_data_dir(&user_data_dir)
            .request_timeout(PER_REQUEST_TIMEOUT)
            .launch_timeout(Duration::from_secs(20))
            .no_sandbox();
        // Headful mode launches Chrome with a real window (needs an
        // X server / Xvfb). It is the most "real" environment we can
        // present to the botguard VM, so it sidesteps the
        // headless-only fingerprint checks. On a headless host pair
        // it with `xvfb-run`. Headless (default) uses `--headless=new`,
        // which shares the rendering pipeline with headed Chrome and
        // passes most checks too. The old `--headless` mode is
        // detected and rejected.
        if inner.headful {
            tracing::info!(
                "chromey: launching Chrome in headful mode (DISPLAY={})",
                std::env::var("DISPLAY").unwrap_or_else(|_| "<unset>".into())
            );
            builder = builder.with_head().window_size(1920, 1080);
        } else {
            tracing::debug!("chromey: launching Chrome in --headless=new mode");
            builder = builder.new_headless_mode();
        }
        // YouTube's botguard VM reads `navigator.webdriver` as one of
        // its fingerprint signals. In a default Chrome launch, Blink
        // sets it to `true` whenever a session is automation-controlled,
        // and the botguard VM rejects the resulting token at mint time.
        // chromey's default args already include this flag, but we
        // also pass it explicitly to be safe if defaults ever change.
        let config = builder
            .arg("--disable-blink-features=AutomationControlled")
            // Pass the UA at the process level so the server sees a
            // single consistent value across fetch and JS contexts.
            .arg(format!("--user-agent={}", inner.user_agent))
            // YouTube's autoplay-on-load doesn't trigger in headless
            // Chrome without a user gesture, which means the player
            // would never actually send a `videoplayback` request
            // and our intercept mode would time out. The flag tells
            // Chrome to skip the user-gesture check for autoplay.
            // Harmless when intercept mode is off (the player won't
            // start on its own anyway because the watch page's own
            // autoplay logic also gates on user gesture).
            .arg("--autoplay-policy=no-user-gesture-required")
            .build()
            .map_err(|e| {
                Error::Extraction(ExtractionError::Chromey(e.to_string().into()))
            })?;

        let (browser, mut handler) = Browser::launch(config).await.map_err(|e| {
            Error::Extraction(ExtractionError::Chromey(e.to_string().into()))
        })?;

        // Spawn the handler task that drains CDP events. Aborting
        // it on Drop / state replacement terminates the websocket
        // reader, which causes the Browser's child process to be
        // killed by its Drop impl.
        let handler_task = tokio::task::spawn(async move {
            while let Some(h) = handler.next().await {
                if h.is_err() {
                    break;
                }
            }
        });

        let page = browser
            .new_page("about:blank")
            .await
            .map_err(|e| {
                Error::Extraction(ExtractionError::Chromey(e.to_string().into()))
            })?;

        // Enable the Runtime and Page CDP domains so the
        // chromey handler receives the events it needs to track
        // execution contexts. Without this, `execution_context()`
        // never returns a context id and `evaluate` calls fail
        // with -32602.
        page.execute(RuntimeEnableParams::default())
            .await
            .map_err(|e| {
                Error::Extraction(ExtractionError::Chromey(e.to_string().into()))
            })?;
        page.execute(PageEnableParams::default())
            .await
            .map_err(|e| {
                Error::Extraction(ExtractionError::Chromey(e.to_string().into()))
            })?;
        // The Network domain is only needed when the
        // `intercept_file` mode is active (to observe the page's
        // `videoplayback` POSTs). Enable it unconditionally
        // when intercept mode is on so the listener task below
        // can attach its `EventRequestWillBeSent` stream.
        if inner.intercept_file.is_some() {
            page.execute(NetworkEnableParams::default())
                .await
                .map_err(|e| {
                    Error::Extraction(ExtractionError::Chromey(
                        format!("Network.enable: {e}").into(),
                    ))
                })?;
        }

        // Register the runner JS on every new document so it
        // runs as soon as the main frame is created when we
        // navigate to youtube.com. We need it set up before
        // YouTube's own botguard finishes, so that the
        // `window[globalName]` poll inside `runSnapshot` can
        // pick up the VM as soon as it lands.
        //
        // `add_script_to_evaluate_on_new_document` is the
        // standard way to run code on a fresh document in CDP;
        // we can't use `evaluate_function` here because
        // youtube.com replaces the page immediately on load.
        let runner_expr = format!(
            "(function() {{\n{}\n}})();",
            RUNNER_JS
        );
        page.execute(
            AddScriptToEvaluateOnNewDocumentParams::builder()
                .source(runner_expr)
                .run_immediately(true)
                .build()
                .map_err(|e| {
                    Error::Extraction(ExtractionError::Chromey(
                        format!("add_script_to_evaluate_on_new_document builder failed: {e}")
                            .into(),
                    ))
                })?,
        )
        .await
        .map_err(|e| {
            Error::Extraction(ExtractionError::Chromey(
                format!("add_script_to_evaluate_on_new_document failed: {e}").into(),
            ))
        })?;

        // The watch page doesn't call jnn/v1/Create until the
        // user actually starts playback and the player detects
        // that a PoToken is needed. We don't want to wait for
        // that — instead we trigger the Create call ourselves
        // by evaluating JS in the page context. This works
        // because the page's fetch() inherits the right
        // cookies, sec-fetch-* headers, origin, etc. that
        // YouTube's botguard-validation endpoint expects.
        //
        // First we have to navigate to a youtube.com page so
        // the page's origin is youtube.com — otherwise the
        // fetch() fails with CORS / "Failed to fetch".
        //
        // When the caller has a video_id, navigate to the
        // actual `watch?v=<VIDEO_ID>` page instead of the
        // root. YouTube's player.js runs the botguard VM
        // inside the watch page, and the VM fingerprints
        // the page's navigation history, referrer, and
        // visible DOM. A minter built on the root page
        // produces PoTokens that GVS rejects with
        // `attestation_required` after the first few SABR
        // segments, because the server cross-checks the VM
        // environment against the page that issued the
        // request. Navigating to the watch page gives the
        // VM the same context YouTube's player would.
        //
        // The player doesn't fully initialize (no autoplay
        // without a user gesture in headless), but the
        // botguard VM is loaded very early in the page's
        // boot — long before playback starts. We wait for
        // `document.readyState === "complete"` and a small
        // post-load delay so the page's botguard-installer
        // has run and the watch page's full DOM is
        // available. The runner JS we registered via
        // `add_script_to_evaluate_on_new_document` runs
        // on every new document so it's already set up
        // when the goto resolves.
        let initial_url = match video_id {
            Some(vid) => {
                eprintln!(
                    "[chromey debug] navigating to youtube.com/watch?v={} (for origin)",
                    vid
                );
                format!("https://www.youtube.com/watch?v={}", vid)
            }
            None => {
                eprintln!("[chromey debug] navigating to youtube.com root (for origin)");
                "https://www.youtube.com/".to_owned()
            }
        };
        page.goto(&initial_url)
            .await
            .map_err(|e| {
                Error::Extraction(ExtractionError::Chromey(e.to_string().into()))
            })?;
        eprintln!("[chromey debug] page loaded, waiting for document.readyState=complete...");

        // Wait for the page to finish loading. The headless
        // page has no user gesture, so the player won't
        // start playback on its own; but the botguard VM
        // is loaded very early in the boot path, so we just
        // need the main document to finish + a small
        // post-load delay for any synchronous module
        // evaluation. Wrap the async function in an IIFE
        // so `await_promise` actually has a Promise to
        // wait on (passing the function directly returns
        // the function object, not its result).
        let ready_expr = format!(
            r#"(async () => {{
            const deadline = Date.now() + 15000;
            while (Date.now() < deadline) {{
                if (document.readyState === "complete") break;
                await new Promise((r) => setTimeout(r, 50));
            }}
            if (document.readyState !== "complete") {{
                throw new Error("document.readyState did not reach 'complete' within 15s");
            }}
            // Give YouTube's post-load scripts a moment to
            // populate window.yt and the botguard hooks.
            // (The player itself doesn't auto-play in
            // headless Chrome without a user gesture, so
            // we don't wait for `<video>` to be ready.)
            await new Promise((r) => setTimeout(r, 1500));
            // Verify the page exposed a few of YouTube's
            // standard globals; if none of them are set
            // the page probably didn't finish loading
            // correctly and our PoTokens will be flagged
            // as bot-environment tokens.
            return {{
                hasYt:        typeof window.yt !== "undefined",
                hasYtcfg:     typeof window.ytcfg !== "undefined",
                hasResponse:  typeof window.ytInitialPlayerResponse !== "undefined",
                hasLocation:  !!window.location && window.location.hostname.endsWith("youtube.com"),
                hasChrome:    typeof window.chrome !== "undefined",
                hasNavigator: !!window.navigator,
                userAgent:    navigator.userAgent.slice(0, 60),
                webdriver:    navigator.webdriver === true,
                plugins:      (navigator.plugins || []).length,
                languages:    (navigator.languages || []).join(","),
                timezone:     (Intl && Intl.DateTimeFormat)
                    ? Intl.DateTimeFormat().resolvedOptions().timeZone
                    : null,
                historyLen:   history.length,
            }};
            }})()"#
        );
        let ready_eval = EvaluateParams::builder()
            .expression(ready_expr.to_string())
            .await_promise(true)
            .return_by_value(true)
            .build()
            .map_err(|e| {
                Error::Extraction(ExtractionError::Chromey(
                    format!("ready EvalParams build: {e}").into(),
                ))
            })?;
        let ready_resp = page.execute(ready_eval).await.map_err(|e| {
            Error::Extraction(ExtractionError::Chromey(
                format!("ready wait failed: {e}").into(),
            ))
        })?;
        let ready_json: serde_json::Value =
            serde_json::to_value(&*ready_resp).unwrap_or(serde_json::Value::Null);
        let signals = ready_json
            .get("result")
            .and_then(|r| r.get("value"))
            .cloned()
            .unwrap_or(serde_json::Value::Null);
        eprintln!(
            "[chromey debug] watch page ready, signals={}",
            signals
        );

        // Optional: let the watch page actually play and capture
        // the PoToken the browser used in its first SABR
        // `videoplayback` request. Useful for debugging — if
        // rustypipe's own mint produces a token that GVS
        // rejects, we want to know whether the issue is the
        // token shape or something else (URL signature, request
        // envelope, IP reputation, etc.). See
        // `Self::with_intercept_file` for the full rationale.
        if let Some(intercept_path) = inner.intercept_file.clone() {
            Self::capture_browser_potoken(page.clone(), &intercept_path).await;
        }

        eprintln!("[chromey debug] calling Create from page context...");

        // The Create endpoint accepts a JSON array payload
        // shaped `[requestKey, contents]`, where `requestKey`
        // is a content-binding key extracted from the player
        // (see the previous `pickChallenge` implementation).
        // For a session-bound Create we use the fixed request
        // key `O43z0dpjhgX20SCx4KAo` from the botguard
        // binary.
        const GOOG_API_KEY: &str = "AIzaSyDyT5W0Jh49F30Pqqtyfdf7pDLFKLJoAnw";
        const X_USER_AGENT: &str = "grpc-web-javascript/0.1";
        const REQUEST_KEY: &str = "O43z0dpjhgX20SCx4KAo";

        // Step 1: call jnn/v1/Create from the page so we get a
        // proper `program` and `globalName` for a real Chrome
        // environment. We do this via fetch() so cookies,
        // sec-fetch-*, and origin headers are populated by
        // Chrome's network stack.
        let create_js = format!(
            r#"async () => {{
                const resp = await fetch(
                    "https://www.youtube.com/api/jnn/v1/Create",
                    {{
                        method: "POST",
                        headers: {{
                            "content-type": "application/json+protobuf",
                            "x-goog-api-key": "{api_key}",
                            "x-user-agent": "{user_agent}",
                        }},
                        body: JSON.stringify([{request_key}]),
                        credentials: "include",
                    }}
                );
                if (!resp.ok) {{
                    throw new Error("Create HTTP " + resp.status);
                }}
                return await resp.text();
            }}"#,
            api_key = GOOG_API_KEY,
            user_agent = X_USER_AGENT,
            request_key = serde_json::to_string(REQUEST_KEY).unwrap(),
        );
        let create_body_str: String = page
            .evaluate_function(create_js)
            .await
            .map_err(|e| {
                Error::Extraction(ExtractionError::Chromey(
                    format!("Create fetch failed: {e}").into(),
                ))
            })?
            .into_value()
            .map_err(|e| {
                Error::Extraction(ExtractionError::Chromey(
                    format!("Create fetch returned bad value: {e}").into(),
                ))
            })?;
        eprintln!(
            "[chromey debug] Create response: {} bytes",
            create_body_str.len()
        );
        // Save the raw response for debugging — the shape is
        // important to know whether the descrambled path is
        // correct.
        let _ = std::fs::write(
            "/tmp/rustypipe-chromey-create.bin",
            create_body_str.as_bytes(),
        );
        let create_body = create_body_str.into_bytes();

        // Parse the Create response. The body is a JSON
        // array `[null, "<base64>"]` where the base64 string,
        // once decoded, has each byte shifted by +97. After
        // the shift the result is a UTF-8 JSON array we can
        // parse normally to extract `program`,
        // `globalName`, and the interpreter JavaScript source
        // (see `parseChallengeData` in bgutils-js).
        let program: String;
        let global_name: String;
        // Field 3 of the bgChallenge proto (interpreterHash) and
        // field 7 (clientExperimentsStateBlob) are both read by
        // player.js's `cV` and threaded into the `im` Minter.
        // The player uses interpreterHash on the *next* Create
        // call; the experiments state blob is forwarded to the
        // VM at snapshot time. We capture both for the same
        // reasons, even though rustypipe only does a single
        // Create per init.
        let interpreter_hash: Option<String>;
        let client_experiments_state_blob: Option<String>;
        let challenge_json: serde_json::Value = {
            let outer: serde_json::Value =
                serde_json::from_slice(&create_body).map_err(|e| {
                    Error::Extraction(ExtractionError::Chromey(
                        format!("Create response not JSON: {e}").into(),
                    ))
                })?;
            let scrambled_b64 = outer
                .as_array()
                .and_then(|a| a.get(1))
                .and_then(|v| v.as_str())
                .ok_or_else(|| {
                    Error::Extraction(ExtractionError::Chromey(
                        "Create: arr[1] not a string (scrambled)".into(),
                    ))
                })?;
            let mut bytes = data_encoding::BASE64
                .decode(scrambled_b64.as_bytes())
                .map_err(|e| {
                    Error::Extraction(ExtractionError::Chromey(
                        format!("Create: base64 decode failed: {e}").into(),
                    ))
                })?;
            for b in bytes.iter_mut() {
                *b = b.wrapping_add(97);
            }
            let plain = String::from_utf8(bytes).map_err(|e| {
                Error::Extraction(ExtractionError::Chromey(
                    format!("Create: descrambled not utf-8: {e}").into(),
                ))
            })?;
            let cj: serde_json::Value = serde_json::from_str(&plain).map_err(|e| {
                Error::Extraction(ExtractionError::Chromey(
                    format!("Create descrambled not JSON: {e}").into(),
                ))
            })?;
            let arr = cj.as_array().ok_or_else(|| {
                Error::Extraction(ExtractionError::Chromey(
                    "Create response not a JSON array".into(),
                ))
            })?;
            program = arr
                .get(4)
                .and_then(|v| v.as_str())
                .ok_or_else(|| {
                    Error::Extraction(ExtractionError::Chromey(
                        "Create: arr[4] not a string (program)".into(),
                    ))
                })?
                .to_owned();
            global_name = arr
                .get(5)
                .and_then(|v| v.as_str())
                .ok_or_else(|| {
                    Error::Extraction(ExtractionError::Chromey(
                        "Create: arr[5] not a string (globalName)".into(),
                    ))
                })?
                .to_owned();
            // Field 3 = interpreterHash (used by the botguard
            // client on subsequent Create calls; we don't need
            // it ourselves but capture it for diagnostics and
            // for any future re-init flow).
            interpreter_hash = arr
                .get(3)
                .and_then(|v| v.as_str())
                .map(|s| s.to_owned());
            // Field 7 = clientExperimentsStateBlob (a stringified
            // JSON `V9O` proto). Player.js forwards this into the
            // `im` Minter (line 6000: `TQ(M.challenge, 7, ...)`)
            // so the VM sees the real experiment state when
            // computing the snapshot. Without it the Minter is
            // built with a default empty V9O, which makes the
            // VM produce tokens GVS rejects as bad refreshes.
            client_experiments_state_blob = arr
                .get(7)
                .and_then(|v| v.as_str())
                .map(|s| s.to_owned());
            eprintln!(
                "[chromey debug] interpreterHash={} clientExperimentsStateBlob={} bytes",
                interpreter_hash.as_deref().unwrap_or("<none>"),
                client_experiments_state_blob.as_deref().map(|s| s.len()).unwrap_or(0),
            );
            cj
        };
        // The interpreter is at `arr[1]` and is itself an
        // array `["", "blob", null, "privateDoNot.../js", null, null, ...]`
        // where one of the entries is the interpreter
        // JavaScript source string. BgUtils finds the first
        // non-null string entry.
        let interpreter_javascript: String = {
            let arr1 = challenge_json
                .as_array()
                .and_then(|a| a.get(1))
                .and_then(|v| v.as_array())
                .ok_or_else(|| {
                    Error::Extraction(ExtractionError::Chromey(
                        "Create: arr[1] not an array".into(),
                    ))
                })?;
            let mut found: Option<String> = None;
            for v in arr1 {
                if let Some(s) = v.as_str() {
                    if !s.is_empty() && s.len() > 100 {
                        found = Some(s.to_owned());
                        break;
                    }
                }
            }
            found.ok_or_else(|| {
                Error::Extraction(ExtractionError::Chromey(
                    "Create: no interpreterJavascript in arr[1]".into(),
                ))
            })?
        };
        eprintln!(
            "[chromey debug] interpreter JS: {} bytes, program: {} bytes, globalName: {}",
            interpreter_javascript.len(),
            program.len(),
            global_name
        );

        // Now we have `program`, `globalName`,
        // `interpreterJavascript`, and
        // `clientExperimentsStateBlob`. Step A: load the
        // interpreter into the page (this attaches the VM
        // to `window[globalName]`). Step B: run the
        // program to get a snapshot. We do both in a single
        // evaluate call (with `context_id = ctx_id`) so the
        // page's main execution context is used for the
        // snapshot too — otherwise chromey may pick a
        // different context for the runSnapshot call and
        // fail with `-32602`.
        eprintln!("[chromey debug] interpreter loaded, running snapshot...");
        // Stash the interpreter on globalThis FIRST in a
        // separate evaluate call (so it can be referenced in
        // the next call without serializing 62KB of JS).
        page.evaluate_function(format!(
            "() => {{ globalThis.__rustypipeInterpreter = {interp}; return true; }}",
            interp = serde_json::to_string(&interpreter_javascript).map_err(|e| {
                Error::Extraction(ExtractionError::Chromey(e.to_string().into()))
            })?,
        ))
        .await
        .map_err(|e| {
            Error::Extraction(ExtractionError::Chromey(
                format!("stash interpreter failed: {e}").into(),
            ))
        })?;
        // Also stash the clientExperimentsStateBlob so the
        // runner's `newMinter` (which calls the botguard
        // VM) can rebuild the `V9O` proto from the actual
        // experiments state the server sent, instead of an
        // empty default. Player.js reads this from the
        // `bgChallenge` proto (line 6000) and feeds it to
        // the VM via `TQ(this.U, 5)` (line 6006). Without
        // it the VM uses a default V9O and the resulting
        // tokens diverge from what the player would mint.
        let cesb_js = match client_experiments_state_blob.as_deref() {
            Some(s) => serde_json::Value::String(s.to_owned()),
            None => serde_json::Value::Null,
        };
        page.evaluate_function(format!(
            "() => {{ globalThis.__rustypipeClientExperimentsStateBlob = {cesb}; return true; }}",
            cesb = serde_json::to_string(&cesb_js).map_err(|e| {
                Error::Extraction(ExtractionError::Chromey(e.to_string().into()))
            })?,
        ))
        .await
        .map_err(|e| {
            Error::Extraction(ExtractionError::Chromey(
                format!("stash cesb failed: {e}").into(),
            ))
        })?;
        // Get the page's main execution context. We have to
        // pass it explicitly because `page.evaluate_function`
        // uses `callFunctionOn` and may not auto-pick the
        // context — without a context id, CDP returns
        // `-32602: Either objectId or executionContextId or
        // uniqueContextId must be specified`.
        let ctx_id = page
            .execution_context()
            .await
            .map_err(|e| {
                Error::Extraction(ExtractionError::Chromey(
                    format!("execution_context: {e}").into(),
                ))
            })?
            .ok_or_else(|| {
                Error::Extraction(ExtractionError::Chromey(
                    "no execution context".into(),
                ))
            })?;
        // `runSnapshot` reads `document.body` and the
        // clientExperimentsStateBlob from `globalThis`; it
        // does not need (and should not get) the
        // contentBinding / signedTimestamp — those are
        // supplied **at mint time** by the caller (player.js
        // does the same: snapshot is `Uu: {}`, real binding
        // is `OE(M,k)` → `dz(k.Uu)` in the minter). Baking
        // the binding into the snapshot/integrityToken
        // instead of the PoToken was the root cause of the
        // sps=3 loop we kept hitting on refreshes.
        let snapshot_expr = format!(
            r#"            globalThis.__rustypipeProgram = {program};
            globalThis.__rustypipeGlobalName = {global_name};
            await globalThis.loadInterpreter(
                globalThis.__rustypipeInterpreter,
                globalThis.__rustypipeGlobalName
            );
            return await globalThis.runSnapshot(
                globalThis.__rustypipeProgram,
                globalThis.__rustypipeGlobalName
            );"#,
            program = serde_json::to_string(&program).map_err(|e| {
                Error::Extraction(ExtractionError::Chromey(e.to_string().into()))
            })?,
            global_name = serde_json::to_string(&global_name).map_err(|e| {
                Error::Extraction(ExtractionError::Chromey(e.to_string().into())
            )})?,
        );
        // Wrap in an async IIFE since Runtime.evaluate can't
        // take top-level await.
        let snapshot_expr = format!(
            "(async () => {{ {} }})()",
            snapshot_expr
        );
        let evaluate = EvaluateParams::builder()
            .expression(snapshot_expr)
            .context_id(ctx_id)
            .await_promise(true)
            .return_by_value(true)
            .build()
            .map_err(|e| {
                Error::Extraction(ExtractionError::Chromey(
                    format!("EvaluateParams build: {e}").into(),
                ))
            })?;
        let resp = page.execute(evaluate).await.map_err(|e| {
            Error::Extraction(ExtractionError::Chromey(
                format!("runSnapshot failed: {e}").into(),
            ))
        })?;
        // The response wraps the JS return value in
        // `{"result": {"type": ..., "value": <value>}}`. For a
        // Promise with `await_promise=true`, the value is the
        // awaited result. We pass `return_by_value=true` so
        // the value is a `serde_json::Value` instead of a
        // remote object id.
        let resp_json: serde_json::Value =
            serde_json::to_value(&*resp).unwrap_or(serde_json::Value::Null);
        let snapshot_result = resp_json
            .get("result")
            .and_then(|r| r.get("value"))
            .cloned()
            .ok_or_else(|| {
                Error::Extraction(ExtractionError::Chromey(
                    format!("runSnapshot bad response: {resp_json}").into(),
                ))
            })?;
        let snapshot_arr = snapshot_result.as_array().ok_or_else(|| {
            Error::Extraction(ExtractionError::Chromey(
                "runSnapshot did not return an array".into(),
            ))
        })?;
        let bg_response = snapshot_arr
            .first()
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                Error::Extraction(ExtractionError::Chromey(
                    "runSnapshot[0] not a string".into(),
                ))
            })?
            .to_owned();

        // Abort the intercept task; we no longer need to
        // keep listening to Fetch.requestPaused events.
        // (No intercept task in this implementation — we
        // call Create directly via fetch().)

        // Step 2: send the botguard response to GenerateIT from
        // Rust. YouTube's server-side botguard validation rejects
        // cross-origin browser fetches that include
        // `sec-fetch-mode`, `sec-fetch-site`, `origin`,
        // `referer`, etc. The botguard binary avoids this by
        // sending the GenerateIT request from Rust via reqwest;
        // we do the same here.
        let payload = serde_json::json!([REQUEST_KEY, bg_response]);
        let resp = inner
            .http
            .post("https://www.youtube.com/api/jnn/v1/GenerateIT")
            .header("content-type", "application/json+protobuf")
            .header("x-goog-api-key", GOOG_API_KEY)
            .header("x-user-agent", X_USER_AGENT)
            .body(payload.to_string())
            .send()
            .await
            .map_err(|e| {
                Error::Extraction(ExtractionError::Chromey(e.to_string().into()))
            })?;
        if !resp.status().is_success() {
            return Err(Error::Extraction(ExtractionError::Chromey(
                format!("GenerateIT returned {}", resp.status()).into(),
            )));
        }
        let arr: serde_json::Value = resp.json().await.map_err(|e| {
            Error::Extraction(ExtractionError::Chromey(e.to_string().into()))
        })?;
        // PoIntegrityTokenResponse is a 4-tuple
        //   [integrityToken, estimatedTtlSecs, mintRefreshThreshold, websafeFallbackToken]
        // (LuanRT/BgUtils getPoIntegrityToken). We were
        // previously reading only the first two fields, which
        // is the bare minimum to mint a PoToken. Reading the
        // other two lets us:
        //  - log the refresh threshold GVS thinks the
        //    integrityToken was minted with (the player
        //    refreshes its minter at this boundary);
        //  - fall back to the server-provided
        //    `websafeFallbackToken` if the botguard VM's
        //    getMinter factory ever returns `PMD:Undefined` /
        //    `APF:Failed` — player.js's `xPy` does exactly
        //    that (`if (TQ(k, 4)) return new Xuj(...)`).
        let arr_ref = arr.as_array().ok_or_else(|| {
            Error::Extraction(ExtractionError::Chromey(
                "GenerateIT: response not a JSON array".into(),
            ))
        })?;
        let integrity_token = arr_ref
            .first()
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                Error::Extraction(ExtractionError::Chromey(
                    "GenerateIT: arr[0] not a string (integrityToken)".into(),
                ))
            })?
            .to_owned();
        let lifetime = arr_ref
            .get(1)
            .and_then(|v| v.as_u64())
            .unwrap_or(43_200);
        let mint_refresh_threshold = arr_ref.get(2).and_then(|v| v.as_u64());
        let websafe_fallback_token = arr_ref
            .get(3)
            .and_then(|v| v.as_str())
            .map(|s| s.to_owned());
        eprintln!(
            "[chromey debug] GenerateIT: ttl={}s refresh_threshold={}s websafe_fallback={}",
            lifetime,
            mint_refresh_threshold
                .map(|s| s.to_string())
                .unwrap_or_else(|| "<none>".into()),
            websafe_fallback_token
                .as_deref()
                .map(|s| format!("{} bytes", s.len()))
                .unwrap_or_else(|| "<none>".into()),
        );

        // Step 3: pass the integrityToken (and our
        // `webPoSignalOutput`) to the browser so it can build
        // a minter. The minter is cached on `globalThis` and
        // reused for all subsequent `mint` calls. We do this
        // as a single call that also performs the actual
        // mint, because `newMinter` and `mint` need to share
        // the same execution context (and therefore the
        // same `globalThis`).
        let ctx_id2 = page
            .execution_context()
            .await
            .map_err(|e| {
                Error::Extraction(ExtractionError::Chromey(
                    format!("execution_context (minter): {e}").into(),
                ))
            })?
            .ok_or_else(|| {
                Error::Extraction(ExtractionError::Chromey(
                    "no execution context (minter)".into(),
                ))
            })?;
        let idents_js: Vec<String> = idents
            .iter()
            .map(|i| {
                serde_json::to_string(i).map_err(|e| {
                    Error::Extraction(ExtractionError::Chromey(
                        e.to_string().into(),
                    ))
                })
            })
            .collect::<Result<_, _>>()?;
        // We deliberately do NOT overwrite
        // `globalThis.__rustypipeWebPoSignalOutput` here.
        // It was stashed by `runSnapshot` and contains a
        // live getMinter function reference. Reassigning
        // it from the JSON-serialised payload (which has
        // lost the function) is exactly the bug we just
        // fixed; newMinter reads the live reference from
        // globalThis, never the JSON.
        let combined_expr = format!(
            r#"globalThis.__rustypipeIntegrityToken = {tok};
            globalThis.__rustypipeIdents = {idents};
            await globalThis.newMinter(globalThis.__rustypipeIntegrityToken);
            const __tokens = [];
            for (const __ident of globalThis.__rustypipeIdents) {{
                __tokens.push(await globalThis.mint(__ident));
            }}
            return __tokens;"#,
            tok = serde_json::to_string(&integrity_token).map_err(|e| {
                Error::Extraction(ExtractionError::Chromey(e.to_string().into()))
            })?,
            idents = serde_json::to_string(&idents_js).map_err(|e| {
                Error::Extraction(ExtractionError::Chromey(e.to_string().into()))
            })?,
        );
        let combined_expr = format!("(async () => {{ {} }})()", combined_expr);
        let evaluate = EvaluateParams::builder()
            .expression(combined_expr)
            .context_id(ctx_id2)
            .await_promise(true)
            .return_by_value(true)
            .build()
            .map_err(|e| {
                Error::Extraction(ExtractionError::Chromey(
                    format!("EvaluateParams build: {e}").into(),
                ))
            })?;
        let resp = page.execute(evaluate).await.map_err(|e| {
            Error::Extraction(ExtractionError::Chromey(
                format!("newMinter+mint failed: {e}").into(),
            ))
        })?;
        let resp_json: serde_json::Value =
            serde_json::to_value(&*resp).unwrap_or(serde_json::Value::Null);
        let tokens_arr = resp_json
            .get("result")
            .and_then(|r| r.get("value"))
            .and_then(|v| v.as_array())
            .ok_or_else(|| {
                Error::Extraction(ExtractionError::Chromey(format!(
                    "newMinter+mint bad response: {resp_json}"
                ).into()))
            })?;
        let mut tokens = Vec::with_capacity(tokens_arr.len());
        for t in tokens_arr {
            let s = t.as_str().ok_or_else(|| {
                Error::Extraction(ExtractionError::Chromey(
                    "token not a string".into(),
                ))
            })?;
            tokens.push(s.to_owned());
        }
        if let Some(first) = tokens.first() {
            use data_encoding::Encoding as _;
            if let Ok(decoded) = data_encoding::BASE64URL.decode(first.as_bytes()) {
                let mut i = 0;
                let mut field6_len: Option<usize> = None;
                while i < decoded.len() {
                    let tag = decoded[i];
                    let field = tag >> 3;
                    let wire = tag & 0x07;
                    i += 1;
                    if wire == 2 {
                        let mut l = 0usize;
                        let mut shift = 0u32;
                        loop {
                            if i >= decoded.len() { break; }
                            let b = decoded[i]; i += 1;
                            l |= ((b & 0x7f) as usize) << shift;
                            if (b & 0x80) == 0 { break; }
                            shift += 7;
                        }
                        if field == 6 { field6_len = Some(l); break; }
                        i += l;
                    } else if wire == 0 {
                        while i < decoded.len() && (decoded[i] & 0x80) != 0 { i += 1; }
                        i += 1;
                    } else {
                        break;
                    }
                }
                eprintln!(
                    "[chromey debug] minted {} tokens (raw_total={} bytes, field6_len={:?}, b64[:40]={}...)",
                    tokens.len(),
                    decoded.len(),
                    field6_len,
                    &first[..first.len().min(40)]
                );
                eprintln!(
                    "[chromey debug] full hex: {}",
                    decoded.iter().map(|b| format!("{:02x}", b)).collect::<Vec<_>>().join("")
                );
            } else {
                eprintln!(
                    "[chromey debug] minted {} tokens (b64[:40]={}...)",
                    tokens.len(),
                    &first[..first.len().min(40)]
                );
            }
        } else {
            eprintln!("[chromey debug] minted 0 tokens");
        }

        let valid_until = OffsetDateTime::now_utc()
            + Duration::from_secs(lifetime.clamp(60, MAX_TOKEN_LIFETIME.as_secs()) as u64);

        debug!(
            "chromey: botguard initialised in {}ms; minter valid for {}s",
            start.elapsed().as_millis(),
            lifetime
        );

        *state = ProviderState::Init {
            _browser: Some(browser),
            _handler_task: Some(handler_task),
            page: Some(page),
            _user_data_dir: Some(user_data_dir),
            cached_tokens: tokens,
            valid_until,
            binding: bound_binding,
            video_id: bound_video_id,
        };
        let tokens = match &*state {
            ProviderState::Init { cached_tokens, .. } => cached_tokens.clone(),
            _ => unreachable!(),
        };

        // Keep the browser and page alive in the state for
        // reuse on subsequent mint calls. Reusing the page
        // (and therefore the same botguard VM and minter
        // installed on `globalThis`) is what lets the
        // attestation refresh in a SABR download be signed by
        // the same VM instance as the original content token
        // from the player request — which is what GVS expects.
        Ok((tokens, valid_until))
    }

    /// Mint PoTokens using the minter already installed on the
    /// page's `globalThis` by a previous `init()` call.
    ///
    /// The minter (`__rustypipeMint`) was built once during init
    /// by passing the integrityToken from `api/jnn/v1/GenerateIT`
    /// to `webPoSignalOutput[0]`. Every PoToken it produces from
    /// then on is bound to that same integrityToken — and
    /// therefore to the same BotGuard VM instance that minted
    /// the original content token for the player request. This
    /// is what GVS expects when a SABR request's
    /// `StreamProtectionStatus` returns `attestation required`:
    /// the refreshed token must come from the same VM.
    async fn mint_with_existing_minter(
        page: &Page,
        idents: &[&str],
    ) -> Result<Vec<String>, Error> {
        let ctx_id = page
            .execution_context()
            .await
            .map_err(|e| {
                Error::Extraction(ExtractionError::Chromey(
                    format!("execution_context (existing minter): {e}").into(),
                ))
            })?
            .ok_or_else(|| {
                Error::Extraction(ExtractionError::Chromey(
                    "no execution context (existing minter)".into(),
                ))
            })?;
        let idents_js: Vec<String> = idents
            .iter()
            .map(|i| {
                serde_json::to_string(i).map_err(|e| {
                    Error::Extraction(ExtractionError::Chromey(e.to_string().into()))
                })
            })
            .collect::<Result<_, _>>()?;
        let combined_expr = format!(
            r#"globalThis.__rustypipeIdents = {idents};
            const __tokens = [];
            for (const __ident of globalThis.__rustypipeIdents) {{
                __tokens.push(await globalThis.__rustypipeMint(__ident));
            }}
            return __tokens;"#,
            idents = serde_json::to_string(&idents_js).map_err(|e| {
                Error::Extraction(ExtractionError::Chromey(e.to_string().into()))
            })?,
        );
        let combined_expr = format!("(async () => {{ {} }})()", combined_expr);
        let evaluate = EvaluateParams::builder()
            .expression(combined_expr)
            .context_id(ctx_id)
            .await_promise(true)
            .return_by_value(true)
            .build()
            .map_err(|e| {
                Error::Extraction(ExtractionError::Chromey(
                    format!("existing-minter EvaluateParams build: {e}").into(),
                ))
            })?;
        let resp = page.execute(evaluate).await.map_err(|e| {
            Error::Extraction(ExtractionError::Chromey(
                format!("existing minter mint failed: {e}").into(),
            ))
        })?;
        let resp_json: serde_json::Value =
            serde_json::to_value(&*resp).unwrap_or(serde_json::Value::Null);
        let tokens_arr = resp_json
            .get("result")
            .and_then(|r| r.get("value"))
            .and_then(|v| v.as_array())
            .ok_or_else(|| {
                Error::Extraction(ExtractionError::Chromey(format!(
                    "existing minter bad response: {resp_json}"
                ).into()))
            })?;
        let mut tokens = Vec::with_capacity(tokens_arr.len());
        for t in tokens_arr {
            let s = t.as_str().ok_or_else(|| {
                Error::Extraction(ExtractionError::Chromey(
                    "existing minter token not a string".into(),
                ))
            })?;
            tokens.push(s.to_owned());
        }
        // Decode the first token to inspect its size and the size of
        // its field-6 payload. This is the diagnostic that proves
        // whether the minter is producing an outer-wrapped protobuf
        // (~110-128 bytes with a 80-100 byte field 6) or just an
        // inner token (~85-90 bytes).
        if let Some(first) = tokens.first() {
            use data_encoding::Encoding as _;
            if let Ok(decoded) = data_encoding::BASE64URL.decode(first.as_bytes()) {
                let mut i = 0;
                let mut field6_len: Option<usize> = None;
                while i < decoded.len() {
                    let tag = decoded[i];
                    let field = tag >> 3;
                    let wire = tag & 0x07;
                    i += 1;
                    if wire == 2 {
                        let mut l = 0usize;
                        let mut shift = 0u32;
                        loop {
                            if i >= decoded.len() { break; }
                            let b = decoded[i]; i += 1;
                            l |= ((b & 0x7f) as usize) << shift;
                            if (b & 0x80) == 0 { break; }
                            shift += 7;
                        }
                        if field == 6 { field6_len = Some(l); break; }
                        i += l;
                    } else if wire == 0 {
                        while i < decoded.len() && (decoded[i] & 0x80) != 0 { i += 1; }
                        i += 1;
                    } else {
                        break;
                    }
                }
                eprintln!(
                    "[chromey debug] minted {} tokens (raw_total={} bytes, field6_len={:?}, b64[:40]={}...)",
                    tokens.len(),
                    decoded.len(),
                    field6_len,
                    &first[..first.len().min(40)]
                );
                eprintln!(
                    "[chromey debug] full hex: {}",
                    decoded.iter().map(|b| format!("{:02x}", b)).collect::<Vec<_>>().join("")
                );
            } else {
                eprintln!(
                    "[chromey debug] minted {} tokens (b64[:40]={}...)",
                    tokens.len(),
                    &first[..first.len().min(40)]
                );
            }
        } else {
            eprintln!("[chromey debug] minted 0 tokens");
        }
        Ok(tokens)
    }

    /// Let the watch page actually start playing, then capture
    /// the first `googlevideo.com/videoplayback?…&sabr=1` POST
    /// the browser sends, extract its
    /// `StreamerContext.po_token` (the same PoToken the browser
    /// just used to talk to GVS successfully), and write the
    /// raw bytes to `path`.
    ///
    /// This is a diagnostic mode — see
    /// `Self::with_intercept_file` for the full rationale. It
    /// doesn't affect the rest of the init flow: even if the
    /// capture fails, we continue and the chromey PoToken we
    /// mint ourselves is still produced. Failures here only
    /// mean "no PoToken written to `path`", which the downloader
    /// treats as "no override available, use the minted token".
    async fn capture_browser_potoken(page: Page, path: &std::path::Path) {
        const INTERCEPT_TIMEOUT: Duration = Duration::from_secs(45);
        eprintln!(
            "[chromey intercept] starting; will write PoToken to {} on first SABR POST",
            path.display()
        );

        // Trigger playback. The autoplay policy flag we passed
        // to Chrome should let the page start on its own, but
        // we also click the play button via JS in case the
        // flag isn't enough (e.g. the page's own autoplay
        // check requires the gesture). Doing both is
        // idempotent — once the video is playing, the calls
        // are no-ops.
        //
        // YouTube's watch page also gates on its own internal
        // `playabilityStatus`. In headless Chrome the click()
        // call sometimes doesn't fully trigger the player
        // state machine (it returns a resolved Promise but
        // never actually starts buffering). We work around
        // that by waiting for `video.currentTime > 0` (a
        // signal that the player has at least one decoded
        // frame) OR for the `playing` event before
        // considering playback "started". If after 8s none
        // of those happen, we still proceed and let the
        // SABR listener below decide — it will simply time
        // out.
        let play_expr = r#"(async () => {
            // 1) Click the visible play button overlay (the
            //    `<button class="ytp-large-play-button">`).
            const btn = document.querySelector(
                ".ytp-large-play-button, .ytp-play-button"
            );
            if (btn) { try { btn.click(); } catch (e) {} }
            // 2) Programmatic play() on the <video> element.
            const v = document.querySelector("video");
            const v0 = v ? { currentTime: v.currentTime, paused: v.paused } : null;
            if (v) {
                try { await v.play(); } catch (e) {}
            }
            // 3) Wait up to 8s for actual playback to start
            //    (currentTime > 0 OR the 'playing' event
            //    fires OR the 'loadstart' event fires on
            //    the <video>).
            let started = false;
            let lastEvent = null;
            if (v) {
                const deadline = Date.now() + 8000;
                while (Date.now() < deadline && !started) {
                    if (v.currentTime > 0) { lastEvent = "currentTime>0"; started = true; break; }
                    if (v.readyState >= 3) { lastEvent = "readyState>=3"; /* HAVE_FUTURE_DATA */ }
                    await new Promise((r) => setTimeout(r, 100));
                }
            }
            return {
                playBtn: !!btn,
                hasVideo: !!v,
                paused: v ? v.paused : null,
                readyState: v ? v.readyState : null,
                currentTime: v ? v.currentTime : null,
                v0,
                lastEvent,
            };
        })()"#;
        let play_eval = match EvaluateParams::builder()
            .expression(play_expr.to_string())
            .await_promise(true)
            .return_by_value(true)
            .build()
        {
            Ok(p) => p,
            Err(e) => {
                eprintln!("[chromey intercept] play EvalParams build failed: {e}");
                return;
            }
        };
        match page.execute(play_eval).await {
            Ok(r) => {
                let v: serde_json::Value =
                    serde_json::to_value(&*r).unwrap_or(serde_json::Value::Null);
                let inner = v
                    .get("result")
                    .and_then(|r| r.get("value"))
                    .cloned()
                    .unwrap_or(serde_json::Value::Null);
                eprintln!("[chromey intercept] play() result = {}", inner);
            }
            Err(e) => {
                eprintln!("[chromey intercept] play() eval failed: {e}");
            }
        }

        // Open an `EventRequestWillBeSent` stream and watch
        // for the first SABR POST.
        let mut events = match page.event_listener::<EventRequestWillBeSent>().await {
            Ok(s) => s,
            Err(e) => {
                eprintln!("[chromey intercept] could not open event listener: {e}");
                return;
            }
        };

        let deadline = Instant::now() + INTERCEPT_TIMEOUT;
        loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                eprintln!(
                    "[chromey intercept] timed out after {}s with no SABR POST",
                    INTERCEPT_TIMEOUT.as_secs()
                );
                return;
            }
            let next = match tokio::time::timeout(remaining, events.next()).await {
                Ok(Some(ev)) => ev,
                Ok(None) => {
                    eprintln!("[chromey intercept] event stream closed");
                    return;
                }
                Err(_) => {
                    eprintln!(
                        "[chromey intercept] timed out after {}s with no SABR POST",
                        INTERCEPT_TIMEOUT.as_secs()
                    );
                    return;
                }
            };

            let is_sabr = next
                .request
                .url
                .contains("googlevideo.com/videoplayback")
                && next.request.url.contains("sabr=1");
            let is_post = next
                .request
                .method
                .eq_ignore_ascii_case("POST");
            if !is_sabr || !is_post {
                continue;
            }
            if !next.request.has_post_data.unwrap_or(false) {
                eprintln!(
                    "[chromey intercept] SABR POST without post_data flag: {}",
                    next.request.url
                );
                continue;
            }
            eprintln!(
                "[chromey intercept] first SABR POST seen, request_id={:?}",
                next.request_id
            );
            // Dump the URL too — the player may put the PoToken
            // in the URL as `?pot=...` rather than in the
            // body. This is the critical diagnostic for
            // "why is GVS rejecting our PoToken?".
            eprintln!(
                "[chromey intercept] SABR URL = {}",
                &next.request.url
            );
            // Check for `pot=` in the URL
            if let Some(pot_idx) = next.request.url.find("pot=") {
                let pot_start = pot_idx + 4;
                let pot_end = next.request.url[pot_start..]
                    .find('&')
                    .map(|e| pot_start + e)
                    .unwrap_or(next.request.url.len());
                eprintln!(
                    "[chromey intercept] URL contains pot=<...> ({}) bytes at offset {}..{}",
                    pot_end - pot_start,
                    pot_start,
                    pot_end
                );
            } else {
                eprintln!("[chromey intercept] URL does NOT contain 'pot=' query param");
            }

            // The event doesn't include the body; pull it
            // via `Network.getRequestPostData`.
            let get_params = match GetRequestPostDataParams::builder()
                .request_id(next.request_id.clone())
                .build()
            {
                Ok(p) => p,
                Err(e) => {
                    eprintln!("[chromey intercept] getRequestPostData build failed: {e}");
                    continue;
                }
            };
            let resp = match page.execute(get_params).await {
                Ok(r) => r,
                Err(e) => {
                    eprintln!("[chromey intercept] getRequestPostData exec failed: {e}");
                    continue;
                }
            };
            let body_b64 = resp.result.post_data.clone();
            if body_b64.is_empty() {
                eprintln!("[chromey intercept] getRequestPostData returned empty body");
                continue;
            }
            use base64::Engine;
            let body = if resp.result.base64_encoded {
                match base64::engine::general_purpose::STANDARD.decode(&body_b64) {
                    Ok(b) => b,
                    Err(e) => {
                        eprintln!("[chromey intercept] base64 decode failed: {e}");
                        continue;
                    }
                }
            } else {
                body_b64.into_bytes()
            };
            eprintln!(
                "[chromey intercept] SABR body = {} bytes; extracting PoToken",
                body.len()
            );

            // Always dump the raw body to a side file for
            // inspection. The browser's actual SABR body is
            // the ground truth — the extracted PoToken alone
            // isn't enough to know whether the wire format
            // matches what `rustypipe` sends.
            let side = path.with_extension("sabr_body.bin");
            if let Err(e) = std::fs::write(&side, &body) {
                eprintln!("[chromey intercept] write {} failed: {e}", side.display());
            } else {
                eprintln!("[chromey intercept] raw body saved to {}", side.display());
            }
            // Also dump the URL for offline diffing.
            let url_side = path.with_extension("sabr_url.txt");
            if let Err(e) = std::fs::write(&url_side, next.request.url.as_bytes()) {
                eprintln!("[chromey intercept] write {} failed: {e}", url_side.display());
            } else {
                eprintln!("[chromey intercept] full URL saved to {}", url_side.display());
            }

            let token = match extract_sabr_potoken(&body) {
                Some(t) => t,
                None => {
                    eprintln!("[chromey intercept] could not extract PoToken from SABR body");
                    // Still write the body to a side file for
                    // manual inspection — knowing the SABR
                    // body's shape is half the diagnostic.
                    let side = path.with_extension("sabr_body.bin");
                    let _ = std::fs::write(&side, &body);
                    eprintln!(
                        "[chromey intercept] (raw body saved to {} for inspection)",
                        side.display()
                    );
                    return;
                }
            };
            eprintln!(
                "[chromey intercept] PoToken = {} bytes (raw, pre-base64)",
                token.len()
            );
            // Write the raw bytes; the downloader's
            // `RUSTYPIPE_SABR_PO_TOKEN_FILE` override reads
            // raw bytes (NOT base64).
            if let Err(e) = std::fs::write(path, &token) {
                eprintln!("[chromey intercept] write {} failed: {e}", path.display());
                return;
            }
            use data_encoding::Encoding;
            let b64 = data_encoding::BASE64URL.encode(&token);
            eprintln!(
                "[chromey intercept] wrote PoToken to {} ({} bytes, b64[:60]={}...)",
                path.display(),
                token.len(),
                &b64[..b64.len().min(60)]
            );
            return;
        }
    }
}

/// Extract `StreamerContext.po_token` (field 19, then field 2)
/// from a SABR `videoplayback` POST body. Returns the raw PoToken
/// bytes (the same bytes that should be put in
/// `StreamerContext.po_token` on a rustypipe SABR request).
fn extract_sabr_potoken(body: &[u8]) -> Option<Vec<u8>> {
    // Parse a base-128 varint starting at `pos`. Returns
    // `(value, byte_offset_past_value)`.
    fn read_varint(data: &[u8], pos: usize) -> Option<(u64, usize)> {
        let mut result: u64 = 0;
        let mut shift = 0u32;
        let mut p = pos;
        while p < data.len() {
            let b = data[p];
            result |= ((b & 0x7f) as u64) << shift;
            p += 1;
            if (b & 0x80) == 0 {
                return Some((result, p));
            }
            shift += 7;
            if shift > 63 {
                return None;
            }
        }
        None
    }

    // Walk the top-level `VideoPlaybackAbrRequest`. We only
    // care about field 19 (the `StreamerContext` submessage);
    // everything else is skipped.
    let mut pos = 0;
    while pos < body.len() {
        let (tag, after_tag) = read_varint(body, pos)?;
        let field = tag >> 3;
        let wire = tag & 0x7;
        match wire {
            // varint
            0 => {
                let (_v, after) = read_varint(body, after_tag)?;
                pos = after;
            }
            // 64-bit
            1 => {
                pos = after_tag + 8;
            }
            // length-delimited (sub-messages, bytes, strings)
            2 => {
                let (l, content_start) = read_varint(body, after_tag)?;
                let content_end = content_start.saturating_add(l as usize);
                if content_end > body.len() {
                    return None;
                }
                if field == 19 {
                    let sub = &body[content_start..content_end];
                    // Walk `StreamerContext` to find field 2
                    // (the `po_token` bytes).
                    let mut sp = 0;
                    while sp < sub.len() {
                        let (stag, safter) = read_varint(sub, sp)?;
                        let sfield = stag >> 3;
                        let swire = stag & 0x7;
                        match swire {
                            0 => {
                                let (_v, after) = read_varint(sub, safter)?;
                                sp = after;
                            }
                            1 => {
                                sp = safter + 8;
                            }
                            2 => {
                                let (sl, sstart) = read_varint(sub, safter)?;
                                let send = sstart.saturating_add(sl as usize);
                                if send > sub.len() {
                                    return None;
                                }
                                if sfield == 2 {
                                    return Some(sub[sstart..send].to_vec());
                                }
                                sp = send;
                            }
                            _ => return None,
                        }
                    }
                    return None; // field 19 found but no field 2 inside
                }
                pos = content_end;
            }
            // 32-bit (deprecated, but legal)
            5 => {
                pos = after_tag + 4;
            }
            _ => return None,
        }
    }
    None
}

impl Drop for ProviderState {
    fn drop(&mut self) {
        if let ProviderState::Init { _user_data_dir, .. } = self {
            if let Some(dir) = _user_data_dir.clone() {
                // Best-effort async cleanup; the dir is also
                // recreated on each init() so a leak here is
                // harmless.
                tokio::task::spawn(async move {
                    let _ = tokio::fs::remove_dir_all(dir).await;
                });
            }
        }
    }
}

fn detect_chrome() -> Result<PathBuf, String> {
    use chromiumoxide::detection::{default_executable, DetectionOptions};
    default_executable(DetectionOptions {
        msedge: false,
        unstable: false,
    })
    .map_err(|e| e.to_string())
}

fn tempdir() -> PathBuf {
    let mut p = std::env::temp_dir();
    // Unique per process invocation AND per init call (a
    // timestamp+random suffix). Chrome writes a
    // `SingletonLock` into the user_data_dir and refuses to
    // launch if one already exists, so we must never reuse
    // a dir from a previous, possibly crashed, run.
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let name = format!(
        "rustypipe-chromey-{}-{}",
        std::process::id(),
        now
    );
    p.push(name);
    let _ = std::fs::remove_dir_all(&p);
    let _ = std::fs::create_dir_all(&p);
    p
}

/// JS that takes the integrityToken (from `jnn/v1/GenerateIT`)
/// and the `webPoSignalOutput` produced by the page's
/// `runSnapshot` and builds a minter, stored on
/// `globalThis.__rustypipeMint`. Returns 43200 (a placeholder; the
/// real lifetime is already known server-side).
const JS_NEW_MINTER_FN: &str = r#"
async function __rustypipeNewMinter(integrityToken, webPoSignalOutput) {
    if (typeof integrityToken !== "string" || integrityToken.length === 0) {
        throw new Error("integrityToken missing");
    }
    if (!Array.isArray(webPoSignalOutput)) {
        throw new Error("webPoSignalOutput missing/not an array");
    }
    await globalThis.newMinter(integrityToken, webPoSignalOutput);
    return 43200;
}
"#;
