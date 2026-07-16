// SABR body capture tool.
//
// This is a debugging tool: it launches a real Chrome via the chromey
// CDP crate, navigates to a YouTube watch page, lets the page actually
// start playback, and captures every SABR `videoplayback` POST body
// the browser sends. The captured bodies are written to /tmp so we
// can compare them against what rustypipe's SABR client emits and find
// the field that's wrong.
//
// Usage:
//   cargo run -p rustypipe-cli --bin sabr_capture --features chromey-po-token -- \
//     --video-id bnhV-OBnGCE --output-dir /tmp/sabr_capture
//
// Requires a Chrome/Chromium binary (auto-detected, or pass
// --chrome-executable /path/to/chrome).
//
// Optional flags:
//   --headful  : launch Chrome in headful mode (requires Xvfb on
//                headless servers). Matches rustypipe's --chromey-headful
//                behaviour so the captured body has the same fingerprint
//                the rustypipe client produces.
//   --max-secs N : stop capturing after N seconds (default 30).

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use chromiumoxide::browser::{Browser, BrowserConfig};
use chromiumoxide::cdp::browser_protocol::network::{
    EnableParams as NetworkEnableParams, EventRequestWillBeSent,
};
use chromiumoxide::cdp::browser_protocol::page::EnableParams as PageEnableParams;
use chromiumoxide::cdp::js_protocol::runtime::EnableParams as RuntimeEnableParams;
use chromiumoxide::page::Page;
use futures_util::stream::StreamExt;

const DEFAULT_UA: &str =
    "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/143.0.0.0 Safari/537.36";

#[derive(Debug)]
struct Args {
    video_id: String,
    output_dir: PathBuf,
    chrome_executable: Option<PathBuf>,
    headful: bool,
    max_secs: u64,
}

fn parse_args() -> Result<Args, String> {
    let mut video_id: Option<String> = None;
    let mut output_dir = PathBuf::from("/tmp/sabr_capture");
    let mut chrome_executable: Option<PathBuf> = None;
    let mut headful = false;
    let mut max_secs: u64 = 30;

    let mut it = std::env::args().skip(1);
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--video-id" => {
                video_id = it.next();
            }
            "--output-dir" => {
                output_dir = it.next().map(PathBuf::from).unwrap_or(output_dir);
            }
            "--chrome-executable" => {
                chrome_executable = it.next().map(PathBuf::from);
            }
            "--headful" => {
                headful = true;
            }
            "--max-secs" => {
                max_secs = it
                    .next()
                    .ok_or_else(|| "--max-secs requires a number".to_string())?
                    .parse()
                    .map_err(|e: std::num::ParseIntError| e.to_string())?;
            }
            other => return Err(format!("unknown arg: {other}")),
        }
    }
    let video_id = video_id.ok_or_else(|| "--video-id is required".to_string())?;
    Ok(Args {
        video_id,
        output_dir,
        chrome_executable,
        headful,
        max_secs,
    })
}

fn detect_chrome() -> Result<PathBuf, String> {
    use chromiumoxide::detection::{default_executable, DetectionOptions};
    default_executable(DetectionOptions {
        msedge: false,
        unstable: false,
    })
    .map_err(|e| e.to_string())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let args = parse_args().map_err(|e| format!("bad args: {e}"))?;
    eprintln!("sabr_capture: {:?}", args);

    std::fs::create_dir_all(&args.output_dir)?;
    let chrome_path = match args.chrome_executable.clone() {
        Some(p) => p,
        None => detect_chrome()?,
    };
    eprintln!("sabr_capture: using chrome at {}", chrome_path.display());

    let user_data_dir = std::env::temp_dir().join(format!(
        "sabr-capture-chrome-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&user_data_dir)?;

    let mut builder = BrowserConfig::builder()
        .chrome_executable(&chrome_path)
        .user_data_dir(&user_data_dir)
        .request_timeout(Duration::from_secs(30))
        .launch_timeout(Duration::from_secs(20))
        .no_sandbox();

    if args.headful {
        eprintln!(
            "sabr_capture: launching Chrome in headful mode (DISPLAY={})",
            std::env::var("DISPLAY").unwrap_or_else(|_| "<unset>".into())
        );
        builder = builder.with_head().window_size(1920, 1080);
    } else {
        eprintln!("sabr_capture: launching Chrome in --headless=new mode");
        builder = builder.new_headless_mode();
    }

    let config = builder
        .arg("--disable-blink-features=AutomationControlled")
        .arg(format!("--user-agent={DEFAULT_UA}"))
        .build()
        .map_err(|e| format!("BrowserConfig build: {e}"))?;

    let (browser, mut handler) =
        Browser::launch(config).await.map_err(|e| format!("Browser::launch: {e}"))?;
    let _handler_task = tokio::task::spawn(async move {
        while let Some(h) = handler.next().await {
            if h.is_err() {
                break;
            }
        }
    });

    let page: Arc<Page> = Arc::new(
        browser
            .new_page("about:blank")
            .await
            .map_err(|e| format!("new_page: {e}"))?,
    );

    // Enable the Network and Page CDP domains so we can listen for
    // requestWillBeSent. We need Page as well because the watch
    // page's JS does setTimeout / setInterval and the navigation
    // needs to be allowed to complete.
    page.execute(NetworkEnableParams::default())
        .await
        .map_err(|e| format!("Network.enable: {e}"))?;
    page.execute(chromiumoxide::cdp::browser_protocol::page::EnableParams::default())
        .await
        .map_err(|e| format!("Page.enable: {e}"))?;
    page.execute(chromiumoxide::cdp::js_protocol::runtime::EnableParams::default())
        .await
        .map_err(|e| format!("Runtime.enable: {e}"))?;

    let mut request_events = page.event_listener::<EventRequestWillBeSent>().await?;
    let output_dir = args.output_dir.clone();
    let capture_page = page.clone();
    let capture_handle = tokio::task::spawn(async move {
        let mut count = 0usize;
        let mut all_count = 0usize;
        while let Some(event) = request_events.next().await {
            all_count += 1;
            if all_count <= 10 {
                eprintln!(
                    "sabr_capture: req[{}] method={} url={}",
                    all_count,
                    event.request.method,
                    &event.request.url[..event.request.url.len().min(150)],
                );
            }
            // We only want SABR (videoplayback) POSTs with bodies.
            let is_sabr = event
                .request
                .url
                .contains("googlevideo.com/videoplayback");
            let is_post = event.request.method.eq_ignore_ascii_case("POST");
            if !is_sabr || !is_post {
                continue;
            }
            // The browser's SABR URL has `&sabr=1&rqh=1` and is a
            // POST with a binary body. It does NOT have `srfvp=1`
            // (that's a LuanRT/googlevideo extension for their
            // virtual-URL handling). Filter on the combination of
            // method=POST, host=videoplayback, and `sabr=1` in the
            // query.
            let is_sabr_stream = event
                .request
                .url
                .contains("/videoplayback")
                && event.request.url.contains("sabr=1");
            if !is_sabr_stream {
                eprintln!(
                    "sabr_capture: skipped non-sabr videoplayback POST: {}",
                    event.request.url
                );
                continue;
            }
            let has_post = event.request.has_post_data.unwrap_or(false);
            if !has_post {
                eprintln!(
                    "sabr_capture: skipped SABR POST without body: {}",
                    event.request.url
                );
                continue;
            }
            // Pull the actual body bytes via Network.getRequestPostData
            // (the event itself doesn't include the body to keep
            // traffic low — only the has_post_data flag).
            let get_params = chromiumoxide::cdp::browser_protocol::network::GetRequestPostDataParams::builder()
                .request_id(event.request_id.clone())
                .build()
                .map_err(|e| format!("getRequestPostData build: {e}"));
            let get_params = match get_params {
                Ok(p) => p,
                Err(e) => {
                    eprintln!("sabr_capture: {}", e);
                    continue;
                }
            };
            let resp = match capture_page.execute(get_params).await {
                Ok(r) => r,
                Err(e) => {
                    eprintln!(
                        "sabr_capture: getRequestPostData failed for {}: {}",
                        event.request.url, e
                    );
                    continue;
                }
            };
            let body_b64: String = resp.result.post_data;
            if body_b64.is_empty() {
                eprintln!(
                    "sabr_capture: getRequestPostData returned empty body for {}",
                    event.request.url
                );
                continue;
            }
            use base64::Engine;
            // `base64_encoded: true` means the post_data string is
            // base64-encoded (binary body); false means it's the
            // raw string body. SABR bodies are binary, so it's
            // almost always true.
            let body = if resp.result.base64_encoded {
                match base64::engine::general_purpose::STANDARD.decode(&body_b64) {
                    Ok(b) => b,
                    Err(e) => {
                        eprintln!("sabr_capture: b64 decode failed: {}", e);
                        continue;
                    }
                }
            } else {
                body_b64.into_bytes()
            };
            count += 1;
            let path = output_dir.join(format!("sabr_browser_{:04}.bin", count));
            if let Err(e) = std::fs::write(&path, &body) {
                eprintln!("sabr_capture: write {:?} failed: {}", path, e);
                continue;
            }
            // Also save the URL and request headers for comparison against rustypipe.
            let url_path = output_dir.join(format!("sabr_browser_{:04}.url", count));
            let _ = std::fs::write(&url_path, &event.request.url);
            // Dump the full event (debug) so we can see exactly what the browser sends.
            let dbg_path = output_dir.join(format!("sabr_browser_{:04}.debug", count));
            let _ = std::fs::write(&dbg_path, format!("{:#?}", event.request));
            eprintln!(
                "sabr_capture: [#{}] {} -> {} bytes body written to {} (url len={})",
                count,
                event.request.method,
                body.len(),
                path.display(),
                event.request.url.len(),
            );
        }
        eprintln!("sabr_capture: capture loop exited (page closed)");
    });

    // Navigate to the watch page and let it play. We don't interact
    // with the player — just let the page load and the player auto-
    // start (it does, after a short delay).
    let url = format!("https://www.youtube.com/watch?v={}", args.video_id);
    eprintln!("sabr_capture: navigating to {}", url);
    page.goto(&url).await.map_err(|e| format!("goto: {e}"))?;
    eprintln!("sabr_capture: page loaded, waiting for SABR requests...");

    tokio::time::sleep(Duration::from_secs(args.max_secs)).await;
    eprintln!("sabr_capture: timed out after {}s, dropping page", args.max_secs);
    drop(page);
    let _ = capture_handle.await;
    let _ = std::fs::remove_dir_all(&user_data_dir);
    eprintln!("sabr_capture: done");
    Ok(())
}
