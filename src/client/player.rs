use std::{
    borrow::Cow,
    cmp::Ordering,
    collections::{BTreeMap, HashMap},
};

use anyhow::{anyhow, bail, Context, Result};
use fancy_regex::Regex;
use log::error;
use once_cell::sync::Lazy;
use reqwest::Method;
use serde::Serialize;
use url::Url;

use super::{
    response::{self, Player},
    ClientType, ContextYT, RustyTube,
};
use crate::{
    client::response::player,
    deobfuscate::Deobfuscator,
    model::{AudioCodec, AudioStream, PlayerData, Thumbnail, VideoCodec, VideoInfo, VideoStream},
    util,
};

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
    pub async fn fetch_player(
        &self,
        video_id: &str,
        client_type: ClientType,
    ) -> Result<PlayerData> {
        let client = self.get_ytclient(client_type);
        let (context, deobf) = tokio::join!(
            client.get_context(false),
            Deobfuscator::from_fetched_info(client.http_client(), self.cache.clone())
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
        };

        let resp = client
            .request_builder(Method::POST, "player")
            .await
            .json(&request_body)
            .send()
            .await?;

        // println!("{}", resp.text().await?);
        // todo!();
        let player_response = resp.json::<response::Player>().await?;
        map_player_data(player_response, &deobf)
    }
}

fn cipher_to_url(
    signature_cipher: &str,
    deobf: &Deobfuscator,
    last_nsig: &mut [String; 2],
) -> Result<String> {
    let params: HashMap<Cow<str>, Cow<str>> =
        url::form_urlencoded::parse(signature_cipher.as_bytes()).collect();

    // Parameters:
    // `s`: Obfuscated signature
    // `sp`: Signature parameter
    // `url`: URL that is missing the signature parameter

    let sig = some_or_bail!(params.get("s"), Err(anyhow!("no s param")));
    let sp = some_or_bail!(params.get("sp"), Err(anyhow!("no sp param")));
    let raw_url = some_or_bail!(params.get("url"), Err(anyhow!("no url param")));
    let parsed_url = Url::parse(raw_url)?;

    // println!("sig: {}", sig);
    let deobf_sig = deobf.deobfuscate_sig(sig)?;

    // Use BTreeMap to get reproducible ordering
    let mut url_params: BTreeMap<Cow<str>, Cow<str>> = parsed_url.query_pairs().collect();
    url_params.insert(Cow::Borrowed(sp), Cow::Borrowed(&deobf_sig));

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

            url_params.insert(Cow::Borrowed("n"), Cow::Borrowed(&nsig));
        }
        None => {}
    }

    let mut url_base = parsed_url.clone();
    url_base.set_query(None);
    let new_url = Url::parse_with_params(url_base.as_str(), url_params.iter())?;
    Ok(new_url.to_string())
}

fn map_url(
    f: &player::Format,
    deobf: &Deobfuscator,
    last_nsig: &mut [String; 2],
) -> Option<String> {
    match &f.url {
        Some(url) => Some(url.to_owned()),
        None => match &f.signature_cipher {
            Some(signature_cipher) => match cipher_to_url(signature_cipher, deobf, last_nsig) {
                Ok(url) => Some(url),
                Err(e) => {
                    error!("Could not decrypt signatureCipher: {}", e);
                    None
                }
            },
            None => None,
        },
    }
}

fn map_video_stream(
    f: &player::Format,
    deobf: &Deobfuscator,
    last_nsig: &mut [String; 2],
) -> Option<VideoStream> {
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
        codec: get_video_codec(&f.mime_type),
    })
}

fn map_audio_stream(
    f: &player::Format,
    deobf: &Deobfuscator,
    last_nsig: &mut [String; 2],
) -> Option<AudioStream> {
    Some(AudioStream {
        url: some_or_bail!(map_url(f, deobf, last_nsig), None),
        itag: f.itag,
        bitrate: f.bitrate,
        average_bitrate: f.average_bitrate,
        size: f.content_length,
        index_range: f.index_range.to_owned(),
        init_range: f.init_range.to_owned(),
        mime: f.mime_type.to_owned(),
        codec: get_audio_codec(&f.mime_type),
    })
}

fn codecs_from_mime(mime: &str) -> Vec<&str> {
    static PATTERN: Lazy<Regex> =
        Lazy::new(|| Regex::new(r#"(\w+/\w+);\scodecs="([a-zA-Z-0-9.,\s]*)""#).unwrap());

    let captures = some_or_bail!(PATTERN.captures(&mime).ok().flatten(), vec![]);
    captures
        .get(2)
        .unwrap()
        .as_str()
        .split(", ")
        .collect::<Vec<&str>>()
}

fn get_video_codec(mime: &str) -> VideoCodec {
    for codec in codecs_from_mime(mime) {
        if codec.starts_with("avc1") {
            return VideoCodec::Avc1;
        } else if codec.starts_with("vp9") {
            return VideoCodec::Vp9;
        } else if codec.starts_with("av01") {
            return VideoCodec::Av01;
        }
    }
    VideoCodec::Unknown
}

fn get_audio_codec(mime: &str) -> AudioCodec {
    for codec in codecs_from_mime(mime) {
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

fn map_player_data(response: Player, deobf: &Deobfuscator) -> Result<PlayerData> {
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

        publish_date: microformat.as_ref().map(|m| m.publish_date),
        upload_date: microformat.as_ref().map(|m| m.upload_date),
        view_count: video_details.view_count,
        keywords: video_details.keywords,
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
    audio_streams.sort_by_key(|s| s.average_bitrate);

    Ok(PlayerData {
        info: video_info,
        video_streams,
        video_only_streams,
        audio_streams,
        subtitles: vec![],
        expires_in_seconds: streaming_data.expires_in_seconds,
    })
}

#[cfg(test)]
mod tests {
    use crate::cache::DeobfData;

    use super::*;
    use test_log::test;

    #[test(tokio::test)]
    async fn t_fetch_stream() {
        let rt = RustyTube::new();
        let player_data = rt
            .fetch_player("ZeerrnuLi5E", ClientType::Desktop)
            .await
            .unwrap();

        dbg!(player_data);
    }

    #[test]
    fn t_cipher_to_url() {
        let deobf = Deobfuscator::from(DeobfData {
            js_url: "https://www.youtube.com/s/player/c8b8a173/player_ias.vflset/en_US/base.js".to_owned(),
            sig_fn: "var oB={B4:function(a){a.reverse()},xm:function(a,b){a.splice(0,b)},dC:function(a,b){var c=a[0];a[0]=a[b%a.length];a[b%a.length]=c}};var Vva=function(a){a=a.split(\"\");oB.dC(a,42);oB.xm(a,3);oB.dC(a,48);oB.B4(a,68);return a.join(\"\")};function deobfuscate(a){return Vva(a);}".to_owned(),
            nsig_fn: "Ska=function(a){var b=a.split(\"\"),c=[-1505243983,function(d,e){e=(e%d.length+d.length)%d.length;d.splice(-e).reverse().forEach(function(f){d.unshift(f)})},\n-1692381986,function(d,e){e=(e%d.length+d.length)%d.length;var f=d[0];d[0]=d[e];d[e]=f},\n-262444939,\"unshift\",function(d){for(var e=d.length;e;)d.push(d.splice(--e,1)[0])},\n1201502951,-546377604,-504264123,-1978377336,1042456724,function(d,e){for(e=(e%d.length+d.length)%d.length;e--;)d.unshift(d.pop())},\n711986897,406699922,-1842537993,-1678108293,1803491779,1671716087,12778705,-718839990,null,null,-1617525823,342523552,-1338406651,-399705108,-696713950,b,function(d,e){e=(e%d.length+d.length)%d.length;d.splice(0,1,d.splice(e,1,d[0])[0])},\nfunction(d,e){e=(e%d.length+d.length)%d.length;d.splice(e,1)},\n-980602034,356396192,null,-1617525823,function(d,e,f){var h=f.length;d.forEach(function(l,m,n){this.push(n[m]=f[(f.indexOf(l)-f.indexOf(this[m])+m+h--)%f.length])},e.split(\"\"))},\n-1029864222,-641353250,-1681901809,-1391247867,1707415199,-1957855835,b,function(){for(var d=64,e=[];++d-e.length-32;)switch(d){case 58:d=96;continue;case 91:d=44;break;case 65:d=47;continue;case 46:d=153;case 123:d-=58;default:e.push(String.fromCharCode(d))}return e},\n-1936558978,-1505243983,function(d){d.reverse()},\n1296889058,-1813915420,-943019300,function(d,e,f){var h=f.length;d.forEach(function(l,m,n){this.push(n[m]=f[(f.indexOf(l)-f.indexOf(this[m])+m+h--)%f.length])},e.split(\"\"))},\n\"join\",b,-2061642263];c[21]=c;c[22]=c;c[33]=c;try{c[3](c[33],c[9]),c[29](c[22],c[25]),c[29](c[22],c[19]),c[29](c[33],c[17]),c[29](c[21],c[2]),c[29](c[42],c[10]),c[1](c[52],c[40]),c[12](c[28],c[8]),c[29](c[21],c[45]),c[1](c[21],c[48]),c[44](c[26]),c[39](c[5],c[2]),c[31](c[53],c[16]),c[30](c[29],c[8]),c[51](c[29],c[6],c[44]()),c[4](c[43],c[1]),c[2](c[23],c[42]),c[2](c[0],c[46]),c[38](c[14],c[52]),c[32](c[5]),c[26](c[29],c[46]),c[26](c[5],c[13]),c[28](c[1],c[37]),c[26](c[31],c[13]),c[26](c[1],c[34]),\nc[46](c[1],c[32],c[40]()),c[26](c[50],c[44]),c[17](c[50],c[51]),c[0](c[3],c[24]),c[32](c[13]),c[43](c[3],c[51]),c[0](c[34],c[17]),c[16](c[45],c[53]),c[29](c[44],c[13]),c[42](c[1],c[50]),c[47](c[22],c[53]),c[37](c[22]),c[13](c[52],c[21]),c[6](c[43],c[34]),c[6](c[31],c[46])}catch(d){return\"enhanced_except_gZYB_un-_w8_\"+a}return b.join(\"\")};function deobfuscate(a){return Ska(a);}".to_owned(),
            sts: "19201".to_owned(),
        });

        let signature_cipher = "s=w%3DAe%3DA6aDNQLkViKS7LOm9QtxZJHKwb53riq9qEFw-ecBWJCAiA%3DcEg0tn3dty9jEHszfzh4Ud__bg9CEHVx4ix-7dKsIPAhIQRw8JQ0qOA&sp=sig&url=https://rr5---sn-h0jelnez.googlevideo.com/videoplayback%3Fexpire%3D1659376413%26ei%3Dvb7nYvH5BMK8gAfBj7ToBQ%26ip%3D2003%253Ade%253Aaf06%253A6300%253Ac750%253A1b77%253Ac74a%253A80e3%26id%3Do-AB_BABwrXZJN428ZwDxq5ScPn2AbcGODnRlTVhCQ3mj2%26itag%3D251%26source%3Dyoutube%26requiressl%3Dyes%26mh%3DhH%26mm%3D31%252C26%26mn%3Dsn-h0jelnez%252Csn-4g5ednsl%26ms%3Dau%252Conr%26mv%3Dm%26mvi%3D5%26pl%3D37%26initcwndbps%3D1588750%26spc%3DlT-Khi831z8dTejFIRCvCEwx_6romtM%26vprv%3D1%26mime%3Daudio%252Fwebm%26ns%3Db_Mq_qlTFcSGlG9RpwpM9xQH%26gir%3Dyes%26clen%3D3781277%26dur%3D229.301%26lmt%3D1655510291473933%26mt%3D1659354538%26fvip%3D5%26keepalive%3Dyes%26fexp%3D24001373%252C24007246%26c%3DWEB%26rbqsm%3Dfr%26txp%3D4532434%26n%3Dd2g6G2hVqWIXxedQ%26sparams%3Dexpire%252Cei%252Cip%252Cid%252Citag%252Csource%252Crequiressl%252Cspc%252Cvprv%252Cmime%252Cns%252Cgir%252Cclen%252Cdur%252Clmt%26lsparams%3Dmh%252Cmm%252Cmn%252Cms%252Cmv%252Cmvi%252Cpl%252Cinitcwndbps%26lsig%3DAG3C_xAwRQIgCKCGJ1iu4wlaGXy3jcJyU3inh9dr1FIfqYOZEG_MdmACIQCbungkQYFk7EhD6K2YvLaHFMjKOFWjw001_tLb0lPDtg%253D%253D";
        let url = cipher_to_url(
            signature_cipher,
            &deobf,
            &mut ["".to_string(), "".to_string()],
        )
        .unwrap();
        assert_eq!(url, "https://rr5---sn-h0jelnez.googlevideo.com/videoplayback?c=WEB&clen=3781277&dur=229.301&ei=vb7nYvH5BMK8gAfBj7ToBQ&expire=1659376413&fexp=24001373%2C24007246&fvip=5&gir=yes&id=o-AB_BABwrXZJN428ZwDxq5ScPn2AbcGODnRlTVhCQ3mj2&initcwndbps=1588750&ip=2003%3Ade%3Aaf06%3A6300%3Ac750%3A1b77%3Ac74a%3A80e3&itag=251&keepalive=yes&lmt=1655510291473933&lsig=AG3C_xAwRQIgCKCGJ1iu4wlaGXy3jcJyU3inh9dr1FIfqYOZEG_MdmACIQCbungkQYFk7EhD6K2YvLaHFMjKOFWjw001_tLb0lPDtg%3D%3D&lsparams=mh%2Cmm%2Cmn%2Cms%2Cmv%2Cmvi%2Cpl%2Cinitcwndbps&mh=hH&mime=audio%2Fwebm&mm=31%2C26&mn=sn-h0jelnez%2Csn-4g5ednsl&ms=au%2Conr&mt=1659354538&mv=m&mvi=5&n=XzXGSfGusw6OCQ&ns=b_Mq_qlTFcSGlG9RpwpM9xQH&pl=37&rbqsm=fr&requiressl=yes&sig=AOq0QJ8wRQIhAPIsKd7-xi4xVHEC9gb__dU4hzfzsHEj9ytd3nt0gEceAiACJWBcw-wFEq9qir35bwKHJZxtQ9mOL7SKiVkLQNDa6A%3D%3D&source=youtube&sparams=expire%2Cei%2Cip%2Cid%2Citag%2Csource%2Crequiressl%2Cspc%2Cvprv%2Cmime%2Cns%2Cgir%2Cclen%2Cdur%2Clmt&spc=lT-Khi831z8dTejFIRCvCEwx_6romtM&txp=4532434&vprv=1");
    }
}
