use std::{
    borrow::Cow,
    collections::{BTreeMap, HashMap},
};

use fancy_regex::Regex;
use once_cell::sync::Lazy;
use serde::Serialize;
use url::Url;

use crate::{
    deobfuscate::Deobfuscator,
    error::{DeobfError, Error, ExtractionError},
    model::{
        AudioCodec, AudioFormat, AudioStream, AudioTrack, ChannelId, QualityOrd, Subtitle,
        VideoCodec, VideoFormat, VideoPlayer, VideoPlayerDetails, VideoStream,
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
    playback_context: Option<QPlaybackContext>,
    /// Content playback nonce (mobile only, 16 random chars)
    #[serde(skip_serializing_if = "Option::is_none")]
    cpn: Option<String>,
    /// YouTube video ID
    video_id: &'a str,
    /// Set to true to allow extraction of streams with sensitive content
    content_check_ok: bool,
    /// Probably refers to allowing sensitive content, too
    racy_check_ok: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct QPlaybackContext {
    content_playback_context: QContentPlaybackContext,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct QContentPlaybackContext {
    /// Signature timestamp extracted from player.js
    signature_timestamp: String,
    /// Referer URL from website
    referer: String,
}

impl RustyPipeQuery {
    pub async fn player<S: AsRef<str>>(&self, video_id: S) -> Result<VideoPlayer, Error> {
        let video_id = video_id.as_ref();
        let q1 = self.clone();
        let android_res = q1.player_from_client(video_id, ClientType::Android).await;

        match android_res {
            Ok(res) => Ok(res),
            Err(Error::Extraction(e)) => {
                if e.switch_client() {
                    let tv_res = self
                        .player_from_client(video_id, ClientType::TvHtml5Embed)
                        .await;

                    match tv_res {
                        // Output android client error if the tv client is unsupported
                        Err(Error::Extraction(ExtractionError::VideoClientUnsupported(_))) => {
                            Err(Error::Extraction(e))
                        }
                        _ => tv_res,
                    }
                } else {
                    Err(Error::Extraction(e))
                }
            }
            Err(e) => Err(e),
        }
    }

    pub async fn player_from_client<S: AsRef<str>>(
        &self,
        video_id: S,
        client_type: ClientType,
    ) -> Result<VideoPlayer, Error> {
        let video_id = video_id.as_ref();
        let (context, deobf) = tokio::join!(
            self.get_context(client_type, false, None),
            self.client.get_deobf()
        );
        let deobf = deobf?;

        let request_body = if client_type.is_web() {
            QPlayer {
                context,
                playback_context: Some(QPlaybackContext {
                    content_playback_context: QContentPlaybackContext {
                        signature_timestamp: deobf.get_sts(),
                        referer: format!("https://www.youtube.com/watch?v={}", video_id),
                    },
                }),
                cpn: None,
                video_id,
                content_check_ok: true,
                racy_check_ok: true,
            }
        } else {
            QPlayer {
                context,
                playback_context: None,
                cpn: Some(util::generate_content_playback_nonce()),
                video_id,
                content_check_ok: true,
                racy_check_ok: true,
            }
        };

        self.execute_request_deobf::<response::Player, _, _>(
            client_type,
            "player",
            video_id,
            "player",
            &request_body,
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
        deobf: Option<&Deobfuscator>,
    ) -> Result<super::MapResult<VideoPlayer>, ExtractionError> {
        let deobf = deobf.unwrap();
        let mut warnings = vec![];

        // Check playability status
        let is_live = match self.playability_status {
            response::player::PlayabilityStatus::Ok { live_streamability } => {
                live_streamability.is_some()
            }
            response::player::PlayabilityStatus::Unplayable { reason } => {
                for word in reason.split_whitespace() {
                    match word {
                        // reason: "This video requires payment to watch."
                        "payment" => return Err(ExtractionError::VideoUnavailable("DRM", reason)),
                        // reason: "The uploader has not made this video available in your country."
                        "country" => return Err(ExtractionError::VideoGeoblocked),
                        // reason (Android): "This video can only be played on newer versions of Android or other supported devices."
                        // reason (TV client): "Playback on other websites has been disabled by the video owner."
                        "Android" | "websites" => {
                            return Err(ExtractionError::VideoClientUnsupported(reason))
                        }
                        _ => {}
                    }
                }
                return Err(ExtractionError::VideoUnavailable(
                    "being unplayable",
                    reason,
                ));
            }
            response::player::PlayabilityStatus::LoginRequired { reason } => {
                // reason (age restriction): "Sign in to confirm your age"
                // or: "This video may be inappropriate for some users."
                // reason (private): "This video is private"
                if reason
                    .split_whitespace()
                    .any(|word| word == "age" || word == "inappropriate")
                {
                    return Err(ExtractionError::VideoAgeRestricted);
                }
                return Err(ExtractionError::VideoUnavailable("being private", reason));
            }
            response::player::PlayabilityStatus::LiveStreamOffline { reason } => {
                return Err(ExtractionError::VideoUnavailable(
                    "offline livestream",
                    reason,
                ))
            }
            response::player::PlayabilityStatus::Error { reason } => {
                // reason (censored): "This video has been removed for violating YouTube's policy on hate speech. Learn more about combating hate speech in your country."
                // reason: "This video is unavailable"
                return Err(ExtractionError::VideoUnavailable(
                    "deletion/censorship",
                    reason,
                ));
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
            title: video_details.title,
            description: video_details.short_description,
            length: video_details.length_seconds,
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
                    (true, true) => {
                        let mut map_res = map_video_stream(f, deobf, &mut last_nsig);
                        warnings.append(&mut map_res.warnings);
                        if let Some(c) = map_res.c {
                            video_streams.push(c);
                        };
                    }
                    (true, false) => {
                        let mut map_res = map_video_stream(f, deobf, &mut last_nsig);
                        warnings.append(&mut map_res.warnings);
                        if let Some(c) = map_res.c {
                            video_only_streams.push(c);
                        };
                    }
                    (false, true) => {
                        let mut map_res = map_audio_stream(f, deobf, &mut last_nsig);
                        warnings.append(&mut map_res.warnings);
                        if let Some(c) = map_res.c {
                            audio_streams.push(c);
                        };
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
                visitor_data: self.response_context.visitor_data,
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
            last_nsig[1].to_owned()
        } else {
            let nsig = deobf.deobfuscate_nsig(n)?;
            last_nsig[0] = n.to_string();
            last_nsig[1] = nsig.to_owned();
            nsig
        };

        url_params.insert("n".to_owned(), nsig);
    };
    Ok(())
}

fn map_url(
    url: &Option<String>,
    signature_cipher: &Option<String>,
    deobf: &Deobfuscator,
    last_nsig: &mut [String; 2],
) -> MapResult<Option<(String, bool)>> {
    let (url_base, mut url_params) = match url {
        Some(url) => ok_or_bail!(
            util::url_to_params(url),
            MapResult {
                c: None,
                warnings: vec![format!("Could not parse url `{}`", url)]
            }
        ),
        None => match signature_cipher {
            Some(signature_cipher) => match cipher_to_url_params(signature_cipher, deobf) {
                Ok(res) => res,
                Err(e) => {
                    return MapResult {
                        c: None,
                        warnings: vec![format!(
                            "Could not deobfuscate signatureCipher `{}`: {}",
                            signature_cipher, e
                        )],
                    };
                }
            },
            None => {
                return MapResult {
                    c: None,
                    warnings: vec!["stream contained neither url nor cipher".to_owned()],
                }
            }
        },
    };

    let mut warnings = vec![];
    let mut throttled = false;
    deobf_nsig(&mut url_params, deobf, last_nsig).unwrap_or_else(|e| {
        warnings.push(format!(
            "Could not deobfuscate nsig (params: {:?}): {}",
            url_params, e
        ));
        throttled = true;
    });

    MapResult {
        c: Some((
            ok_or_bail!(
                Url::parse_with_params(url_base.as_str(), url_params.iter()),
                MapResult {
                    c: None,
                    warnings: vec![format!(
                        "url could not be joined. url: `{}` params: {:?}",
                        url_base, url_params
                    )],
                }
            )
            .to_string(),
            throttled,
        )),
        warnings,
    }
}

fn map_video_stream(
    f: player::Format,
    deobf: &Deobfuscator,
    last_nsig: &mut [String; 2],
) -> MapResult<Option<VideoStream>> {
    let (mtype, codecs) = some_or_bail!(
        parse_mime(&f.mime_type),
        MapResult {
            c: None,
            warnings: vec![format!(
                "Invalid mime type `{}` in video format {:?}",
                &f.mime_type, &f
            )]
        }
    );
    let map_res = map_url(&f.url, &f.signature_cipher, deobf, last_nsig);

    match map_res.c {
        Some((url, throttled)) => MapResult {
            c: Some(VideoStream {
                url,
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
                hdr: f.color_info.unwrap_or_default().primaries
                    == player::Primaries::ColorPrimariesBt2020,
                mime: f.mime_type.to_owned(),
                format: some_or_bail!(
                    get_video_format(mtype),
                    MapResult {
                        c: None,
                        warnings: vec![format!("invalid video format. itag: {}", f.itag)]
                    }
                ),
                codec: get_video_codec(codecs),
                throttled,
            }),
            warnings: map_res.warnings,
        },
        None => MapResult {
            c: None,
            warnings: map_res.warnings,
        },
    }
}

fn map_audio_stream(
    f: player::Format,
    deobf: &Deobfuscator,
    last_nsig: &mut [String; 2],
) -> MapResult<Option<AudioStream>> {
    static LANG_PATTERN: Lazy<Regex> = Lazy::new(|| Regex::new(r#"^([a-z]{2,3})\."#).unwrap());

    let (mtype, codecs) = some_or_bail!(
        parse_mime(&f.mime_type),
        MapResult {
            c: None,
            warnings: vec![format!(
                "Invalid mime type `{}` in video format {:?}",
                &f.mime_type, &f
            )]
        }
    );
    let map_res = map_url(&f.url, &f.signature_cipher, deobf, last_nsig);

    match map_res.c {
        Some((url, throttled)) => MapResult {
            c: Some(AudioStream {
                url,
                itag: f.itag,
                bitrate: f.bitrate,
                average_bitrate: f.average_bitrate.unwrap_or(f.bitrate),
                size: f.content_length.unwrap(),
                index_range: f.index_range,
                init_range: f.init_range,
                duration_ms: f.approx_duration_ms,
                mime: f.mime_type.to_owned(),
                format: some_or_bail!(
                    get_audio_format(mtype),
                    MapResult {
                        c: None,
                        warnings: vec![format!("invalid audio format. itag: {}", f.itag)]
                    }
                ),
                codec: get_audio_codec(codecs),
                channels: f.audio_channels,
                loudness_db: f.loudness_db,
                throttled,
                track: match f.audio_track {
                    Some(t) => {
                        let lang = LANG_PATTERN
                            .captures(&t.id)
                            .ok()
                            .flatten()
                            .map(|m| m.get(1).unwrap().as_str().to_owned());

                        Some(AudioTrack {
                            id: t.id,
                            lang,
                            lang_name: t.display_name,
                            is_default: t.audio_is_default,
                        })
                    }
                    None => None,
                },
            }),
            warnings: map_res.warnings,
        },
        None => MapResult {
            c: None,
            warnings: map_res.warnings,
        },
    }
}

fn parse_mime(mime: &str) -> Option<(&str, Vec<&str>)> {
    static PATTERN: Lazy<Regex> =
        Lazy::new(|| Regex::new(r#"(\w+/\w+);\scodecs="([a-zA-Z-0-9.,\s]*)""#).unwrap());

    let captures = some_or_bail!(PATTERN.captures(mime).ok().flatten(), None);
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

#[cfg(test)]
mod tests {
    use std::{fs::File, io::BufReader};

    use path_macro::path;
    use rstest::rstest;

    use super::*;
    use crate::deobfuscate::DeobfData;

    static DEOBFUSCATOR: Lazy<Deobfuscator> = Lazy::new(|| {
        Deobfuscator::from(DeobfData {
        js_url: "https://www.youtube.com/s/player/c8b8a173/player_ias.vflset/en_US/base.js".to_owned(),
        sig_fn: "var oB={B4:function(a){a.reverse()},xm:function(a,b){a.splice(0,b)},dC:function(a,b){var c=a[0];a[0]=a[b%a.length];a[b%a.length]=c}};var Vva=function(a){a=a.split(\"\");oB.dC(a,42);oB.xm(a,3);oB.dC(a,48);oB.B4(a,68);return a.join(\"\")};function deobfuscate(a){return Vva(a);}".to_owned(),
        nsig_fn: "Ska=function(a){var b=a.split(\"\"),c=[-1505243983,function(d,e){e=(e%d.length+d.length)%d.length;d.splice(-e).reverse().forEach(function(f){d.unshift(f)})},\n-1692381986,function(d,e){e=(e%d.length+d.length)%d.length;var f=d[0];d[0]=d[e];d[e]=f},\n-262444939,\"unshift\",function(d){for(var e=d.length;e;)d.push(d.splice(--e,1)[0])},\n1201502951,-546377604,-504264123,-1978377336,1042456724,function(d,e){for(e=(e%d.length+d.length)%d.length;e--;)d.unshift(d.pop())},\n711986897,406699922,-1842537993,-1678108293,1803491779,1671716087,12778705,-718839990,null,null,-1617525823,342523552,-1338406651,-399705108,-696713950,b,function(d,e){e=(e%d.length+d.length)%d.length;d.splice(0,1,d.splice(e,1,d[0])[0])},\nfunction(d,e){e=(e%d.length+d.length)%d.length;d.splice(e,1)},\n-980602034,356396192,null,-1617525823,function(d,e,f){var h=f.length;d.forEach(function(l,m,n){this.push(n[m]=f[(f.indexOf(l)-f.indexOf(this[m])+m+h--)%f.length])},e.split(\"\"))},\n-1029864222,-641353250,-1681901809,-1391247867,1707415199,-1957855835,b,function(){for(var d=64,e=[];++d-e.length-32;)switch(d){case 58:d=96;continue;case 91:d=44;break;case 65:d=47;continue;case 46:d=153;case 123:d-=58;default:e.push(String.fromCharCode(d))}return e},\n-1936558978,-1505243983,function(d){d.reverse()},\n1296889058,-1813915420,-943019300,function(d,e,f){var h=f.length;d.forEach(function(l,m,n){this.push(n[m]=f[(f.indexOf(l)-f.indexOf(this[m])+m+h--)%f.length])},e.split(\"\"))},\n\"join\",b,-2061642263];c[21]=c;c[22]=c;c[33]=c;try{c[3](c[33],c[9]),c[29](c[22],c[25]),c[29](c[22],c[19]),c[29](c[33],c[17]),c[29](c[21],c[2]),c[29](c[42],c[10]),c[1](c[52],c[40]),c[12](c[28],c[8]),c[29](c[21],c[45]),c[1](c[21],c[48]),c[44](c[26]),c[39](c[5],c[2]),c[31](c[53],c[16]),c[30](c[29],c[8]),c[51](c[29],c[6],c[44]()),c[4](c[43],c[1]),c[2](c[23],c[42]),c[2](c[0],c[46]),c[38](c[14],c[52]),c[32](c[5]),c[26](c[29],c[46]),c[26](c[5],c[13]),c[28](c[1],c[37]),c[26](c[31],c[13]),c[26](c[1],c[34]),\nc[46](c[1],c[32],c[40]()),c[26](c[50],c[44]),c[17](c[50],c[51]),c[0](c[3],c[24]),c[32](c[13]),c[43](c[3],c[51]),c[0](c[34],c[17]),c[16](c[45],c[53]),c[29](c[44],c[13]),c[42](c[1],c[50]),c[47](c[22],c[53]),c[37](c[22]),c[13](c[52],c[21]),c[6](c[43],c[34]),c[6](c[31],c[46])}catch(d){return\"enhanced_except_gZYB_un-_w8_\"+a}return b.join(\"\")};function deobfuscate(a){return Ska(a);}".to_owned(),
        sts: "19201".to_owned(),
    })
    });

    #[rstest]
    #[case::desktop("desktop")]
    #[case::desktop_music("desktopmusic")]
    #[case::tv_html5_embed("tvhtml5embed")]
    #[case::android("android")]
    #[case::ios("ios")]
    fn map_player_data(#[case] name: &str) {
        let json_path = path!("testfiles" / "player" / format!("{}_video.json", name));
        let json_file = File::open(json_path).unwrap();

        let resp: response::Player = serde_json::from_reader(BufReader::new(json_file)).unwrap();
        let map_res = resp
            .map_response("pPvd8UxmSbQ", Language::En, Some(&DEOBFUSCATOR))
            .unwrap();

        assert!(
            map_res.warnings.is_empty(),
            "deserialization/mapping warnings: {:?}",
            map_res.warnings
        );
        let is_desktop = name == "desktop" || name == "desktopmusic";
        insta::assert_ron_snapshot!(format!("map_player_data_{}", name), map_res.c, {
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
        let mut last_nsig: [String; 2] = ["".to_owned(), "".to_owned()];
        let map_res = map_url(
            &None,
            &Some(signature_cipher.to_owned()),
            &DEOBFUSCATOR,
            &mut last_nsig,
        );
        let (url, throttled) = map_res.c.unwrap();

        assert_eq!(url, "https://rr5---sn-h0jelnez.googlevideo.com/videoplayback?c=WEB&clen=3781277&dur=229.301&ei=vb7nYvH5BMK8gAfBj7ToBQ&expire=1659376413&fexp=24001373%2C24007246&fvip=5&gir=yes&id=o-AB_BABwrXZJN428ZwDxq5ScPn2AbcGODnRlTVhCQ3mj2&initcwndbps=1588750&ip=2003%3Ade%3Aaf06%3A6300%3Ac750%3A1b77%3Ac74a%3A80e3&itag=251&keepalive=yes&lmt=1655510291473933&lsig=AG3C_xAwRQIgCKCGJ1iu4wlaGXy3jcJyU3inh9dr1FIfqYOZEG_MdmACIQCbungkQYFk7EhD6K2YvLaHFMjKOFWjw001_tLb0lPDtg%3D%3D&lsparams=mh%2Cmm%2Cmn%2Cms%2Cmv%2Cmvi%2Cpl%2Cinitcwndbps&mh=hH&mime=audio%2Fwebm&mm=31%2C26&mn=sn-h0jelnez%2Csn-4g5ednsl&ms=au%2Conr&mt=1659354538&mv=m&mvi=5&n=XzXGSfGusw6OCQ&ns=b_Mq_qlTFcSGlG9RpwpM9xQH&pl=37&rbqsm=fr&requiressl=yes&sig=AOq0QJ8wRQIhAPIsKd7-xi4xVHEC9gb__dU4hzfzsHEj9ytd3nt0gEceAiACJWBcw-wFEq9qir35bwKHJZxtQ9mOL7SKiVkLQNDa6A%3D%3D&source=youtube&sparams=expire%2Cei%2Cip%2Cid%2Citag%2Csource%2Crequiressl%2Cspc%2Cvprv%2Cmime%2Cns%2Cgir%2Cclen%2Cdur%2Clmt&spc=lT-Khi831z8dTejFIRCvCEwx_6romtM&txp=4532434&vprv=1");
        assert_eq!(throttled, false);
        assert!(
            map_res.warnings.is_empty(),
            "deserialization/mapping warnings: {:?}",
            map_res.warnings
        );
    }
}
