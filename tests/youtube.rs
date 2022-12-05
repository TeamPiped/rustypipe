use std::collections::HashSet;
use std::fmt::Display;

use fancy_regex::Regex;
use once_cell::sync::Lazy;
use rstest::rstest;
use time::macros::date;
use time::OffsetDateTime;

use rustypipe::client::{ClientType, RustyPipe, RustyPipeQuery};
use rustypipe::error::{Error, ExtractionError};
use rustypipe::model::{
    richtext::ToPlaintext, AlbumType, AudioCodec, AudioFormat, Channel, FromYtItem,
    MusicEntityType, MusicGenre, Paginator, UrlTarget, Verification, VideoCodec, VideoFormat,
    YouTubeItem, YtStream,
};
use rustypipe::param::{
    search_filter::{self, SearchFilter},
    Country,
};

//#PLAYER

#[rstest]
#[case::desktop(ClientType::Desktop)]
#[case::tv_html5_embed(ClientType::TvHtml5Embed)]
#[case::android(ClientType::Android)]
#[case::ios(ClientType::Ios)]
#[tokio::test]
async fn get_player_from_client(#[case] client_type: ClientType) {
    let rp = RustyPipe::builder().strict().build();
    let player_data = rp
        .query()
        .player_from_client("n4tK7LYFxI0", client_type)
        .await
        .unwrap();

    // dbg!(&player_data);

    assert_eq!(player_data.details.id, "n4tK7LYFxI0");
    assert_eq!(player_data.details.title, "Spektrem - Shine [NCS Release]");
    if client_type == ClientType::DesktopMusic {
        assert!(player_data.details.description.is_none());
    } else {
        assert!(player_data.details.description.unwrap().starts_with(
            "NCS (NoCopyrightSounds): Empowering Creators through Copyright / Royalty Free Music"
        ));
    }
    assert_eq!(player_data.details.length, 259);
    assert!(!player_data.details.thumbnail.is_empty());
    assert_eq!(player_data.details.channel.id, "UC_aEa8K-EOJ3D6gOs7HcyNg");
    assert_eq!(player_data.details.channel.name, "NoCopyrightSounds");
    assert_gte(player_data.details.view_count, 146_818_808, "view count");
    assert_eq!(player_data.details.keywords[0], "spektrem");
    assert_eq!(player_data.details.is_live_content, false);

    if client_type == ClientType::Ios {
        let video = player_data
            .video_only_streams
            .into_iter()
            .find(|s| s.itag == 247)
            .unwrap();
        let audio = player_data
            .audio_streams
            .into_iter()
            .find(|s| s.itag == 140)
            .unwrap();

        // Bitrates may change between requests
        assert_approx(video.bitrate as f64, 1507068.0);
        assert_eq!(video.average_bitrate, 1345149);
        assert_eq!(video.size.unwrap(), 43553412);
        assert_eq!(video.width, 1280);
        assert_eq!(video.height, 720);
        assert_eq!(video.fps, 30);
        assert_eq!(video.quality, "720p");
        assert_eq!(video.hdr, false);
        assert_eq!(video.mime, "video/webm; codecs=\"vp09.00.31.08\"");
        assert_eq!(video.format, VideoFormat::Webm);
        assert_eq!(video.codec, VideoCodec::Vp9);

        assert_approx(audio.bitrate as f64, 130685.0);
        assert_approx(audio.average_bitrate as f64, 129496.0);
        assert_approx(audio.size as f64, 4193863.0);
        assert_eq!(audio.mime, "audio/mp4; codecs=\"mp4a.40.2\"");
        assert_eq!(audio.format, AudioFormat::M4a);
        assert_eq!(audio.codec, AudioCodec::Mp4a);

        check_video_stream(video).await;
        check_video_stream(audio).await;
    } else {
        let video = player_data
            .video_only_streams
            .into_iter()
            .find(|s| s.itag == 398)
            .unwrap();
        let audio = player_data
            .audio_streams
            .into_iter()
            .find(|s| s.itag == 251)
            .unwrap();

        assert_approx(video.bitrate as f64, 1340829.0);
        assert_approx(video.average_bitrate as f64, 1233444.0);
        assert_approx(video.size.unwrap() as f64, 39936630.0);
        assert_eq!(video.width, 1280);
        assert_eq!(video.height, 720);
        assert_eq!(video.fps, 30);
        assert_eq!(video.quality, "720p");
        assert_eq!(video.hdr, false);
        assert_eq!(video.mime, "video/mp4; codecs=\"av01.0.05M.08\"");
        assert_eq!(video.format, VideoFormat::Mp4);
        assert_eq!(video.codec, VideoCodec::Av01);
        assert_eq!(video.throttled, false);

        assert_approx(audio.bitrate as f64, 142718.0);
        assert_approx(audio.average_bitrate as f64, 130708.0);
        assert_approx(audio.size as f64, 4232344.0);
        assert_eq!(audio.mime, "audio/webm; codecs=\"opus\"");
        assert_eq!(audio.format, AudioFormat::Webm);
        assert_eq!(audio.codec, AudioCodec::Opus);
        assert_eq!(audio.throttled, false);

        check_video_stream(video).await;
        check_video_stream(audio).await;
    }

    assert!(player_data.expires_in_seconds > 10000);
}

/// Request the given stream to check if it returns a valid response
async fn check_video_stream(s: impl YtStream) {
    let http = reqwest::Client::new();

    let resp = http
        .get(s.url())
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();

    if let Some(size) = s.size() {
        assert_eq!(resp.content_length().unwrap(), size)
    }
}

#[rstest]
#[case::music(
    "ihUZMeYFZHA",
    "Oonagh - Nan Úye",
    "Offizielle AlbumPlaylist:",
    260,
    "UC2llNlEM62gU-_fXPHfgbDg",
    "Oonagh",
    830900,
    false,
    false
)]
#[case::hdr(
    "LXb3EKWsInQ",
    "COSTA RICA IN 4K 60fps HDR (ULTRA HD)",
    "We've re-mastered and re-uploaded our favorite video in HDR!",
    314,
    "UCYq-iAOSZBvoUxvfzwKIZWA",
    "Jacob + Katie Schwarz",
    220_000_000,
    false,
    false
)]
#[case::multilanguage(
    "tVWWp1PqDus",
    "100 Girls Vs 100 Boys For $500,000",
    "Giving away $25k on Current!",
    1013,
    "UCX6OQ3DkcsbYNE6H8uQQuVA",
    "MrBeast",
    82_000_000,
    false,
    false
)]
#[case::live(
    "86YLFOog4GM",
    "🌎 Nasa Live Stream  - Earth From Space :  Live Views from the ISS",
    "The station is crewed by NASA astronauts as well as Russian Cosmonauts",
    0,
    "UCakgsb0w7QB0VHdnCc-OVEA",
    "Space Videos",
    10,
    true,
    true
)]
#[case::was_live(
    "pxY4OXVyMe4",
    "Minecraft GENESIS LIVESTREAM!!",
    "FÜR MEHR LIVESTREAMS AUF YOUTUBE MACHT FOLGENDES",
    5535,
    "UCQM0bS4_04-Y4JuYrgmnpZQ",
    "Chaosflo44",
    500_000,
    false,
    true
)]
#[case::agelimit(
    "laru0QoJUmI",
    "DJ Robin x Schürze - Layla (Official Video)",
    "Endlich ist es soweit! Zwei Männer aus dem Schwabenland",
    188,
    "UCkJfSrMnLonOZWh-q5os5bg",
    "Summerfield Records",
    10_000_000,
    false,
    false
)]
#[tokio::test]
async fn get_player(
    #[case] id: &str,
    #[case] title: &str,
    #[case] description: &str,
    #[case] length: u32,
    #[case] channel_id: &str,
    #[case] channel_name: &str,
    #[case] views: u64,
    #[case] is_live: bool,
    #[case] is_live_content: bool,
) {
    let rp = RustyPipe::builder().strict().build();
    let player_data = rp.query().player(id).await.unwrap();
    let details = player_data.details;

    assert_eq!(details.id, id);
    assert_eq!(details.title, title);
    let desc = details.description.unwrap();
    assert!(desc.contains(description), "description: {}", desc);
    assert_eq!(details.length, length);
    assert_eq!(details.channel.id, channel_id);
    assert_eq!(details.channel.name, channel_name);
    assert_gte(details.view_count, views, "views");
    assert_eq!(details.is_live, is_live);
    assert_eq!(details.is_live_content, is_live_content);

    if is_live {
        assert!(player_data.hls_manifest_url.is_some());
        assert!(player_data.dash_manifest_url.is_some());
    } else {
        assert!(!player_data.video_only_streams.is_empty());
        assert!(!player_data.audio_streams.is_empty());
    }

    match id {
        // HDR
        "LXb3EKWsInQ" => {
            assert!(
                player_data
                    .video_only_streams
                    .iter()
                    .any(|stream| stream.hdr),
                "no hdr streams"
            );
        }
        // Multilanguage
        "tVWWp1PqDus" => {
            let langs = player_data
                .audio_streams
                .iter()
                .filter_map(|stream| {
                    stream
                        .track
                        .as_ref()
                        .map(|t| t.lang.as_ref().unwrap().to_owned())
                })
                .collect::<HashSet<_>>();

            for l in ["en", "es", "fr", "pt", "ru"] {
                assert!(langs.contains(l), "missing lang: {}", l);
            }
        }
        _ => {}
    };

    assert_gte(player_data.expires_in_seconds, 10_000, "expiry time");
}

#[rstest]
#[case::not_found(
    "86abcdefghi",
    "extraction error: Video cant be played because of deletion/censorship. Reason (from YT): "
)]
#[case::deleted(
    "64DYi_8ESh0",
    "extraction error: Video cant be played because of deletion/censorship. Reason (from YT): "
)]
#[case::censored(
    "6SJNVb0GnPI",
    "extraction error: Video cant be played because of deletion/censorship. Reason (from YT): "
)]
// This video is geoblocked outside of Japan, so expect this test case to fail when using a Japanese IP address.
#[case::geoblock(
    "sJL6WA-aGkQ",
    "extraction error: Video is not available in your country"
)]
#[case::drm(
    "1bfOsni7EgI",
    "extraction error: Video cant be played because of DRM. Reason (from YT): "
)]
#[case::private(
    "s7_qI6_mIXc",
    "extraction error: Video cant be played because of being private. Reason (from YT): "
)]
#[case::age_restricted("CUO8secmc0g", "extraction error: Video is age restricted")]
#[tokio::test]
async fn get_player_error(#[case] id: &str, #[case] msg: &str) {
    let rp = RustyPipe::builder().strict().build();
    let err = rp.query().player(id).await.unwrap_err().to_string();

    assert!(
        err.starts_with(msg),
        "got error msg: `{}`, expected: `{}`",
        err,
        msg
    );
}

//#PLAYLIST

#[rstest]
#[case::long(
    "PL5dDx681T4bR7ZF1IuWzOv1omlRbE7PiJ",
    "Die schönsten deutschen Lieder | Beliebteste Lieder | Beste Deutsche Musik 2022",
    true,
    None,
    Some(("UCIekuFeMaV78xYfvpmoCnPg", "Best Music")),
)]
#[case::short(
    "RDCLAK5uy_kFQXdnqMaQCVx2wpUM4ZfbsGCDibZtkJk",
    "Easy Pop",
    false,
    None,
    None
)]
#[case::nomusic(
    "PL1J-6JOckZtE_P9Xx8D3b2O6w0idhuKBe",
    "Minecraft SHINE",
    false,
    Some("SHINE - Survival Hardcore in New Environment: Auf einem Server machen sich tapfere Spieler auf, mystische Welten zu erkunden, magische Technologien zu erforschen und vorallem zu überleben...".to_owned()),
    Some(("UCQM0bS4_04-Y4JuYrgmnpZQ", "Chaosflo44")),
)]
#[tokio::test]
async fn get_playlist(
    #[case] id: &str,
    #[case] name: &str,
    #[case] is_long: bool,
    #[case] description: Option<String>,
    #[case] channel: Option<(&str, &str)>,
) {
    let rp = RustyPipe::builder().strict().build();
    let playlist = rp.query().playlist(id).await.unwrap();

    assert_eq!(playlist.id, id);
    assert_eq!(playlist.name, name);
    assert!(!playlist.videos.is_empty());
    assert_eq!(!playlist.videos.is_exhausted(), is_long);
    assert!(playlist.video_count > 10);
    assert_eq!(playlist.video_count > 100, is_long);
    assert_eq!(playlist.description, description);

    if let Some(expect) = channel {
        let c = playlist.channel.unwrap();
        assert_eq!(c.id, expect.0);
        assert_eq!(c.name, expect.1);
    }
    assert!(!playlist.thumbnail.is_empty());
}

#[tokio::test]
async fn playlist_cont() {
    let rp = RustyPipe::builder().strict().build();
    let mut playlist = rp
        .query()
        .playlist("PLbZIPy20-1pN7mqjckepWF78ndb6ci_qi")
        .await
        .unwrap();

    playlist
        .videos
        .extend_pages(rp.query(), usize::MAX)
        .await
        .unwrap();
    assert!(playlist.videos.items.len() > 100);
    assert!(playlist.videos.count.unwrap() > 100);
}

#[tokio::test]
async fn playlist_cont2() {
    let rp = RustyPipe::builder().strict().build();
    let mut playlist = rp
        .query()
        .playlist("PLbZIPy20-1pN7mqjckepWF78ndb6ci_qi")
        .await
        .unwrap();

    playlist.videos.extend_limit(rp.query(), 101).await.unwrap();
    assert!(playlist.videos.items.len() > 100);
    assert!(playlist.videos.count.unwrap() > 100);
}

#[tokio::test]
async fn playlist_not_found() {
    let rp = RustyPipe::builder().strict().build();
    let err = rp
        .query()
        .playlist("PLbZIPy20-1pN7mqjckepWF78ndb6ci_qz")
        .await
        .unwrap_err();

    assert!(
        matches!(
            err,
            Error::Extraction(ExtractionError::ContentUnavailable(_))
        ),
        "got: {}",
        err
    );
}

//#VIDEO DETAILS

#[tokio::test]
async fn get_video_details() {
    let rp = RustyPipe::builder().strict().build();
    let details = rp.query().video_details("ZeerrnuLi5E").await.unwrap();

    // dbg!(&details);

    assert_eq!(details.id, "ZeerrnuLi5E");
    assert_eq!(details.title, "aespa 에스파 'Black Mamba' MV");
    let desc = details.description.to_plaintext();
    assert!(
        desc.contains("Listen and download aespa's debut single \"Black Mamba\""),
        "bad description: {}",
        desc
    );

    assert_eq!(details.channel.id, "UCEf_Bc-KVd7onSeifS3py9g");
    assert_eq!(details.channel.name, "SMTOWN");
    assert!(!details.channel.avatar.is_empty(), "no channel avatars");
    assert_eq!(details.channel.verification, Verification::Verified);
    assert_gte(
        details.channel.subscriber_count.unwrap(),
        30_000_000,
        "subscribers",
    );
    assert_gte(details.view_count, 232_000_000, "views");
    assert_gte(details.like_count.unwrap(), 4_000_000, "likes");

    let date = details.publish_date.unwrap();
    assert_eq!(date.date(), date!(2020 - 11 - 17));

    assert!(!details.is_live);
    assert!(!details.is_ccommons);

    assert!(details.recommended.visitor_data.is_some());
    assert_next(details.recommended, rp.query(), 10, 2).await;

    assert_gte(details.top_comments.count.unwrap(), 700_000, "comments");
    assert!(!details.top_comments.is_exhausted());
    assert!(!details.latest_comments.is_exhausted());
}

#[tokio::test]
async fn get_video_details_music() {
    let rp = RustyPipe::builder().strict().build();
    let details = rp.query().video_details("XuM2onMGvTI").await.unwrap();

    // dbg!(&details);

    assert_eq!(details.id, "XuM2onMGvTI");
    assert_eq!(details.title, "Gäa");
    let desc = details.description.to_plaintext();
    assert!(desc.contains("Gäa · Oonagh"), "bad description: {}", desc);

    assert_eq!(details.channel.id, "UCVGvnqB-5znqPSbMGlhF4Pw");
    assert_eq!(details.channel.name, "Sentamusic");
    assert!(!details.channel.avatar.is_empty(), "no channel avatars");
    assert_eq!(details.channel.verification, Verification::Artist);
    assert_gte(
        details.channel.subscriber_count.unwrap(),
        33_000,
        "subscribers",
    );
    assert_gte(details.view_count, 20_309, "views");
    assert_gte(details.like_count.unwrap(), 145, "likes");

    let date = details.publish_date.unwrap();
    assert_eq!(date.date(), date!(2020 - 8 - 6));

    assert!(!details.is_live);
    assert!(!details.is_ccommons);

    assert!(details.recommended.visitor_data.is_some());
    assert_next(details.recommended, rp.query(), 10, 2).await;

    // Update(01.11.2022): comments are sometimes enabled
    /*
    assert_eq!(details.top_comments.count, Some(0));
    assert_eq!(details.latest_comments.count, Some(0));
    assert!(details.top_comments.is_empty());
    assert!(details.latest_comments.is_empty());
    */
}

#[tokio::test]
async fn get_video_details_ccommons() {
    let rp = RustyPipe::builder().strict().build();
    let details = rp.query().video_details("0rb9CfOvojk").await.unwrap();

    // dbg!(&details);

    assert_eq!(details.id, "0rb9CfOvojk");
    assert_eq!(
        details.title,
        "BahnMining - Pünktlichkeit ist eine Zier (David Kriesel)"
    );
    let desc = details.description.to_plaintext();
    assert!(
            desc.contains("Seit Anfang 2019 hat David jeden einzelnen Halt jeder einzelnen Zugfahrt auf jedem einzelnen Fernbahnhof in ganz Deutschland"),
            "bad description: {}",
            desc
        );

    assert_eq!(details.channel.id, "UC2TXq_t06Hjdr2g_KdKpHQg");
    assert_eq!(details.channel.name, "media.ccc.de");
    assert!(!details.channel.avatar.is_empty(), "no channel avatars");
    assert_eq!(details.channel.verification, Verification::None);
    assert_gte(
        details.channel.subscriber_count.unwrap(),
        170_000,
        "subscribers",
    );
    assert_gte(details.view_count, 2_517_358, "views");
    assert_gte(details.like_count.unwrap(), 52_330, "likes");

    let date = details.publish_date.unwrap();
    assert_eq!(date.date(), date!(2019 - 12 - 29));

    assert!(!details.is_live);
    assert!(details.is_ccommons);

    assert!(details.recommended.visitor_data.is_some());
    assert_next(details.recommended, rp.query(), 10, 2).await;

    assert_gte(details.top_comments.count.unwrap(), 2199, "comments");
    assert!(!details.top_comments.is_exhausted());
    assert!(!details.latest_comments.is_exhausted());
}

#[tokio::test]
async fn get_video_details_chapters() {
    let rp = RustyPipe::builder().strict().build();
    let details = rp.query().video_details("nFDBxBUfE74").await.unwrap();

    // dbg!(&details);

    assert_eq!(details.id, "nFDBxBUfE74");
    assert_eq!(details.title, "The Prepper PC");
    let desc = details.description.to_plaintext();
    assert!(
            desc.contains("These days, you can game almost anywhere on the planet, anytime. But what if that planet was in the middle of an apocalypse"),
            "bad description: {}",
            desc
        );

    assert_eq!(details.channel.id, "UCXuqSBlHAE6Xw-yeJA0Tunw");
    assert_eq!(details.channel.name, "Linus Tech Tips");
    assert!(!details.channel.avatar.is_empty(), "no channel avatars");
    assert_eq!(details.channel.verification, Verification::Verified);
    assert_gte(
        details.channel.subscriber_count.unwrap(),
        14_700_000,
        "subscribers",
    );
    assert_gte(details.view_count, 1_157_262, "views");
    assert_gte(details.like_count.unwrap(), 54_670, "likes");

    let date = details.publish_date.unwrap();
    assert_eq!(date.date(), date!(2022 - 9 - 15));

    assert!(!details.is_live);
    assert!(!details.is_ccommons);

    // In rare cases, YouTube does not return chapters here
    if !details.chapters.is_empty() {
        insta::assert_ron_snapshot!(details.chapters, {
            "[].thumbnail" => insta::dynamic_redaction(move |value, _path| {
                assert!(!value.as_slice().unwrap().is_empty());
                "[ok]"
            }),
        }, @r###"
        [
          Chapter(
            title: "Intro",
            position: 0,
            thumbnail: "[ok]",
          ),
          Chapter(
            title: "The PC Built for Super Efficiency",
            position: 42,
            thumbnail: "[ok]",
          ),
          Chapter(
            title: "Our BURIAL ENCLOSURE?!",
            position: 161,
            thumbnail: "[ok]",
          ),
          Chapter(
            title: "Our Power Solution (Thanks Jackery!)",
            position: 211,
            thumbnail: "[ok]",
          ),
          Chapter(
            title: "Diggin\' Holes",
            position: 287,
            thumbnail: "[ok]",
          ),
          Chapter(
            title: "Colonoscopy?",
            position: 330,
            thumbnail: "[ok]",
          ),
          Chapter(
            title: "Diggin\' like a man",
            position: 424,
            thumbnail: "[ok]",
          ),
          Chapter(
            title: "The world\'s worst woodsman",
            position: 509,
            thumbnail: "[ok]",
          ),
          Chapter(
            title: "Backyard cable management",
            position: 543,
            thumbnail: "[ok]",
          ),
          Chapter(
            title: "Time to bury this boy",
            position: 602,
            thumbnail: "[ok]",
          ),
          Chapter(
            title: "Solar Power Generation",
            position: 646,
            thumbnail: "[ok]",
          ),
          Chapter(
            title: "Issues",
            position: 697,
            thumbnail: "[ok]",
          ),
          Chapter(
            title: "First Play Test",
            position: 728,
            thumbnail: "[ok]",
          ),
          Chapter(
            title: "Conclusion",
            position: 800,
            thumbnail: "[ok]",
          ),
        ]
        "###);
    }

    assert!(details.recommended.visitor_data.is_some());
    assert_next(details.recommended, rp.query(), 10, 2).await;

    assert_gte(details.top_comments.count.unwrap(), 3200, "comments");
    assert!(!details.top_comments.is_exhausted());
    assert!(!details.latest_comments.is_exhausted());
}

#[tokio::test]
async fn get_video_details_live() {
    let rp = RustyPipe::builder().strict().build();
    let details = rp.query().video_details("86YLFOog4GM").await.unwrap();

    // dbg!(&details);

    assert_eq!(details.id, "86YLFOog4GM");
    assert_eq!(
        details.title,
        "🌎 Nasa Live Stream  - Earth From Space :  Live Views from the ISS"
    );
    let desc = details.description.to_plaintext();
    assert!(
        desc.contains("The station is crewed by NASA astronauts as well as Russian Cosmonauts"),
        "bad description: {}",
        desc
    );

    assert_eq!(details.channel.id, "UCakgsb0w7QB0VHdnCc-OVEA");
    assert_eq!(details.channel.name, "Space Videos");
    assert!(!details.channel.avatar.is_empty(), "no channel avatars");
    assert_eq!(details.channel.verification, Verification::Verified);
    assert_gte(
        details.channel.subscriber_count.unwrap(),
        5_500_000,
        "subscribers",
    );
    assert_gte(details.view_count, 10, "views");
    assert_gte(details.like_count.unwrap(), 872_290, "likes");

    let date = details.publish_date.unwrap();
    assert_eq!(date.date(), date!(2021 - 9 - 23));

    assert!(details.is_live);
    assert!(!details.is_ccommons);

    assert!(details.recommended.visitor_data.is_some());
    assert_next(details.recommended, rp.query(), 10, 2).await;

    // No comments because livestream
    assert_eq!(details.top_comments.count, Some(0));
    assert_eq!(details.latest_comments.count, Some(0));
    assert!(details.top_comments.is_empty());
    assert!(details.latest_comments.is_empty());
}

#[tokio::test]
async fn get_video_details_agegate() {
    let rp = RustyPipe::builder().strict().build();
    let details = rp.query().video_details("HRKu0cvrr_o").await.unwrap();

    // dbg!(&details);

    assert_eq!(details.id, "HRKu0cvrr_o");
    assert_eq!(
        details.title,
        "AlphaOmegaSin Fanboy Logic: Likes/Dislikes Disabled = Point Invalid Lol wtf?"
    );
    insta::assert_ron_snapshot!(details.description, @"RichText([])");

    assert_eq!(details.channel.id, "UCQT2yul0lr6Ie9qNQNmw-sg");
    assert_eq!(details.channel.name, "PrinceOfFALLEN");
    assert!(!details.channel.avatar.is_empty(), "no channel avatars");
    assert_eq!(details.channel.verification, Verification::None);
    assert_gte(
        details.channel.subscriber_count.unwrap(),
        1400,
        "subscribers",
    );
    assert_gte(details.view_count, 200, "views");
    assert!(details.like_count.is_none(), "like count not hidden");

    let date = details.publish_date.unwrap();
    assert_eq!(date.date(), date!(2019 - 1 - 2));

    assert!(!details.is_live);
    assert!(!details.is_ccommons);

    // No recommendations because agegate
    assert_eq!(details.recommended.count, Some(0));
    assert!(details.recommended.items.is_empty());
}

#[tokio::test]
async fn get_video_details_not_found() {
    let rp = RustyPipe::builder().strict().build();
    let err = rp.query().video_details("abcdefgLi5X").await.unwrap_err();

    assert!(
        matches!(
            err,
            Error::Extraction(ExtractionError::ContentUnavailable(_))
        ),
        "got: {}",
        err
    )
}

#[tokio::test]
async fn get_video_comments() {
    let rp = RustyPipe::builder().strict().build();
    let details = rp.query().video_details("ZeerrnuLi5E").await.unwrap();

    let top_comments = details
        .top_comments
        .next(rp.query())
        .await
        .unwrap()
        .unwrap();
    assert_gte(top_comments.items.len(), 10, "comments");
    assert!(!top_comments.is_exhausted());
    assert!(top_comments.visitor_data.is_some());

    let n_comments = top_comments.count.unwrap();
    assert_gte(n_comments, 700_000, "comments");

    let latest_comments = details
        .latest_comments
        .next(rp.query())
        .await
        .unwrap()
        .unwrap();
    assert_gte(latest_comments.items.len(), 10, "next comments");
    assert!(!latest_comments.is_exhausted());
    assert!(latest_comments.visitor_data.is_some());
}

//#CHANNEL

#[tokio::test]
async fn channel_videos() {
    let rp = RustyPipe::builder().strict().build();
    let channel = rp
        .query()
        .channel_videos("UC2DjFE7Xf11URZqWBigcVOQ")
        .await
        .unwrap();

    // dbg!(&channel);
    assert_channel_eevblog(&channel);

    assert!(
        !channel.content.items.is_empty() && !channel.content.is_exhausted(),
        "got no videos"
    );

    let first_video = &channel.content.items[0];
    let first_video_date = first_video.publish_date.unwrap();
    let age_days = (OffsetDateTime::now_utc() - first_video_date).whole_days();

    assert!(age_days < 60, "latest video older than 60 days");

    assert_next(channel.content, rp.query(), 15, 2).await;
}

#[tokio::test]
async fn channel_shorts() {
    let rp = RustyPipe::builder().strict().build();
    let channel = rp
        .query()
        .channel_shorts("UCh8gHdtzO2tXd593_bjErWg")
        .await
        .unwrap();

    // dbg!(&channel);
    assert_eq!(channel.id, "UCh8gHdtzO2tXd593_bjErWg");
    assert_eq!(channel.name, "Doobydobap");
    assert_gte(channel.subscriber_count.unwrap(), 2_800_000, "subscribers");
    assert!(!channel.avatar.is_empty(), "got no thumbnails");
    assert_eq!(channel.verification, Verification::Verified);
    assert!(channel
        .description
        .contains("Hi, I\u{2019}m Tina, aka Doobydobap"));
    assert_eq!(
        channel.vanity_url.as_ref().unwrap(),
        "https://www.youtube.com/@Doobydobap"
    );
    assert!(!channel.banner.is_empty(), "got no banners");
    assert!(!channel.mobile_banner.is_empty(), "got no mobile banners");
    assert!(!channel.tv_banner.is_empty(), "got no tv banners");

    assert!(
        !channel.content.items.is_empty() && !channel.content.is_exhausted(),
        "got no shorts"
    );

    assert_next(channel.content, rp.query(), 15, 1).await;
}

#[tokio::test]
async fn channel_livestreams() {
    let rp = RustyPipe::builder().strict().build();
    let channel = rp
        .query()
        .channel_livestreams("UC2DjFE7Xf11URZqWBigcVOQ")
        .await
        .unwrap();

    // dbg!(&channel);
    assert_channel_eevblog(&channel);

    assert!(
        !channel.content.items.is_empty() && !channel.content.is_exhausted(),
        "got no streams"
    );

    assert_next(channel.content, rp.query(), 5, 1).await;
}

#[tokio::test]
async fn channel_playlists() {
    let rp = RustyPipe::builder().strict().build();
    let channel = rp
        .query()
        .channel_playlists("UC2DjFE7Xf11URZqWBigcVOQ")
        .await
        .unwrap();

    assert_channel_eevblog(&channel);

    assert!(
        !channel.content.items.is_empty() && !channel.content.is_exhausted(),
        "got no playlists"
    );

    assert_next(channel.content, rp.query(), 15, 1).await;
}

#[tokio::test]
async fn channel_info() {
    let rp = RustyPipe::builder().strict().build();
    let channel = rp
        .query()
        .channel_info("UC2DjFE7Xf11URZqWBigcVOQ")
        .await
        .unwrap();

    // dbg!(&channel);
    assert_channel_eevblog(&channel);

    let created = channel.content.create_date.unwrap();
    assert_eq!(created, date!(2009 - 4 - 4));

    assert_gte(
        channel.content.view_count.unwrap(),
        186854340,
        "channel views",
    );

    insta::assert_ron_snapshot!(channel.content.links, @r###"
        [
          ("EEVblog Web Site", "http://www.eevblog.com/"),
          ("Twitter", "http://www.twitter.com/eevblog"),
          ("Facebook", "http://www.facebook.com/EEVblog"),
          ("EEVdiscover", "https://www.youtube.com/channel/UCkGvUEt8iQLmq3aJIMjT2qQ"),
          ("The EEVblog Forum", "http://www.eevblog.com/forum"),
          ("EEVblog Merchandise (T-Shirts)", "http://www.eevblog.com/merch"),
          ("EEVblog Donations", "http://www.eevblog.com/donations/"),
          ("Patreon", "https://www.patreon.com/eevblog"),
          ("SubscribeStar", "https://www.subscribestar.com/eevblog"),
          ("The AmpHour Radio Show", "http://www.theamphour.com/"),
          ("Flickr", "http://www.flickr.com/photos/eevblog"),
          ("EEVblog AMAZON Store", "http://www.amazon.com/gp/redirect.html?ie=UTF8&location=http%3A%2F%2Fwww.amazon.com%2F&tag=ee04-20&linkCode=ur2&camp=1789&creative=390957"),
          ("2nd EEVblog Channel", "http://www.youtube.com/EEVblog2"),
        ]
        "###);
}

fn assert_channel_eevblog<T>(channel: &Channel<T>) {
    assert_eq!(channel.id, "UC2DjFE7Xf11URZqWBigcVOQ");
    assert_eq!(channel.name, "EEVblog");
    assert_gte(channel.subscriber_count.unwrap(), 880_000, "subscribers");
    assert!(!channel.avatar.is_empty(), "got no thumbnails");
    assert_eq!(channel.verification, Verification::Verified);
    assert_eq!(channel.description, "NO SCRIPT, NO FEAR, ALL OPINION\nAn off-the-cuff Video Blog about Electronics Engineering, for engineers, hobbyists, enthusiasts, hackers and Makers\nHosted by Dave Jones from Sydney Australia\n\nDONATIONS:\nBitcoin: 3KqyH1U3qrMPnkLufM2oHDU7YB4zVZeFyZ\nEthereum: 0x99ccc4d2654ba40744a1f678d9868ecb15e91206\nPayPal: david@alternatezone.com\n\nPatreon: https://www.patreon.com/eevblog\n\nEEVblog2: http://www.youtube.com/EEVblog2\nEEVdiscover: https://www.youtube.com/channel/UCkGvUEt8iQLmq3aJIMjT2qQ\n\nEMAIL:\nAdvertising/Commercial: eevblog+business@gmail.com\nFan mail: eevblog+fan@gmail.com\nHate Mail: eevblog+hate@gmail.com\n\nI DON'T DO PAID VIDEO SPONSORSHIPS, DON'T ASK!\n\nPLEASE:\nDo NOT ask for personal advice on something, post it in the EEVblog forum.\nI read ALL email, but please don't be offended if I don't have time to reply, I get a LOT of email.\n\nMailbag\nPO Box 7949\nBaulkham Hills NSW 2153\nAUSTRALIA");
    assert!(!channel.tags.is_empty(), "got no tags");
    assert_eq!(
        channel.vanity_url.as_ref().unwrap(),
        "https://www.youtube.com/@EEVblog"
    );
    assert!(!channel.banner.is_empty(), "got no banners");
    assert!(!channel.mobile_banner.is_empty(), "got no mobile banners");
    assert!(!channel.tv_banner.is_empty(), "got no tv banners");
}

#[rstest]
#[case::artist("UC_vmjW5e1xEHhYjY2a0kK1A", "Oonagh - Topic", false, false)]
#[case::shorts("UCh8gHdtzO2tXd593_bjErWg", "Doobydobap", true, true)]
#[case::livestream(
    "UChs0pSaEoNLV4mevBFGaoKA",
    "The Good Life Radio x Sensual Musique",
    true,
    true
)]
#[case::music("UC-9-kyTW8ZkZNDHQJ6FgpwQ", "Music", false, false)]
#[case::live("UC4R8DWoMoI7CAwX8_LjQHig", "Live", false, false)]
#[case::news("UCYfdidRxbB8Qhf0Nx7ioOYw", "News", false, false)]
#[tokio::test]
async fn channel_more(
    #[case] id: &str,
    #[case] name: &str,
    #[case] has_videos: bool,
    #[case] has_playlists: bool,
) {
    let rp = RustyPipe::builder().strict().build();

    fn assert_channel<T>(channel: &Channel<T>, id: &str, name: &str) {
        assert_eq!(channel.id, id);
        assert_eq!(channel.name, name);
    }

    let channel_videos = rp.query().channel_videos(&id).await.unwrap();
    assert_channel(&channel_videos, id, name);
    if has_videos {
        assert!(!channel_videos.content.items.is_empty(), "got no videos");
    } else {
        assert!(channel_videos.content.items.is_empty(), "got videos");
    }

    let channel_playlists = rp.query().channel_playlists(&id).await.unwrap();
    assert_channel(&channel_playlists, id, name);
    if has_playlists {
        assert!(
            !channel_playlists.content.items.is_empty(),
            "got no playlists"
        );
    } else {
        assert!(channel_playlists.content.items.is_empty(), "got playlists");
    }

    let channel_info = rp.query().channel_info(&id).await.unwrap();
    assert_channel(&channel_info, id, name);
}

#[rstest]
#[case::not_exist("UCOpNcN46UbXVtpKMrmU4Abx")]
#[case::gaming("UCOpNcN46UbXVtpKMrmU4Abg")]
#[case::movies("UCuJcl0Ju-gPDoksRjK1ya-w")]
#[case::sports("UCEgdi0XIXXZ-qJOFPf4JSKw")]
#[case::learning("UCtFRv9O2AHqOZjjynzrv-xg")]
#[tokio::test]
async fn channel_not_found(#[case] id: &str) {
    let rp = RustyPipe::builder().strict().build();
    let err = rp.query().channel_videos(&id).await.unwrap_err();

    assert!(
        matches!(
            err,
            Error::Extraction(ExtractionError::ContentUnavailable(_))
        ),
        "got: {}",
        err
    );
}

//#CHANNEL_RSS

#[cfg(feature = "rss")]
mod channel_rss {
    use super::*;
    use time::macros::datetime;

    #[tokio::test]
    async fn get_channel_rss() {
        let rp = RustyPipe::builder().strict().build();
        let channel = rp
            .query()
            .channel_rss("UCHnyfMqiRRG1u-2MsSQLbXA")
            .await
            .unwrap();

        assert_eq!(channel.id, "UCHnyfMqiRRG1u-2MsSQLbXA");
        assert_eq!(channel.name, "Veritasium");
        assert_eq!(channel.create_date, datetime!(2010-07-21 7:18:02 +0));

        assert!(!channel.videos.is_empty());
    }

    #[tokio::test]
    async fn get_channel_rss_not_found() {
        let rp = RustyPipe::builder().strict().build();
        let err = rp
            .query()
            .channel_rss("UCHnyfMqiRRG1u-2MsSQLbXZ")
            .await
            .unwrap_err();

        assert!(
            matches!(
                err,
                Error::Extraction(ExtractionError::ContentUnavailable(_))
            ),
            "got: {}",
            err
        );
    }
}

//#SEARCH

#[tokio::test]
async fn search() {
    let rp = RustyPipe::builder().strict().build();
    let result = rp.query().search("doobydoobap").await.unwrap();

    assert!(
        result.items.count.unwrap() > 7000,
        "expected > 7000 total results, got {}",
        result.items.count.unwrap()
    );
    assert_eq!(result.corrected_query.unwrap(), "doobydobap");

    assert_next(result.items, rp.query(), 10, 2).await;
}

#[rstest]
#[case::video(search_filter::Entity::Video)]
#[case::channel(search_filter::Entity::Channel)]
#[case::playlist(search_filter::Entity::Playlist)]
#[tokio::test]
async fn search_filter_entity(#[case] entity: search_filter::Entity) {
    let rp = RustyPipe::builder().strict().build();
    let mut result = rp
        .query()
        .search_filter("with no videos", &SearchFilter::new().entity(entity))
        .await
        .unwrap();

    result.items.extend(rp.query()).await.unwrap();
    assert_gte(result.items.items.len(), 20, "items");

    result.items.items.iter().for_each(|item| match item {
        YouTubeItem::Video(_) => {
            assert_eq!(entity, search_filter::Entity::Video);
        }
        YouTubeItem::Channel(_) => {
            assert_eq!(entity, search_filter::Entity::Channel);
        }
        YouTubeItem::Playlist(_) => {
            assert_eq!(entity, search_filter::Entity::Playlist);
        }
    });
}

#[tokio::test]
async fn search_empty() {
    let rp = RustyPipe::builder().strict().build();
    let result = rp
        .query()
        .search_filter(
            "test",
            &search_filter::SearchFilter::new()
                .feature(search_filter::Feature::IsLive)
                .feature(search_filter::Feature::Is3d),
        )
        .await
        .unwrap();

    assert!(result.items.is_empty());
}

#[tokio::test]
async fn search_suggestion() {
    let rp = RustyPipe::builder().strict().build();
    let result = rp.query().search_suggestion("hunger ga").await.unwrap();

    assert!(result.contains(&"hunger games".to_owned()));
}

#[tokio::test]
async fn search_suggestion_empty() {
    let rp = RustyPipe::builder().strict().build();
    let result = rp
        .query()
        .search_suggestion("fjew327%4ifjelwfvnewg49")
        .await
        .unwrap();

    assert!(result.is_empty());
}

//#URL RESOLVER

#[rstest]
#[case("https://www.youtube.com/LinusTechTips", UrlTarget::Channel {id: "UCXuqSBlHAE6Xw-yeJA0Tunw".to_owned()})]
#[case("https://www.youtube.com/@AndroidAuthority", UrlTarget::Channel {id: "UCgyqtNWZmIxTx3b6OxTSALw".to_owned()})]
#[case("https://www.youtube.com/channel/UC5I2hjZYiW9gZPVkvzM8_Cw", UrlTarget::Channel {id: "UC5I2hjZYiW9gZPVkvzM8_Cw".to_owned()})]
#[case("https://www.youtube.com/c", UrlTarget::Channel {id: "UCXE6F2oZzy_6xEXiJiUFo2w".to_owned()})]
#[case("https://www.youtube.com/user/MrBeast6000", UrlTarget::Channel {id: "UCX6OQ3DkcsbYNE6H8uQQuVA".to_owned()})]
#[case("https://www.youtube.com/watch?v=dQw4w9WgXcQ", UrlTarget::Video {id: "dQw4w9WgXcQ".to_owned(), start_time: 0})]
#[case("https://www.youtube.com/watch?v=dQw4w9WgXcQ&t=60", UrlTarget::Video {id: "dQw4w9WgXcQ".to_owned(), start_time: 60})]
#[case("https://www.youtube.com/playlist?list=PL4lEESSgxM_5O81EvKCmBIm_JT5Q7JeaI", UrlTarget::Playlist {id: "PL4lEESSgxM_5O81EvKCmBIm_JT5Q7JeaI".to_owned()})]
#[case("https://www.youtube.com/playlist?list=RDCLAK5uy_kFQXdnqMaQCVx2wpUM4ZfbsGCDibZtkJk", UrlTarget::Playlist {id: "RDCLAK5uy_kFQXdnqMaQCVx2wpUM4ZfbsGCDibZtkJk".to_owned()})]
#[case("https://youtu.be/dQw4w9WgXcQ", UrlTarget::Video {id: "dQw4w9WgXcQ".to_owned(), start_time: 0})]
#[case("https://youtu.be/dQw4w9WgXcQ?t=60", UrlTarget::Video {id: "dQw4w9WgXcQ".to_owned(), start_time: 60})]
#[case("https://youtu.be/dQw4w9WgXcQ", UrlTarget::Video {id: "dQw4w9WgXcQ".to_owned(), start_time: 0})]
#[case("https://youtu.be/dQw4w9WgXcQ?t=60", UrlTarget::Video {id: "dQw4w9WgXcQ".to_owned(), start_time: 60})]
#[case("https://piped.mha.fi/watch?v=dQw4w9WgXcQ", UrlTarget::Video {id: "dQw4w9WgXcQ".to_owned(), start_time: 0})]
// Both a video ID and a channel name => returns channel
#[case("https://piped.mha.fi/dQw4w9WgXcQ", UrlTarget::Channel {id: "UCoG6BrhgmivrkcbEHcYtK4Q".to_owned()})]
// Both a video ID and a channel name + video time param => returns video
#[case("https://piped.mha.fi/dQw4w9WgXcQ?t=0", UrlTarget::Video {id: "dQw4w9WgXcQ".to_owned(), start_time: 0})]
#[case("https://music.youtube.com/playlist?list=OLAK5uy_k0yFrZlFRgCf3rLPza-lkRmCrtLPbK9pE", UrlTarget::Album {id: "MPREb_GyH43gCvdM5".to_owned()})]
#[case("https://music.youtube.com/browse/MPREb_GyH43gCvdM5", UrlTarget::Album {id: "MPREb_GyH43gCvdM5".to_owned()})]
#[case("https://music.youtube.com/browse/UC5I2hjZYiW9gZPVkvzM8_Cw", UrlTarget::Channel {id: "UC5I2hjZYiW9gZPVkvzM8_Cw".to_owned()})]
#[tokio::test]
async fn resolve_url(#[case] url: &str, #[case] expect: UrlTarget) {
    let rp = RustyPipe::builder().strict().build();
    let target = rp.query().resolve_url(url, true).await.unwrap();
    assert_eq!(target, expect);
}

#[rstest]
#[case("LinusTechTips", UrlTarget::Channel {id: "UCXuqSBlHAE6Xw-yeJA0Tunw".to_owned()})]
#[case("@AndroidAuthority", UrlTarget::Channel {id: "UCgyqtNWZmIxTx3b6OxTSALw".to_owned()})]
#[case("UC5I2hjZYiW9gZPVkvzM8_Cw", UrlTarget::Channel {id: "UC5I2hjZYiW9gZPVkvzM8_Cw".to_owned()})]
#[case("c", UrlTarget::Channel {id: "UCXE6F2oZzy_6xEXiJiUFo2w".to_owned()})]
#[case("user/MrBeast6000", UrlTarget::Channel {id: "UCX6OQ3DkcsbYNE6H8uQQuVA".to_owned()})]
#[case("@AndroidAuthority", UrlTarget::Channel {id: "UCgyqtNWZmIxTx3b6OxTSALw".to_owned()})]
#[case("dQw4w9WgXcQ", UrlTarget::Video {id: "dQw4w9WgXcQ".to_owned(), start_time: 0})]
#[case("PL4lEESSgxM_5O81EvKCmBIm_JT5Q7JeaI", UrlTarget::Playlist {id: "PL4lEESSgxM_5O81EvKCmBIm_JT5Q7JeaI".to_owned()})]
#[case("RDCLAK5uy_kFQXdnqMaQCVx2wpUM4ZfbsGCDibZtkJk", UrlTarget::Playlist {id: "RDCLAK5uy_kFQXdnqMaQCVx2wpUM4ZfbsGCDibZtkJk".to_owned()})]
#[case("OLAK5uy_k0yFrZlFRgCf3rLPza-lkRmCrtLPbK9pE", UrlTarget::Album {id: "MPREb_GyH43gCvdM5".to_owned()})]
#[case("MPREb_GyH43gCvdM5", UrlTarget::Album {id: "MPREb_GyH43gCvdM5".to_owned()})]
#[tokio::test]
async fn resolve_string(#[case] string: &str, #[case] expect: UrlTarget) {
    let rp = RustyPipe::builder().strict().build();
    let target = rp.query().resolve_string(string, true).await.unwrap();
    assert_eq!(target, expect);
}

#[tokio::test]
async fn resolve_channel_not_found() {
    let rp = RustyPipe::builder().strict().build();
    let err = rp
        .query()
        .resolve_url("https://www.youtube.com/feeqegnhq3rkwghjq43ruih43io3", true)
        .await
        .unwrap_err();

    assert!(matches!(
        err,
        Error::Extraction(ExtractionError::ContentUnavailable(_))
    ));
}

//#TRENDS

#[tokio::test]
async fn startpage() {
    let rp = RustyPipe::builder().strict().build();
    let startpage = rp.query().startpage().await.unwrap();

    // The startpage requires visitor data to fetch continuations
    assert!(startpage.visitor_data.is_some());

    assert_next(startpage, rp.query(), 20, 2).await;
}

#[tokio::test]
async fn trending() {
    let rp = RustyPipe::builder().strict().build();
    let result = rp.query().trending().await.unwrap();

    assert_gte(result.len(), 50, "items");
}

//#MUSIC

#[rstest]
#[case::long(
    "PL5dDx681T4bR7ZF1IuWzOv1omlRbE7PiJ",
    "Die schönsten deutschen Lieder | Beliebteste Lieder | Beste Deutsche Musik 2022",
    true,
    None,
    Some(("UCIekuFeMaV78xYfvpmoCnPg", "Best Music")),
    false,
)]
#[case::short(
    "RDCLAK5uy_kFQXdnqMaQCVx2wpUM4ZfbsGCDibZtkJk",
    "Easy Pop",
    false,
    Some("Stress-free tunes from classic rockers and newer artists.".to_owned()),
    None,
    true
)]
#[case::nomusic(
    "PL1J-6JOckZtE_P9Xx8D3b2O6w0idhuKBe",
    "Minecraft SHINE",
    false,
    Some("SHINE - Survival Hardcore in New Environment: Auf einem Server machen sich tapfere Spieler auf, mystische Welten zu erkunden, magische Technologien zu erforschen und vorallem zu überleben...".to_owned()),
    Some(("UCQM0bS4_04-Y4JuYrgmnpZQ", "Chaosflo44")),
    false,
)]
#[tokio::test]
async fn music_playlist(
    #[case] id: &str,
    #[case] name: &str,
    #[case] is_long: bool,
    #[case] description: Option<String>,
    #[case] channel: Option<(&str, &str)>,
    #[case] from_ytm: bool,
) {
    let rp = RustyPipe::builder().strict().build();
    let playlist = rp.query().music_playlist(id).await.unwrap();

    assert_eq!(playlist.id, id);
    assert_eq!(playlist.name, name);
    assert!(!playlist.tracks.is_empty());
    assert_eq!(!playlist.tracks.is_exhausted(), is_long);
    assert!(playlist.track_count.unwrap() > 10);
    assert_eq!(playlist.track_count.unwrap() > 100, is_long);
    assert_eq!(playlist.description, description);

    if let Some(expect) = channel {
        let c = playlist.channel.unwrap();
        assert_eq!(c.id, expect.0);
        assert_eq!(c.name, expect.1);
    }
    assert!(!playlist.thumbnail.is_empty());
    assert_eq!(playlist.from_ytm, from_ytm);
}

#[tokio::test]
async fn music_playlist_cont() {
    let rp = RustyPipe::builder().strict().build();
    let mut playlist = rp
        .query()
        .music_playlist("PLbZIPy20-1pN7mqjckepWF78ndb6ci_qi")
        .await
        .unwrap();

    playlist
        .tracks
        .extend_pages(rp.query(), usize::MAX)
        .await
        .unwrap();

    assert_gte(playlist.tracks.items.len(), 100, "tracks");
    assert_gte(playlist.tracks.count.unwrap(), 100, "track count");
}

#[tokio::test]
async fn music_playlist_related() {
    let rp = RustyPipe::builder().strict().build();
    let mut playlist = rp
        .query()
        .music_playlist("PLbZIPy20-1pN7mqjckepWF78ndb6ci_qi")
        .await
        .unwrap();

    playlist.related_playlists.extend(rp.query()).await.unwrap();

    assert_gte(
        playlist.related_playlists.items.len(),
        10,
        "related playlists",
    );
}

#[tokio::test]
async fn music_playlist_not_found() {
    let rp = RustyPipe::builder().strict().build();
    let err = rp
        .query()
        .music_playlist("PLbZIPy20-1pN7mqjckepWF78ndb6ci_qz")
        .await
        .unwrap_err();

    assert!(
        matches!(
            err,
            Error::Extraction(ExtractionError::ContentUnavailable(_))
        ),
        "got: {}",
        err
    );
}

#[rstest]
#[case::one_artist("one_artist", "MPREb_nlBWQROfvjo")]
#[case::various_artists("various_artists", "MPREb_8QkDeEIawvX")]
#[case::single("single", "MPREb_bHfHGoy7vuv")]
#[case::ep("ep", "MPREb_u1I69lSAe5v")]
#[case::audiobook("audiobook", "MPREb_gaoNzsQHedo")]
#[case::show("show", "MPREb_cwzk8EUwypZ")]
#[case::unavailable("unavailable", "MPREb_AzuWg8qAVVl")]
#[tokio::test]
async fn music_album(#[case] name: &str, #[case] id: &str) {
    let rp = RustyPipe::builder().strict().build();
    let album = rp.query().music_album(id).await.unwrap();

    assert!(!album.cover.is_empty(), "got no cover");

    insta::assert_ron_snapshot!(format!("music_album_{}", name), album,
        {".cover" => "[cover]"}
    );
}

#[tokio::test]
async fn music_album_not_found() {
    let rp = RustyPipe::builder().strict().build();
    let err = rp
        .query()
        .music_album("MPREb_nlBWQROfvjoz")
        .await
        .unwrap_err();

    assert!(
        matches!(
            err,
            Error::Extraction(ExtractionError::ContentUnavailable(_))
        ),
        "got: {}",
        err
    );
}

#[rstest]
#[case::basic_all("basic_all", "UC7cl4MmM6ZZ2TcFyMk_b4pg", true, 15, 2)]
#[case::basic("basic", "UC7cl4MmM6ZZ2TcFyMk_b4pg", false, 15, 2)]
#[case::no_more_albums("no_more_albums", "UC_vmjW5e1xEHhYjY2a0kK1A", true, 12, 0)]
#[case::only_singles("only_singles", "UCfwCE5VhPMGxNPFxtVv7lRw", false, 13, 0)]
#[case::no_artist("no_artist", "UCh8gHdtzO2tXd593_bjErWg", false, 0, 2)]
#[tokio::test]
async fn music_artist(
    #[case] name: &str,
    #[case] id: &str,
    #[case] all_albums: bool,
    #[case] min_tracks: usize,
    #[case] min_playlists: usize,
) {
    let rp = RustyPipe::builder().strict().build();
    let mut artist = rp.query().music_artist(id, all_albums).await.unwrap();

    assert_gte(artist.tracks.len(), min_tracks, "tracks");
    assert_gte(artist.playlists.len(), min_playlists, "playlists");

    if name == "no_artist" {
        assert!(artist.similar_artists.is_empty());
        assert!(artist.subscriber_count.is_none());
    } else {
        assert_gte(artist.subscriber_count.unwrap(), 30000, "subscribers");
    }

    // Check images
    assert!(!artist.header_image.is_empty(), "got no header image");
    artist
        .tracks
        .iter()
        .for_each(|t| assert!(!t.cover.is_empty()));
    artist
        .albums
        .iter()
        .for_each(|t| assert!(!t.cover.is_empty()));
    artist
        .playlists
        .iter()
        .for_each(|t| assert!(!t.thumbnail.is_empty()));
    artist
        .similar_artists
        .iter()
        .for_each(|t| assert!(!t.avatar.is_empty()));

    // Sort albums to ensure consistent order
    artist.albums.sort_by_key(|a| a.id.to_owned());

    insta::assert_ron_snapshot!(format!("music_album_{}", name), artist, {
        ".header_image" => "[header_image]",
        ".subscriber_count" => "[subscriber_count]",
        ".albums[].cover" => "[cover]",
        ".tracks" => "[tracks]",
        ".playlists" => "[playlists]",
        ".similar_artists" => "[artists]",
    });
}

#[tokio::test]
async fn music_artist_not_found() {
    let rp = RustyPipe::builder().strict().build();
    let err = rp
        .query()
        .music_artist("UC7cl4MmM6ZZ2TcFyMk_b4pq", false)
        .await
        .unwrap_err();

    assert!(
        matches!(
            err,
            Error::Extraction(ExtractionError::ContentUnavailable(_))
        ),
        "got: {}",
        err
    );
}

#[rstest]
#[case::default(false)]
#[case::typo(true)]
#[tokio::test]
async fn music_search(#[case] typo: bool) {
    let rp = RustyPipe::builder().strict().build();
    let res = rp
        .query()
        .music_search(match typo {
            false => "black mamba",
            true => "blck mamba",
        })
        .await
        .unwrap();

    assert!(!res.tracks.is_empty(), "no tracks");
    assert!(!res.albums.is_empty(), "no albums");
    assert!(!res.artists.is_empty(), "no artists");
    assert!(!res.playlists.is_empty(), "no playlists");
    assert_eq!(res.order[0], MusicEntityType::Track);

    if typo {
        assert_eq!(res.corrected_query.unwrap(), "black mamba");
    } else {
        assert_eq!(res.corrected_query, None);
    }

    let track = &res.tracks.iter().find(|a| a.id == "ZeerrnuLi5E").unwrap();

    assert_eq!(track.title, "Black Mamba");
    assert_eq!(track.duration.unwrap(), 230);
    assert!(!track.cover.is_empty(), "got no cover");

    assert_eq!(track.artists.len(), 1);
    let track_artist = &track.artists[0];
    assert_eq!(
        track_artist.id.as_ref().unwrap(),
        "UCEdZAdnnKqbaHOlv8nM6OtA"
    );
    assert_eq!(track_artist.name, "aespa");
    assert_eq!(track.album, None);
    assert_gte(track.view_count.unwrap(), 230_000_000, "views");
    assert!(track.is_video, "got no video");
    assert_eq!(track.track_nr, None);
}

#[tokio::test]
async fn music_search_tracks() {
    let rp = RustyPipe::builder().strict().build();
    let res = rp.query().music_search_tracks("black mamba").await.unwrap();

    let track = &res
        .items
        .items
        .iter()
        .find(|a| a.id == "BL-aIpCLWnU")
        .unwrap();

    assert_eq!(track.title, "Black Mamba");
    assert!(!track.cover.is_empty(), "got no cover");
    assert!(!track.is_video);
    assert_eq!(track.track_nr, None);

    assert_eq!(track.artists.len(), 1);
    let track_artist = &track.artists[0];
    assert_eq!(
        track_artist.id.as_ref().unwrap(),
        "UCEdZAdnnKqbaHOlv8nM6OtA"
    );
    assert_eq!(track_artist.name, "aespa");

    assert_eq!(track.duration.unwrap(), 175);

    let album = track.album.as_ref().unwrap();
    assert_eq!(album.id, "MPREb_OpHWHwyNOuY");
    assert_eq!(album.name, "Black Mamba");

    assert_next(res.items, rp.query(), 15, 2).await;
}

#[tokio::test]
async fn music_search_videos() {
    let rp = RustyPipe::builder().strict().build();
    let res = rp.query().music_search_videos("black mamba").await.unwrap();

    let track = &res
        .items
        .items
        .iter()
        .find(|a| a.id == "ZeerrnuLi5E")
        .unwrap();

    assert_eq!(track.title, "Black Mamba");
    assert!(!track.cover.is_empty(), "got no cover");
    assert!(track.is_video);
    assert_eq!(track.track_nr, None);

    assert_eq!(track.artists.len(), 1);
    let track_artist = &track.artists[0];
    assert_eq!(
        track_artist.id.as_ref().unwrap(),
        "UCEdZAdnnKqbaHOlv8nM6OtA"
    );
    assert_eq!(track_artist.name, "aespa");

    assert_eq!(track.duration.unwrap(), 230);
    assert_eq!(track.album, None);
    assert_gte(track.view_count.unwrap(), 230_000_000, "views");

    assert_next(res.items, rp.query(), 15, 2).await;
}

// This podcast was removed from YouTube Music and I could not find another one
/*
#[tokio::test]
async fn music_search_episode() {
    let rp = RustyPipe::builder().strict().build();
    let res = rp
        .query()
        .music_search_videos("Blond - Da muss man dabei gewesen sein: Das Hörspiel - Fall #1")
        .await
        .unwrap();

    let track = &res
        .items
        .items
        .iter()
        .find(|a| a.id == "Zq_-LDy7AgE")
        .unwrap();

    assert_eq!(
        track.title,
        "Blond - Da muss man dabei gewesen sein: Das Hörspiel - Fall #1"
    );
    assert!(!track.cover.is_empty(), "got no cover");
}*/

#[rstest]
#[case::single(
    "lea zu dir",
    "Zu dir (Akustik Version)",
    "MPREb_kaDtXa1zj2Z",
    "LEA",
    "UC_MxOdawj_BStPs4CKBYD0Q",
    2018,
    AlbumType::Single
)]
#[case::ep(
    "waldbrand",
    "Waldbrand",
    "MPREb_u1I69lSAe5v",
    "Madeline Juno",
    "UCpJyCbFbdTrx0M90HCNBHFQ",
    2016,
    AlbumType::Ep
)]
#[case::album(
    "märchen enden gut",
    "Märchen enden gut",
    "MPREb_nlBWQROfvjo",
    "Oonagh",
    "UC_vmjW5e1xEHhYjY2a0kK1A",
    2016,
    AlbumType::Album
)]
#[tokio::test]
async fn music_search_albums(
    #[case] query: &str,
    #[case] name: &str,
    #[case] id: &str,
    #[case] artist: &str,
    #[case] artist_id: &str,
    #[case] year: u16,
    #[case] album_type: AlbumType,
) {
    let rp = RustyPipe::builder().strict().build();
    let res = rp.query().music_search_albums(query).await.unwrap();

    let album = &res.items.items.iter().find(|a| a.id == id).unwrap();
    assert_eq!(album.name, name);

    assert_eq!(album.artists.len(), 1);
    let album_artist = &album.artists[0];
    assert_eq!(album_artist.id.as_ref().unwrap(), artist_id);
    assert_eq!(album_artist.name, artist);

    assert!(!album.cover.is_empty(), "got no cover");
    assert_eq!(album.year.as_ref().unwrap(), &year);
    assert_eq!(album.album_type, album_type);

    assert_eq!(res.corrected_query, None);

    assert_next(res.items, rp.query(), 15, 1).await;
}

#[tokio::test]
async fn music_search_artists() {
    let rp = RustyPipe::builder().strict().build();
    let res = rp.query().music_search_artists("namika").await.unwrap();

    let artist = res
        .items
        .items
        .iter()
        .find(|a| a.id == "UCIh4j8fXWf2U0ro0qnGU8Mg")
        .unwrap();
    assert_eq!(artist.name, "Namika");
    assert!(!artist.avatar.is_empty(), "got no avatar");
    assert!(
        artist.subscriber_count.unwrap() > 735_000,
        "expected >735K subscribers, got {}",
        artist.subscriber_count.unwrap()
    );
    assert_eq!(res.corrected_query, None);
}

#[tokio::test]
async fn music_search_artists_cont() {
    let rp = RustyPipe::builder().strict().build();
    let res = rp.query().music_search_artists("band").await.unwrap();

    assert_eq!(res.corrected_query, None);
    assert_next(res.items, rp.query(), 15, 2).await;
}

#[rstest]
#[case::ytm(false)]
#[case::default(true)]
#[tokio::test]
async fn music_search_playlists(#[case] with_community: bool) {
    let rp = RustyPipe::builder().strict().build();
    let res = if with_community {
        rp.query().music_search_playlists("easy pop").await.unwrap()
    } else {
        rp.query()
            .music_search_playlists_filter("easy pop", false)
            .await
            .unwrap()
    };

    assert_eq!(res.corrected_query, None);
    let playlist = res
        .items
        .items
        .iter()
        .find(|p| p.id == "RDCLAK5uy_kFQXdnqMaQCVx2wpUM4ZfbsGCDibZtkJk")
        .unwrap();

    assert_eq!(playlist.name, "Easy Pop");
    assert!(!playlist.thumbnail.is_empty(), "got no thumbnail");
    assert_gte(playlist.track_count.unwrap(), 80, "tracks");
    assert_eq!(playlist.channel, None);
    assert!(playlist.from_ytm);
}

#[tokio::test]
async fn music_search_playlists_community() {
    let rp = RustyPipe::builder().strict().build();
    let res = rp
        .query()
        .music_search_playlists_filter("Best Pop Music Videos - Top Pop Hits Playlist", true)
        .await
        .unwrap();

    assert_eq!(res.corrected_query, None);
    let playlist = res
        .items
        .items
        .iter()
        .find(|p| p.id == "PLMC9KNkIncKtGvr2kFRuXBVmBev6cAJ2u")
        .unwrap();

    assert_eq!(
        playlist.name,
        "Best Pop Music Videos - Top Pop Hits Playlist"
    );
    assert!(!playlist.thumbnail.is_empty(), "got no thumbnail");
    assert_gte(playlist.track_count.unwrap(), 250, "tracks");

    let channel = playlist.channel.as_ref().unwrap();
    assert_eq!(channel.id, "UCs72iRpTEuwV3y6pdWYLgiw");
    assert_eq!(channel.name, "Redlist - Just Hits");
    assert!(!playlist.from_ytm);
}

/// The YouTube Music search sometimes shows genre radio items. They should be skipped.
#[tokio::test]
async fn music_search_genre_radio() {
    let rp = RustyPipe::builder().strict().build();
    rp.query().music_search("pop radio").await.unwrap();
}

#[rstest]
#[case::default("ed sheer", Some("ed sheeran"))]
#[case::empty("reujbhevmfndxnjrze", None)]
#[tokio::test]
async fn music_search_suggestion(#[case] query: &str, #[case] expect: Option<&str>) {
    let rp = RustyPipe::builder().strict().build();
    let suggestion = rp.query().music_search_suggestion(query).await.unwrap();

    match expect {
        Some(expect) => assert!(
            suggestion.iter().any(|s| s == expect),
            "suggestion: {:?}, expected: {}",
            suggestion,
            expect
        ),
        None => assert!(
            suggestion.is_empty(),
            "suggestion: {:?}, expected to be empty",
            suggestion
        ),
    }
}

#[rstest]
#[case::mv("mv", "ZeerrnuLi5E")]
#[case::track("track", "7nigXQS1Xb0")]
#[tokio::test]
async fn music_details(#[case] name: &str, #[case] id: &str) {
    let rp = RustyPipe::builder().strict().build();
    let track = rp.query().music_details(id).await.unwrap();

    assert!(!track.track.cover.is_empty(), "got no cover");
    if name == "mv" {
        assert_gte(track.track.view_count.unwrap(), 235_000_000, "view count");
    } else {
        assert!(track.track.view_count.is_none());
    }

    insta::assert_ron_snapshot!(format!("music_details_{}", name), track,
        {
            ".track.cover" => "[cover]",
            ".track.view_count" => "[view_count]"
        }
    );
}

#[tokio::test]
async fn music_lyrics() {
    let rp = RustyPipe::builder().strict().build();
    let track = rp.query().music_details("NO8Arj4yeww").await.unwrap();
    let lyrics = rp
        .query()
        .music_lyrics(&track.lyrics_id.unwrap())
        .await
        .unwrap();
    insta::assert_ron_snapshot!(lyrics);
}

#[tokio::test]
async fn music_lyrics_not_found() {
    let rp = RustyPipe::builder().strict().build();
    let track = rp.query().music_details("ekXI8qrbe1s").await.unwrap();
    let err = rp
        .query()
        .music_lyrics(&track.lyrics_id.unwrap())
        .await
        .unwrap_err();

    assert!(
        matches!(
            err,
            Error::Extraction(ExtractionError::ContentUnavailable(_))
        ),
        "got: {}",
        err
    );
}

#[rstest]
#[case::a("7nigXQS1Xb0", true)]
#[case::b("4t3SUDZCBaQ", false)]
#[tokio::test]
async fn music_related(#[case] id: &str, #[case] full: bool) {
    let rp = RustyPipe::builder().strict().build();
    let track = rp.query().music_details(id).await.unwrap();
    let related = rp
        .query()
        .music_related(&track.related_id.unwrap())
        .await
        .unwrap();

    let n_tracks = related.tracks.len();
    let mut track_artists = 0;
    let mut track_artist_ids = 0;
    let mut n_tracks_ytm = 0;
    let mut track_albums = 0;

    for track in related.tracks {
        assert_video_id(&track.id);
        assert!(!track.title.is_empty());
        assert!(!track.cover.is_empty(), "got no cover");

        if let Some(artist_id) = track.artist_id {
            assert_channel_id(&artist_id);
            track_artist_ids += 1;
        }

        let artist = track.artists.first().unwrap();
        assert!(!artist.name.is_empty());
        if let Some(artist_id) = &artist.id {
            assert_channel_id(artist_id);
            track_artists += 1;
        }

        if track.is_video {
            assert!(track.album.is_none());
            assert_gte(track.view_count.unwrap(), 10_000, "views")
        } else {
            n_tracks_ytm += 1;

            assert!(track.view_count.is_none());
            if let Some(album) = track.album {
                assert_album_id(&album.id);
                assert!(!album.name.is_empty());
                track_albums += 1;
            }
        }
    }

    assert_gte(n_tracks, 20, "tracks");
    assert_gte(n_tracks_ytm, 10, "tracks_ytm");

    assert_gte(track_artists, n_tracks - 3, "track_artists");
    assert_gte(track_artist_ids, n_tracks - 3, "track_artists");
    assert_gte(track_albums, n_tracks_ytm - 3, "track_artists");

    if full {
        assert_gte(related.albums.len(), 10, "albums");
        for album in related.albums {
            assert_album_id(&album.id);
            assert!(!album.name.is_empty());
            assert!(!album.cover.is_empty(), "got no cover");

            let artist = album.artists.first().unwrap();
            assert_channel_id(&artist.id.as_ref().unwrap());
            assert!(!artist.name.is_empty());
        }

        assert_gte(related.artists.len(), 10, "artists");
        for artist in related.artists {
            assert_channel_id(&artist.id);
            assert!(!artist.name.is_empty());
            assert!(!artist.avatar.is_empty(), "got no avatar");
            assert_gte(artist.subscriber_count.unwrap(), 5000, "subscribers")
        }

        assert_gte(related.playlists.len(), 10, "playlists");
        for playlist in related.playlists {
            assert_playlist_id(&playlist.id);
            assert!(!playlist.name.is_empty());
            assert!(
                !playlist.thumbnail.is_empty(),
                "pl: {}, got no playlist thumbnail",
                playlist.id
            );
            if !playlist.from_ytm {
                assert!(
                    playlist.channel.is_some(),
                    "pl: {}, got no channel",
                    playlist.id
                );
                let channel = playlist.channel.unwrap();
                assert_channel_id(&channel.id);
                assert!(!channel.name.is_empty());

                assert_gte(playlist.track_count.unwrap(), 2, "tracks");
            } else {
                assert!(playlist.channel.is_none());
            }
        }
    }
}

#[tokio::test]
async fn music_details_not_found() {
    let rp = RustyPipe::builder().strict().build();
    let err = rp.query().music_details("7nigXQS1XbZ").await.unwrap_err();

    assert!(
        matches!(
            err,
            Error::Extraction(ExtractionError::ContentUnavailable(_))
        ),
        "got: {}",
        err
    );
}

#[tokio::test]
async fn music_radio_track() {
    let rp = RustyPipe::builder().strict().build();
    let tracks = rp.query().music_radio_track("ZeerrnuLi5E").await.unwrap();
    assert_next_items(tracks, rp.query(), 20).await;
}

#[tokio::test]
async fn music_radio_track_not_found() {
    let rp = RustyPipe::builder().strict().build();
    let err = rp
        .query()
        .music_radio_track("7nigXQS1XbZ")
        .await
        .unwrap_err();

    assert!(
        matches!(
            err,
            Error::Extraction(ExtractionError::ContentUnavailable(_))
        ),
        "got: {}",
        err
    );
}

#[tokio::test]
async fn music_radio_playlist() {
    let rp = RustyPipe::builder().strict().build();
    let tracks = rp
        .query()
        .music_radio_playlist("PL5dDx681T4bR7ZF1IuWzOv1omlRbE7PiJ")
        .await
        .unwrap();
    assert_next_items(tracks, rp.query(), 20).await;
}

#[tokio::test]
async fn music_radio_playlist_not_found() {
    let rp = RustyPipe::builder().strict().build();
    let res = rp
        .query()
        .music_radio_playlist("PL5dDx681T4bR7ZF1IuWzOv1omlZZZZZZZ")
        .await;

    // Currently this returns valid data
    if let Err(err) = res {
        assert!(
            matches!(
                err,
                Error::Extraction(ExtractionError::ContentUnavailable(_))
            ),
            "got: {}",
            err
        );
    }
}

#[rstest]
#[case::de(
    Country::De,
    "PL4fGSI1pDJn4X-OicSCOy-dChXWdTgziQ",
    "PL0sHkSjKd2rpxgOMD-vlUlIDqvQ5ChYJh"
)]
#[case::us(
    Country::Us,
    "PL4fGSI1pDJn69On1f-8NAvX_CYlx7QyZc",
    "PLrEnWoR732-DtKgaDdnPkezM_nDidBU9H"
)]
#[tokio::test]
async fn music_charts(#[case] country: Country, #[case] plid_top: &str, #[case] plid_trend: &str) {
    let rp = RustyPipe::builder().strict().build();
    let charts = rp.query().music_charts(Some(country)).await.unwrap();

    assert_eq!(charts.top_playlist_id.unwrap(), plid_top);
    assert_eq!(charts.trending_playlist_id.unwrap(), plid_trend);

    assert_gte(charts.top_tracks.len(), 40, "top tracks");
    assert_gte(charts.artists.len(), 40, "top artists");
    assert_gte(charts.trending_tracks.len(), 20, "trending tracks");

    // Chart playlists only available in USA
    if country == Country::Us {
        assert_gte(charts.playlists.len(), 8, "charts playlists");
    }
}

#[tokio::test]
async fn music_new_albums() {
    let rp = RustyPipe::builder().strict().build();
    let albums = rp.query().music_new_albums().await.unwrap();
    assert_gte(albums.len(), 10, "albums");

    for album in albums {
        assert_album_id(&album.id);
        assert!(!album.name.is_empty());
        assert!(!album.cover.is_empty(), "got no cover");
    }
}

#[tokio::test]
async fn music_new_videos() {
    let rp = RustyPipe::builder().strict().build();
    let videos = rp.query().music_new_videos().await.unwrap();
    assert_gte(videos.len(), 10, "videos");

    for video in videos {
        assert_video_id(&video.id);
        assert!(!video.title.is_empty());
        assert!(!video.cover.is_empty(), "got no cover");
        assert_gte(video.view_count.unwrap(), 1000, "views");
        assert!(video.is_video);
    }
}

#[tokio::test]
async fn music_genres() {
    let rp = RustyPipe::builder().strict().build();
    let genres = rp.query().music_genres().await.unwrap();

    let chill = genres
        .iter()
        .find(|g| g.id == "ggMPOg1uX1JOQWZFeDByc2Jm")
        .unwrap();
    assert_eq!(chill.name, "Chill");
    assert!(chill.is_mood);

    let pop = genres
        .iter()
        .find(|g| g.id == "ggMPOg1uX1lMbVZmbzl6NlJ3")
        .unwrap();
    assert_eq!(pop.name, "Pop");
    assert!(!pop.is_mood);

    genres.iter().for_each(|g| {
        assert_gte(g.color, 0xff000000, "color");
    });
}

#[rstest]
#[case::chill("ggMPOg1uX1JOQWZFeDByc2Jm", "Chill")]
#[case::pop("ggMPOg1uX1lMbVZmbzl6NlJ3", "Pop")]
#[tokio::test]
async fn music_genre(#[case] id: &str, #[case] name: &str) {
    let rp = RustyPipe::builder().strict().build();
    let genre = rp.query().music_genre(id).await.unwrap();

    fn check_music_genre(genre: MusicGenre, id: &str, name: &str) -> Vec<(String, String)> {
        assert_eq!(genre.id, id);
        assert_eq!(genre.name, name);
        assert_gte(genre.sections.len(), 2, "genre sections");

        let mut subgenres = Vec::new();
        genre.sections.iter().for_each(|section| {
            assert!(!section.name.is_empty());
            section.playlists.iter().for_each(|playlist| {
                assert_playlist_id(&playlist.id);
                assert!(!playlist.name.is_empty());
                assert!(!playlist.thumbnail.is_empty(), "got no cover");

                if !playlist.from_ytm {
                    assert!(
                        playlist.channel.is_some(),
                        "pl: {}, got no channel",
                        playlist.id
                    );
                    let channel = playlist.channel.as_ref().unwrap();
                    assert_channel_id(&channel.id);
                    assert!(!channel.name.is_empty());

                    assert_gte(playlist.track_count.unwrap(), 2, "tracks");
                } else {
                    assert!(playlist.channel.is_none());
                }
            });
            if let Some(subgenre_id) = &section.subgenre_id {
                subgenres.push((subgenre_id.to_owned(), section.name.to_owned()));
            }
        });
        subgenres
    }

    let subgenres = check_music_genre(genre, id, name);

    if name == "Chill" {
        assert_gte(subgenres.len(), 2, "subgenres");
    }

    for (id, name) in subgenres {
        let genre = rp.query().music_genre(&id).await.unwrap();
        check_music_genre(genre, &id, &name);
    }
}

#[tokio::test]
async fn music_genre_not_found() {
    let rp = RustyPipe::builder().strict().build();
    let err = rp
        .query()
        .music_genre("ggMPOg1uX1JOQWZFeDByc2zz")
        .await
        .unwrap_err();

    assert!(
        matches!(
            err,
            Error::Extraction(ExtractionError::ContentUnavailable(_))
        ),
        "got: {}",
        err
    );
}

//#AB TESTS

const VISITOR_DATA_SEARCH_CHANNEL_HANDLES: &str = "CgszYlc1Yk1WZGRCSSjrwOSbBg%3D%3D";

#[tokio::test]
async fn ab3_search_channel_handles() {
    let rp = RustyPipe::builder()
        .strict()
        .visitor_data(VISITOR_DATA_SEARCH_CHANNEL_HANDLES)
        .build();

    rp.query()
        .search_filter(
            "test",
            &SearchFilter::new().entity(search_filter::Entity::Channel),
        )
        .await
        .unwrap();
}

//#TESTUTIL

/// Assert equality within 10% margin
fn assert_approx(left: f64, right: f64) {
    if left != right {
        let f = left / right;
        assert!(
            0.9 < f && f < 1.1,
            "{} not within 10% margin of {}",
            left,
            right
        );
    }
}

fn assert_gte<T: PartialOrd + Display>(a: T, b: T, msg: &str) {
    assert!(a >= b, "expected {} {}, got {}", b, msg, a);
}

async fn assert_next<T: FromYtItem, Q: AsRef<RustyPipeQuery>>(
    paginator: Paginator<T>,
    query: Q,
    min_items: usize,
    n_pages: usize,
) {
    let mut p = paginator;
    let query = query.as_ref();
    assert_gte(p.items.len(), min_items, "items on page 0");

    for i in 0..n_pages {
        p = p.next(query).await.unwrap().expect("paginator exhausted");
        assert_gte(
            p.items.len(),
            min_items,
            &format!("items on page {}", i + 1),
        );
    }
}

async fn assert_next_items<T: FromYtItem, Q: AsRef<RustyPipeQuery>>(
    paginator: Paginator<T>,
    query: Q,
    n_items: usize,
) {
    let mut p = paginator;
    let query = query.as_ref();
    p.extend_limit(query, n_items).await.unwrap();
    assert_gte(p.items.len(), n_items, "items");
}

fn assert_video_id(id: &str) {
    static VIDEO_ID_REGEX: Lazy<Regex> = Lazy::new(|| Regex::new(r"^[A-Za-z0-9_-]{11}$").unwrap());

    assert!(
        VIDEO_ID_REGEX.is_match(id).unwrap_or_default(),
        "invalid video id: `{}`",
        id
    );
}

fn assert_channel_id(id: &str) {
    static CHANNEL_ID_REGEX: Lazy<Regex> =
        Lazy::new(|| Regex::new(r"^UC[A-Za-z0-9_-]{22}$").unwrap());

    assert!(
        CHANNEL_ID_REGEX.is_match(id).unwrap_or_default(),
        "invalid channel id: `{}`",
        id
    );
}

fn assert_album_id(id: &str) {
    static ALBUM_ID_REGEX: Lazy<Regex> =
        Lazy::new(|| Regex::new(r"^MPREb_[A-Za-z0-9_-]{11}$").unwrap());

    assert!(
        ALBUM_ID_REGEX.is_match(id).unwrap_or_default(),
        "invalid album id: `{}`",
        id
    );
}

fn assert_playlist_id(id: &str) {
    static PLAYLIST_ID_REGEX: Lazy<Regex> =
        Lazy::new(|| Regex::new(r"^(?:PL|RD|OLAK)[A-Za-z0-9_-]{30,}$").unwrap());

    assert!(
        PLAYLIST_ID_REGEX.is_match(id).unwrap_or_default(),
        "invalid album id: `{}`",
        id
    );
}
