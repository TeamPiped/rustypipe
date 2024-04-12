#![allow(clippy::too_many_arguments, clippy::items_after_test_module)]

use std::collections::{HashMap, HashSet};
use std::fmt::Display;
use std::str::FromStr;

use rstest::{fixture, rstest};
use time::{macros::date, OffsetDateTime};

use rustypipe::client::{ClientType, RustyPipe, RustyPipeQuery};
use rustypipe::error::{Error, ExtractionError, UnavailabilityReason};
use rustypipe::model::{
    paginator::{ContinuationEndpoint, Paginator},
    richtext::ToPlaintext,
    traits::{FromYtItem, YtStream},
    AlbumType, AudioCodec, AudioFormat, AudioTrackType, Channel, Frameset, MusicGenre, MusicItem,
    UrlTarget, Verification, VideoCodec, VideoFormat, YouTubeItem,
};
use rustypipe::param::{
    search_filter::{self, SearchFilter},
    ChannelOrder, ChannelVideoTab, Country, Language,
};
use rustypipe::validate;

//#PLAYER

#[rstest]
#[case::desktop(ClientType::Desktop)]
#[case::tv_html5_embed(ClientType::TvHtml5Embed)]
#[case::android(ClientType::Android)]
#[case::ios(ClientType::Ios)]
#[tokio::test]
async fn get_player_from_client(#[case] client_type: ClientType, rp: RustyPipe) {
    let player_data = rp
        .query()
        .player_from_client("n4tK7LYFxI0", client_type)
        .await
        .unwrap();

    // dbg!(&player_data);

    assert_eq!(player_data.details.id, "n4tK7LYFxI0");
    assert_eq!(
        player_data.details.name,
        "Spektrem - Shine | Progressive House | NCS - Copyright Free Music"
    );
    if client_type == ClientType::DesktopMusic {
        assert!(player_data.details.description.is_none());
    } else {
        assert!(player_data
            .details
            .description
            .expect("description")
            .contains(
            "NCS (NoCopyrightSounds): Empowering Creators through Copyright / Royalty Free Music"
        ));
    }
    assert_eq!(player_data.details.length, 259);
    assert!(!player_data.details.thumbnail.is_empty());
    assert_eq!(player_data.details.channel.id, "UC_aEa8K-EOJ3D6gOs7HcyNg");
    assert_eq!(player_data.details.channel.name, "NoCopyrightSounds");
    assert_gte(player_data.details.view_count, 146_818_808, "view count");
    assert_eq!(player_data.details.keywords[0], "spektrem");
    assert!(!player_data.details.is_live_content);

    if client_type == ClientType::Ios {
        let video = player_data
            .video_only_streams
            .into_iter()
            .find(|s| s.itag == 247)
            .expect("video #247");
        let audio = player_data
            .audio_streams
            .into_iter()
            .find(|s| s.itag == 140)
            .expect("audio #140");

        // Bitrates may change between requests
        assert_approx(f64::from(video.bitrate), 1_851_854.0);
        assert_eq!(video.average_bitrate, 923_766);
        assert_eq!(video.size, Some(29_909_835));
        assert_eq!(video.width, 1280);
        assert_eq!(video.height, 720);
        assert_eq!(video.fps, 30);
        assert_eq!(video.quality, "720p");
        assert!(!video.hdr);
        assert_eq!(video.mime, "video/webm; codecs=\"vp09.00.31.08\"");
        assert_eq!(video.format, VideoFormat::Webm);
        assert_eq!(video.codec, VideoCodec::Vp9);

        assert_approx(f64::from(audio.bitrate), 130_685.0);
        assert_approx(f64::from(audio.average_bitrate), 129_496.0);
        assert_approx(audio.size as f64, 4_193_863.0);
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
            .expect("video stream not found");
        let audio = player_data
            .audio_streams
            .into_iter()
            .find(|s| s.itag == 251)
            .expect("audio stream not found");

        assert_approx(f64::from(video.bitrate), 1_340_829.0);
        assert_approx(f64::from(video.average_bitrate), 1_046_557.0);
        assert_approx(video.size.expect("video size") as f64, 33_885_572.0);
        assert_eq!(video.width, 1280);
        assert_eq!(video.height, 720);
        assert_eq!(video.fps, 30);
        assert_eq!(video.quality, "720p");
        assert!(!video.hdr);
        assert_eq!(video.mime, "video/mp4; codecs=\"av01.0.05M.08\"");
        assert_eq!(video.format, VideoFormat::Mp4);
        assert_eq!(video.codec, VideoCodec::Av01);
        assert!(!video.throttled);

        assert_approx(f64::from(audio.bitrate), 142_718.0);
        assert_approx(f64::from(audio.average_bitrate), 130_708.0);
        assert_approx(audio.size as f64, 4_232_344.0);
        assert_eq!(audio.mime, "audio/webm; codecs=\"opus\"");
        assert_eq!(audio.format, AudioFormat::Webm);
        assert_eq!(audio.codec, AudioCodec::Opus);
        assert!(!audio.throttled);

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
        assert_eq!(resp.content_length(), Some(size))
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
    830_900,
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
    "100 Boys Vs 100 Girls For $500,000",
    "Giving away $25k on Current!",
    1013,
    "UCX6OQ3DkcsbYNE6H8uQQuVA",
    "MrBeast",
    82_000_000,
    false,
    false
)]
#[case::live(
    "jfKfPfyJRdk",
    "lofi hip hop radio 📚 - beats to relax/study to",
    "Listen on Spotify, Apple music and more",
    0,
    "UCSJ4gkVC6NrvII8umztf0Ow",
    "Lofi Girl",
    100,
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
#[allow(clippy::too_many_arguments)]
async fn get_player(
    #[case] id: &str,
    #[case] name: &str,
    #[case] description: &str,
    #[case] length: u32,
    #[case] channel_id: &str,
    #[case] channel_name: &str,
    #[case] views: u64,
    #[case] is_live: bool,
    #[case] is_live_content: bool,
    rp: RustyPipe,
) {
    let player_data = rp.query().player(id).await.unwrap();
    let details = player_data.details;

    assert_eq!(details.id, id);
    assert_eq!(details.name, name);
    let desc = details.description.expect("description");
    assert!(desc.contains(description), "description: {desc}");
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
                        .map(|t| (t.lang.as_deref().unwrap(), t.track_type.unwrap()))
                })
                .collect::<HashMap<_, _>>();

            assert_eq!(
                langs.get("en-US"),
                Some(&AudioTrackType::Original),
                "missing lang: en-US"
            );

            for l in ["es", "fr", "pt", "ru"] {
                assert_eq!(
                    langs.get(l),
                    Some(&AudioTrackType::Dubbed),
                    "missing lang: {l}"
                );
            }
        }
        _ => {}
    };

    assert_gte(player_data.expires_in_seconds, 10_000, "expiry time");

    if !is_live {
        assert_gte(player_data.preview_frames.len(), 3, "preview framesets");
        player_data.preview_frames.iter().for_each(assert_frameset);
    }
}

#[rstest]
#[case::not_found("86abcdefghi", UnavailabilityReason::Deleted)]
#[case::deleted("64DYi_8ESh0", UnavailabilityReason::Deleted)]
#[case::censored("6SJNVb0GnPI", UnavailabilityReason::Deleted)]
// This video is geoblocked outside of Japan, so expect this test case to fail when using a Japanese IP address.
#[case::geoblock("sJL6WA-aGkQ", UnavailabilityReason::Geoblocked)]
#[case::private("s7_qI6_mIXc", UnavailabilityReason::Private)]
#[case::age_restricted("CUO8secmc0g", UnavailabilityReason::AgeRestricted)]
#[case::premium_only("3LvozjEOUxU", UnavailabilityReason::Premium)]
#[case::members_only("vYmAhoZYg64", UnavailabilityReason::MembersOnly)]
#[tokio::test]
async fn get_player_error(#[case] id: &str, #[case] expect: UnavailabilityReason, rp: RustyPipe) {
    let err = rp.query().player(id).await.unwrap_err();

    match err {
        Error::Extraction(ExtractionError::Unavailable { reason, .. }) => {
            assert_eq!(reason, expect, "got {err}")
        }
        _ => panic!("got {err}"),
    }
}

#[rstest]
#[tokio::test]
async fn get_player_error_paid(rp: RustyPipe) {
    let err = rp.query().player("N8ee9OLumrs").await.unwrap_err();
    match err {
        // Sometimes YouTube shows an 'unplayable' error on paid videos
        Error::Extraction(ExtractionError::Unavailable { reason, .. }) => {
            assert!(
                matches!(
                    reason,
                    UnavailabilityReason::Paid | UnavailabilityReason::Unplayable
                ),
                "got {err}"
            )
        }
        _ => panic!("got {err}"),
    }
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
#[case::nomusic(
    "PL1J-6JOckZtE_P9Xx8D3b2O6w0idhuKBe",
    "Minecraft SHINE",
    false,
    Some("SHINE - Survival Hardcore in New Environment: Auf einem Server machen sich tapfere Spieler auf, mystische Welten zu erkunden, magische Technologien zu erforschen und vorallem zu überleben...".to_owned()),
    Some(("UCQM0bS4_04-Y4JuYrgmnpZQ", "Chaosflo44")),
)]
#[case::live(
    "UULVvqRdlKsE5Q8mf8YXbdIJLw",
    "Live streams",
    true,
    None,
    Some(("UCvqRdlKsE5Q8mf8YXbdIJLw", "LoL Esports"))
)]
#[tokio::test]
async fn get_playlist(
    #[case] id: &str,
    #[case] name: &str,
    #[case] is_long: bool,
    #[case] description: Option<String>,
    #[case] channel: Option<(&str, &str)>,
    rp: RustyPipe,
    unlocalized: bool,
) {
    let playlist = rp.query().playlist(id).await.unwrap();

    assert_eq!(playlist.id, id);
    if unlocalized {
        assert_eq!(playlist.name, name);
    }
    assert!(!playlist.videos.is_empty());
    assert_eq!(!playlist.videos.is_exhausted(), is_long);
    assert_gte(
        playlist.video_count,
        if is_long { 100 } else { 10 },
        "track count",
    );
    assert_eq!(playlist.description.map(|d| d.to_plaintext()), description);

    if let Some(expect) = channel {
        let c = playlist.channel.expect("channel");
        assert_eq!(c.id, expect.0);
        assert_eq!(c.name, expect.1);
    }
    assert!(!playlist.thumbnail.is_empty());
}

#[rstest]
#[tokio::test]
async fn playlist_cont(rp: RustyPipe) {
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
    assert_gte(playlist.videos.items.len(), 101, "video items");
    assert_gteo(playlist.videos.count, 101, "video count");
}

#[rstest]
#[tokio::test]
async fn playlist_cont2(rp: RustyPipe) {
    let mut playlist = rp
        .query()
        .playlist("PLbZIPy20-1pN7mqjckepWF78ndb6ci_qi")
        .await
        .unwrap();

    playlist.videos.extend_limit(rp.query(), 101).await.unwrap();
    assert_gte(playlist.videos.items.len(), 101, "video items");
    assert_gteo(playlist.videos.count, 101, "video count");
}

#[rstest]
#[tokio::test]
async fn playlist_not_found(rp: RustyPipe) {
    let err = rp
        .query()
        .playlist("PLbZIPy20-1pN7mqjckepWF78ndb6ci_qz")
        .await
        .unwrap_err();

    assert!(
        matches!(err, Error::Extraction(ExtractionError::NotFound { .. })),
        "got: {err}"
    );
}

//#VIDEO DETAILS

#[rstest]
#[tokio::test]
async fn get_video_details(rp: RustyPipe) {
    let details = rp.query().video_details("ZeerrnuLi5E").await.unwrap();

    // dbg!(&details);

    assert_eq!(details.id, "ZeerrnuLi5E");
    assert_eq!(details.name, "aespa 에스파 'Black Mamba' MV");
    let desc = details.description.to_plaintext();
    assert!(
        desc.contains("Listen and download aespa's debut single \"Black Mamba\""),
        "bad description: {desc}"
    );

    assert_eq!(details.channel.id, "UCEf_Bc-KVd7onSeifS3py9g");
    assert_eq!(details.channel.name, "SMTOWN");
    assert!(!details.channel.avatar.is_empty(), "no channel avatars");
    assert_eq!(details.channel.verification, Verification::Verified);
    assert_gteo(details.channel.subscriber_count, 30_000_000, "subscribers");
    assert_gte(details.view_count, 232_000_000, "views");
    assert_gteo(details.like_count, 4_000_000, "likes");

    let date = details.publish_date.expect("publish_date");
    assert_eq!(date.date(), date!(2020 - 11 - 17));

    assert!(!details.is_live);
    assert!(!details.is_ccommons);

    assert!(details.recommended.visitor_data.is_some());
    assert_next(details.recommended, rp.query(), 10, 1, false).await;

    assert_gteo(details.top_comments.count, 700_000, "comments");
    assert!(!details.top_comments.is_exhausted());
    assert!(!details.latest_comments.is_exhausted());
}

#[rstest]
#[tokio::test]
async fn get_video_details_music(rp: RustyPipe) {
    let details = rp.query().video_details("XuM2onMGvTI").await.unwrap();

    // dbg!(&details);

    assert_eq!(details.id, "XuM2onMGvTI");
    assert_eq!(details.name, "Gäa");
    let desc = details.description.to_plaintext();
    assert!(desc.contains("Gäa · Oonagh"), "bad description: {desc}");

    assert_eq!(details.channel.id, "UCVGvnqB-5znqPSbMGlhF4Pw");
    assert_eq!(details.channel.name, "Sentamusic");
    assert!(!details.channel.avatar.is_empty(), "no channel avatars");
    assert_eq!(details.channel.verification, Verification::Artist);
    assert_gteo(details.channel.subscriber_count, 33_000, "subscribers");
    assert_gte(details.view_count, 20_309, "views");
    assert_gteo(details.like_count, 145, "likes");

    let date = details.publish_date.unwrap();
    assert_eq!(date.date(), date!(2020 - 8 - 6));

    assert!(!details.is_live);
    assert!(!details.is_ccommons);

    assert!(details.recommended.visitor_data.is_some());
    assert_next(details.recommended, rp.query(), 10, 1, false).await;

    // Update(01.11.2022): comments are sometimes enabled
    /*
    assert_eq!(details.top_comments.count, Some(0));
    assert_eq!(details.latest_comments.count, Some(0));
    assert!(details.top_comments.is_empty());
    assert!(details.latest_comments.is_empty());
    */
}

#[rstest]
#[tokio::test]
async fn get_video_details_ccommons(rp: RustyPipe) {
    let details = rp.query().video_details("0rb9CfOvojk").await.unwrap();

    // dbg!(&details);

    assert_eq!(details.id, "0rb9CfOvojk");
    assert_eq!(
        details.name,
        "BahnMining - Pünktlichkeit ist eine Zier (David Kriesel)"
    );
    let desc = details.description.to_plaintext();
    assert!(
            desc.contains("Seit Anfang 2019 hat David jeden einzelnen Halt jeder einzelnen Zugfahrt auf jedem einzelnen Fernbahnhof in ganz Deutschland"),
            "bad description: {desc}"
        );

    assert_eq!(details.channel.id, "UC2TXq_t06Hjdr2g_KdKpHQg");
    assert_eq!(details.channel.name, "media.ccc.de");
    assert!(!details.channel.avatar.is_empty(), "no channel avatars");
    assert_eq!(details.channel.verification, Verification::None);
    assert_gteo(details.channel.subscriber_count, 170_000, "subscribers");
    assert_gte(details.view_count, 2_517_358, "views");
    assert_gteo(details.like_count, 52_330, "likes");

    let date = details.publish_date.expect("publish_date");
    assert_eq!(date.date(), date!(2019 - 12 - 29));

    assert!(!details.is_live);
    assert!(details.is_ccommons);

    assert!(details.recommended.visitor_data.is_some());
    assert_next(details.recommended, rp.query(), 10, 1, false).await;

    assert_eq!(details.top_comments.count.unwrap(), 0);
    assert!(details.top_comments.is_exhausted());
    assert!(details.latest_comments.is_exhausted());
}

#[rstest]
#[tokio::test]
async fn get_video_details_chapters(rp: RustyPipe) {
    let details = rp.query().video_details("nFDBxBUfE74").await.unwrap();

    // dbg!(&details);

    assert_eq!(details.id, "nFDBxBUfE74");
    assert_eq!(details.name, "The Prepper PC");
    let desc = details.description.to_plaintext();
    assert!(
            desc.contains("These days, you can game almost anywhere on the planet, anytime. But what if that planet was in the middle of an apocalypse"),
            "bad description: {desc}"
        );

    assert_eq!(details.channel.id, "UCXuqSBlHAE6Xw-yeJA0Tunw");
    assert_eq!(details.channel.name, "Linus Tech Tips");
    assert!(!details.channel.avatar.is_empty(), "no channel avatars");
    assert_eq!(details.channel.verification, Verification::Verified);
    assert_gteo(details.channel.subscriber_count, 14_700_000, "subscribers");
    assert_gte(details.view_count, 1_157_262, "views");
    assert_gteo(details.like_count, 54_670, "likes");

    let date = details.publish_date.expect("publish_date");
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
            name: "Intro",
            position: 0,
            thumbnail: "[ok]",
          ),
          Chapter(
            name: "The PC Built for Super Efficiency",
            position: 42,
            thumbnail: "[ok]",
          ),
          Chapter(
            name: "Our BURIAL ENCLOSURE?!",
            position: 161,
            thumbnail: "[ok]",
          ),
          Chapter(
            name: "Our Power Solution (Thanks Jackery!)",
            position: 211,
            thumbnail: "[ok]",
          ),
          Chapter(
            name: "Diggin\' Holes",
            position: 287,
            thumbnail: "[ok]",
          ),
          Chapter(
            name: "Colonoscopy?",
            position: 330,
            thumbnail: "[ok]",
          ),
          Chapter(
            name: "Diggin\' like a man",
            position: 424,
            thumbnail: "[ok]",
          ),
          Chapter(
            name: "The world\'s worst woodsman",
            position: 509,
            thumbnail: "[ok]",
          ),
          Chapter(
            name: "Backyard cable management",
            position: 543,
            thumbnail: "[ok]",
          ),
          Chapter(
            name: "Time to bury this boy",
            position: 602,
            thumbnail: "[ok]",
          ),
          Chapter(
            name: "Solar Power Generation",
            position: 646,
            thumbnail: "[ok]",
          ),
          Chapter(
            name: "Issues",
            position: 697,
            thumbnail: "[ok]",
          ),
          Chapter(
            name: "First Play Test",
            position: 728,
            thumbnail: "[ok]",
          ),
          Chapter(
            name: "Conclusion",
            position: 800,
            thumbnail: "[ok]",
          ),
        ]
        "###);
    }

    assert!(details.recommended.visitor_data.is_some());
    assert_next(details.recommended, rp.query(), 10, 1, false).await;

    assert_gteo(details.top_comments.count, 3000, "comments");
    assert!(!details.top_comments.is_exhausted());
    assert!(!details.latest_comments.is_exhausted());
}

#[rstest]
#[tokio::test]
async fn get_video_details_live(rp: RustyPipe) {
    let details = rp.query().video_details("jfKfPfyJRdk").await.unwrap();

    // dbg!(&details);

    assert_eq!(details.id, "jfKfPfyJRdk");
    assert_eq!(
        details.name,
        "lofi hip hop radio 📚 - beats to relax/study to"
    );
    let desc = details.description.to_plaintext();
    assert!(
        desc.contains("Listen on Spotify, Apple music and more"),
        "bad description: {desc}"
    );

    assert_eq!(details.channel.id, "UCSJ4gkVC6NrvII8umztf0Ow");
    assert_eq!(details.channel.name, "Lofi Girl");
    assert!(!details.channel.avatar.is_empty(), "no channel avatars");
    assert_eq!(details.channel.verification, Verification::Verified);
    assert_gteo(details.channel.subscriber_count, 5_500_000, "subscribers");
    assert_gte(details.view_count, 100, "views");
    assert_gteo(details.like_count, 1_800_000, "likes");

    let date = details.publish_date.expect("publish_date");
    assert_eq!(date.date(), date!(2022 - 7 - 12));

    assert!(details.is_live);
    assert!(!details.is_ccommons);

    assert!(details.recommended.visitor_data.is_some());
    assert_next(details.recommended, rp.query(), 10, 1, false).await;

    // No comments because livestream
    assert_eq!(details.top_comments.count, Some(0));
    assert_eq!(details.latest_comments.count, Some(0));
    assert!(details.top_comments.is_empty());
    assert!(details.latest_comments.is_empty());
}

#[rstest]
#[tokio::test]
async fn get_video_details_agegate(rp: RustyPipe) {
    let details = rp.query().video_details("laru0QoJUmI").await.unwrap();

    // dbg!(&details);

    assert_eq!(details.id, "laru0QoJUmI");
    assert_eq!(details.name, "DJ Robin x Schürze - Layla (Official Video)");
    insta::assert_ron_snapshot!(details.description, @"RichText([])");

    assert_eq!(details.channel.id, "UCkJfSrMnLonOZWh-q5os5bg");
    assert_eq!(details.channel.name, "Summerfield Records");
    assert!(!details.channel.avatar.is_empty(), "no channel avatars");
    assert_eq!(details.channel.verification, Verification::Verified);
    assert_gteo(details.channel.subscriber_count, 250_000, "subscribers");
    assert_gte(details.view_count, 10_000_000, "views");
    assert_gteo(details.like_count, 150_000, "likes");

    let date = details.publish_date.expect("publish_date");
    assert_eq!(date.date(), date!(2022 - 5 - 13));

    assert!(!details.is_live);
    assert!(!details.is_ccommons);

    // No recommendations because agegate
    assert_eq!(details.recommended.count, Some(0));
    assert!(details.recommended.items.is_empty());
}

#[rstest]
#[tokio::test]
async fn get_video_details_no_desc(rp: RustyPipe) {
    let details = rp.query().video_details("BXpTGEEZpV8").await.unwrap();

    assert_eq!(details.id, "BXpTGEEZpV8");
    assert_eq!(
        details.name,
        "The Streets Make Me Feel Alive  Stitches  Official Audio"
    );
    assert_eq!(details.channel.id, "UC3oJqv3NXiHkBVUW4i9R_pQ");
    assert_eq!(details.channel.name, "Test Upload channel");
    assert!(!details.channel.avatar.is_empty(), "no channel avatars");

    assert!(
        details.description.is_empty(),
        "got desc: `{}`",
        details.description.to_plaintext()
    );

    let date = details.publish_date.expect("publish_date");
    assert_eq!(date.date(), date!(2017 - 4 - 28));

    assert!(!details.is_live);
    assert!(!details.is_ccommons);
}

#[rstest]
#[tokio::test]
async fn get_video_details_not_found(rp: RustyPipe) {
    let err = rp.query().video_details("abcdefgLi5X").await.unwrap_err();

    assert!(
        matches!(err, Error::Extraction(ExtractionError::NotFound { .. })),
        "got: {err}"
    )
}

#[rstest]
#[tokio::test]
async fn get_video_comments(rp: RustyPipe) {
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

    assert_gteo(top_comments.count, 700_000, "comments");

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

#[rstest]
#[tokio::test]
async fn channel_videos(rp: RustyPipe) {
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

    assert_next(channel.content, rp.query(), 15, 2, true).await;
}

#[rstest]
#[tokio::test]
async fn channel_shorts(rp: RustyPipe) {
    let channel = rp
        .query()
        .channel_videos_tab("UCh8gHdtzO2tXd593_bjErWg", ChannelVideoTab::Shorts)
        .await
        .unwrap();

    // dbg!(&channel);
    assert_eq!(channel.id, "UCh8gHdtzO2tXd593_bjErWg");
    assert_eq!(channel.name, "Doobydobap");
    assert_gteo(channel.subscriber_count, 2_800_000, "subscribers");
    assert!(!channel.avatar.is_empty(), "got no thumbnails");
    assert_eq!(channel.verification, Verification::Verified);
    assert!(channel
        .description
        .contains("Hi, I\u{2019}m Tina, aka Doobydobap"));
    assert_eq!(
        channel.vanity_url.as_deref(),
        Some("https://www.youtube.com/@Doobydobap")
    );
    assert!(!channel.banner.is_empty(), "got no banners");

    assert!(
        !channel.content.items.is_empty() && !channel.content.is_exhausted(),
        "got no shorts"
    );

    // assert_next(channel.content, rp.query(), 15, 1, true).await;
}

#[rstest]
#[tokio::test]
async fn channel_livestreams(rp: RustyPipe) {
    let channel = rp
        .query()
        .channel_videos_tab("UC2DjFE7Xf11URZqWBigcVOQ", ChannelVideoTab::Live)
        .await
        .unwrap();

    // dbg!(&channel);
    assert_channel_eevblog(&channel);

    assert!(
        !channel.content.items.is_empty() && !channel.content.is_exhausted(),
        "got no streams"
    );

    assert_next(channel.content, rp.query(), 5, 1, true).await;
}

#[rstest]
#[tokio::test]
async fn channel_playlists(rp: RustyPipe) {
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

    assert_next(channel.content, rp.query(), 15, 1, true).await;
}

#[rstest]
#[tokio::test]
async fn channel_info(rp: RustyPipe) {
    let info = rp
        .query()
        .channel_info("UC2DjFE7Xf11URZqWBigcVOQ")
        .await
        .unwrap();

    assert_eq!(info.create_date, Some(date!(2009 - 4 - 4)));
    assert_gteo(info.view_count, 186_854_340, "channel views");
    assert_gteo(info.video_count, 1920, "channel videos");
    assert_gteo(info.subscriber_count, 920_000, "subscribers");
    assert_eq!(info.country, Some(Country::Au));

    insta::assert_ron_snapshot!(info.links, @r###"
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

#[rstest]
#[tokio::test]
async fn channel_search(rp: RustyPipe) {
    let channel = rp
        .query()
        .channel_search("UC2DjFE7Xf11URZqWBigcVOQ", "test")
        .await
        .unwrap();

    assert_channel_eevblog(&channel);
    assert_next(channel.content, rp.query(), 20, 2, true).await;
}

fn assert_channel_eevblog<T>(channel: &Channel<T>) {
    assert_eq!(channel.id, "UC2DjFE7Xf11URZqWBigcVOQ");
    assert_eq!(channel.name, "EEVblog");
    assert_gteo(channel.subscriber_count, 880_000, "subscribers");
    assert!(!channel.avatar.is_empty(), "got no thumbnails");
    assert_eq!(channel.verification, Verification::Verified);
    assert_eq!(channel.description, "NO SCRIPT, NO FEAR, ALL OPINION\nAn off-the-cuff Video Blog about Electronics Engineering, for engineers, hobbyists, enthusiasts, hackers and Makers\nHosted by Dave Jones from Sydney Australia\n\nDONATIONS:\nBitcoin: 3KqyH1U3qrMPnkLufM2oHDU7YB4zVZeFyZ\nEthereum: 0x99ccc4d2654ba40744a1f678d9868ecb15e91206\nPayPal: david@alternatezone.com\n\nPatreon: https://www.patreon.com/eevblog\n\nEEVblog2: http://www.youtube.com/EEVblog2\nEEVdiscover: https://www.youtube.com/channel/UCkGvUEt8iQLmq3aJIMjT2qQ\n\nEMAIL:\nAdvertising/Commercial: eevblog+business@gmail.com\nFan mail: eevblog+fan@gmail.com\nHate Mail: eevblog+hate@gmail.com\n\nI DON'T DO PAID VIDEO SPONSORSHIPS, DON'T ASK!\n\nPLEASE:\nDo NOT ask for personal advice on something, post it in the EEVblog forum.\nI read ALL email, but please don't be offended if I don't have time to reply, I get a LOT of email.\n\nMailbag\nPO Box 7949\nBaulkham Hills NSW 2153\nAUSTRALIA");
    assert!(!channel.tags.is_empty(), "got no tags");
    assert_eq!(
        channel.vanity_url.as_deref(),
        Some("https://www.youtube.com/@EEVblog")
    );
    assert!(!channel.banner.is_empty(), "got no banners");
}

#[rstest]
#[case::artist("UC_vmjW5e1xEHhYjY2a0kK1A", "Oonagh - Topic", false, false, false)]
#[case::shorts("UCh8gHdtzO2tXd593_bjErWg", "Doobydobap", true, true, true)]
#[case::livestream(
    "UChs0pSaEoNLV4mevBFGaoKA",
    "The Good Life Radio x Sensual Musique",
    true,
    true,
    true
)]
#[case::music("UC-9-kyTW8ZkZNDHQJ6FgpwQ", "Music", false, false, false)]
#[tokio::test]
async fn channel_more(
    #[case] id: &str,
    #[case] name: &str,
    #[case] has_videos: bool,
    #[case] has_playlists: bool,
    #[case] name_unlocalized: bool,
    rp: RustyPipe,
    unlocalized: bool,
) {
    fn assert_channel<T>(channel: &Channel<T>, id: &str, name: &str, unlocalized: bool) {
        assert_eq!(channel.id, id);

        if unlocalized {
            assert_eq!(channel.name, name);
        }
    }

    if has_videos {
        let channel_videos = rp.query().channel_videos(&id).await.unwrap();
        assert_channel(&channel_videos, id, name, unlocalized || name_unlocalized);
        assert!(!channel_videos.content.items.is_empty(), "got no videos");
    }

    if has_playlists {
        let channel_playlists = rp.query().channel_playlists(&id).await.unwrap();
        assert_channel(
            &channel_playlists,
            id,
            name,
            unlocalized || name_unlocalized,
        );
        assert!(
            !channel_playlists.content.items.is_empty(),
            "got no playlists"
        );
    }

    let info = rp.query().channel_info(&id).await.unwrap();
    assert_eq!(info.id, id);
}

#[rstest]
#[case::videos("UCcdwLMPsaU2ezNSJU1nFoBQ", ChannelVideoTab::Videos)]
#[case::live("UCvqRdlKsE5Q8mf8YXbdIJLw", ChannelVideoTab::Live)]
#[case::shorts("UCcdwLMPsaU2ezNSJU1nFoBQ", ChannelVideoTab::Shorts)]
#[tokio::test]
async fn channel_order_latest(#[case] id: &str, #[case] tab: ChannelVideoTab, rp: RustyPipe) {
    let latest = rp
        .query()
        .channel_videos_tab_order(id, tab, ChannelOrder::Latest)
        .await
        .unwrap();
    // Upload dates should be in descending order
    if tab == ChannelVideoTab::Videos {
        let mut latest_items = latest.items.iter().peekable();
        while let (Some(v), Some(next_v)) = (latest_items.next(), latest_items.peek()) {
            if !v.is_upcoming && !v.is_live && !next_v.is_upcoming && !next_v.is_live {
                assert_gte(
                    v.publish_date.expect("publish_date"),
                    next_v.publish_date.expect("publish_date"),
                    "latest video date",
                );
            }
        }
    }

    if tab != ChannelVideoTab::Shorts {
        assert_next(latest, rp.query(), 15, 1, true).await;
    }
}

#[rstest]
#[case::videos("UCcdwLMPsaU2ezNSJU1nFoBQ", ChannelVideoTab::Videos, "XqZsoesa55w")]
#[case::live("UCvqRdlKsE5Q8mf8YXbdIJLw", ChannelVideoTab::Live, "ojes5ULOqhc")]
#[case::shorts("UCcdwLMPsaU2ezNSJU1nFoBQ", ChannelVideoTab::Shorts, "k91vRvXGwHs")]
#[tokio::test]
async fn channel_order_popular(
    #[case] id: &str,
    #[case] tab: ChannelVideoTab,
    #[case] most_popular: &str,
    rp: RustyPipe,
) {
    let popular = rp
        .query()
        .channel_videos_tab_order(id, tab, ChannelOrder::Popular)
        .await
        .unwrap();
    // Most popular video should be in top 5
    assert!(
        popular.items.iter().take(5).any(|v| v.id == most_popular),
        "most popular video {most_popular} not found"
    );

    // View counts should be in descending order
    if tab != ChannelVideoTab::Shorts {
        let mut popular_items = popular.items.iter().peekable();
        while let (Some(v), Some(next_v)) = (popular_items.next(), popular_items.peek()) {
            let vc = v.view_count.expect("views");
            assert_gte(
                vc + (vc as f64 * 0.05) as u64,
                next_v.view_count.expect("views"),
                "most popular view count",
            );
        }
    }

    if tab != ChannelVideoTab::Shorts {
        assert_next(popular, rp.query(), 15, 1, true).await;
    }
}

#[rstest]
#[case::videos("UCcdwLMPsaU2ezNSJU1nFoBQ", ChannelVideoTab::Videos, "P2gDffkC0rY")]
#[case::live("UCvqRdlKsE5Q8mf8YXbdIJLw", ChannelVideoTab::Live, "aW43RH1kQ70")]
#[tokio::test]
async fn channel_order_oldest(
    #[case] id: &str,
    #[case] tab: ChannelVideoTab,
    #[case] oldest: &str,
    rp: RustyPipe,
) {
    let videos = rp
        .query()
        .channel_videos_tab_order(id, tab, ChannelOrder::Oldest)
        .await
        .unwrap();

    // Check oldest video
    assert_eq!(videos.items.first().expect("no videos").id, oldest);

    // Upload dates should be in ascending order
    let mut latest_items = videos.items.iter().peekable();
    while let (Some(v), Some(next_v)) = (latest_items.next(), latest_items.peek()) {
        if !v.is_upcoming && !v.is_live && !next_v.is_upcoming && !next_v.is_live {
            assert_gte(
                next_v.publish_date.expect("publish_date"),
                v.publish_date.expect("publish_date"),
                "oldest video date",
            );
        }
    }
    assert_next(videos, rp.query(), 15, 1, true).await;
}

#[rstest]
#[case::not_exist("UCOpNcN46UbXVtpKMrmU4Abx")]
#[case::gaming("UCOpNcN46UbXVtpKMrmU4Abg")]
#[case::movies("UCuJcl0Ju-gPDoksRjK1ya-w")]
#[case::sports("UCEgdi0XIXXZ-qJOFPf4JSKw")]
#[case::learning("UCtFRv9O2AHqOZjjynzrv-xg")]
#[case::live("UC4R8DWoMoI7CAwX8_LjQHig")]
// #[case::news("UCYfdidRxbB8Qhf0Nx7ioOYw")]
#[tokio::test]
async fn channel_not_found(#[case] id: &str, rp: RustyPipe) {
    let err = rp.query().channel_videos(&id).await.unwrap_err();

    assert!(
        matches!(err, Error::Extraction(ExtractionError::NotFound { .. })),
        "got: {err}"
    );
}

#[rstest]
#[case::shorts(ChannelVideoTab::Shorts)]
#[case::live(ChannelVideoTab::Live)]
#[tokio::test]
async fn channel_tab_not_found(#[case] tab: ChannelVideoTab, rp: RustyPipe) {
    let channel = rp
        .query()
        .channel_videos_tab("UCGiJh0NZ52wRhYKYnuZI08Q", tab)
        .await;

    // YouTube removed empty tabs from the menu, so they may return no data
    match channel {
        Ok(channel) => assert!(channel.content.is_empty(), "got: {:?}", channel.content),
        Err(err) => assert!(
            matches!(err, Error::Extraction(ExtractionError::NotFound { .. })),
            "got: {err}"
        ),
    }
}

#[rstest]
#[tokio::test]
async fn channel_age_restriction(rp: RustyPipe) {
    let id = "UCbfnHqxXs_K3kvaH-WlNlig";

    let res = rp.query().channel_videos(&id).await;
    if let Err(Error::Extraction(ExtractionError::Unavailable { reason, msg })) = res {
        assert_eq!(reason, UnavailabilityReason::AgeRestricted);
        assert!(msg.starts_with("Laphroaig Whisky: "));
    } else {
        panic!("invalid res: {res:?}")
    }

    let res = rp.query().channel_info(&id).await;
    if let Err(Error::Extraction(ExtractionError::Unavailable { reason, msg })) = res {
        assert_eq!(reason, UnavailabilityReason::AgeRestricted);
        assert!(msg.starts_with("Laphroaig Whisky: "));
    } else {
        panic!("invalid res: {res:?}")
    }
}

//#CHANNEL_RSS

#[cfg(feature = "rss")]
mod channel_rss {
    use super::*;
    use time::macros::datetime;

    #[rstest]
    #[tokio::test]
    async fn get_channel_rss(rp: RustyPipe) {
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

    #[rstest]
    #[tokio::test]
    async fn get_channel_rss_empty(rp: RustyPipe) {
        let channel = rp
            .query()
            .channel_rss("UCAyFbMjB3qAQSZBj6NCuBSg")
            .await
            .unwrap();

        assert_eq!(channel.id, "UCAyFbMjB3qAQSZBj6NCuBSg");
        assert_eq!(channel.name, "Cheryl Calogero");

        assert!(channel.videos.is_empty());
    }

    #[rstest]
    #[tokio::test]
    async fn get_channel_rss_not_found(rp: RustyPipe) {
        let err = rp
            .query()
            .channel_rss("UCHnyfMqiRRG1u-2MsSQLbXZ")
            .await
            .unwrap_err();

        assert!(
            matches!(err, Error::Extraction(ExtractionError::NotFound { .. })),
            "got: {}",
            err
        );
    }
}

//#SEARCH

#[rstest]
#[tokio::test]
async fn search(rp: RustyPipe, unlocalized: bool) {
    let result = rp
        .query()
        .search::<YouTubeItem, _>("doobydoobap")
        .await
        .unwrap();

    assert_gteo(
        result.items.count,
        if unlocalized { 7000 } else { 150 },
        "results",
    );

    if unlocalized {
        assert_eq!(result.corrected_query.as_deref(), Some("doobydobap"));
    }

    assert_next(result.items, rp.query(), 10, 2, true).await;
}

#[rstest]
#[case::video(search_filter::ItemType::Video)]
#[case::channel(search_filter::ItemType::Channel)]
#[case::playlist(search_filter::ItemType::Playlist)]
#[tokio::test]
async fn search_filter_item_type(#[case] item_type: search_filter::ItemType, rp: RustyPipe) {
    let mut result = rp
        .query()
        .search_filter::<YouTubeItem, _>(
            "with no videos",
            &SearchFilter::new().item_type(item_type),
        )
        .await
        .unwrap();

    result.items.extend(rp.query()).await.unwrap();
    assert_gte(result.items.items.len(), 20, "items");

    result.items.items.iter().for_each(|item| match item {
        YouTubeItem::Video(_) => {
            assert_eq!(item_type, search_filter::ItemType::Video);
        }
        YouTubeItem::Channel(_) => {
            assert_eq!(item_type, search_filter::ItemType::Channel);
        }
        YouTubeItem::Playlist(_) => {
            assert_eq!(item_type, search_filter::ItemType::Playlist);
        }
    });
}

#[rstest]
#[tokio::test]
async fn search_empty(rp: RustyPipe) {
    let result = rp
        .query()
        .search_filter::<YouTubeItem, _>(
            "3gig84hgi34gu8vj34gj489",
            &search_filter::SearchFilter::new()
                .feature(search_filter::Feature::IsLive)
                .feature(search_filter::Feature::Is3d),
        )
        .await
        .unwrap();

    assert!(result.items.is_empty());
}

#[rstest]
#[case::no_filter(false)]
#[case::filter(true)]
#[tokio::test]
async fn search_sensitive(rp: RustyPipe, #[case] filter: bool) {
    let q = "suicide";
    let result = if filter {
        rp.query()
            .search_filter::<YouTubeItem, _>(q, &search_filter::SearchFilter::new())
            .await
    } else {
        rp.query().search::<YouTubeItem, _>(q).await
    }
    .unwrap();
    assert_gteo(result.items.count, 10_000, "results");
    assert_next(result.items, rp.query(), 10, 2, true).await;
}

#[rstest]
#[tokio::test]
async fn search_suggestion(rp: RustyPipe) {
    let result = rp.query().search_suggestion("hunger ga").await.unwrap();

    assert!(result.iter().any(|s| s.starts_with("hunger games")));
    assert_gte(result.len(), 10, "search suggestions");
}

#[rstest]
#[tokio::test]
async fn search_suggestion_empty(rp: RustyPipe) {
    let result = rp
        .query()
        .search_suggestion("fjew327p4ifjelwfvnewg49")
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
#[case("https://music.youtube.com/browse/MPADUC7cl4MmM6ZZ2TcFyMk_b4pg", UrlTarget::Channel {id: "UC7cl4MmM6ZZ2TcFyMk_b4pg".to_owned()})]
#[tokio::test]
async fn resolve_url(#[case] url: &str, #[case] expect: UrlTarget, rp: RustyPipe) {
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
#[case("MPADUC7cl4MmM6ZZ2TcFyMk_b4pg", UrlTarget::Channel {id: "UC7cl4MmM6ZZ2TcFyMk_b4pg".to_owned()})]
#[tokio::test]
async fn resolve_string(#[case] string: &str, #[case] expect: UrlTarget, rp: RustyPipe) {
    let target = rp.query().resolve_string(string, true).await.unwrap();
    assert_eq!(target, expect);
}

#[rstest]
#[tokio::test]
async fn resolve_channel_not_found(rp: RustyPipe) {
    let err = rp
        .query()
        .resolve_url("https://www.youtube.com/feeqegnhq3rkwghjq43ruih43io3", true)
        .await
        .unwrap_err();

    assert!(matches!(
        err,
        Error::Extraction(ExtractionError::NotFound { .. })
    ));
}

//#TRENDS

#[rstest]
#[tokio::test]
#[ignore]
async fn startpage(rp: RustyPipe) {
    let startpage = rp.query().startpage().await.unwrap();

    // The startpage requires visitor data to fetch continuations
    assert!(startpage.visitor_data.is_some());

    assert_next(startpage, rp.query(), 8, 2, true).await;
}

#[rstest]
#[tokio::test]
async fn trending(rp: RustyPipe) {
    let result = rp.query().trending().await.unwrap();

    assert_gte(result.len(), 40, "items");
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
    "RDCLAK5uy_nLNY4ReQKH2kx5U23cyGMHql9ciHD9RSM",
    "Presenting BLACKPINK (블랙핑크)",
    false,
    Some("The most played hits and essential tracks. #blackpink #best #kpop".to_owned()),
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
    rp: RustyPipe,
    unlocalized: bool,
) {
    let playlist = rp.query().music_playlist(id).await.unwrap();

    assert_eq!(playlist.id, id);
    assert!(!playlist.tracks.is_empty());
    assert_eq!(!playlist.tracks.is_exhausted(), is_long);
    assert_gteo(
        playlist.track_count,
        if is_long { 100 } else { 10 },
        "track count",
    );
    if unlocalized {
        assert_eq!(playlist.name, name);
        assert_eq!(playlist.description.map(|d| d.to_plaintext()), description);
    }

    if let Some(expect) = channel {
        let c = playlist.channel.expect("channel");
        assert_eq!(c.id, expect.0);
        assert_eq!(c.name, expect.1);
    }
    assert!(!playlist.thumbnail.is_empty());
    assert_eq!(playlist.from_ytm, from_ytm);
}

#[rstest]
#[case::user("PLMC9KNkIncKtPzgY-5rmhvj7fax8fdxoj")]
#[case::ytm("RDCLAK5uy_kb7EBi6y3GrtJri4_ZH56Ms786DFEimbM")]
#[tokio::test]
async fn music_playlist_cont(#[case] id: &str, rp: RustyPipe) {
    let mut playlist = rp.query().music_playlist(id).await.unwrap();

    playlist.tracks.extend_pages(rp.query(), 5).await.unwrap();

    let track_count = playlist.track_count.unwrap();
    assert_gte(track_count, 100, "tracks");

    assert_eq!(track_count, playlist.tracks.count.unwrap());
    assert_gte(
        usize::try_from(track_count).unwrap(),
        playlist.tracks.items.len(),
        "tracks",
    );
}

#[rstest]
#[tokio::test]
async fn music_playlist_related(rp: RustyPipe) {
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

#[rstest]
#[tokio::test]
async fn music_playlist_not_found(rp: RustyPipe) {
    let err = rp
        .query()
        .music_playlist("PLbZIPy20-1pN7mqjckepWF78ndb6ci_qz")
        .await
        .unwrap_err();

    assert!(
        matches!(err, Error::Extraction(ExtractionError::NotFound { .. })),
        "got: {err}"
    );
}

#[rstest]
#[case::one_artist("one_artist", "MPREb_rMegpebUBPU")]
#[case::various_artists("various_artists", "MPREb_8QkDeEIawvX")]
#[case::single("single", "MPREb_bHfHGoy7vuv")]
#[case::ep("ep", "MPREb_u1I69lSAe5v")]
// #[case::audiobook("audiobook", "MPREb_gaoNzsQHedo")]
#[case::show("show", "MPREb_aDDw2kVEFtM")]
#[case::unavailable("unavailable", "MPREb_AzuWg8qAVVl")]
#[case::no_year("no_year", "MPREb_F3Af9UZZVxX")]
#[case::version_no_artist("version_no_artist", "MPREb_h8ltx5oKvyY")]
#[case::no_artist("no_artist", "MPREb_bqWA6mAZFWS")]
#[tokio::test]
async fn music_album(#[case] name: &str, #[case] id: &str, rp: RustyPipe, unlocalized: bool) {
    let album = rp.query().music_album(id).await.unwrap();

    assert!(!album.cover.is_empty(), "got no cover");

    // TODO: check full snapshot if A/B#13 is stabilized
    if unlocalized && name != "one_artist" {
        insta::assert_ron_snapshot!(format!("music_album_{name}"), album,
            {".cover" => "[cover]", ".tracks[].view_count" => "[view_count]"}
        );
    } else {
        insta::assert_ron_snapshot!(format!("music_album_{name}_intl"), album,
            {
                ".name" => "[name]",
                ".cover" => "[cover]",
                ".description" => "[description]",
                ".artists[].name" => "[name]",
                ".tracks[].name" => "[name]",
                ".tracks[].view_count" => "[view_count]",
                ".tracks[].album.name" => "[name]",
                ".tracks[].artists[].name" => "[name]",
                ".variants[].artists[].name" => "[name]",
            }
        );
    }
}

#[rstest]
#[tokio::test]
async fn music_album_not_found(rp: RustyPipe) {
    let err = rp
        .query()
        .music_album("MPREb_nlBWQROfvjoz")
        .await
        .unwrap_err();

    assert!(
        matches!(err, Error::Extraction(ExtractionError::NotFound { .. })),
        "got: {err}"
    );
}

#[rstest]
#[case::basic_all("basic_all", "UCFKUUtHjT4iq3p0JJA13SOA", true, 15, 1)]
// TODO: wait for A/B test 6 to stabilize
// #[case::basic("basic", "UC7cl4MmM6ZZ2TcFyMk_b4pg", false, 15, 2)]
#[case::no_more_albums("no_more_albums", "UCOR4_bSVIXPsGa4BbCSt60Q", true, 15, 0)]
#[case::only_singles("only_singles", "UCfwCE5VhPMGxNPFxtVv7lRw", false, 13, 0)]
#[case::no_artist("no_artist", "UCh8gHdtzO2tXd593_bjErWg", false, 0, 0)]
// querying Trailerpark's secondary YouTube channel should result in the YTM channel being fetched
#[case::secondary_channel("no_more_albums", "UCC9192yGQD25eBZgFZ84MPw", true, 15, 0)]
#[tokio::test]
async fn music_artist(
    #[case] name: &str,
    #[case] id: &str,
    #[case] all_albums: bool,
    #[case] min_tracks: usize,
    #[case] min_playlists: usize,
    rp: RustyPipe,
    unlocalized: bool,
) {
    let mut artist = rp.query().music_artist(id, all_albums).await.unwrap();

    assert_gte(artist.tracks.len(), min_tracks, "tracks");
    assert_gte(artist.playlists.len(), min_playlists, "playlists");

    if name == "no_artist" {
        assert!(artist.similar_artists.is_empty());
        assert!(artist.subscriber_count.is_none());
    } else {
        assert_gteo(artist.subscriber_count, 10_000, "subscribers");
    }

    artist.tracks.iter().for_each(|t| {
        assert!(!t.cover.is_empty());
        if t.is_video {
            assert!(t.view_count.is_some());
        } else {
            assert!(t.album.is_some());
        }
    });

    // Check images
    assert!(!artist.header_image.is_empty(), "got no header image");
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
    artist.albums.sort_by_key(|a| a.id.clone());

    if unlocalized {
        insta::assert_ron_snapshot!(format!("music_artist_{name}"), artist, {
            ".header_image" => "[header_image]",
            ".subscriber_count" => "[subscriber_count]",
            ".albums[].cover" => "[cover]",
            ".tracks" => "[tracks]",
            ".playlists" => "[playlists]",
            ".similar_artists" => "[artists]",
        });
    } else {
        insta::assert_ron_snapshot!(format!("music_artist_{name}_intl"), artist, {
            ".name" => "[name]",
            ".header_image" => "[header_image]",
            ".description" => "[description]",
            ".wikipedia_url" => "[wikipedia_url]",
            ".subscriber_count" => "[subscriber_count]",
            ".albums[].name" => "[name]",
            ".albums[].cover" => "[cover]",
            ".albums[].artists[].name" => "[name]",
            ".tracks" => "[tracks]",
            ".playlists" => "[playlists]",
            ".similar_artists" => "[artists]",
        });
    }

    // Fetch albums seperately
    if name != "no_artist" {
        let albums = rp.query().music_artist_albums(id).await.unwrap();
        let albums_expect = artist
            .albums
            .iter()
            .map(|a| a.id.to_owned())
            .collect::<HashSet<_>>();
        let albums_got = albums
            .iter()
            .map(|a| a.id.to_owned())
            .collect::<HashSet<_>>();

        if all_albums {
            assert_eq!(albums_got, albums_expect);
        } else {
            assert!(albums_expect.is_subset(&albums_expect));
        }
    }
}

#[rstest]
#[tokio::test]
async fn music_artist_not_found(rp: RustyPipe) {
    let err = rp
        .query()
        .music_artist("UC7cl4MmM6ZZ2TcFyMk_b4pq", false)
        .await
        .unwrap_err();

    assert!(
        matches!(err, Error::Extraction(ExtractionError::NotFound { .. })),
        "got: {err}"
    );
}

#[rstest]
#[tokio::test]
async fn music_artist_albums_not_found(rp: RustyPipe) {
    let err = rp
        .query()
        .music_artist_albums("UC7cl4MmM6ZZ2TcFyMk_b4pq")
        .await
        .unwrap_err();

    assert!(
        matches!(err, Error::Extraction(ExtractionError::NotFound { .. })),
        "got: {err}"
    );
}

#[rstest]
#[case::default(false)]
#[case::typo(true)]
#[tokio::test]
async fn music_search_main(#[case] typo: bool, rp: RustyPipe, unlocalized: bool) {
    let res = rp
        .query()
        .music_search_main(match typo {
            false => "lieblingsmensch namika",
            true => "lieblingsmesch namika",
        })
        .await
        .unwrap();
    let items = res.items.items;

    check_search_result(&items);

    if typo {
        if unlocalized {
            assert_eq!(
                res.corrected_query.as_deref(),
                Some("lieblingsmensch namika")
            );
        }
    } else {
        assert_eq!(res.corrected_query, None);
    }

    let track = items
        .iter()
        .find_map(|itm| {
            if let MusicItem::Track(track) = itm {
                if track.id == "6485PhOtHzY" {
                    Some(track)
                } else {
                    None
                }
            } else {
                None
            }
        })
        .unwrap_or_else(|| {
            panic!("could not find track, got {:#?}", &items);
        });

    assert_eq!(track.name, "Lieblingsmensch");
    assert_eq!(track.duration, Some(191));
    assert!(!track.cover.is_empty(), "got no cover");

    assert_eq!(track.artists.len(), 1);
    let track_artist = &track.artists[0];
    assert_eq!(track_artist.id.as_deref(), Some("UCIh4j8fXWf2U0ro0qnGU8Mg"));
    if unlocalized {
        assert_eq!(track_artist.name, "Namika");
    }

    let track_album = track.album.as_ref().expect("track_album");
    assert_eq!(track_album.id, "MPREb_RXHxrUFfrvQ");
    assert_eq!(track_album.name, "Lieblingsmensch");

    assert!(!track.is_video, "got mv");
    assert_eq!(track.track_nr, None);
}

#[rstest]
#[tokio::test]
async fn music_search_main2(rp: RustyPipe, unlocalized: bool) {
    let res = rp.query().music_search_main("taylor swift").await.unwrap();
    let items = res.items.items;

    check_search_result(&items);

    let artist = items
        .iter()
        .find_map(|itm| {
            if let MusicItem::Artist(artist) = itm {
                if artist.id == "UCPC0L1d253x-KuMNwa05TpA" {
                    Some(artist)
                } else {
                    None
                }
            } else {
                None
            }
        })
        .unwrap_or_else(|| {
            panic!("could not find artist, got {:#?}", &items);
        });

    if unlocalized {
        assert_eq!(artist.name, "Taylor Swift");
    }
    assert!(!artist.avatar.is_empty(), "got no avatar");
}

fn check_search_result(items: &[MusicItem]) {
    assert_gte(items.len(), 10, "search results");

    let mut has_tracks = false;
    let mut has_videos = false;
    let mut has_albums = false;
    let mut has_artists = false;
    let mut has_playlists = false;

    for itm in items {
        match itm {
            MusicItem::Track(t) => {
                if t.is_video {
                    has_videos = true
                } else {
                    has_tracks = true
                }
            }
            MusicItem::Album(_) => has_albums = true,
            MusicItem::Artist(_) => has_artists = true,
            MusicItem::Playlist(_) => has_playlists = true,
        }
    }

    assert!(has_tracks, "no tracks");
    assert!(has_videos, "no videos");
    assert!(has_albums, "no albums");
    assert!(has_artists, "no artists");
    assert!(has_playlists, "no playlists");
}

#[rstest]
#[tokio::test]
async fn music_search_tracks(rp: RustyPipe, unlocalized: bool) {
    let res = rp.query().music_search_tracks("black mamba").await.unwrap();

    let track = &res
        .items
        .items
        .iter()
        .find(|a| a.id == "BL-aIpCLWnU")
        .unwrap_or_else(|| {
            panic!("could not find track, got {:#?}", &res.items.items);
        });

    assert_eq!(track.name, "Black Mamba");
    assert!(!track.cover.is_empty(), "got no cover");
    assert!(!track.is_video);
    assert_eq!(track.track_nr, None);

    assert_eq!(track.artists.len(), 1);
    let track_artist = &track.artists[0];
    assert_eq!(track_artist.id.as_deref(), Some("UCEdZAdnnKqbaHOlv8nM6OtA"));
    if unlocalized {
        assert_eq!(track_artist.name, "aespa");
    }

    assert_eq!(track.duration, Some(175));

    let album = track.album.as_ref().expect("track_album");
    assert_eq!(album.id, "MPREb_OpHWHwyNOuY");
    assert_eq!(album.name, "Black Mamba");

    assert_next(res.items, rp.query(), 15, 2, true).await;
}

#[rstest]
#[tokio::test]
async fn music_search_videos(rp: RustyPipe, unlocalized: bool) {
    let res = rp.query().music_search_videos("black mamba").await.unwrap();

    let track = &res
        .items
        .items
        .iter()
        .find(|a| a.id == "ZeerrnuLi5E")
        .unwrap_or_else(|| {
            panic!("could not find video, got {:#?}", &res.items.items);
        });

    assert_eq!(track.name, "Black Mamba");
    assert!(!track.cover.is_empty(), "got no cover");
    assert!(track.is_video);
    assert_eq!(track.track_nr, None);

    assert_eq!(track.artists.len(), 1);
    let track_artist = &track.artists[0];
    assert_eq!(track_artist.id.as_deref(), Some("UCEdZAdnnKqbaHOlv8nM6OtA"));
    if unlocalized {
        assert_eq!(track_artist.name, "aespa");
    }

    assert_eq!(track.duration, Some(230));
    assert_eq!(track.album, None);
    assert_gteo(track.view_count, 230_000_000, "views");

    assert_next(res.items, rp.query(), 15, 2, true).await;
}

#[rstest]
#[case::main(false)]
#[case::videos(true)]
#[tokio::test]
async fn music_search_episode(rp: RustyPipe, #[case] videos: bool) {
    let query = "Blond - Da muss man dabei gewesen sein: Das Hörspiel - Fall #1";
    let track_id = "Zq_-LDy7AgE";

    let track = if videos {
        let items = rp
            .query()
            .music_search_videos(query)
            .await
            .unwrap()
            .items
            .items;
        items.iter().find(|a| a.id == track_id).cloned()
    } else {
        let items = rp
            .query()
            .music_search_main(query)
            .await
            .unwrap()
            .items
            .items;
        items
            .iter()
            .find_map(|itm| {
                if let MusicItem::Track(track) = itm {
                    if track.id == track_id {
                        Some(track)
                    } else {
                        None
                    }
                } else {
                    None
                }
            })
            .cloned()
    }
    .expect("could not find episode");

    assert_eq!(track.artists.len(), 1);
    let track_artist = &track.artists[0];
    assert_eq!(
        track.name,
        "Blond - Da muss man dabei gewesen sein: Das Hörspiel - Fall #1"
    );
    assert_eq!(track_artist.name, "BLOND_OFFICIAL");
    assert_eq!(track_artist.id.as_deref(), Some("UC8SmM4bue6bKHT4p-_YFZHQ"));
    assert_eq!(track.artist_id.as_deref(), Some("UC8SmM4bue6bKHT4p-_YFZHQ"));
    assert!(!track.cover.is_empty(), "got no cover");
    assert!(track.is_video);
}

#[rstest]
#[case::single(
    "lea okay",
    "Okay",
    "MPREb_3t4vp0Dj8B0",
    "LEA",
    "UC_MxOdawj_BStPs4CKBYD0Q",
    2020,
    AlbumType::Single,
    false
)]
#[case::ep(
    "waldbrand",
    "Waldbrand",
    "MPREb_u1I69lSAe5v",
    "Madeline Juno",
    "UCpJyCbFbdTrx0M90HCNBHFQ",
    2016,
    AlbumType::Ep,
    false
)]
#[case::album(
    "unbroken",
    "Unbroken",
    "MPREb_rMegpebUBPU",
    "Demi Lovato",
    "UCwem2sj-QUJCiWiPAo9JuAw",
    2011,
    AlbumType::Album,
    true
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
    #[case] more: bool,
    rp: RustyPipe,
    unlocalized: bool,
) {
    let res = rp.query().music_search_albums(query).await.unwrap();

    let album = &res
        .items
        .items
        .iter()
        .find(|a| a.id == id)
        .unwrap_or_else(|| {
            panic!("could not find album, got {:#?}", &res.items.items);
        });
    assert_eq!(album.name, name);

    assert_eq!(album.artists.len(), 1);
    let album_artist = &album.artists[0];
    assert_eq!(album_artist.id.as_ref().expect("artist.id"), artist_id);
    if unlocalized {
        assert_eq!(album_artist.name, artist);
    }

    assert_eq!(album.artist_id.as_ref().expect("artist_id"), artist_id);
    assert!(!album.cover.is_empty(), "got no cover");
    assert_eq!(album.year, Some(year));
    assert_eq!(album.album_type, album_type);

    assert_eq!(res.corrected_query, None);

    if more && unlocalized {
        assert_next(res.items, rp.query(), 15, 1, true).await;
    }
}

#[rstest]
#[tokio::test]
async fn music_search_artists(rp: RustyPipe, unlocalized: bool) {
    let res = rp.query().music_search_artists("namika").await.unwrap();

    let artist = res
        .items
        .items
        .iter()
        .find(|a| a.id == "UCIh4j8fXWf2U0ro0qnGU8Mg")
        .unwrap_or_else(|| {
            panic!("could not find artist, got {:#?}", &res.items.items);
        });
    if unlocalized {
        assert_eq!(artist.name, "Namika");
    }
    assert!(!artist.avatar.is_empty(), "got no avatar");
    assert_gteo(artist.subscriber_count, 735_000, "subscribers");
    assert_eq!(res.corrected_query, None);
}

#[rstest]
#[tokio::test]
async fn music_search_artists_cont(rp: RustyPipe) {
    let res = rp.query().music_search_artists("band").await.unwrap();

    assert_eq!(res.corrected_query, None);
    assert_next(res.items, rp.query(), 15, 2, true).await;
}

#[rstest]
#[tokio::test]
async fn music_search_playlists(rp: RustyPipe, unlocalized: bool) {
    let res = rp
        .query()
        .music_search_playlists("Massive Rock Hits", false)
        .await
        .unwrap();

    assert_eq!(res.corrected_query, None);
    let playlist = res
        .items
        .items
        .iter()
        .find(|p| p.id == "RDCLAK5uy_k7h5535MeHE8xmgHsrZx7HOKH4lb5vAfY")
        .unwrap_or_else(|| {
            panic!("could not find playlist, got {:#?}", &res.items.items);
        });

    if unlocalized {
        assert_eq!(playlist.name, "Massive Rock Hits");
    }
    assert!(!playlist.thumbnail.is_empty(), "got no thumbnail");
    assert_gteo(playlist.track_count, 40, "tracks");
    assert_eq!(playlist.channel, None);
    assert!(playlist.from_ytm);

    assert!(
        res.items.items.iter().all(|p| p.from_ytm),
        "community items found"
    )
}

#[rstest]
#[tokio::test]
async fn music_search_playlists_community(rp: RustyPipe) {
    let res = rp
        .query()
        .music_search_playlists("Best Pop Music Videos - Top Pop Hits Playlist", true)
        .await
        .unwrap();

    assert_eq!(res.corrected_query, None);
    let playlist = res
        .items
        .items
        .iter()
        .find(|p| p.id == "PLMC9KNkIncKtGvr2kFRuXBVmBev6cAJ2u")
        .unwrap_or_else(|| {
            panic!("could not find playlist, got {:#?}", &res.items.items);
        });

    assert_eq!(
        playlist.name,
        "Best Pop Music Videos - Top Pop Hits Playlist"
    );
    assert!(!playlist.thumbnail.is_empty(), "got no thumbnail");

    let channel = playlist.channel.as_ref().unwrap();
    assert_eq!(channel.id, "UCs72iRpTEuwV3y6pdWYLgiw");
    assert_eq!(channel.name, "Redlist - Just Hits");
    assert!(!playlist.from_ytm);
}

/// The YouTube Music search sometimes shows genre radio items. They should be skipped.
#[rstest]
#[tokio::test]
async fn music_search_genre_radio(rp: RustyPipe) {
    rp.query().music_search_main("pop radio").await.unwrap();
}

#[rstest]
#[case::default("ed sheer", Some("ed sheeran"), Some("UClmXPfaYhXOYsNn_QUyheWQ"))]
#[case::empty("reujbhevmfndxnjrze", None, None)]
#[tokio::test]
async fn music_search_suggestion(
    #[case] query: &str,
    #[case] term: Option<&str>,
    #[case] artist: Option<&str>,
    rp: RustyPipe,
) {
    let suggestion = rp.query().music_search_suggestion(query).await.unwrap();

    match term {
        Some(expect) => assert!(
            suggestion.terms.iter().any(|s| s == expect),
            "suggestion: {suggestion:?}, expected: {expect}"
        ),
        None => assert!(
            suggestion.terms.is_empty(),
            "suggestion: {suggestion:?}, expected to be empty"
        ),
    }

    if let Some(artist) = artist {
        assert!(suggestion.items.iter().any(|s| match s {
            rustypipe::model::MusicItem::Artist(a) => a.id == artist,
            _ => false,
        }));
    }
}

#[rstest]
#[case::mv("mv", "ZeerrnuLi5E")]
#[case::track("track", "qIZ-vvg-wiU")]
#[case::track_details("track_details", "1eekOcpx_iQ")]
#[tokio::test]
async fn music_details(#[case] name: &str, #[case] id: &str, rp: RustyPipe) {
    let track = rp.query().music_details(id).await.unwrap();

    assert!(!track.track.cover.is_empty(), "got no cover");
    if name == "mv" {
        assert_gteo(track.track.view_count, 235_000_000, "view count");
    } else {
        assert!(track.track.view_count.is_none());
    }

    insta::assert_ron_snapshot!(format!("music_details_{name}"), track,
        {
            ".track.cover" => "[cover]",
            ".track.view_count" => "[view_count]"
        }
    );
}

#[rstest]
#[tokio::test]
async fn music_lyrics(rp: RustyPipe) {
    let track = rp.query().music_details("60ImQ8DS3Vs").await.unwrap();
    let lyrics = rp
        .query()
        .music_lyrics(&track.lyrics_id.unwrap())
        .await
        .unwrap();
    insta::assert_ron_snapshot!(lyrics.body);
    assert!(
        lyrics.footer.contains("Musixmatch"),
        "footer text: {}",
        lyrics.footer
    )
}

#[rstest]
#[tokio::test]
async fn music_lyrics_not_found(rp: RustyPipe) {
    let track = rp.query().music_details("ekXI8qrbe1s").await.unwrap();
    let err = rp
        .query()
        .music_lyrics(&track.lyrics_id.unwrap())
        .await
        .unwrap_err();

    assert!(
        matches!(err, Error::Extraction(ExtractionError::NotFound { .. })),
        "got: {err}"
    );
}

#[rstest]
#[case::a("7nigXQS1Xb0", true)]
#[case::b("4t3SUDZCBaQ", false)]
#[tokio::test]
async fn music_related(#[case] id: &str, #[case] full: bool, rp: RustyPipe) {
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
        validate::video_id(&track.id).unwrap();
        assert!(!track.name.is_empty());
        assert!(!track.cover.is_empty(), "got no cover");

        if let Some(artist_id) = track.artist_id {
            validate::channel_id(&artist_id).unwrap();
            track_artist_ids += 1;
        }

        let artist = track.artists.first().expect("track_artist");
        assert!(!artist.name.is_empty());
        if let Some(artist_id) = &artist.id {
            validate::channel_id(artist_id).unwrap();
            track_artists += 1;
        }

        if track.is_video {
            assert!(track.album.is_none());
            assert_gteo(track.view_count, 10_000, "views")
        } else {
            n_tracks_ytm += 1;

            assert!(track.view_count.is_none());
            if let Some(album) = track.album {
                validate::album_id(&album.id).unwrap();
                assert!(!album.name.is_empty());
                track_albums += 1;
            }
        }
    }

    assert_gte(n_tracks, 20, "tracks");
    assert_gte(n_tracks_ytm, 10, "tracks_ytm");

    assert_gte(track_artists, n_tracks - 4, "track_artists");
    assert_gte(track_artist_ids, n_tracks - 4, "track_artists");
    assert_gte(track_albums, n_tracks_ytm - 4, "track_artists");

    if full {
        assert_gte(related.albums.len(), 10, "albums");
        for album in related.albums {
            validate::album_id(&album.id).unwrap();
            assert!(!album.name.is_empty());
            assert!(!album.cover.is_empty(), "got no cover");

            let artist = album.artists.first().expect("album artist");
            validate::channel_id(artist.id.as_ref().expect("artist id")).unwrap();
            assert!(!artist.name.is_empty());
        }

        assert_gte(related.artists.len(), 10, "artists");
        for artist in related.artists {
            validate::channel_id(&artist.id).unwrap();
            assert!(!artist.name.is_empty());
            assert!(!artist.avatar.is_empty(), "got no avatar");
            assert_gteo(artist.subscriber_count, 5000, "subscribers")
        }

        assert_gte(related.playlists.len(), 10, "playlists");
        for playlist in related.playlists {
            validate::playlist_id(&playlist.id).unwrap();
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
                let channel = playlist.channel.expect("playlist channel");
                validate::channel_id(&channel.id).unwrap();
                assert!(!channel.name.is_empty());
            } else {
                assert!(playlist.channel.is_none());
            }
        }
    }
}

#[rstest]
#[tokio::test]
async fn music_details_not_found(rp: RustyPipe) {
    let err = rp.query().music_details("7nigXQS1XbZ").await.unwrap_err();

    assert!(
        matches!(err, Error::Extraction(ExtractionError::NotFound { .. })),
        "got: {err}"
    );
}

#[rstest]
#[tokio::test]
async fn music_radio_track(rp: RustyPipe) {
    let tracks = rp.query().music_radio_track("ZeerrnuLi5E").await.unwrap();
    assert_next_items(tracks, rp.query(), 20).await;
}

#[rstest]
#[tokio::test]
async fn music_radio_track_not_found(rp: RustyPipe) {
    let err = rp
        .query()
        .music_radio_track("7nigXQS1XbZ")
        .await
        .unwrap_err();

    assert!(
        matches!(err, Error::Extraction(ExtractionError::NotFound { .. })),
        "got: {err}"
    );
}

#[rstest]
#[tokio::test]
async fn music_radio_playlist(rp: RustyPipe) {
    let tracks = rp
        .query()
        .music_radio_playlist("PL5dDx681T4bR7ZF1IuWzOv1omlRbE7PiJ")
        .await
        .unwrap();
    assert_next_items(tracks, rp.query(), 20).await;
}

#[rstest]
#[tokio::test]
async fn music_radio_playlist_not_found(rp: RustyPipe) {
    let res = rp
        .query()
        .music_radio_playlist("PL5dDx681T4bR7ZF1IuWzOv1omlZZZZZZZ")
        .await;

    if let Err(err) = res {
        assert!(
            matches!(err, Error::Extraction(ExtractionError::NotFound { .. })),
            "got: {err}"
        );
    }
}

#[rstest]
#[tokio::test]
async fn music_radio_artist(rp: RustyPipe) {
    let tracks = rp
        .query()
        .music_radio("RDEM_Ktu-TilkxtLvmc9wX1MLQ")
        .await
        .unwrap();
    assert_next_items(tracks, rp.query(), 20).await;
}

#[rstest]
#[tokio::test]
async fn music_radio_not_found(rp: RustyPipe) {
    let err = rp.query().music_radio("RDEM_foo").await.unwrap_err();

    assert!(
        matches!(err, Error::Extraction(ExtractionError::NotFound { .. })),
        "got: {err}"
    );
}

#[rstest]
#[case::de(
    Country::De,
    "PL4fGSI1pDJn4X-OicSCOy-dChXWdTgziQ",
    "OLAK5uy_nPQBmQpOIYYxuvYW4gIHsjgDqX9kc--Dg"
)]
#[case::us(
    Country::Us,
    "PL4fGSI1pDJn69On1f-8NAvX_CYlx7QyZc",
    "PLrEnWoR732-DtKgaDdnPkezM_nDidBU9H"
)]
#[tokio::test]
async fn music_charts(
    #[case] country: Country,
    #[case] plid_top: &str,
    #[case] plid_trend: &str,
    rp: RustyPipe,
) {
    let charts = rp.query().music_charts(Some(country)).await.unwrap();

    assert_eq!(charts.top_playlist_id.expect("top_playlist_id"), plid_top);

    assert_gte(charts.top_tracks.len(), 30, "top tracks");
    assert_gte(charts.artists.len(), 30, "top artists");

    // Currently (01.02.2024) is no trending playlist shown for Global and US
    if country != Country::Us {
        assert_eq!(
            charts.trending_playlist_id.expect("trending_playlist_id"),
            plid_trend
        );
        assert_gte(charts.trending_tracks.len(), 15, "trending tracks");
    }

    // Chart playlists only available in USA
    if country == Country::Us {
        assert_gte(charts.playlists.len(), 8, "charts playlists");
    }
}

#[rstest]
#[tokio::test]
async fn music_new_albums(rp: RustyPipe) {
    let albums = rp.query().music_new_albums().await.unwrap();
    assert_gte(albums.len(), 10, "albums");

    for album in albums {
        validate::album_id(&album.id).unwrap();
        assert!(!album.name.is_empty());
        assert!(!album.cover.is_empty(), "got no cover");
    }
}

#[rstest]
#[tokio::test]
async fn music_new_videos(rp: RustyPipe) {
    let videos = rp.query().music_new_videos().await.unwrap();
    assert_gte(videos.len(), 5, "videos");

    for video in videos {
        validate::video_id(&video.id).unwrap();
        assert!(!video.name.is_empty());
        assert!(!video.cover.is_empty(), "got no cover");
        if let Some(view_count) = video.view_count {
            assert_gte(view_count, 500, "views");
        } else {
            // Podcast episode: shows duration instead of view count
            assert!(video.duration.is_some(), "no view count or duration");
        }
        assert!(video.is_video);
    }
}

#[rstest]
#[tokio::test]
async fn music_genres(rp: RustyPipe, unlocalized: bool) {
    let genres = rp.query().music_genres().await.unwrap();

    let chill = genres
        .iter()
        .find(|g| g.id == "ggMPOg1uX1JOQWZFeDByc2Jm")
        .expect("genre: Chill");
    if unlocalized {
        assert_eq!(chill.name, "Chill");
    }
    assert!(chill.is_mood);

    let pop = genres
        .iter()
        .find(|g| g.id == "ggMPOg1uX1lMbVZmbzl6NlJ3" || g.id == "ggMPOg1uX1BmNzc2V2p0YXJ5")
        .expect("genre: Pop");
    assert_eq!(pop.name, "Pop");
    assert!(!pop.is_mood);

    for g in &genres {
        validate::genre_id(&g.id).unwrap();
        assert_gte(g.color, 0xff00_0000, "color");
    }
}

#[rstest]
#[case::chill("ggMPOg1uX1JOQWZFeDByc2Jm", "Chill")]
#[case::pop("ggMPOg1uX1lMbVZmbzl6NlJ3", "Pop")]
#[tokio::test]
async fn music_genre(#[case] id: &str, #[case] name: &str, rp: RustyPipe, unlocalized: bool) {
    let genre = rp.query().music_genre(id).await.unwrap();

    fn check_music_genre(
        genre: MusicGenre,
        id: &str,
        name: &str,
        unlocalized: bool,
    ) -> Vec<(String, String)> {
        assert_eq!(genre.id, id);
        if unlocalized {
            assert_eq!(genre.name, name);
        }
        assert_gte(genre.sections.len(), 2, "genre sections");

        let mut subgenres = Vec::new();
        genre.sections.iter().for_each(|section| {
            assert!(!section.name.is_empty());
            section.playlists.iter().for_each(|playlist| {
                validate::playlist_id(&playlist.id).unwrap();
                assert!(!playlist.name.is_empty());
                assert!(!playlist.thumbnail.is_empty(), "got no cover");

                if !playlist.from_ytm {
                    if let Some(channel) = playlist.channel.as_ref() {
                        validate::channel_id(&channel.id).unwrap();
                        assert!(!channel.name.is_empty());
                    }
                } else {
                    assert!(playlist.channel.is_none());
                }
            });
            if let Some(subgenre_id) = &section.subgenre_id {
                subgenres.push((subgenre_id.clone(), section.name.clone()));
            }
        });
        subgenres
    }

    let subgenres = check_music_genre(genre, id, name, unlocalized);

    if name == "Chill" {
        assert_gte(subgenres.len(), 2, "subgenres");
    }

    for (id, name) in subgenres {
        let genre = rp.query().music_genre(&id).await.unwrap();
        check_music_genre(genre, &id, &name, unlocalized);
    }
}

#[rstest]
#[tokio::test]
async fn music_genre_not_found(rp: RustyPipe) {
    let err = rp
        .query()
        .music_genre("ggMPOg1uX1JOQWZFeDByc2zz")
        .await
        .unwrap_err();

    assert!(
        matches!(err, Error::Extraction(ExtractionError::NotFound { .. })),
        "got: {err}"
    );
}

//#AB TESTS

const VISITOR_DATA_SEARCH_CHANNEL_HANDLES: &str = "CgszYlc1Yk1WZGRCSSjrwOSbBg%3D%3D";

#[tokio::test]
async fn ab3_search_channel_handles() {
    let rp = rp_visitor_data(VISITOR_DATA_SEARCH_CHANNEL_HANDLES);

    rp.query()
        .search_filter::<YouTubeItem, _>(
            "test",
            &SearchFilter::new().item_type(search_filter::ItemType::Channel),
        )
        .await
        .unwrap();
}

//#MISCELLANEOUS

#[rstest]
#[case::desktop(ContinuationEndpoint::Browse)]
#[case::music(ContinuationEndpoint::MusicBrowse)]
#[tokio::test]
async fn invalid_ctoken(#[case] ep: ContinuationEndpoint, rp: RustyPipe) {
    let e = rp
        .query()
        .continuation::<YouTubeItem, _>("Abcd", ep, None)
        .await
        .unwrap_err();

    match e {
        Error::Extraction(e) => match e {
            ExtractionError::BadRequest(msg) => {
                assert_eq!(msg, "Request contains an invalid argument.")
            }
            _ => panic!("invalid error: {e}"),
        },
        _ => panic!("invalid error: {e}"),
    }
}

//#TESTUTIL

/// Get the language setting from the environment variable
#[fixture]
fn lang() -> Language {
    std::env::var("YT_LANG")
        .ok()
        .map_or(Language::En, |l| Language::from_str(&l).unwrap())
}

/// Get a new RustyPipe instance
#[fixture]
fn rp(lang: Language) -> RustyPipe {
    let vdata = std::env::var("YT_VDATA").ok();
    RustyPipe::builder()
        .strict()
        .lang(lang)
        .visitor_data_opt(vdata)
        .build()
        .unwrap()
}

/// Get a flag signaling if the language is set to English
#[fixture]
fn unlocalized(lang: Language) -> bool {
    lang == Language::En
}

/// Get a new RustyPipe instance with pre-set visitor data
fn rp_visitor_data(vdata: &str) -> RustyPipe {
    RustyPipe::builder()
        .strict()
        .visitor_data(vdata)
        .build()
        .unwrap()
}

/// Assert equality within 10% margin
#[track_caller]
fn assert_approx(left: f64, right: f64) {
    if left != right {
        let f = left / right;
        assert!(
            0.9 < f && f < 1.1,
            "{left} not within 10% margin of {right}"
        );
    }
}

/// Assert that number A is greater than or equal to number B
#[track_caller]
fn assert_gte<T: PartialOrd + Display>(a: T, b: T, msg: &str) {
    assert!(a >= b, "expected >= {b} {msg}, got {a}");
}

/// Assert that optional number A is greater than or equal to number B
#[track_caller]
fn assert_gteo<T: PartialOrd + Display>(a: Option<T>, b: T, msg: &str) {
    match a {
        Some(a) => assert_gte(a, b, msg),
        None => panic!("expected >= {b} {msg}, got None"),
    }
}

/// Assert that the paginator produces at least n pages
async fn assert_next<T: FromYtItem, Q: AsRef<RustyPipeQuery>>(
    paginator: Paginator<T>,
    query: Q,
    min_items: usize,
    n_pages: usize,
    on_first: bool,
) {
    let mut p = paginator;
    let query = query.as_ref();
    if on_first {
        assert_gte(p.items.len(), min_items, "items on page 0");
    }

    for i in 0..n_pages {
        p = p.next(query).await.unwrap().expect("paginator exhausted");
        assert_gte(
            p.items.len(),
            min_items,
            &format!("items on page {}", i + 1),
        );
    }
}

/// Assert that the paginator produces at least n items
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

#[track_caller]
fn assert_frameset(frameset: &Frameset) {
    assert_gte(frameset.frame_height, 20, "frame height");
    assert_gte(frameset.frame_height, 20, "frame width");
    assert_gte(frameset.page_count, 1, "page count");
    assert_gte(frameset.total_count, 50, "total count");
    assert_gte(frameset.frames_per_page_x, 3, "frames per page x");
    assert_gte(frameset.frames_per_page_y, 3, "frames per page y");

    let n = frameset.urls().count() as u32;
    assert_eq!(n, frameset.page_count);
}
