use std::{
    borrow::Cow,
    cmp::Ordering,
    collections::{BTreeMap, HashMap},
    sync::Arc,
};

use anyhow::{anyhow, bail, Result};
use chrono::{DateTime, NaiveDateTime, NaiveTime, Utc};
use fancy_regex::Regex;
use log::error;
use once_cell::sync::Lazy;
use reqwest::Method;
use serde::Serialize;
use url::Url;

use super::{response, ClientType, ContextYT, RustyTube, YTClient};
use crate::{client::response::player, deobfuscate::Deobfuscator, model::*, util};

// REQUEST

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct QPlayer {
    context: ContextYT,
    /// Website playback context
    #[serde(skip_serializing_if = "Option::is_none")]
    playback_context: Option<QPlaybackContext>,
    /// Content playback nonce (mobile only, 16 random chars)
    #[serde(skip_serializing_if = "Option::is_none")]
    cpn: Option<String>,
    /// YouTube video ID
    video_id: String,
    /// Set to true to allow extraction of streams with sensitive content
    content_check_ok: bool,
    /// Probably refers to allowing sensitive content, too
    racy_check_ok: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct QPlaybackContext {
    content_playback_context: QContentPlaybackContext,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct QContentPlaybackContext {
    /// Signature timestamp extracted from player.js
    signature_timestamp: String,
    /// Referer URL from website
    referer: String,
}

impl RustyTube {
    pub async fn get_player(&self, video_id: &str, client_type: ClientType) -> Result<PlayerData> {
        let client = self.get_ytclient(client_type);
        let (context, deobf) = tokio::join!(
            client.get_context(false),
            Deobfuscator::from_fetched_info(client.http_client(), self.cache.clone())
        );
        let deobf = deobf?;
        let request_body = build_request_body(client.clone(), &deobf, context, video_id);

        let resp = client
            .request_builder(Method::POST, "player")
            .await
            .json(&request_body)
            .send()
            .await?
            .error_for_status()?;

        let player_response = resp.json::<response::Player>().await?;
        map_player_data(player_response, &deobf)
    }
}

fn build_request_body(
    client: Arc<dyn YTClient>,
    deobf: &Deobfuscator,
    context: ContextYT,
    video_id: &str,
) -> QPlayer {
    if client.get_type().is_web() {
        QPlayer {
            context,
            playback_context: Some(QPlaybackContext {
                content_playback_context: QContentPlaybackContext {
                    signature_timestamp: deobf.get_sts(),
                    referer: format!("https://www.youtube.com/watch?v={}", video_id),
                },
            }),
            cpn: None,
            video_id: video_id.to_owned(),
            content_check_ok: true,
            racy_check_ok: true,
        }
    } else {
        QPlayer {
            context,
            playback_context: None,
            cpn: Some(util::generate_content_playback_nonce()),
            video_id: video_id.to_owned(),
            content_check_ok: true,
            racy_check_ok: true,
        }
    }
}

fn url_to_params(url: &str) -> Result<(String, BTreeMap<String, String>)> {
    let parsed_url = Url::parse(url)?;
    let url_params: BTreeMap<String, String> = parsed_url
        .query_pairs()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();

    let mut url_base = parsed_url.clone();
    url_base.set_query(None);

    Ok((url_base.to_string(), url_params))
}

fn cipher_to_url_params(
    signature_cipher: &str,
    deobf: &Deobfuscator,
) -> Result<(String, BTreeMap<String, String>)> {
    let params: HashMap<Cow<str>, Cow<str>> =
        url::form_urlencoded::parse(signature_cipher.as_bytes()).collect();

    // Parameters:
    // `s`: Obfuscated signature
    // `sp`: Signature parameter
    // `url`: URL that is missing the signature parameter

    let sig = some_or_bail!(params.get("s"), Err(anyhow!("no s param")));
    let sp = some_or_bail!(params.get("sp"), Err(anyhow!("no sp param")));
    let raw_url = some_or_bail!(params.get("url"), Err(anyhow!("no url param")));
    let (url_base, mut url_params) = url_to_params(raw_url)?;

    // println!("sig: {}", sig);
    let deobf_sig = deobf.deobfuscate_sig(sig)?;
    url_params.insert(sp.to_string(), deobf_sig);

    Ok((url_base, url_params))
}

fn deobf_nsig(
    url_params: &mut BTreeMap<String, String>,
    deobf: &Deobfuscator,
    last_nsig: &mut [String; 2],
) -> Result<()> {
    let nsig: String;
    match url_params.get("n") {
        Some(n) => {
            // println!("n: {}", n);

            nsig = if n.to_owned() == last_nsig[0] {
                last_nsig[1].to_owned()
            } else {
                let nsig = deobf.deobfuscate_nsig(n)?;
                last_nsig[0] = n.to_string();
                last_nsig[1] = nsig.to_owned();
                nsig
            };

            url_params.insert("n".to_owned(), nsig);
        }
        None => {}
    };
    Ok(())
}

fn map_url(
    f: &player::Format,
    deobf: &Deobfuscator,
    last_nsig: &mut [String; 2],
) -> Option<String> {
    let (url_base, mut url_params) = match &f.url {
        Some(url) => ok_or_bail!(url_to_params(url), None),
        None => match &f.signature_cipher {
            Some(signature_cipher) => match cipher_to_url_params(signature_cipher, deobf) {
                Ok(res) => res,
                Err(e) => {
                    error!("Could not deobfuscate signatureCipher: {}", e);
                    return None;
                }
            },
            None => return None,
        },
    };

    match deobf_nsig(&mut url_params, deobf, last_nsig) {
        Ok(_) => Some(
            ok_or_bail!(
                Url::parse_with_params(url_base.as_str(), url_params.iter()),
                None
            )
            .to_string(),
        ),
        Err(e) => {
            error!("Could not deobfuscate nsig: {}", e);
            None
        }
    }
}

fn map_video_stream(
    f: &player::Format,
    deobf: &Deobfuscator,
    last_nsig: &mut [String; 2],
) -> Option<VideoStream> {
    let (mtype, codecs) = some_or_bail!(parse_mime(&f.mime_type), None);

    Some(VideoStream {
        url: some_or_bail!(map_url(f, deobf, last_nsig), None),
        itag: f.itag,
        bitrate: f.bitrate,
        average_bitrate: f.average_bitrate,
        size: f.content_length,
        index_range: f.index_range.clone(),
        init_range: f.init_range.clone(),
        width: some_or_bail!(f.width, None),
        height: some_or_bail!(f.height, None),
        fps: some_or_bail!(f.fps, None),
        quality: some_or_bail!(f.quality_label.clone(), None),
        hdr: f.color_info.clone().unwrap_or_default().primaries
            == player::Primaries::ColorPrimariesBt2020,
        mime: f.mime_type.to_owned(),
        format: some_or_bail!(get_video_format(mtype), None),
        codec: get_video_codec(codecs),
    })
}

fn map_audio_stream(
    f: &player::Format,
    deobf: &Deobfuscator,
    last_nsig: &mut [String; 2],
) -> Option<AudioStream> {
    let (mtype, codecs) = some_or_bail!(parse_mime(&f.mime_type), None);

    Some(AudioStream {
        url: some_or_bail!(map_url(f, deobf, last_nsig), None),
        itag: f.itag,
        bitrate: f.bitrate,
        average_bitrate: f.average_bitrate,
        size: f.content_length,
        index_range: f.index_range.to_owned(),
        init_range: f.init_range.to_owned(),
        mime: f.mime_type.to_owned(),
        format: some_or_bail!(get_audio_format(mtype), None),
        codec: get_audio_codec(codecs),
    })
}

fn parse_mime(mime: &str) -> Option<(&str, Vec<&str>)> {
    static PATTERN: Lazy<Regex> =
        Lazy::new(|| Regex::new(r#"(\w+/\w+);\scodecs="([a-zA-Z-0-9.,\s]*)""#).unwrap());

    let captures = some_or_bail!(PATTERN.captures(&mime).ok().flatten(), None);
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

fn cmp_video_streams(a: &VideoStream, b: &VideoStream) -> Ordering {
    match (a.width * a.height).cmp(&(b.width * b.height)) {
        Ordering::Less => Ordering::Less,
        Ordering::Greater => Ordering::Greater,
        Ordering::Equal => match a.codec.cmp(&b.codec) {
            Ordering::Less => Ordering::Less,
            Ordering::Greater => Ordering::Greater,
            Ordering::Equal => a.average_bitrate.cmp(&b.average_bitrate),
        },
    }
}

fn cmp_audio_streams(a: &AudioStream, b: &AudioStream) -> Ordering {
    fn cmp_bitrate(s: &AudioStream) -> u32 {
        match s.codec {
            // Opus is more efficient
            AudioCodec::Opus => (s.average_bitrate as f32 * 1.3) as u32,
            _ => s.average_bitrate,
        }
    }

    cmp_bitrate(a).cmp(&cmp_bitrate(b))
}

fn map_player_data(response: response::Player, deobf: &Deobfuscator) -> Result<PlayerData> {
    // Check playability status
    match response.playability_status {
        response::player::PlayabilityStatus::Ok { live_streamability } => {
            if live_streamability.is_some() {
                bail!("Active livestreams are not supported")
            }
        }
        response::player::PlayabilityStatus::Unplayable { reason } => {
            bail!("Video is unplayable. Reason: {}", reason)
        }
        response::player::PlayabilityStatus::LoginRequired { reason } => {
            bail!("Playback requires login. Reason: {}", reason)
        }
        response::player::PlayabilityStatus::LiveStreamOffline { reason } => {
            bail!("Livestream is offline. Reason: {}", reason)
        }
        response::player::PlayabilityStatus::Error { reason } => {
            bail!("Video was deleted. Reason: {}", reason)
        }
    };

    let streaming_data = some_or_bail!(
        response.streaming_data,
        Err(anyhow!("No streaming data was returned"))
    );
    let video_details = some_or_bail!(
        response.video_details,
        Err(anyhow!("No video details were returned"))
    );
    let microformat = response.microformat.map(|m| m.player_microformat_renderer);

    let video_info = VideoInfo {
        id: video_details.video_id,
        title: video_details.title,
        description: video_details.short_description,
        length: video_details.length_seconds,
        thumbnails: video_details
            .thumbnail
            .unwrap_or_default()
            .thumbnails
            .iter()
            .map(|t| Thumbnail {
                url: t.url.to_owned(),
                height: t.height,
                width: t.width,
            })
            .collect(),

        channel_id: video_details.channel_id,
        channel_name: video_details.author,

        publish_date: microformat.as_ref().map(|m| {
            let ndt = NaiveDateTime::new(m.publish_date, NaiveTime::from_hms(0, 0, 0));
            DateTime::from_utc(ndt, Utc)
        }),
        view_count: video_details.view_count,
        keywords: video_details
            .keywords
            .or_else(|| microformat.as_ref().map_or(None, |mf| mf.tags.clone()))
            .unwrap_or_default(),
        category: microformat.as_ref().map(|m| m.category.to_owned()),
        is_live_content: video_details.is_live_content,
        is_family_safe: microformat.as_ref().map(|m| m.is_family_safe),
    };

    let mut formats = streaming_data.formats.clone();
    formats.append(&mut streaming_data.adaptive_formats.clone());

    let mut last_nsig: [String; 2] = ["".to_owned(), "".to_owned()];

    let mut video_streams: Vec<VideoStream> = Vec::new();
    let mut video_only_streams: Vec<VideoStream> = Vec::new();
    let mut audio_streams: Vec<AudioStream> = Vec::new();

    for f in formats {
        if f.format_type == player::FormatType::FormatStreamTypeOtf {
            continue;
        }

        match (f.is_video(), f.is_audio()) {
            (true, true) => match map_video_stream(&f, deobf, &mut last_nsig) {
                Some(stream) => video_streams.push(stream),
                None => {}
            },
            (true, false) => match map_video_stream(&f, deobf, &mut last_nsig) {
                Some(stream) => video_only_streams.push(stream),
                None => {}
            },
            (false, true) => match map_audio_stream(&f, deobf, &mut last_nsig) {
                Some(stream) => audio_streams.push(stream),
                None => {}
            },
            (false, false) => {}
        }
    }

    // Sort streams by quality
    video_streams.sort_by(cmp_video_streams);
    video_only_streams.sort_by(cmp_video_streams);
    audio_streams.sort_by(cmp_audio_streams);

    let subtitles = response.captions.map_or(vec![], |captions| {
        captions
            .player_captions_tracklist_renderer
            .caption_tracks
            .iter()
            .map(|caption| {
                let lang_auto = caption.name.strip_suffix(" (auto-generated)");

                Subtitle {
                    url: caption.base_url.to_owned(),
                    lang: caption.language_code.to_owned(),
                    lang_name: lang_auto.unwrap_or(&caption.name).to_owned(),
                    auto_generated: lang_auto.is_some(),
                }
            })
            .collect()
    });

    Ok(PlayerData {
        info: video_info,
        video_streams,
        video_only_streams,
        audio_streams,
        subtitles,
        expires_in_seconds: streaming_data.expires_in_seconds,
    })
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use crate::{cache::DeobfData, client::CLIENT_TYPES};

    use super::*;
    use rstest::rstest;

    static DEOBFUSCATOR: Lazy<Deobfuscator> = Lazy::new(|| {
        Deobfuscator::from(DeobfData {
        js_url: "https://www.youtube.com/s/player/c8b8a173/player_ias.vflset/en_US/base.js".to_owned(),
        sig_fn: "var oB={B4:function(a){a.reverse()},xm:function(a,b){a.splice(0,b)},dC:function(a,b){var c=a[0];a[0]=a[b%a.length];a[b%a.length]=c}};var Vva=function(a){a=a.split(\"\");oB.dC(a,42);oB.xm(a,3);oB.dC(a,48);oB.B4(a,68);return a.join(\"\")};function deobfuscate(a){return Vva(a);}".to_owned(),
        nsig_fn: "Ska=function(a){var b=a.split(\"\"),c=[-1505243983,function(d,e){e=(e%d.length+d.length)%d.length;d.splice(-e).reverse().forEach(function(f){d.unshift(f)})},\n-1692381986,function(d,e){e=(e%d.length+d.length)%d.length;var f=d[0];d[0]=d[e];d[e]=f},\n-262444939,\"unshift\",function(d){for(var e=d.length;e;)d.push(d.splice(--e,1)[0])},\n1201502951,-546377604,-504264123,-1978377336,1042456724,function(d,e){for(e=(e%d.length+d.length)%d.length;e--;)d.unshift(d.pop())},\n711986897,406699922,-1842537993,-1678108293,1803491779,1671716087,12778705,-718839990,null,null,-1617525823,342523552,-1338406651,-399705108,-696713950,b,function(d,e){e=(e%d.length+d.length)%d.length;d.splice(0,1,d.splice(e,1,d[0])[0])},\nfunction(d,e){e=(e%d.length+d.length)%d.length;d.splice(e,1)},\n-980602034,356396192,null,-1617525823,function(d,e,f){var h=f.length;d.forEach(function(l,m,n){this.push(n[m]=f[(f.indexOf(l)-f.indexOf(this[m])+m+h--)%f.length])},e.split(\"\"))},\n-1029864222,-641353250,-1681901809,-1391247867,1707415199,-1957855835,b,function(){for(var d=64,e=[];++d-e.length-32;)switch(d){case 58:d=96;continue;case 91:d=44;break;case 65:d=47;continue;case 46:d=153;case 123:d-=58;default:e.push(String.fromCharCode(d))}return e},\n-1936558978,-1505243983,function(d){d.reverse()},\n1296889058,-1813915420,-943019300,function(d,e,f){var h=f.length;d.forEach(function(l,m,n){this.push(n[m]=f[(f.indexOf(l)-f.indexOf(this[m])+m+h--)%f.length])},e.split(\"\"))},\n\"join\",b,-2061642263];c[21]=c;c[22]=c;c[33]=c;try{c[3](c[33],c[9]),c[29](c[22],c[25]),c[29](c[22],c[19]),c[29](c[33],c[17]),c[29](c[21],c[2]),c[29](c[42],c[10]),c[1](c[52],c[40]),c[12](c[28],c[8]),c[29](c[21],c[45]),c[1](c[21],c[48]),c[44](c[26]),c[39](c[5],c[2]),c[31](c[53],c[16]),c[30](c[29],c[8]),c[51](c[29],c[6],c[44]()),c[4](c[43],c[1]),c[2](c[23],c[42]),c[2](c[0],c[46]),c[38](c[14],c[52]),c[32](c[5]),c[26](c[29],c[46]),c[26](c[5],c[13]),c[28](c[1],c[37]),c[26](c[31],c[13]),c[26](c[1],c[34]),\nc[46](c[1],c[32],c[40]()),c[26](c[50],c[44]),c[17](c[50],c[51]),c[0](c[3],c[24]),c[32](c[13]),c[43](c[3],c[51]),c[0](c[34],c[17]),c[16](c[45],c[53]),c[29](c[44],c[13]),c[42](c[1],c[50]),c[47](c[22],c[53]),c[37](c[22]),c[13](c[52],c[21]),c[6](c[43],c[34]),c[6](c[31],c[46])}catch(d){return\"enhanced_except_gZYB_un-_w8_\"+a}return b.join(\"\")};function deobfuscate(a){return Ska(a);}".to_owned(),
        sts: "19201".to_owned(),
    })
    });

    #[allow(dead_code)]
    // #[test_log::test(tokio::test)]
    async fn download_testfiles() {
        let tf_dir = Path::new("testfiles/player");
        let video_id = "pPvd8UxmSbQ";

        let rt = RustyTube::new();

        for client_type in CLIENT_TYPES {
            let client = rt.get_ytclient(client_type);
            let context = client.get_context(false).await;

            let request_body = build_request_body(client.clone(), &DEOBFUSCATOR, context, video_id);

            let resp = client
                .request_builder(Method::POST, "player")
                .await
                .json(&request_body)
                .send()
                .await
                .unwrap()
                .error_for_status()
                .unwrap();

            let mut json_path = tf_dir.to_path_buf();
            json_path.push(format!("{:?}_video.json", client_type).to_lowercase());

            let mut file = std::fs::File::create(json_path).unwrap();
            let mut content = std::io::Cursor::new(resp.bytes().await.unwrap());
            std::io::copy(&mut content, &mut file).unwrap();
        }
    }

    #[rstest]
    #[case::desktop("desktop", include_str!("../../testfiles/player/desktop_video.json"))]
    #[case::desktop_music("desktop_music", include_str!("../../testfiles/player/desktopmusic_video.json"))]
    #[case::tv_html5_embed("tvhtml5embed", include_str!("../../testfiles/player/tvhtml5embed_video.json"))]
    #[case::android("android", include_str!("../../testfiles/player/android_video.json"))]
    #[case::ios("ios", include_str!("../../testfiles/player/ios_video.json"))]
    fn t_map_player_data(#[case] name: &str, #[case] json_str: &str) {
        let resp = serde_json::from_str::<response::Player>(json_str).unwrap();
        let player_data = map_player_data(resp, &DEOBFUSCATOR).unwrap();
        insta::assert_yaml_snapshot!(format!("map_player_data_{}", name), player_data)
    }

    #[rstest]
    #[case::desktop(ClientType::Desktop)]
    // #[case::desktop_music(ClientType::DesktopMusic)]
    #[case::tv_html5_embed(ClientType::TvHtml5Embed)]
    #[case::android(ClientType::Android)]
    #[case::ios(ClientType::Ios)]
    #[test_log::test(tokio::test)]
    async fn t_get_player(#[case] client_type: ClientType) {
        let rt = RustyTube::new();
        let player_data = rt.get_player("n4tK7LYFxI0", client_type).await.unwrap();

        // dbg!(player_data.clone());

        assert_eq!(player_data.info.id, "n4tK7LYFxI0");
        assert_eq!(player_data.info.title, "Spektrem - Shine [NCS Release]");
        if client_type == ClientType::DesktopMusic {
            assert!(player_data.info.description.is_none());
        } else {
            assert!(player_data.info.description.unwrap().starts_with(
                "NCS (NoCopyrightSounds): Empowering Creators through Copyright / Royalty Free Music"
            ));
        }
        assert_eq!(player_data.info.length, 259);
        assert!(!player_data.info.thumbnails.is_empty());
        assert_eq!(player_data.info.channel_id, "UC_aEa8K-EOJ3D6gOs7HcyNg");
        assert_eq!(player_data.info.channel_name, "NoCopyrightSounds");
        assert!(player_data.info.view_count > 146818808);
        assert_eq!(player_data.info.keywords[0], "spektrem");
        assert_eq!(player_data.info.is_live_content, false);

        if client_type == ClientType::Desktop || client_type == ClientType::DesktopMusic {
            assert_eq!(
                player_data.info.publish_date.unwrap().to_string(),
                "2013-05-05 00:00:00 UTC"
            );
            assert_eq!(player_data.info.category.unwrap(), "Music");
            assert_eq!(player_data.info.is_family_safe.unwrap(), true);
        }

        if client_type == ClientType::Ios {
            let video = player_data
                .video_only_streams
                .iter()
                .find(|s| s.itag == 247)
                .unwrap();
            let audio = player_data
                .audio_streams
                .iter()
                .find(|s| s.itag == 140)
                .unwrap();

            assert_eq!(video.bitrate, 1507068);
            assert_eq!(video.average_bitrate, 1345149);
            assert_eq!(video.size, 43553412);
            assert_eq!(video.width, 1280);
            assert_eq!(video.height, 720);
            assert_eq!(video.fps, 30);
            assert_eq!(video.quality, "720p");
            assert_eq!(video.hdr, false);
            assert_eq!(video.mime, "video/webm; codecs=\"vp09.00.31.08\"");
            assert_eq!(video.format, VideoFormat::Webm);
            assert_eq!(video.codec, VideoCodec::Vp9);

            assert_eq!(audio.bitrate, 130685);
            assert_eq!(audio.average_bitrate, 129496);
            assert_eq!(audio.size, 4193863);
            assert_eq!(audio.mime, "audio/mp4; codecs=\"mp4a.40.2\"");
            assert_eq!(audio.format, AudioFormat::M4a);
            assert_eq!(audio.codec, AudioCodec::Mp4a);
        } else {
            let video = player_data
                .video_only_streams
                .iter()
                .find(|s| s.itag == 398)
                .unwrap();
            let audio = player_data
                .audio_streams
                .iter()
                .find(|s| s.itag == 251)
                .unwrap();

            assert_eq!(video.bitrate, 1340829);
            assert_eq!(video.average_bitrate, 1233444);
            assert_eq!(video.size, 39936630);
            assert_eq!(video.width, 1280);
            assert_eq!(video.height, 720);
            assert_eq!(video.fps, 30);
            assert_eq!(video.quality, "720p");
            assert_eq!(video.hdr, false);
            assert_eq!(video.mime, "video/mp4; codecs=\"av01.0.05M.08\"");
            assert_eq!(video.format, VideoFormat::Mp4);
            assert_eq!(video.codec, VideoCodec::Av01);

            assert_eq!(audio.bitrate, 142718);
            assert_eq!(audio.average_bitrate, 130708);
            assert_eq!(audio.size, 4232344);
            assert_eq!(audio.mime, "audio/webm; codecs=\"opus\"");
            assert_eq!(audio.format, AudioFormat::Webm);
            assert_eq!(audio.codec, AudioCodec::Opus);
        }

        assert!(player_data.expires_in_seconds > 10000);
    }

    #[test]
    fn t_cipher_to_url() {
        let signature_cipher = "s=w%3DAe%3DA6aDNQLkViKS7LOm9QtxZJHKwb53riq9qEFw-ecBWJCAiA%3DcEg0tn3dty9jEHszfzh4Ud__bg9CEHVx4ix-7dKsIPAhIQRw8JQ0qOA&sp=sig&url=https://rr5---sn-h0jelnez.googlevideo.com/videoplayback%3Fexpire%3D1659376413%26ei%3Dvb7nYvH5BMK8gAfBj7ToBQ%26ip%3D2003%253Ade%253Aaf06%253A6300%253Ac750%253A1b77%253Ac74a%253A80e3%26id%3Do-AB_BABwrXZJN428ZwDxq5ScPn2AbcGODnRlTVhCQ3mj2%26itag%3D251%26source%3Dyoutube%26requiressl%3Dyes%26mh%3DhH%26mm%3D31%252C26%26mn%3Dsn-h0jelnez%252Csn-4g5ednsl%26ms%3Dau%252Conr%26mv%3Dm%26mvi%3D5%26pl%3D37%26initcwndbps%3D1588750%26spc%3DlT-Khi831z8dTejFIRCvCEwx_6romtM%26vprv%3D1%26mime%3Daudio%252Fwebm%26ns%3Db_Mq_qlTFcSGlG9RpwpM9xQH%26gir%3Dyes%26clen%3D3781277%26dur%3D229.301%26lmt%3D1655510291473933%26mt%3D1659354538%26fvip%3D5%26keepalive%3Dyes%26fexp%3D24001373%252C24007246%26c%3DWEB%26rbqsm%3Dfr%26txp%3D4532434%26n%3Dd2g6G2hVqWIXxedQ%26sparams%3Dexpire%252Cei%252Cip%252Cid%252Citag%252Csource%252Crequiressl%252Cspc%252Cvprv%252Cmime%252Cns%252Cgir%252Cclen%252Cdur%252Clmt%26lsparams%3Dmh%252Cmm%252Cmn%252Cms%252Cmv%252Cmvi%252Cpl%252Cinitcwndbps%26lsig%3DAG3C_xAwRQIgCKCGJ1iu4wlaGXy3jcJyU3inh9dr1FIfqYOZEG_MdmACIQCbungkQYFk7EhD6K2YvLaHFMjKOFWjw001_tLb0lPDtg%253D%253D";
        let (url_base, mut url_params) =
            cipher_to_url_params(signature_cipher, &DEOBFUSCATOR).unwrap();
        deobf_nsig(
            &mut url_params,
            &DEOBFUSCATOR,
            &mut ["".to_owned(), "".to_owned()],
        )
        .unwrap();
        let url = Url::parse_with_params(url_base.as_str(), url_params.iter())
            .unwrap()
            .to_string();

        assert_eq!(url, "https://rr5---sn-h0jelnez.googlevideo.com/videoplayback?c=WEB&clen=3781277&dur=229.301&ei=vb7nYvH5BMK8gAfBj7ToBQ&expire=1659376413&fexp=24001373%2C24007246&fvip=5&gir=yes&id=o-AB_BABwrXZJN428ZwDxq5ScPn2AbcGODnRlTVhCQ3mj2&initcwndbps=1588750&ip=2003%3Ade%3Aaf06%3A6300%3Ac750%3A1b77%3Ac74a%3A80e3&itag=251&keepalive=yes&lmt=1655510291473933&lsig=AG3C_xAwRQIgCKCGJ1iu4wlaGXy3jcJyU3inh9dr1FIfqYOZEG_MdmACIQCbungkQYFk7EhD6K2YvLaHFMjKOFWjw001_tLb0lPDtg%3D%3D&lsparams=mh%2Cmm%2Cmn%2Cms%2Cmv%2Cmvi%2Cpl%2Cinitcwndbps&mh=hH&mime=audio%2Fwebm&mm=31%2C26&mn=sn-h0jelnez%2Csn-4g5ednsl&ms=au%2Conr&mt=1659354538&mv=m&mvi=5&n=XzXGSfGusw6OCQ&ns=b_Mq_qlTFcSGlG9RpwpM9xQH&pl=37&rbqsm=fr&requiressl=yes&sig=AOq0QJ8wRQIhAPIsKd7-xi4xVHEC9gb__dU4hzfzsHEj9ytd3nt0gEceAiACJWBcw-wFEq9qir35bwKHJZxtQ9mOL7SKiVkLQNDa6A%3D%3D&source=youtube&sparams=expire%2Cei%2Cip%2Cid%2Citag%2Csource%2Crequiressl%2Cspc%2Cvprv%2Cmime%2Cns%2Cgir%2Cclen%2Cdur%2Clmt&spc=lT-Khi831z8dTejFIRCvCEwx_6romtM&txp=4532434&vprv=1");
    }
}
