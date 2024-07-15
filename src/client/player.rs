use std::{
    borrow::Cow,
    collections::{BTreeMap, HashMap},
    fmt::Debug,
};

use once_cell::sync::Lazy;
use regex::Regex;
use serde::Serialize;
use url::Url;

use crate::{
    deobfuscate::Deobfuscator,
    error::{internal::DeobfError, Error, ExtractionError, UnavailabilityReason},
    model::{
        traits::QualityOrd, AudioCodec, AudioFormat, AudioStream, AudioTrack, ChannelId, Frameset,
        Subtitle, VideoCodec, VideoFormat, VideoPlayer, VideoPlayerDetails, VideoStream,
    },
    param::Language,
    util,
};

use super::{
    response::{self, player},
    ClientType, MapResponse, MapResult, RustyPipeQuery, YTContext,
};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct QPlayer<'a> {
    context: YTContext<'a>,
    /// Website playback context
    #[serde(skip_serializing_if = "Option::is_none")]
    playback_context: Option<QPlaybackContext<'a>>,
    /// Content playback nonce (mobile only, 16 random chars)
    #[serde(skip_serializing_if = "Option::is_none")]
    cpn: Option<String>,
    /// YouTube video ID
    video_id: &'a str,
    /// Set to true to allow extraction of streams with sensitive content
    content_check_ok: bool,
    /// Probably refers to allowing sensitive content, too
    racy_check_ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    params: Option<&'a str>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct QPlaybackContext<'a> {
    content_playback_context: QContentPlaybackContext<'a>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct QContentPlaybackContext<'a> {
    /// Signature timestamp extracted from player.js
    signature_timestamp: &'a str,
    /// Referer URL from website
    referer: String,
}

impl RustyPipeQuery {
    /// Get YouTube player data (video/audio streams + basic metadata)
    #[tracing::instrument(skip(self))]
    pub async fn player<S: AsRef<str> + Debug>(&self, video_id: S) -> Result<VideoPlayer, Error> {
        let video_id = video_id.as_ref();
        let desktop_res = self.player_from_client(video_id, ClientType::Desktop).await;

        match desktop_res {
            Ok(res) => Ok(res),
            Err(Error::Extraction(e)) => {
                if e.switch_client() {
                    let tv_res = self
                        .player_from_client(video_id, ClientType::TvHtml5Embed)
                        .await;

                    match tv_res {
                        // Output desktop client error if the tv client is unsupported
                        Err(Error::Extraction(ExtractionError::Unavailable {
                            reason: UnavailabilityReason::UnsupportedClient,
                            ..
                        })) => Err(Error::Extraction(e)),
                        _ => tv_res,
                    }
                } else {
                    Err(Error::Extraction(e))
                }
            }
            Err(e) => Err(e),
        }
    }

    /// Get YouTube player data (video/audio streams + basic metadata) using the specified client
    #[tracing::instrument(skip(self))]
    pub async fn player_from_client<S: AsRef<str> + Debug>(
        &self,
        video_id: S,
        client_type: ClientType,
    ) -> Result<VideoPlayer, Error> {
        let video_id = video_id.as_ref();
        let (context, deobf) = tokio::join!(
            self.get_context(client_type, false, None),
            self.client.get_deobf_data()
        );
        let deobf = deobf?;

        let request_body = if client_type.is_web() {
            QPlayer {
                context,
                playback_context: Some(QPlaybackContext {
                    content_playback_context: QContentPlaybackContext {
                        signature_timestamp: &deobf.sts,
                        referer: format!("https://www.youtube.com/watch?v={video_id}"),
                    },
                }),
                cpn: None,
                video_id,
                content_check_ok: true,
                racy_check_ok: true,
                params: None,
            }
        } else {
            QPlayer {
                context,
                playback_context: None,
                cpn: Some(util::generate_content_playback_nonce()),
                video_id,
                content_check_ok: true,
                racy_check_ok: true,
                // Source: https://github.com/TeamNewPipe/NewPipeExtractor/pull/1168
                params: Some("CgIIAQ%3D%3D").filter(|_| client_type == ClientType::Android),
            }
        };

        self.execute_request_deobf::<response::Player, _, _>(
            client_type,
            "player",
            video_id,
            "player",
            &request_body,
            None,
            Some(&deobf),
        )
        .await
    }
}

impl MapResponse<VideoPlayer> for response::Player {
    fn map_response(
        self,
        id: &str,
        _lang: Language,
        deobf: Option<&crate::deobfuscate::DeobfData>,
        vdata: Option<&str>,
    ) -> Result<super::MapResult<VideoPlayer>, ExtractionError> {
        let deobf = Deobfuscator::new(deobf.unwrap())?;
        let mut warnings = vec![];

        // Check playability status
        let is_live = match self.playability_status {
            response::player::PlayabilityStatus::Ok { live_streamability } => {
                live_streamability.is_some()
            }
            response::player::PlayabilityStatus::Unplayable {
                reason,
                error_screen,
            } => {
                let mut msg = reason;
                if let Some(error_screen) = error_screen {
                    msg.push_str(" - ");
                    msg.push_str(&error_screen.player_error_message_renderer.subreason);
                }

                let reason = msg
                    .split_whitespace()
                    .find_map(|word| match word {
                        "payment" => Some(UnavailabilityReason::Paid),
                        "Premium" => Some(UnavailabilityReason::Premium),
                        "members-only" => Some(UnavailabilityReason::MembersOnly),
                        "country" => Some(UnavailabilityReason::Geoblocked),
                        "Android" | "websites" => Some(UnavailabilityReason::UnsupportedClient),
                        "bot" => Some(UnavailabilityReason::IpBan),
                        _ => None,
                    })
                    .unwrap_or_default();
                return Err(ExtractionError::Unavailable { reason, msg });
            }
            response::player::PlayabilityStatus::LoginRequired { reason, messages } => {
                let mut msg = reason;
                for m in &messages {
                    if !msg.is_empty() {
                        msg.push(' ');
                    }
                    msg.push_str(m);
                }

                // reason (age restriction): "Sign in to confirm your age"
                // or: "This video may be inappropriate for some users."
                // reason (private): "This video is private"
                let reason = msg
                    .split_whitespace()
                    .find_map(|word| match word {
                        "age" | "inappropriate" => Some(UnavailabilityReason::AgeRestricted),
                        "private" => Some(UnavailabilityReason::Private),
                        _ => None,
                    })
                    .unwrap_or_default();
                return Err(ExtractionError::Unavailable { reason, msg });
            }
            response::player::PlayabilityStatus::LiveStreamOffline { reason } => {
                return Err(ExtractionError::Unavailable {
                    reason: UnavailabilityReason::OfflineLivestream,
                    msg: reason,
                });
            }
            response::player::PlayabilityStatus::Error { reason } => {
                // reason (censored): "This video has been removed for violating YouTube's policy on hate speech. Learn more about combating hate speech in your country."
                // reason: "This video is unavailable"
                return Err(ExtractionError::Unavailable {
                    reason: UnavailabilityReason::Deleted,
                    msg: reason,
                });
            }
        };

        let mut streaming_data =
            self.streaming_data
                .ok_or(ExtractionError::InvalidData(Cow::Borrowed(
                    "no streaming data",
                )))?;
        let video_details =
            self.video_details
                .ok_or(ExtractionError::InvalidData(Cow::Borrowed(
                    "no video details",
                )))?;

        if video_details.video_id != id {
            return Err(ExtractionError::WrongResult(format!(
                "video id {}, expected {}",
                video_details.video_id, id
            )));
        }

        let video_info = VideoPlayerDetails {
            id: video_details.video_id,
            name: video_details.title,
            description: video_details.short_description,
            duration: video_details.length_seconds,
            thumbnail: video_details.thumbnail.into(),
            channel: ChannelId {
                id: video_details.channel_id,
                name: video_details.author,
            },
            view_count: video_details.view_count,
            keywords: video_details.keywords,
            is_live,
            is_live_content: video_details.is_live_content,
        };

        let mut formats = streaming_data.formats.c;
        formats.append(&mut streaming_data.adaptive_formats.c);

        let mut video_streams: Vec<VideoStream> = Vec::new();
        let mut video_only_streams: Vec<VideoStream> = Vec::new();
        let mut audio_streams: Vec<AudioStream> = Vec::new();

        if !is_live {
            let mut last_nsig: [String; 2] = [String::new(), String::new()];

            warnings.append(&mut streaming_data.formats.warnings);
            warnings.append(&mut streaming_data.adaptive_formats.warnings);

            for f in formats {
                if f.format_type == player::FormatType::FormatStreamTypeOtf {
                    continue;
                }

                match (f.is_video(), f.is_audio()) {
                    (true, true) => match map_video_stream(f, &deobf, &mut last_nsig) {
                        Ok(c) => video_streams.push(c),
                        Err(e) => warnings.push(e.to_string()),
                    },
                    (true, false) => match map_video_stream(f, &deobf, &mut last_nsig) {
                        Ok(c) => video_only_streams.push(c),
                        Err(e) => warnings.push(e.to_string()),
                    },
                    (false, true) => {
                        match map_audio_stream(f, &deobf, &mut last_nsig, &mut warnings) {
                            Ok(c) => audio_streams.push(c),
                            Err(e) => warnings.push(e.to_string()),
                        }
                    }
                    (false, false) => warnings.push(format!("invalid stream: itag {}", f.itag)),
                }
            }
        }

        video_streams.sort_by(QualityOrd::quality_cmp);
        video_only_streams.sort_by(QualityOrd::quality_cmp);
        audio_streams.sort_by(QualityOrd::quality_cmp);

        let subtitles = self.captions.map_or(Vec::new(), |captions| {
            captions
                .player_captions_tracklist_renderer
                .caption_tracks
                .into_iter()
                .map(|c| {
                    let lang_auto = c.name.strip_suffix(" (auto-generated)");
                    Subtitle {
                        url: c.base_url,
                        lang: c.language_code,
                        lang_name: lang_auto.unwrap_or(&c.name).to_owned(),
                        auto_generated: lang_auto.is_some(),
                    }
                })
                .collect()
        });

        let preview_frames = self
            .storyboards
            .and_then(|sb| {
                let spec = sb.player_storyboard_spec_renderer.spec;
                let mut spec_parts = spec.split('|');
                let url_tmpl = spec_parts.next()?;

                Some(
                    spec_parts
                        .enumerate()
                        .filter_map(|(i, fs_spec)| {
                            // Example: 160#90#131#5#5#2000#M$M#rs$AOn4CLCV3TJ2Nty5fbw2r-Lqg4VDOZcVvQ
                            let mut parts = fs_spec.split('#');

                            let frame_width = parts.next()?.parse().ok()?;
                            let frame_height = parts.next()?.parse().ok()?;
                            let total_count = parts.next()?.parse().ok()?;
                            let frames_per_page_x = parts.next()?.parse().ok()?;
                            let frames_per_page_y = parts.next()?.parse().ok()?;
                            let duration_per_frame = parts.next()?.parse().ok()?;

                            let n = parts.next()?;
                            let sigh = parts.next()?;

                            let url = url_tmpl.replace("$L", &i.to_string()).replace("$N", n)
                                + "&sigh="
                                + sigh;

                            let sprite_count =
                                util::div_ceil(total_count, frames_per_page_x * frames_per_page_y);

                            Some(Frameset {
                                url_template: url,
                                frame_width,
                                frame_height,
                                page_count: sprite_count,
                                total_count,
                                duration_per_frame,
                                frames_per_page_x,
                                frames_per_page_y,
                            })
                        })
                        .collect(),
                )
            })
            .unwrap_or_default();

        Ok(MapResult {
            c: VideoPlayer {
                details: video_info,
                video_streams,
                video_only_streams,
                audio_streams,
                subtitles,
                expires_in_seconds: streaming_data.expires_in_seconds,
                hls_manifest_url: streaming_data.hls_manifest_url,
                dash_manifest_url: streaming_data.dash_manifest_url,
                preview_frames,
                visitor_data: self
                    .response_context
                    .visitor_data
                    .or_else(|| vdata.map(str::to_owned)),
            },
            warnings,
        })
    }
}

fn cipher_to_url_params(
    signature_cipher: &str,
    deobf: &Deobfuscator,
) -> Result<(Url, BTreeMap<String, String>), DeobfError> {
    let params: HashMap<Cow<str>, Cow<str>> =
        url::form_urlencoded::parse(signature_cipher.as_bytes()).collect();

    // Parameters:
    // `s`: Obfuscated signature
    // `sp`: Signature parameter
    // `url`: URL that is missing the signature parameter

    let sig = params.get("s").ok_or(DeobfError::Extraction("s param"))?;
    let sp = params.get("sp").ok_or(DeobfError::Extraction("sp param"))?;
    let raw_url = params
        .get("url")
        .ok_or(DeobfError::Extraction("no url param"))?;
    let (url_base, mut url_params) =
        util::url_to_params(raw_url).or(Err(DeobfError::Extraction("url params")))?;

    let deobf_sig = deobf.deobfuscate_sig(sig)?;
    url_params.insert(sp.to_string(), deobf_sig);

    Ok((url_base, url_params))
}

fn deobf_nsig(
    url_params: &mut BTreeMap<String, String>,
    deobf: &Deobfuscator,
    last_nsig: &mut [String; 2],
) -> Result<(), DeobfError> {
    let nsig: String;
    if let Some(n) = url_params.get("n") {
        nsig = if n == &last_nsig[0] {
            last_nsig[1].clone()
        } else {
            let nsig = deobf.deobfuscate_nsig(n)?;
            last_nsig[0] = n.to_string();
            last_nsig[1].clone_from(&nsig);
            nsig
        };

        url_params.insert("n".to_owned(), nsig);
    };
    Ok(())
}

struct UrlMapRes {
    url: String,
    xtags: Option<String>,
}

fn map_url(
    url: &Option<String>,
    signature_cipher: &Option<String>,
    deobf: &Deobfuscator,
    last_nsig: &mut [String; 2],
) -> Result<UrlMapRes, ExtractionError> {
    let (url_base, mut url_params) = match url {
        Some(url) => util::url_to_params(url).map_err(|_| {
            ExtractionError::InvalidData(format!("Could not parse url `{url}`").into())
        }),
        None => match signature_cipher {
            Some(signature_cipher) => cipher_to_url_params(signature_cipher, deobf).map_err(|e| {
                ExtractionError::InvalidData(
                    format!("Could not deobfuscate signatureCipher `{signature_cipher}`: {e}")
                        .into(),
                )
            }),
            None => Err(ExtractionError::InvalidData(
                "stream contained neither url or cipher".into(),
            )),
        },
    }?;

    deobf_nsig(&mut url_params, deobf, last_nsig)?;
    let url = Url::parse_with_params(url_base.as_str(), url_params.iter())
        .map_err(|_| ExtractionError::InvalidData("could not combine URL".into()))?;

    Ok(UrlMapRes {
        url: url.to_string(),
        xtags: url_params.get("xtags").cloned(),
    })
}

fn map_video_stream(
    f: player::Format,
    deobf: &Deobfuscator,
    last_nsig: &mut [String; 2],
) -> Result<VideoStream, ExtractionError> {
    let Some((mtype, codecs)) = parse_mime(&f.mime_type) else {
        return Err(ExtractionError::InvalidData(
            format!(
                "Invalid mime type `{}` in video format {:?}",
                &f.mime_type, &f
            )
            .into(),
        ));
    };
    let Some(format) = get_video_format(mtype) else {
        return Err(ExtractionError::InvalidData(
            format!("invalid video format. itag: {}", f.itag).into(),
        ));
    };
    let map_res = map_url(&f.url, &f.signature_cipher, deobf, last_nsig)?;

    Ok(VideoStream {
        url: map_res.url,
        itag: f.itag,
        bitrate: f.bitrate,
        average_bitrate: f.average_bitrate.unwrap_or(f.bitrate),
        size: f.content_length,
        index_range: f.index_range,
        init_range: f.init_range,
        duration_ms: f.approx_duration_ms,
        // Note that the format has already been verified using
        // is_video(), so these unwraps are safe
        width: f.width.unwrap(),
        height: f.height.unwrap(),
        fps: f.fps.unwrap(),
        quality: f.quality_label.unwrap(),
        hdr: f.color_info.unwrap_or_default().primaries == player::Primaries::ColorPrimariesBt2020,
        format,
        codec: get_video_codec(codecs),
        mime: f.mime_type,
    })
}

fn map_audio_stream(
    f: player::Format,
    deobf: &Deobfuscator,
    last_nsig: &mut [String; 2],
    warnings: &mut Vec<String>,
) -> Result<AudioStream, ExtractionError> {
    let Some((mtype, codecs)) = parse_mime(&f.mime_type) else {
        return Err(ExtractionError::InvalidData(
            format!(
                "Invalid mime type `{}` in video format {:?}",
                &f.mime_type, &f
            )
            .into(),
        ));
    };
    let format = get_audio_format(mtype).ok_or_else(|| {
        ExtractionError::InvalidData(format!("invalid audio format. itag: {}", f.itag).into())
    })?;
    let map_res = map_url(&f.url, &f.signature_cipher, deobf, last_nsig)?;

    Ok(AudioStream {
        url: map_res.url,
        itag: f.itag,
        bitrate: f.bitrate,
        average_bitrate: f.average_bitrate.unwrap_or(f.bitrate),
        size: f.content_length.unwrap(),
        index_range: f.index_range,
        init_range: f.init_range,
        duration_ms: f.approx_duration_ms,
        format,
        codec: get_audio_codec(codecs),
        mime: f.mime_type,
        channels: f.audio_channels,
        loudness_db: f.loudness_db,
        track: f
            .audio_track
            .map(|t| map_audio_track(t, map_res.xtags, warnings)),
    })
}

fn parse_mime(mime: &str) -> Option<(&str, Vec<&str>)> {
    static PATTERN: Lazy<Regex> =
        Lazy::new(|| Regex::new(r#"(\w+/\w+);\scodecs="([a-zA-Z-0-9.,\s]*)""#).unwrap());

    let captures = PATTERN.captures(mime)?;
    Some((
        captures.get(1).unwrap().as_str(),
        captures
            .get(2)
            .unwrap()
            .as_str()
            .split(", ")
            .collect::<Vec<&str>>(),
    ))
}

fn get_video_format(mtype: &str) -> Option<VideoFormat> {
    match mtype {
        "video/3gpp" => Some(VideoFormat::ThreeGp),
        "video/mp4" => Some(VideoFormat::Mp4),
        "video/webm" => Some(VideoFormat::Webm),
        _ => None,
    }
}

fn get_video_codec(codecs: Vec<&str>) -> VideoCodec {
    for codec in codecs {
        if codec.starts_with("avc1") {
            return VideoCodec::Avc1;
        } else if codec.starts_with("vp9") || codec.starts_with("vp09") {
            return VideoCodec::Vp9;
        } else if codec.starts_with("av01") {
            return VideoCodec::Av01;
        } else if codec.starts_with("mp4v") {
            return VideoCodec::Mp4v;
        }
    }
    VideoCodec::Unknown
}

fn get_audio_format(mtype: &str) -> Option<AudioFormat> {
    match mtype {
        "audio/mp4" => Some(AudioFormat::M4a),
        "audio/webm" => Some(AudioFormat::Webm),
        _ => None,
    }
}

fn get_audio_codec(codecs: Vec<&str>) -> AudioCodec {
    for codec in codecs {
        if codec.starts_with("mp4a") {
            return AudioCodec::Mp4a;
        } else if codec.starts_with("opus") {
            return AudioCodec::Opus;
        }
    }
    AudioCodec::Unknown
}

fn map_audio_track(
    track: response::player::AudioTrack,
    xtags: Option<String>,
    warnings: &mut Vec<String>,
) -> AudioTrack {
    let mut lang = None;
    let mut track_type = None;

    if let Some(xtags) = xtags {
        xtags
            .split(':')
            .filter_map(|param| param.split_once('='))
            .for_each(|(k, v)| match k {
                "lang" => {
                    lang = Some(v.to_owned());
                }
                "acont" => match serde_plain::from_str(v) {
                    Ok(v) => {
                        track_type = Some(v);
                    }
                    Err(_) => {
                        warnings.push(format!("could not parse audio track type `{v}`"));
                    }
                },
                _ => {}
            });
    }

    AudioTrack {
        id: track.id,
        lang,
        lang_name: track.display_name,
        is_default: track.audio_is_default,
        track_type,
    }
}

#[cfg(test)]
mod tests {
    use std::{fs::File, io::BufReader};

    use path_macro::path;
    use rstest::rstest;

    use super::*;
    use crate::{deobfuscate::DeobfData, util::tests::TESTFILES};

    static DEOBF_DATA: Lazy<DeobfData> = Lazy::new(|| {
        DeobfData {
        js_url: "https://www.youtube.com/s/player/c8b8a173/player_ias.vflset/en_US/base.js".to_owned(),
        sig_fn: "var oB={B4:function(a){a.reverse()},xm:function(a,b){a.splice(0,b)},dC:function(a,b){var c=a[0];a[0]=a[b%a.length];a[b%a.length]=c}};var Vva=function(a){a=a.split(\"\");oB.dC(a,42);oB.xm(a,3);oB.dC(a,48);oB.B4(a,68);return a.join(\"\")};var deobf_sig=Vva;".to_owned(),
        nsig_fn: "Ska=function(a){var b=a.split(\"\"),c=[-1505243983,function(d,e){e=(e%d.length+d.length)%d.length;d.splice(-e).reverse().forEach(function(f){d.unshift(f)})},\n-1692381986,function(d,e){e=(e%d.length+d.length)%d.length;var f=d[0];d[0]=d[e];d[e]=f},\n-262444939,\"unshift\",function(d){for(var e=d.length;e;)d.push(d.splice(--e,1)[0])},\n1201502951,-546377604,-504264123,-1978377336,1042456724,function(d,e){for(e=(e%d.length+d.length)%d.length;e--;)d.unshift(d.pop())},\n711986897,406699922,-1842537993,-1678108293,1803491779,1671716087,12778705,-718839990,null,null,-1617525823,342523552,-1338406651,-399705108,-696713950,b,function(d,e){e=(e%d.length+d.length)%d.length;d.splice(0,1,d.splice(e,1,d[0])[0])},\nfunction(d,e){e=(e%d.length+d.length)%d.length;d.splice(e,1)},\n-980602034,356396192,null,-1617525823,function(d,e,f){var h=f.length;d.forEach(function(l,m,n){this.push(n[m]=f[(f.indexOf(l)-f.indexOf(this[m])+m+h--)%f.length])},e.split(\"\"))},\n-1029864222,-641353250,-1681901809,-1391247867,1707415199,-1957855835,b,function(){for(var d=64,e=[];++d-e.length-32;)switch(d){case 58:d=96;continue;case 91:d=44;break;case 65:d=47;continue;case 46:d=153;case 123:d-=58;default:e.push(String.fromCharCode(d))}return e},\n-1936558978,-1505243983,function(d){d.reverse()},\n1296889058,-1813915420,-943019300,function(d,e,f){var h=f.length;d.forEach(function(l,m,n){this.push(n[m]=f[(f.indexOf(l)-f.indexOf(this[m])+m+h--)%f.length])},e.split(\"\"))},\n\"join\",b,-2061642263];c[21]=c;c[22]=c;c[33]=c;try{c[3](c[33],c[9]),c[29](c[22],c[25]),c[29](c[22],c[19]),c[29](c[33],c[17]),c[29](c[21],c[2]),c[29](c[42],c[10]),c[1](c[52],c[40]),c[12](c[28],c[8]),c[29](c[21],c[45]),c[1](c[21],c[48]),c[44](c[26]),c[39](c[5],c[2]),c[31](c[53],c[16]),c[30](c[29],c[8]),c[51](c[29],c[6],c[44]()),c[4](c[43],c[1]),c[2](c[23],c[42]),c[2](c[0],c[46]),c[38](c[14],c[52]),c[32](c[5]),c[26](c[29],c[46]),c[26](c[5],c[13]),c[28](c[1],c[37]),c[26](c[31],c[13]),c[26](c[1],c[34]),\nc[46](c[1],c[32],c[40]()),c[26](c[50],c[44]),c[17](c[50],c[51]),c[0](c[3],c[24]),c[32](c[13]),c[43](c[3],c[51]),c[0](c[34],c[17]),c[16](c[45],c[53]),c[29](c[44],c[13]),c[42](c[1],c[50]),c[47](c[22],c[53]),c[37](c[22]),c[13](c[52],c[21]),c[6](c[43],c[34]),c[6](c[31],c[46])}catch(d){return\"enhanced_except_gZYB_un-_w8_\"+a}return b.join(\"\")};var deobf_nsig=Ska;".to_owned(),
        sts: "19201".to_owned(),
    }
    });

    #[rstest]
    #[case::desktop("desktop")]
    #[case::desktop_music("desktopmusic")]
    #[case::tv_html5_embed("tvhtml5embed")]
    #[case::android("android")]
    #[case::ios("ios")]
    fn map_player_data(#[case] name: &str) {
        let json_path = path!(*TESTFILES / "player" / format!("{name}_video.json"));
        let json_file = File::open(json_path).unwrap();

        let resp: response::Player = serde_json::from_reader(BufReader::new(json_file)).unwrap();
        let map_res = resp
            .map_response("pPvd8UxmSbQ", Language::En, Some(&DEOBF_DATA), None)
            .unwrap();

        assert!(
            map_res.warnings.is_empty(),
            "deserialization/mapping warnings: {:?}",
            map_res.warnings
        );
        let is_desktop = name == "desktop" || name == "desktopmusic";
        insta::assert_ron_snapshot!(format!("map_player_data_{name}"), map_res.c, {
            ".details.publish_date" => insta::dynamic_redaction(move |value, _path| {
                if is_desktop {
                    assert!(value.as_str().unwrap().starts_with("2019-05-30T00:00:00"));
                    "2019-05-30T00:00:00"
                } else {
                    assert_eq!(value, insta::internals::Content::None);
                    "~"
                }
            }),
        });
    }

    #[test]
    fn cipher_to_url() {
        let signature_cipher = "s=w%3DAe%3DA6aDNQLkViKS7LOm9QtxZJHKwb53riq9qEFw-ecBWJCAiA%3DcEg0tn3dty9jEHszfzh4Ud__bg9CEHVx4ix-7dKsIPAhIQRw8JQ0qOA&sp=sig&url=https://rr5---sn-h0jelnez.googlevideo.com/videoplayback%3Fexpire%3D1659376413%26ei%3Dvb7nYvH5BMK8gAfBj7ToBQ%26ip%3D2003%253Ade%253Aaf06%253A6300%253Ac750%253A1b77%253Ac74a%253A80e3%26id%3Do-AB_BABwrXZJN428ZwDxq5ScPn2AbcGODnRlTVhCQ3mj2%26itag%3D251%26source%3Dyoutube%26requiressl%3Dyes%26mh%3DhH%26mm%3D31%252C26%26mn%3Dsn-h0jelnez%252Csn-4g5ednsl%26ms%3Dau%252Conr%26mv%3Dm%26mvi%3D5%26pl%3D37%26initcwndbps%3D1588750%26spc%3DlT-Khi831z8dTejFIRCvCEwx_6romtM%26vprv%3D1%26mime%3Daudio%252Fwebm%26ns%3Db_Mq_qlTFcSGlG9RpwpM9xQH%26gir%3Dyes%26clen%3D3781277%26dur%3D229.301%26lmt%3D1655510291473933%26mt%3D1659354538%26fvip%3D5%26keepalive%3Dyes%26fexp%3D24001373%252C24007246%26c%3DWEB%26rbqsm%3Dfr%26txp%3D4532434%26n%3Dd2g6G2hVqWIXxedQ%26sparams%3Dexpire%252Cei%252Cip%252Cid%252Citag%252Csource%252Crequiressl%252Cspc%252Cvprv%252Cmime%252Cns%252Cgir%252Cclen%252Cdur%252Clmt%26lsparams%3Dmh%252Cmm%252Cmn%252Cms%252Cmv%252Cmvi%252Cpl%252Cinitcwndbps%26lsig%3DAG3C_xAwRQIgCKCGJ1iu4wlaGXy3jcJyU3inh9dr1FIfqYOZEG_MdmACIQCbungkQYFk7EhD6K2YvLaHFMjKOFWjw001_tLb0lPDtg%253D%253D";
        let mut last_nsig: [String; 2] = [String::new(), String::new()];
        let deobf = Deobfuscator::new(&DEOBF_DATA).unwrap();
        let url = map_url(
            &None,
            &Some(signature_cipher.to_owned()),
            &deobf,
            &mut last_nsig,
        )
        .unwrap()
        .url;

        assert_eq!(url, "https://rr5---sn-h0jelnez.googlevideo.com/videoplayback?c=WEB&clen=3781277&dur=229.301&ei=vb7nYvH5BMK8gAfBj7ToBQ&expire=1659376413&fexp=24001373%2C24007246&fvip=5&gir=yes&id=o-AB_BABwrXZJN428ZwDxq5ScPn2AbcGODnRlTVhCQ3mj2&initcwndbps=1588750&ip=2003%3Ade%3Aaf06%3A6300%3Ac750%3A1b77%3Ac74a%3A80e3&itag=251&keepalive=yes&lmt=1655510291473933&lsig=AG3C_xAwRQIgCKCGJ1iu4wlaGXy3jcJyU3inh9dr1FIfqYOZEG_MdmACIQCbungkQYFk7EhD6K2YvLaHFMjKOFWjw001_tLb0lPDtg%3D%3D&lsparams=mh%2Cmm%2Cmn%2Cms%2Cmv%2Cmvi%2Cpl%2Cinitcwndbps&mh=hH&mime=audio%2Fwebm&mm=31%2C26&mn=sn-h0jelnez%2Csn-4g5ednsl&ms=au%2Conr&mt=1659354538&mv=m&mvi=5&n=XzXGSfGusw6OCQ&ns=b_Mq_qlTFcSGlG9RpwpM9xQH&pl=37&rbqsm=fr&requiressl=yes&sig=AOq0QJ8wRQIhAPIsKd7-xi4xVHEC9gb__dU4hzfzsHEj9ytd3nt0gEceAiACJWBcw-wFEq9qir35bwKHJZxtQ9mOL7SKiVkLQNDa6A%3D%3D&source=youtube&sparams=expire%2Cei%2Cip%2Cid%2Citag%2Csource%2Crequiressl%2Cspc%2Cvprv%2Cmime%2Cns%2Cgir%2Cclen%2Cdur%2Clmt&spc=lT-Khi831z8dTejFIRCvCEwx_6romtM&txp=4532434&vprv=1");
    }
}
