use std::collections::HashSet;

use chrono::Datelike;
use rstest::rstest;

use rustypipe::client::{ClientType, RustyPipe};
use rustypipe::error::{Error, ExtractionError};
use rustypipe::model::richtext::ToPlaintext;
use rustypipe::model::{
    AudioCodec, AudioFormat, Channel, SearchItem, UrlTarget, Verification, VideoCodec, VideoFormat,
};
use rustypipe::param::{
    search_filter::{self, SearchFilter},
    ChannelOrder,
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
    assert!(player_data.details.view_count > 146_818_808);
    assert_eq!(player_data.details.keywords[0], "spektrem");
    assert_eq!(player_data.details.is_live_content, false);

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
    }

    assert!(player_data.expires_in_seconds > 10000);
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
    "Live NASA - Views Of Earth from Space",
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
    assert!(
        details.view_count > views,
        "expected > {} views, got {}",
        views,
        details.view_count
    );
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

    assert!(player_data.expires_in_seconds > 10000);
}

#[rstest]
#[case::not_found("86abcdefghi", "extraction error: Video cant be played because of deletion/censorship. Reason (from YT): This video is unavailable")]
#[case::deleted("64DYi_8ESh0", "extraction error: Video cant be played because of deletion/censorship. Reason (from YT): This video is unavailable")]
#[case::censored("6SJNVb0GnPI", "extraction error: Video cant be played because of deletion/censorship. Reason (from YT): This video has been removed for violating YouTube's policy on hate speech. Learn more about combating hate speech in your country.")]
// This video is geoblocked outside of Japan, so expect this test case to fail when using a Japanese IP address.
#[case::geoblock("sJL6WA-aGkQ", "extraction error: Video cant be played because of DRM/Geoblock. Reason (from YT): The uploader has not made this video available in your country")]
#[case::drm("1bfOsni7EgI", "extraction error: Video cant be played because of DRM/Geoblock. Reason (from YT): This video can only be played on newer versions of Android or other supported devices.")]
#[case::private("s7_qI6_mIXc", "extraction error: Video cant be played because of private video. Reason (from YT): This video is private")]
#[case::t1("CUO8secmc0g", "extraction error: Video cant be played because of DRM/Geoblock. Reason (from YT): Playback on other websites has been disabled by the video owner")]
#[tokio::test]
async fn get_player_error(#[case] id: &str, #[case] msg: &str) {
    let rp = RustyPipe::builder().strict().build();
    let err = rp.query().player(id).await.unwrap_err();

    assert_eq!(err.to_string(), msg);
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
    assert!(
        details.channel.subscriber_count.unwrap() > 30000000,
        "expected >30M subs, got {}",
        details.channel.subscriber_count.unwrap()
    );

    assert!(
        details.view_count > 232000000,
        "expected > 232M views, got {}",
        details.view_count
    );
    assert!(
        details.like_count.unwrap() > 4000000,
        "expected > 4M likes, got {}",
        details.like_count.unwrap()
    );

    let date = details.publish_date.unwrap();
    assert_eq!(date.year(), 2020);
    assert_eq!(date.month(), 11);
    assert_eq!(date.day(), 17);

    assert!(!details.is_live);
    assert!(!details.is_ccommons);

    assert!(!details.recommended.items.is_empty());
    assert!(!details.recommended.is_exhausted());

    assert!(
        details.top_comments.count.unwrap() > 700000,
        "expected > 700K comments, got {}",
        details.top_comments.count.unwrap()
    );
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
    assert!(
        details.channel.subscriber_count.unwrap() > 33000,
        "expected >33K subs, got {}",
        details.channel.subscriber_count.unwrap()
    );

    assert!(
        details.view_count > 20309,
        "expected > 20309 views, got {}",
        details.view_count
    );
    assert!(
        details.like_count.unwrap() > 145,
        "expected > 145 likes, got {}",
        details.like_count.unwrap()
    );

    let date = details.publish_date.unwrap();
    assert_eq!(date.year(), 2020);
    assert_eq!(date.month(), 8);
    assert_eq!(date.day(), 6);

    assert!(!details.is_live);
    assert!(!details.is_ccommons);

    assert!(!details.recommended.items.is_empty());
    assert!(!details.recommended.is_exhausted());

    // Comments are disabled for this video
    assert_eq!(details.top_comments.count, Some(0));
    assert_eq!(details.latest_comments.count, Some(0));
    assert!(details.top_comments.is_empty());
    assert!(details.latest_comments.is_empty());
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
    assert!(
        details.channel.subscriber_count.unwrap() > 170000,
        "expected >170K subs, got {}",
        details.channel.subscriber_count.unwrap()
    );

    assert!(
        details.view_count > 2517358,
        "expected > 2517358 views, got {}",
        details.view_count
    );
    assert!(
        details.like_count.unwrap() > 52330,
        "expected > 52330 likes, got {}",
        details.like_count.unwrap()
    );

    let date = details.publish_date.unwrap();
    assert_eq!(date.year(), 2019);
    assert_eq!(date.month(), 12);
    assert_eq!(date.day(), 29);

    assert!(!details.is_live);
    assert!(details.is_ccommons);

    assert!(!details.recommended.items.is_empty());
    assert!(!details.recommended.is_exhausted());

    assert!(
        details.top_comments.count.unwrap() > 2199,
        "expected > 2199 comments, got {}",
        details.top_comments.count.unwrap()
    );
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
    assert!(
        details.channel.subscriber_count.unwrap() > 14700000,
        "expected >14.7M subs, got {}",
        details.channel.subscriber_count.unwrap()
    );

    assert!(
        details.view_count > 1157262,
        "expected > 1157262 views, got {}",
        details.view_count
    );
    assert!(
        details.like_count.unwrap() > 54670,
        "expected > 54670 likes, got {}",
        details.like_count.unwrap()
    );

    let date = details.publish_date.unwrap();
    assert_eq!(date.year(), 2022);
    assert_eq!(date.month(), 9);
    assert_eq!(date.day(), 15);

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

    assert!(!details.recommended.items.is_empty());
    assert!(!details.recommended.is_exhausted());

    assert!(
        details.top_comments.count.unwrap() > 3199,
        "expected > 3199 comments, got {}",
        details.top_comments.count.unwrap()
    );
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
        desc.contains("Live NASA - Views Of Earth from Space"),
        "bad description: {}",
        desc
    );

    assert_eq!(details.channel.id, "UCakgsb0w7QB0VHdnCc-OVEA");
    assert_eq!(details.channel.name, "Space Videos");
    assert!(!details.channel.avatar.is_empty(), "no channel avatars");
    assert_eq!(details.channel.verification, Verification::Verified);
    assert!(
        details.channel.subscriber_count.unwrap() > 5500000,
        "expected >5.5M subs, got {}",
        details.channel.subscriber_count.unwrap()
    );

    assert!(
        details.view_count > 10,
        "expected > 10 views, got {}",
        details.view_count
    );
    assert!(
        details.like_count.unwrap() > 872290,
        "expected > 872290 likes, got {}",
        details.like_count.unwrap()
    );

    let date = details.publish_date.unwrap();
    assert_eq!(date.year(), 2021);
    assert_eq!(date.month(), 9);
    assert_eq!(date.day(), 23);

    assert!(details.is_live);
    assert!(!details.is_ccommons);

    assert!(!details.recommended.items.is_empty());
    assert!(!details.recommended.is_exhausted());

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
    assert!(
        details.channel.subscriber_count.unwrap() > 1400,
        "expected >1400 subs, got {}",
        details.channel.subscriber_count.unwrap()
    );

    assert!(
        details.view_count > 200,
        "expected > 200 views, got {}",
        details.view_count
    );
    assert!(details.like_count.is_none(), "like count not hidden");

    let date = details.publish_date.unwrap();
    assert_eq!(date.year(), 2019);
    assert_eq!(date.month(), 1);
    assert_eq!(date.day(), 2);

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
async fn get_video_recommendations() {
    let rp = RustyPipe::builder().strict().build();
    let details = rp.query().video_details("ZeerrnuLi5E").await.unwrap();
    let next_recommendations = details.recommended.next(rp.query()).await.unwrap().unwrap();
    // dbg!(&next_recommendations);

    assert!(
        next_recommendations.items.len() > 10,
        "expected > 10 next recommendations, got {}",
        next_recommendations.items.len()
    );
    assert!(!next_recommendations.is_exhausted());
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
    assert!(
        top_comments.items.len() > 10,
        "expected > 10 next comments, got {}",
        top_comments.items.len()
    );
    assert!(!top_comments.is_exhausted());

    let n_comments = top_comments.count.unwrap();
    assert!(
        n_comments > 700000,
        "expected > 700k comments, got {}",
        n_comments
    );
    // Comment count should be exact after fetching first page
    assert!(n_comments % 1000 != 0);

    let latest_comments = details
        .latest_comments
        .next(rp.query())
        .await
        .unwrap()
        .unwrap();
    assert!(
        latest_comments.items.len() > 10,
        "expected > 10 next comments, got {}",
        latest_comments.items.len()
    );
    assert!(!latest_comments.is_exhausted());
}

//#CHANNEL

#[rstest]
#[case::latest(ChannelOrder::Latest)]
#[case::oldest(ChannelOrder::Oldest)]
#[case::popular(ChannelOrder::Popular)]
#[tokio::test]
async fn channel_videos(#[case] order: ChannelOrder) {
    let rp = RustyPipe::builder().strict().build();
    let channel = rp
        .query()
        .channel_videos_ordered("UC2DjFE7Xf11URZqWBigcVOQ", order)
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
    let age_days = (chrono::Local::now() - first_video_date).num_days();

    match order {
        ChannelOrder::Latest => {
            assert!(age_days < 60, "latest video older than 60 days")
        }
        ChannelOrder::Oldest => {
            assert!(age_days > 4700, "oldest video newer than 4700 days")
        }
        ChannelOrder::Popular => {
            assert!(
                first_video.view_count > 2300000,
                "most popular video < 2.3M views"
            )
        }
        _ => unimplemented!(),
    }

    let next = channel.content.next(rp.query()).await.unwrap().unwrap();
    assert!(
        !next.is_exhausted() && !next.items.is_empty(),
        "no more videos"
    );
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

    let next = channel.content.next(rp.query()).await.unwrap().unwrap();
    assert!(
        !next.is_exhausted() && !next.items.is_empty(),
        "no more playlists"
    );
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
    assert_eq!(created.year(), 2009);
    assert_eq!(created.month(), 4);
    assert_eq!(created.day(), 4);

    assert!(
        channel.content.view_count.unwrap() > 186854340,
        "exp >186M views, got {}",
        channel.content.view_count.unwrap()
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
    assert!(
        channel.subscriber_count.unwrap() > 880000,
        "exp >880K subscribers, got {}",
        channel.subscriber_count.unwrap()
    );
    assert!(!channel.avatar.is_empty(), "got no thumbnails");
    assert_eq!(channel.description, "NO SCRIPT, NO FEAR, ALL OPINION\nAn off-the-cuff Video Blog about Electronics Engineering, for engineers, hobbyists, enthusiasts, hackers and Makers\nHosted by Dave Jones from Sydney Australia\n\nDONATIONS:\nBitcoin: 3KqyH1U3qrMPnkLufM2oHDU7YB4zVZeFyZ\nEthereum: 0x99ccc4d2654ba40744a1f678d9868ecb15e91206\nPayPal: david@alternatezone.com\n\nPatreon: https://www.patreon.com/eevblog\n\nEEVblog2: http://www.youtube.com/EEVblog2\nEEVdiscover: https://www.youtube.com/channel/UCkGvUEt8iQLmq3aJIMjT2qQ\n\nEMAIL:\nAdvertising/Commercial: eevblog+business@gmail.com\nFan mail: eevblog+fan@gmail.com\nHate Mail: eevblog+hate@gmail.com\n\nI DON'T DO PAID VIDEO SPONSORSHIPS, DON'T ASK!\n\nPLEASE:\nDo NOT ask for personal advice on something, post it in the EEVblog forum.\nI read ALL email, but please don't be offended if I don't have time to reply, I get a LOT of email.\n\nMailbag\nPO Box 7949\nBaulkham Hills NSW 2153\nAUSTRALIA");
    assert!(!channel.tags.is_empty(), "got no tags");
    assert_eq!(
        channel.vanity_url.as_ref().unwrap(),
        "https://www.youtube.com/c/EevblogDave"
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

    use chrono::Timelike;

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
        assert_eq!(channel.create_date.year(), 2010);
        assert_eq!(channel.create_date.month(), 7);
        assert_eq!(channel.create_date.day(), 21);
        assert_eq!(channel.create_date.hour(), 7);
        assert_eq!(channel.create_date.minute(), 18);

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
    assert!(
        result.items.items.len() > 10,
        "expected > 10 search results, got {}",
        result.items.items.len()
    );
    assert!(!result.items.is_exhausted());

    assert_eq!(result.corrected_query.unwrap(), "doobydobap");
}

#[rstest]
#[case::video(search_filter::Entity::Video)]
#[case::video(search_filter::Entity::Channel)]
#[case::video(search_filter::Entity::Playlist)]
#[tokio::test]
async fn search_filter_entity(#[case] entity: search_filter::Entity) {
    let rp = RustyPipe::builder().strict().build();
    let result = rp
        .query()
        .search_filter("music", &SearchFilter::new().entity(entity))
        .await
        .unwrap();

    assert!(
        result.items.items.len() > 10,
        "expected > 10 search results, got {}",
        result.items.items.len()
    );
    assert!(!result.items.is_exhausted());

    result.items.items.iter().for_each(|item| match item {
        SearchItem::Video(_) => {
            assert_eq!(entity, search_filter::Entity::Video);
        }
        SearchItem::Channel(_) => {
            assert_eq!(entity, search_filter::Entity::Channel);
        }
        SearchItem::Playlist(_) => {
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
#[tokio::test]
async fn resolve_url(#[case] url: &str, #[case] expect: UrlTarget) {
    let rp = RustyPipe::builder().strict().build();
    let target = rp.query().resolve_url(url).await.unwrap();
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
#[tokio::test]
async fn resolve_string(#[case] string: &str, #[case] expect: UrlTarget) {
    let rp = RustyPipe::builder().strict().build();
    let target = rp.query().resolve_string(string).await.unwrap();
    assert_eq!(target, expect);
}

#[tokio::test]
async fn resolve_channel_not_found() {
    let rp = RustyPipe::builder().strict().build();
    let err = rp
        .query()
        .resolve_url("https://www.youtube.com/feeqegnhq3rkwghjq43ruih43io3")
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
    let result = rp.query().startpage().await.unwrap();

    assert!(
        result.items.len() >= 20,
        "expected >= 20 items, got {}",
        result.items.len()
    );
    assert!(!result.is_exhausted());
}

#[tokio::test]
async fn startpage_cont() {
    let rp = RustyPipe::builder().strict().build();
    let startpage = rp.query().startpage().await.unwrap();

    let next = startpage.next(rp.query()).await.unwrap().unwrap();

    assert!(
        next.items.len() >= 20,
        "expected >= 20 items, got {}",
        next.items.len()
    );
    assert!(!next.is_exhausted());
}

#[tokio::test]
async fn trending() {
    let rp = RustyPipe::builder().strict().build();
    let result = rp.query().trending().await.unwrap();

    assert!(
        result.len() >= 50,
        "expected >= 50 items, got {}",
        result.len()
    );
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
