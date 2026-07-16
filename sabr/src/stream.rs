//! SABR streaming client.
//!
//! Adapted from <https://github.com/FineFindus/sabr> (MIT licensed).

use std::collections::{HashMap, HashSet};

use prost::Message;
use wreq::header::HeaderValue;

use crate::{
    error::Error,
    proto::{
        misc::{AuthorizedFormat, FormatId, PlaybackAuthorization},
        video_streaming::{
            sabr_context_update::SabrContextWritePolicy,
            streamer_context::{ClientInfo, SabrContext},
            ClientAbrState, FormatInitializationMetadata, MediaHeader, NextRequestPolicy,
            SabrContextSendingPolicy, SabrContextUpdate, SabrError, SabrRedirect, SabrVisibilityHint,
            StreamProtectionStatus, StreamerContext, UmpPartId, VideoPlaybackAbrRequest,
        },
    },
    ump, Bytes,
};

const ENCODING: &str = "identity";
// The SABR request's User-Agent is supplied by the caller via
// `Stream::new`. YouTube correlates the UA on the SABR request
// with the one that fetched the player, so the SABR client must
// use the same UA the player-fetching HTTP client used. We keep
// a fallback here for backwards compatibility, but new code paths
// (e.g. chromey) pass their own UA.
const DEFAULT_USER_AGENT: &str =
    "Mozilla/5.0 (X11; Linux x86_64; rv:128.0) Gecko/20100101 Firefox/128.0";

/// A segment of a media stream.
#[derive(Debug)]
pub struct Segment {
    header: MediaHeader,
    sequence_number: i64,
    data: Vec<Bytes>,
    duration: i64,
}

impl Segment {
    /// Duration in milliseconds.
    pub fn duration(&self) -> i64 {
        self.duration
    }

    /// Raw media data chunks.
    pub fn data(&self) -> &[Bytes] {
        &self.data
    }

    /// Total length of the in-segment media data.
    pub fn len(&self) -> usize {
        self.data.iter().map(|b| b.len()).sum()
    }
}

#[derive(Debug)]
struct InitializedFormat {
    id: FormatId,
    downloaded_segments: HashMap<i64, Segment>,
    /// Highest segment sequence number that has already been
    /// drained by the downloader. The SABR server's
    /// attestation-required flow re-sends the same audio
    /// segments each time the client mints a fresh PoToken,
    /// and we don't want to write them to disk twice. We can't
    /// just check `downloaded_segments` because `drain_segments`
    /// removes the segment from that map. Tracking the max
    /// drained seq here means we can detect the re-send and
    /// skip it before the segment ever lands in
    /// `downloaded_segments`.
    drained_max_seq: i64,
    end_segment_number: i64,
    last_downloaded_segment: i64,
    duration: i64,
    downloaded_duration: i64,
}

impl InitializedFormat {
    fn drain_segments(&mut self) -> Vec<Segment> {
        let mut segments = Vec::new();
        for seq in self.last_downloaded_segment..=self.end_segment_number {
            let Some(segment) = self.downloaded_segments.remove(&seq) else {
                self.last_downloaded_segment = seq;
                break;
            };
            if seq > self.drained_max_seq {
                self.drained_max_seq = seq;
            }
            segments.push(segment);
        }
        segments
    }
}

/// A SABR stream that can be polled for media data.
#[derive(Debug)]
pub struct Stream<'a> {
    video_id: &'a str,
    url: String,
    initialized_formats: HashMap<i32, InitializedFormat>,
    partial_segments: HashMap<u32, Segment>,
    /// base64-decoded `ustreamerConfig` blob. Required for SABR; YouTube
    /// rejects requests without it (or returns `AttestationRequired`).
    ustreamer_config: Bytes,
    /// Content-bound PO token. Goes in `streamer_context.po_token` of the SABR
    /// request body. YouTube validates this against the player response.
    po_token: Option<Bytes>,
    /// Visitor data (content binding) used to construct the cold-start
    /// PoToken (`?pot=...`) sent on the first few SABR requests. Must
    /// match the identifier the eventual content-bound token is bound
    /// to, so the server's session-binding logic sees a consistent
    /// identifier stream.
    visitor_data: Option<String>,
    /// Client version reported in the `StreamerContext`. YouTube validates this
    /// against the version the player response was fetched with, so the SABR
    /// client **must** use the same version the WEB player used.
    client_version: String,
    client: wreq::Client,
    audio_format: FormatId,
    video_format: Option<FormatId>,
    /// All audio formats the player advertised in the `adaptiveFormats` list.
    /// Sent as `preferred_audio_format_ids` so the server can hand us a
    /// compatible stream when the chosen one becomes unavailable.
    preferred_audio_formats: Vec<FormatId>,
    /// All video formats the player advertised in the `adaptiveFormats` list.
    /// Sent as `preferred_video_format_ids` for the same reason.
    preferred_video_formats: Vec<FormatId>,
    playback_cookie: Option<Bytes>,
    backoff_time: Option<i32>,
    sabr_contexts: HashMap<i32, crate::proto::video_streaming::streamer_context::SabrContext>,
    active_sabr_contexts: HashSet<i32>,
    player_time: i64,
    /// Monotonically increasing request number, appended to the SABR URL as `rn`.
    /// YouTube uses this for de-duplication and stream continuity.
    request_number: i64,
    /// Client playback nonce. YouTube-generated 16-char hex string used to
    /// disambiguate concurrent playback sessions from the same client. Required
    /// in the SABR URL as `&cpn=`.
    cpn: String,
    /// Estimated wall-clock bandwidth in bytes per second. Reflects the user's
    /// network capacity; YouTube uses it for format selection.
    bandwidth_estimate: i64,
    /// Start time of the last seek action, in ms. YouTube uses this to
    /// disambiguate seek behaviour in the buffered ranges.
    time_since_last_seek_ms: i64,
    /// Wall-clock time since the last user action, in ms.
    time_since_last_action_ms: i64,
    /// Wall-clock time since playback was first started, in ms.
    elapsed_wall_time_ms: i64,
    /// Stream start instant. Used to compute `elapsed_wall_time_ms` /
    /// `time_since_last_action_ms` for the browser-style `ClientAbrState`.
    started_at: std::time::Instant,
}

impl<'a> Stream<'a> {
    /// Create a new [`Stream`].
    ///
    /// `ustreamer_config` must be the raw base64-decoded bytes of the
    /// `ustreamerConfig` field from the player response. YouTube rejects
    /// requests that omit it.
    ///
    /// `user_agent` is the User-Agent header sent with every SABR
    /// request. YouTube cross-checks this against the UA that
    /// fetched the player response, and against the environment
    /// the PoToken was minted in. Pass the same UA the player
    /// request used.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        video_id: &'a str,
        url: String,
        ustreamer_config: Vec<u8>,
        po_token: Option<Vec<u8>>,
        client_version: String,
        audio_format: FormatId,
        video_format: Option<FormatId>,
        preferred_audio_formats: Vec<FormatId>,
        preferred_video_formats: Vec<FormatId>,
        user_agent: &str,
    ) -> Self {
        // The real browser sends a UA + Accept-Language only, with no
        // Content-Type, Accept, or Accept-Encoding header. Replicating the
        // browser's minimal header set avoids YouTube flagging our request as
        // a non-browser client.
        //
        // YouTube also correlates the SABR request's cookie consent state
        // (`SOCS=CAISAiAD`) with the player request. We add it here so the
        // server doesn't reject us as a non-consented client.
        // TLS impersonation: GVS fingerprints the TLS handshake (JA3/JA4)
        // and rejects clients whose fingerprint doesn't match a real
        // browser. We impersonate Chrome 133 on Linux, which mirrors the
        // cipher suite order, ALPN, extension list, and HTTP/2 SETTINGS
        // frame a real browser would emit. Without this, GVS rejects the
        // request with 403 Forbidden before it ever reaches the SABR
        // body validation.
        let client = wreq::Client::builder()
            .emulation(wreq_util::Emulation::Chrome133)
            .default_headers({
                let mut h = wreq::header::HeaderMap::new();
                h.insert(
                    wreq::header::USER_AGENT,
                    HeaderValue::from_str(user_agent)
                        .unwrap_or_else(|_| HeaderValue::from_static(DEFAULT_USER_AGENT)),
                );
                h.insert(
                    wreq::header::ACCEPT_LANGUAGE,
                    HeaderValue::from_static("en-US"),
                );
                // Browser sends Referer: https://www.youtube.com/ (no
                // video id) on SABR POSTs. The earlier code matched the
                // watch page URL, but the actual browser just sends the
                // bare host. Some GVS heuristics reject requests with
                // extra referer context, so we mirror the browser here.
                h.insert(
                    wreq::header::REFERER,
                    HeaderValue::from_static("https://www.youtube.com/"),
                );
                // The browser does NOT send X-YouTube-Client-Name or
                // X-YouTube-Client-Version on the SABR POST itself. It
                // identifies the client via the `c=WEB` and `cver=` URL
                // query params (which are part of the signed URL). The
                // earlier code added `X-YouTube-Client-Name: 56`
                // (WEB_EMBEDDED_PLAYER) thinking it was required, but
                // that's only used by innerTube requests, not SABR.
                //
                // The browser also sends no `Origin` header on the SABR
                // POST (cross-origin from a googlevideo.com frame in
                // a youtube.com page, the server doesn't require an
                // explicit Origin).
                h.insert(
                    wreq::header::COOKIE,
                    HeaderValue::from_static("SOCS=CAISAiAD"),
                );
                h
            })
            .build()
            .expect("sabr http client");

        // Generate a 16-hex-char client playback nonce. The browser uses a
        // cryptographically random value here, but a per-stream random ID
        // works fine for our purposes.
        let cpn = {
            use std::fmt::Write;
            let mut s = String::with_capacity(16);
            let bytes: [u8; 8] = rand_bytes();
            for b in bytes {
                let _ = write!(s, "{:02x}", b);
            }
            s
        };

        Self {
            video_id,
            url,
            ustreamer_config: Bytes::from(ustreamer_config),
            po_token: po_token.map(Bytes::from),
            visitor_data: None,
            client_version,
            initialized_formats: HashMap::new(),
            partial_segments: HashMap::new(),
            client,
            audio_format,
            video_format,
            preferred_audio_formats,
            preferred_video_formats,
            playback_cookie: None,
            sabr_contexts: HashMap::new(),
            active_sabr_contexts: HashSet::new(),
            backoff_time: None,
            player_time: 0,
            request_number: 0,
            cpn,
            // The browser reports real bandwidth from `navigator.connection`;
            // GVS rejects requests that report a tiny number. Use a realistic
            // initial value (browsers typically report 500-1000 kbps when
            // they don't know better) and let the server adjust later.
            bandwidth_estimate: 655_360,
            time_since_last_seek_ms: 0,
            time_since_last_action_ms: 0,
            elapsed_wall_time_ms: 0,
            // Wall-clock start for the request-handling timeline. Browser
            // uses `performance.now()`-style values; we use a std::time::Instant
            // and convert to ms in `fetch_stream_data`.
            started_at: std::time::Instant::now(),
        }
    }
    /// Update the PO token.
    pub fn set_po_token(&mut self, po_token: Option<Vec<u8>>) {
        self.po_token = po_token.map(Bytes::from);
    }

    /// Set the visitor data (content binding) used to construct the
    /// cold-start PoToken sent on the first few SABR requests. Must
    /// match the identifier the eventual content-bound token is bound
    /// to, so the server's session-binding logic sees a consistent
    /// identifier stream.
    pub fn set_visitor_data(&mut self, visitor_data: Option<String>) {
        self.visitor_data = visitor_data;
    }

    /// Returns true if every initialized format has reached its declared
    /// duration, i.e. the server has handed us everything it intends to.
    ///
    /// SABR sessions can be cut short by the server with a
    /// `StreamProtectionStatus.attestation_required` (status 3) part even
    /// after the full media has been delivered — the server is just
    /// signaling that further playback would need a fresh attestation, not
    /// that the data is incomplete. Use this to distinguish a real
    /// "stream ended cleanly" from "stream cut at an arbitrary boundary".
    /// Returns true if every initialized format looks complete enough
    /// to treat the stream as finished.
    ///
    /// The server's `FormatInitializationMetadata.end_segment_number` is
    /// only an estimate of where the stream *would* end if the client
    /// kept requesting, and can be wrong by a factor of 2× (e.g. the
    /// server announces `end_segment=15` but only sends 7 segments
    /// covering the full audio). The reliable signal is
    /// `downloaded_duration` vs `duration`: if we've downloaded at
    /// least the declared duration, the format is complete.
    pub fn is_complete(&self) -> bool {
        if self.initialized_formats.is_empty() {
            return false;
        }
        self.initialized_formats
            .values()
            .all(|f| f.duration > 0 && f.downloaded_duration >= f.duration)
    }

    /// Pull the next chunks of media data.
    ///
    /// Returns `Ok(None)` once the entire media has been downloaded.
    pub async fn media(
        &mut self,
    ) -> Result<Option<(Vec<Segment>, Vec<Segment>)>> {
        if self
            .initialized_formats
            .get(&self.audio_format.itag())
            .is_some_and(|f| self.player_time >= f.duration)
        {
            return Ok(None);
        }

        let data = self.fetch_stream_data().await?;

        let mut parser = ump::Parser::new(data);
        while let Some(part) = parser.read_part() {
            tracing::debug!("parsing {:?}", part.ty);
            self.process_part(part)?;
        }

        if let Some(updated_player_time) = self
            .initialized_formats
            .values()
            .map(|f| f.downloaded_duration)
            .min()
        {
            self.player_time = updated_player_time;
        }

        let audio = self
            .initialized_formats
            .get_mut(&self.audio_format.itag())
            .map(|f| f.drain_segments())
            .unwrap_or_default();

        let video = if let Some(format) = &self.video_format {
            self.initialized_formats
                .get_mut(&format.itag())
                .map(|f| f.drain_segments())
                .unwrap_or_default()
        } else {
            Vec::new()
        };

        Ok(Some((audio, video)))
    }

    async fn fetch_stream_data(&mut self) -> Result<Bytes> {
        if let Some(backoff_time) = self.backoff_time.take() {
            tracing::debug!("waiting for {backoff_time}ms before next request");
            tokio::time::sleep(std::time::Duration::from_millis(backoff_time.max(0) as u64))
                .await;
        }

        // Increment the request number up front; YouTube uses it in the URL
        // for response correlation and to de-duplicate concurrent requests.
        let rn = self.request_number;
        self.request_number += 1;
        let is_first_request = rn == 0;

        // Browser-style timing: elapsed wall-clock, time since last action,
        // time since last seek. YouTube uses these for stream-continuity
        // heuristics. We treat the first request as the "start" and tick
        // `elapsed_wall_time_ms` and `time_since_last_action_ms` from
        // `started_at` so the server sees monotonically increasing values
        // (matching what a real browser does after page-load).
        let now = std::time::Instant::now();
        let raw_elapsed_ms = now.duration_since(self.started_at).as_millis() as i64;
        // YouTube's GVS treats `elapsed_wall_time_ms = 0` as "never played"
        // and uses it to decide whether to demand an attestation PoToken.
        // The browser always sends a non-zero value (the page has been
        // alive for at least a few tens of ms when SABR kicks off). Mirror
        // that by bumping the very first request's elapsed to a small
        // realistic value (≈39 ms in our browser capture). Subsequent
        // requests keep the raw elapsed since `started_at`.
        let elapsed_ms = if is_first_request {
            39
        } else {
            raw_elapsed_ms
        };
        // `time_since_last_action_ms` is the wall-clock since the most
        // recent user-visible interaction (autoplay counts as a "recent
        // action" while loading). The browser reports it as roughly
        // `elapsed_wall_time_ms + 40` once playback starts; before that
        // it's `elapsed` itself. Replicate that small offset so GVS
        // doesn't see a zero.
        let time_since_last_action_ms = if is_first_request {
            79
        } else {
            elapsed_ms
        };
        // `time_since_last_seek` resets on every seek; the browser's
        // first request reports ~10 ms after the implicit autoplay seek.
        let time_since_last_seek_ms = if is_first_request
            && self.time_since_last_seek_ms == 0
        {
            10
        } else {
            self.time_since_last_seek_ms
        };
        // The browser's first SABR body reports `bandwidth_estimate`
        // from `navigator.connection.downlink * 1000 / 8` rounded; in
        // our capture it's 101 580 bytes/s. The previous hard-coded
        // 655 360 caused GVS to reject the request with 403 because the
        // value was suspiciously round and inconsistent with the rest
        // of the request shape. Use the browser's exact value here so
        // the first SABR POST looks like a real Chrome client.
        if is_first_request {
            self.bandwidth_estimate = 101_580;
        }

        // The browser reports the viewport in CSS pixels, not screen pixels.
        // YouTube uses this to shape the format recommendations. The exact
        // value depends on the player's box (sidebar + watch panel eat
        // width), so the browser's typical 768x432 is a good default for
        // desktop. The numbers don't have to be exact; the server just
        // wants something plausible.
        let client_state = ClientAbrState {
            player_time_ms: Some(self.player_time),
            // Browser's first SABR body reports viewport 640x360 (the
            // player hasn't fully loaded yet). After a few requests it
            // grows to 768x432 or larger as the player UI fills in. We
            // use the same initial value as the browser.
            client_viewport_width: Some(640),
            client_viewport_height: Some(360),
            sticky_resolution: Some(0),
            // Browser sends 655360 (~640 KB/s) on the first request. We
            // match that exactly so GVS doesn't see a suspiciously
            // large initial bandwidth on a fresh client.
            bandwidth_estimate: Some(self.bandwidth_estimate),
            // 0 = visible. The browser uses 0 even though we'd expect 1 from
            // a non-visible audio downloader.
            visibility: Some(0),
            prefer_vp9: Some(false),
            av1_quality_threshold: Some(1080),
            // Browser sends `drc_enabled = true` on every request. The
            // generated proto calls this `drc_enabled: Option<bool>` at
            // tag 46. We set it explicitly so the field is present.
            drc_enabled: Some(true),
            // Browser sends `sabr_force_max_network_interruption_duration_ms = 0`
            // explicitly (not absent). Without it GVS rejects the request
            // because it expects the field to be present.
            sabr_force_max_network_interruption_duration_ms: Some(0),
            enable_voice_boost: Some(false),
            // YouTube wants these timing fields populated — they affect
            // format selection and stream continuity heuristics. Browser
            // sends monotonically increasing values after the first
            // request; we replicate that with values derived from
            // `started_at`. The server treats 0 here as "never played",
            // which can make it send attestation-required responses even
            // with a valid PoToken. The browser does NOT clamp to 1, it
            // just sends the elapsed value (which is small at startup).
            time_since_last_seek: Some(time_since_last_seek_ms),
            elapsed_wall_time_ms: Some(elapsed_ms),
            time_since_last_action_ms: Some(time_since_last_action_ms),
            // The browser ships a `playback_authorization` that lists every
            // track type (video / audio, HDR / SDR) the player has signed
            // rights to. We replicate the broadest reasonable set.
            playback_authorization: Some(PlaybackAuthorization {
                authorized_formats: vec![
                    AuthorizedFormat {
                        track_type: Some(1),
                        is_hdr: Some(false),
                    },
                    AuthorizedFormat {
                        track_type: Some(2),
                        is_hdr: Some(false),
                    },
                    AuthorizedFormat {
                        track_type: Some(2),
                        is_hdr: Some(true),
                    },
                ],
                ..Default::default()
            }),
            // Fields 71/72/80 carry a 1080p screen-size hint. YouTube uses
            // these to limit the recommended formats on small viewports.
            field71: Some(1),
            field72: Some(SabrVisibilityHint {
                field1: Some(0),
                field2: Some(1080),
                field3: Some(0),
                field4: Some(0),
                field5: Some(1080),
                field6: Some(0),
            }),
            field80: Some(1),
            ..Default::default()
        };

        let streamer_context = StreamerContext {
            client_info: Some(ClientInfo {
                // 1 = WEB client (mirrors what the browser sends).
                client_name: Some(1),
                client_version: Some(self.client_version.clone()),
                // The browser always reports the platform as "X11" (or
                // "Windows" / "Macintosh") — never the kernel version.
                os_name: Some("X11".to_string()),
                // The browser sends `accept_language` (a.k.a. field 1 in the
                // actual GVS ClientInfo proto — note: the proto field number
                // differs from our local .proto file's field 21). Without it
                // the server can't localize format recommendations.
                field1: Some("en_US".to_string()),
                // NOTE: We deliberately do NOT send `device_make`. The
                // browser doesn't send it (only client_name/version/os
                // are required for the WEB client identity). Sending
                // extra fields can confuse GVS heuristics.
                ..Default::default()
            }),
            // The PoToken lives in the URL as `?pot=<base64url>`,
            // NOT in the body — the player's streamer_context
            // proto doesn't even have a po_token field. We mirror
            // that by leaving this field empty and putting the
            // token in the URL below.
            po_token: None,
            // PoToken lives in the URL as `?pot=<base64url>`, NOT
            // in the body — see the URL construction below.
            sabr_contexts: self
                .active_sabr_contexts
                .iter()
                .filter_map(|t| self.sabr_contexts.get(t).cloned())
                .collect(),
            unsent_sabr_contexts: self
                .sabr_contexts
                .keys()
                .filter(|t| !self.active_sabr_contexts.contains(t))
                .copied()
                .collect(),
            playback_cookie: self.playback_cookie.clone(),
            ..Default::default()
        };

        // The browser always leaves `selected_format_ids` empty. The
        // `preferred_*_format_ids` fields (set below) carry the format hints
        // to the server instead.
        let selected_format_ids: Vec<_> = Vec::new();

        let req = VideoPlaybackAbrRequest {
            client_abr_state: Some(client_state),
            selected_format_ids,
            buffered_ranges: vec![],
            video_playback_ustreamer_config: Some(self.ustreamer_config.clone()),
            // The browser sends the FULL set of advertised audio/video
            // formats so the server can pick a compatible fallback when the
            // chosen one becomes unavailable. Without this YouTube returns
            // 403 on the cold start.
            preferred_audio_format_ids: self.preferred_audio_formats.clone(),
            preferred_video_format_ids: self.preferred_video_formats.clone(),
            streamer_context: Some(streamer_context.clone()),
            ..Default::default()
        };
        let body = req.encode_to_vec();

        // Build the request URL with the SABR-specific query parameters.
        // YouTube requires `cpn`, `cver`, `rn`, `alr` on every SABR request.
        // The browser appends them in this exact order; matching it avoids
        // 403s from the GVS server.
        //
        // The `pot` parameter is the PoToken — sent as base64url-encoded
        // raw bytes (the full PoTokenMsg from the botguard mint, NOT just
        // its field 6 payload). For the first ~4 requests we send a
        // cold-start PoToken (8 random bytes wrapped as a 10-byte
        // `OIPoTokenMsg` field-4 placeholder) so the server lets the
        // first few segments through. After that we switch to the minted
        // PoToken. Both are sent in the URL, NEVER in the body — the
        // player's SABR body has no PoToken field; the streamer_context
        // proto's `am` encoder only writes `clientInfo` (f1), `lP` (f2),
        // `playbackCookie` (f3), `N0` (f4), `mS` (f5), `jH` (f6),
        // `ZU8` (f7), `Lh` (f8).
        // Build the cold-start PoToken wire shape, matching
        // BgUtils' `generateColdStartToken` exactly:
        //
        //   packet[0]   = 0x22   (field 4, wire type 2)
        //   packet[1]   = payload length
        //   packet[2..] = payload (XOR-encrypted)
        //
        // The payload is:
        //   2 random bytes  (XOR key)
        //   1 byte = 0      (some masked value)
        //   1 byte = clientState
        //   4 bytes BE      (unix timestamp)
        //   N bytes         (UTF-8 content binding — e.g. visitor_data)
        //
        // The XOR is performed *on the payload* (bytes 2..) using
        // the 2-byte key, starting at offset 2 (i.e. over the
        // header from byte 2 onward, including the content binding).
        //
        // The browser sends a content-bound cold-start token to
        // bind the upcoming stream to the same identifier the
        // eventual full attestation will use. The real PoToken
        // (used after the prewarm) is the full mintCallback
        // output (~85–120 bytes depending on VM version) which
        // is also bound to the same identifier.
        let pot_b64url: String = if self.request_number < 4 {
            // The cold-start identifier. We use the same
            // identifier (visitorData) that the eventual
            // content-bound token will be minted with, so the
            // server's session-binding logic sees a consistent
            // stream of identifiers.
            let identifier = self
                .visitor_data
                .as_deref()
                .unwrap_or("rustypipe-cold-start");
            let encoded_identifier = identifier.as_bytes();
            let timestamp = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0);
            // 2-byte XOR key
            let k = rand_bytes();
            let k0 = k[0];
            let k1 = k[1];
            let client_state: u8 = 1;
            // payload = k0, k1, 0, clientState, ts4bytes, identifier
            let payload_len = 2 + 2 + 4 + encoded_identifier.len();
            let mut packet = Vec::with_capacity(2 + payload_len);
            packet.push(0x22);
            packet.push(payload_len as u8);
            packet.push(k0);
            packet.push(k1);
            packet.push(0);
            packet.push(client_state);
            packet.push((timestamp >> 24) as u8);
            packet.push((timestamp >> 16) as u8);
            packet.push((timestamp >> 8) as u8);
            packet.push(timestamp as u8);
            packet.extend_from_slice(encoded_identifier);
            // XOR the payload (bytes 2..end) using the 2-byte key
            for i in 2..packet.len() {
                let key_byte = if i % 2 == 0 { k0 } else { k1 };
                packet[i] ^= key_byte;
            }
            use base64::Engine;
            base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(&packet)
        } else {
            match self.po_token.as_ref() {
                Some(bytes) => {
                    use base64::Engine;
                    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes.as_ref())
                }
                None => {
                    use base64::Engine;
                    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(rand_bytes())
                }
            }
        };
        let url = match url::Url::parse(&self.url) {
            Ok(mut u) => {
                let _ = u
                    .query_pairs_mut()
                    .append_pair("cpn", &self.cpn);
                let _ = u
                    .query_pairs_mut()
                    .append_pair("cver", &self.client_version);
                let _ = u.query_pairs_mut().append_pair("rn", &rn.to_string());
                let _ = u.query_pairs_mut().append_pair("alr", "yes");
                let _ = u.query_pairs_mut().append_pair("pot", &pot_b64url);
                u.to_string()
            }
            Err(_) => self.url.clone(),
        };
        let final_url = url.clone();

        // Diagnostic: dump the first N requests to /tmp for diffing
        // against the browser's actual SABR body. Set
        // RUSTYPIPE_SABR_DUMP=1 to enable. The files are named
        // sabr_rustypipe_<count>.bin in /tmp.
        if std::env::var("RUSTYPIPE_SABR_DUMP").is_ok() {
            use std::sync::atomic::{AtomicU32, Ordering};
            static COUNTER: AtomicU32 = AtomicU32::new(0);
            let n = COUNTER.fetch_add(1, Ordering::Relaxed);
            let path = format!("/tmp/sabr_rustypipe_{:04}.bin", n);
            let _ = std::fs::write(&path, &body);
            let url_path = format!("/tmp/sabr_rustypipe_{:04}.url", n);
            let _ = std::fs::write(&url_path, final_url.as_bytes());
            eprintln!(
                "sabr_dump: wrote {} bytes to {} (rn={}) url={}",
                body.len(),
                path,
                rn,
                final_url
            );
        }

        let res = self
            .client
            .post(&url)
            .body(body)
            .send()
            .await?;

        let status = res.status();
        let resp_headers = res.headers().clone();
        let bytes = res.bytes().await?;
        if !status.is_success() {
            tracing::warn!(
                "SABR request to {} returned non-success status {}: body[:200]={:?}",
                url,
                status,
                String::from_utf8_lossy(&bytes[..bytes.len().min(200)])
            );
            tracing::warn!("SABR response headers: {:?}", resp_headers);
            // Dump the failed response for inspection. Useful for
            // debugging PoToken / spc / sig mismatches without
            // running a network capture.
            let _ = std::fs::write(
                "/tmp/rustypipe-sabr-response.bin",
                &bytes,
            );
            // Some SABR servers return 403 with a UMP body containing an
            // AttestationRequired part. The caller parses the body regardless.
        }
        Ok(bytes)
    }

    fn process_part(&mut self, part: ump::Part) -> Result<()> {
        match part.ty {
            UmpPartId::MediaHeader => {
                let header = MediaHeader::decode(part.data.as_ref())?;
                let video_id = header.video_id.clone().unwrap_or_default();
                let header_id = header.header_id.unwrap_or_default();
                let sequence_number = header.sequence_number.unwrap_or_default();
                let duration = header.duration_ms.unwrap_or_else(|| {
                    if let Some(tr) = &header.time_range {
                        let dur = tr.duration_ticks.unwrap_or(0) as f64;
                        let ts = tr.timescale.unwrap_or(1).max(1) as f64;
                        ((dur / ts) * 1000.0).ceil() as i64
                    } else {
                        0
                    }
                });

                if video_id != self.video_id {
                    tracing::error!("received media header for unexpected video {video_id}");
                    return Err(Error::HeaderMismatch);
                }

                let itag = header
                    .format_id
                    .as_ref()
                    .and_then(|f| f.itag)
                    .unwrap_or_default();
                let format = self
                    .initialized_formats
                    .get_mut(&itag)
                    .ok_or(Error::InvalidData)?;

                if format.downloaded_segments.contains_key(&sequence_number) {
                    tracing::warn!("segment {sequence_number} already downloaded, ignoring");
                    return Ok(());
                }
                // SABR's attestation-required flow re-sends the
                // same audio segments each time the client mints
                // a fresh PoToken. `drained_max_seq` tracks the
                // highest seq we have already handed to the
                // downloader (and thus to disk), so we can drop
                // the re-send here before it ever enters
                // `downloaded_segments` — otherwise the next
                // `drain_segments` call would hand the same
                // bytes back to the downloader and it would
                // write them again, ballooning the .sabr.part
                // file.
                if sequence_number <= format.drained_max_seq {
                    tracing::warn!(
                        "segment {sequence_number} already drained (max={}), ignoring",
                        format.drained_max_seq
                    );
                    return Ok(());
                }

                self.partial_segments.insert(
                    header_id,
                    Segment {
                        sequence_number,
                        header,
                        data: Vec::new(),
                        duration,
                    },
                );
            }
            UmpPartId::Media => {
                let mut parser = ump::Parser::new(part.data);
                let header_id = parser
                    .read_varint()
                    .ok_or(Error::InvalidData)?;

                let Some(segment) = self.partial_segments.get_mut(&header_id) else {
                    return Ok(());
                };
                segment.data.push(parser.data());
            }
            UmpPartId::MediaEnd => {
                let mut parser = ump::Parser::new(part.data);
                let header_id = parser
                    .read_varint()
                    .ok_or(Error::InvalidData)?;
                let Some(segment) = self.partial_segments.remove(&header_id) else {
                    return Ok(());
                };

                let segment_length = segment.len();
                let expected = segment
                    .header
                    .content_length
                    .unwrap_or_default() as usize;
                if segment_length != expected {
                    tracing::warn!(
                        "segment {header_id} content-length mismatch: expected {expected}, got {segment_length}"
                    );
                    return Err(Error::ContentLengthMismatch);
                }

                let itag = segment
                    .header
                    .format_id
                    .as_ref()
                    .and_then(|f| f.itag)
                    .unwrap_or_default();
                let format = self
                    .initialized_formats
                    .get_mut(&itag)
                    .ok_or(Error::InvalidData)?;
                format.downloaded_duration += segment.duration;
                format
                    .downloaded_segments
                    .insert(segment.sequence_number, segment);
            }
            UmpPartId::NextRequestPolicy => {
                let mut policy = NextRequestPolicy::decode(part.data.as_ref())?;
                self.backoff_time = policy.backoff_time_ms;
                if let Some(cookie) = policy.playback_cookie.as_ref() {
                    self.playback_cookie = Some(Bytes::from(cookie.encode_to_vec()));
                }
            }
            UmpPartId::FormatInitializationMetadata => {
                let metadata = FormatInitializationMetadata::decode(part.data.as_ref())?;
                let duration = metadata.end_time_ms.unwrap_or_default();
                let end_segment_number = metadata.end_segment_number.unwrap_or_default();
                let format_id = metadata.format_id.clone().ok_or(Error::InvalidData)?;
                let itag = format_id.itag();

                if self.initialized_formats.contains_key(&itag) {
                    tracing::warn!("skipping already initialized format {itag}");
                    return Ok(());
                }

                let format = InitializedFormat {
                    id: format_id,
                    downloaded_segments: HashMap::with_capacity(end_segment_number as usize + 1),
                    drained_max_seq: -1,
                    duration,
                    end_segment_number,
                    last_downloaded_segment: 0,
                    downloaded_duration: 0,
                };
                self.initialized_formats.insert(itag, format);
            }
            UmpPartId::SabrRedirect => {
                let redirect = SabrRedirect::decode(part.data.as_ref())?;
                if let Some(url) = redirect.url {
                    self.url = url;
                }
            }
            UmpPartId::SabrContextUpdate => {
                let update = SabrContextUpdate::decode(part.data.as_ref())?;
                let ty = update.r#type.unwrap_or_default();

                if update.write_policy() == SabrContextWritePolicy::KeepExisting
                    && self.sabr_contexts.contains_key(&ty)
                {
                    return Ok(());
                }
                if update.send_by_default.unwrap_or_default() {
                    self.active_sabr_contexts.insert(ty);
                }
                self.sabr_contexts.insert(
                    ty,
                    SabrContext {
                        r#type: update.r#type,
                        value: update.value,
                    },
                );
            }
            UmpPartId::SabrContextSendingPolicy => {
                let policy = SabrContextSendingPolicy::decode(part.data.as_ref())?;
                for t in policy.start_policy {
                    if self.active_sabr_contexts.contains(&t) {
                        self.active_sabr_contexts.insert(t);
                    }
                }
                for t in policy.stop_policy {
                    self.active_sabr_contexts.remove(&t);
                }
                for t in policy.discard_policy {
                    self.sabr_contexts.remove(&t);
                }
            }
            UmpPartId::StreamProtectionStatus => {
                let status = StreamProtectionStatus::decode(part.data.as_ref())?;
                match status.status.unwrap_or_default() {
                    1 => tracing::debug!("[StreamProtectionStatus] OK"),
                    2 => tracing::debug!("[StreamProtectionStatus] attestation pending"),
                    3 => {
                        tracing::warn!(
                            "[StreamProtectionStatus] attestation required (max_retries={:?})",
                            status.max_retries
                        );
                        return Err(Error::AttestationRequired);
                    }
                    v => tracing::warn!("[StreamProtectionStatus] unknown status {v}"),
                }
            }
            UmpPartId::SabrError => {
                let err = SabrError::decode(part.data.as_ref())?;
                return Err(err.into());
            }
            _ => {}
        }
        Ok(())
    }
}

/// `Result` alias for SABR operations.
pub type Result<T> = core::result::Result<T, Error>;

/// Generate 8 random-looking bytes for the client playback nonce.
///
/// We don't need a CSPRNG here; the only requirement is that the nonce
/// differs between concurrent sessions so YouTube can route responses
/// correctly. A 64-bit hash of the system time and a counter is more
/// than sufficient.
fn rand_bytes() -> [u8; 8] {
    use std::cell::Cell;
    use std::hash::{BuildHasher, Hasher, RandomState};

    thread_local! {
        static COUNTER: Cell<u64> = const { Cell::new(0) };
    }

    let mut h = RandomState::new().build_hasher();
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0);
    h.write_u64(nanos);
    let ctr = COUNTER.with(|c| {
        let n = c.get().wrapping_add(1);
        c.set(n);
        n
    });
    h.write_u64(ctr);
    let out = h.finish();
    out.to_ne_bytes()
}
