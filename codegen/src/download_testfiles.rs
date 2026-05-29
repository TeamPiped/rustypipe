use std::{
    fs::File,
    ops::Sub,
    path::{Path, PathBuf},
    sync::Mutex,
};

use path_macro::path;
use rustypipe::{
    client::{ClientType, RustyPipe},
    model::YouTubeItem,
    param::{
        search_filter::{self, ItemType, SearchFilter},
        ChannelVideoTab, Country,
    },
    report::{Report, Reporter},
};

use crate::util::TESTFILES_DIR;

pub async fn download_testfiles() {
    player().await;
    player_model().await;
    playlist().await;
    playlist_cont().await;
    video_details().await;
    comments_top().await;
    comments_latest().await;
    recommendations().await;
    channel_videos().await;
    channel_shorts().await;
    channel_livestreams().await;
    channel_playlists().await;
    channel_info().await;
    channel_videos_cont().await;
    channel_playlists_cont().await;
    search().await;
    search_cont().await;
    search_playlists().await;
    search_empty().await;
    trending().await;

    music_playlist().await;
    music_playlist_cont().await;
    music_playlist_related().await;
    music_album().await;
    music_search().await;
    music_search_tracks().await;
    music_search_albums().await;
    music_search_artists().await;
    music_search_playlists().await;
    music_search_cont().await;
    music_search_suggestion().await;
    music_artist().await;
    music_details().await;
    music_lyrics().await;
    music_related().await;
    music_radio().await;
    music_radio_cont().await;
    music_new_albums().await;
    music_new_videos().await;
    music_charts().await;
    music_genres().await;
    music_genre().await;

    // User data
    history().await;
    subscriptions().await;
    subscription_feed().await;

    music_history().await;
    music_saved_artists().await;
    music_saved_albums().await;
    music_saved_tracks().await;
    music_saved_playlists().await;
}

const CLIENT_TYPES: [ClientType; 5] = [
    ClientType::Desktop,
    ClientType::DesktopMusic,
    ClientType::Tv,
    ClientType::Android,
    ClientType::Ios,
];

/// Store pretty-printed response json
pub struct TestFileReporter {
    path: PathBuf,
    count: Mutex<u8>,
}

impl TestFileReporter {
    pub fn new<P: AsRef<Path>>(path: P) -> Self {
        Self {
            path: path.as_ref().to_path_buf(),
            count: Mutex::new(0),
        }
    }
}

impl Reporter for TestFileReporter {
    fn report(&self, report: &Report) {
        if report.level != rustypipe::report::Level::DBG {
            println!("Error: {}", report.error.as_deref().unwrap_or_default());
            return;
        }

        let mut root = self.path.clone();
        root.set_file_name("");
        std::fs::create_dir_all(root).unwrap();

        let count = {
            let mut cl = self.count.lock().unwrap();
            *cl += 1;
            cl.sub(1)
        };

        let path = if count == 0 {
            self.path.clone()
        } else {
            let mut p = self.path.clone();
            p.set_file_name(format!(
                "{}_{}.{}",
                p.file_stem().unwrap_or_default().to_string_lossy(),
                count,
                p.extension().unwrap_or_default().to_string_lossy()
            ));
            p
        };

        let data =
            serde_json::from_str::<serde_json::Value>(&report.http_request.resp_body).unwrap();
        let file = File::create(&path).unwrap();
        serde_json::to_writer_pretty(file, &data).unwrap();

        println!("Downloaded {}", path.display());
    }
}

fn rp_testfile(json_path: &Path) -> RustyPipe {
    let reporter = TestFileReporter::new(json_path);
    RustyPipe::builder()
        .reporter(Box::new(reporter))
        .report()
        .strict()
        .build()
        .unwrap()
}

async fn player() {
    let video_id = "pPvd8UxmSbQ";

    for client_type in CLIENT_TYPES {
        let json_path =
            path!(*TESTFILES_DIR / "player" / format!("{client_type:?}_video.json").to_lowercase());
        if json_path.exists() {
            continue;
        }

        let rp = rp_testfile(&json_path);
        rp.query()
            .player_from_client(video_id, client_type)
            .await
            .unwrap();
    }
}

async fn player_model() {
    let rp = RustyPipe::builder().strict().build().unwrap();

    for (name, id) in [("multilanguage", "tVWWp1PqDus"), ("hdr", "LXb3EKWsInQ")] {
        let json_path =
            path!(*TESTFILES_DIR / "player_model" / format!("{name}.json").to_lowercase());
        if json_path.exists() {
            continue;
        }

        let player_data = rp
            .query()
            .player_from_client(id, ClientType::Desktop)
            .await
            .unwrap();
        let file = File::create(&json_path).unwrap();
        serde_json::to_writer_pretty(file, &player_data).unwrap();

        println!("Downloaded {}", json_path.display());
    }
}

async fn playlist() {
    for (name, id) in [
        ("short", "RDCLAK5uy_kFQXdnqMaQCVx2wpUM4ZfbsGCDibZtkJk"),
        ("long", "PL5dDx681T4bR7ZF1IuWzOv1omlRbE7PiJ"),
        ("nomusic", "PL1J-6JOckZtE_P9Xx8D3b2O6w0idhuKBe"),
        ("live", "UULVvqRdlKsE5Q8mf8YXbdIJLw"),
    ] {
        let json_path = path!(*TESTFILES_DIR / "playlist" / format!("playlist_{name}.json"));
        if json_path.exists() {
            continue;
        }

        let rp = rp_testfile(&json_path);
        rp.query().playlist(id).await.unwrap();
    }
}

async fn playlist_cont() {
    let json_path = path!(*TESTFILES_DIR / "playlist" / "playlist_cont.json");
    if json_path.exists() {
        return;
    }

    let rp = RustyPipe::new();
    let playlist = rp
        .query()
        .playlist("PL5dDx681T4bR7ZF1IuWzOv1omlRbE7PiJ")
        .await
        .unwrap();

    let rp = rp_testfile(&json_path);
    playlist.videos.next(rp.query()).await.unwrap().unwrap();
}

async fn video_details() {
    for (name, id) in [
        ("music", "XuM2onMGvTI"),
        ("mv", "ZeerrnuLi5E"),
        ("ccommons", "0rb9CfOvojk"),
        ("chapters", "nFDBxBUfE74"),
        ("live", "86YLFOog4GM"),
        ("agegate", "HRKu0cvrr_o"),
        ("collaborators", "G78AnHpIw5w"),
    ] {
        let json_path =
            path!(*TESTFILES_DIR / "video_details" / format!("video_details_{name}.json"));
        if json_path.exists() {
            continue;
        }

        let rp = rp_testfile(&json_path);
        rp.query().video_details(id).await.unwrap();
    }
}

async fn comments_top() {
    let json_path = path!(*TESTFILES_DIR / "video_details" / "comments_top.json");
    if json_path.exists() {
        return;
    }

    let rp = RustyPipe::new();
    let details = rp.query().video_details("ZeerrnuLi5E").await.unwrap();

    let rp = rp_testfile(&json_path);
    details
        .top_comments
        .next(rp.query())
        .await
        .unwrap()
        .unwrap();
}

async fn comments_latest() {
    let json_path = path!(*TESTFILES_DIR / "video_details" / "comments_latest.json");
    if json_path.exists() {
        return;
    }

    let rp = RustyPipe::new();
    let details = rp.query().video_details("ZeerrnuLi5E").await.unwrap();

    let rp = rp_testfile(&json_path);
    details
        .latest_comments
        .next(rp.query())
        .await
        .unwrap()
        .unwrap();
}

async fn recommendations() {
    let json_path = path!(*TESTFILES_DIR / "video_details" / "recommendations.json");
    if json_path.exists() {
        return;
    }

    let rp = RustyPipe::new();
    let details = rp.query().video_details("ZeerrnuLi5E").await.unwrap();

    let rp = rp_testfile(&json_path);
    details.recommended.next(rp.query()).await.unwrap();
}

async fn channel_videos() {
    for (name, id) in [
        ("base", "UC2DjFE7Xf11URZqWBigcVOQ"),
        ("music", "UC_vmjW5e1xEHhYjY2a0kK1A"), // YouTube Music channels have no videos
        ("shorts", "UCh8gHdtzO2tXd593_bjErWg"), // shorts and livestreams are rendered differently
        ("live", "UChs0pSaEoNLV4mevBFGaoKA"),
        ("empty", "UCxBa895m48H5idw5li7h-0g"),
        ("upcoming", "UCcvfHa-GHSOHFAjU0-Ie57A"),
    ] {
        let json_path = path!(*TESTFILES_DIR / "channel" / format!("channel_videos_{name}.json"));
        if json_path.exists() {
            continue;
        }

        let rp = rp_testfile(&json_path);
        rp.query().channel_videos(id).await.unwrap();
    }
}

async fn channel_shorts() {
    let json_path = path!(*TESTFILES_DIR / "channel" / "channel_shorts.json");
    if json_path.exists() {
        return;
    }

    let rp = rp_testfile(&json_path);
    rp.query()
        .channel_videos_tab("UCh8gHdtzO2tXd593_bjErWg", ChannelVideoTab::Shorts)
        .await
        .unwrap();
}

async fn channel_livestreams() {
    let json_path = path!(*TESTFILES_DIR / "channel" / "channel_livestreams.json");
    if json_path.exists() {
        return;
    }

    let rp = rp_testfile(&json_path);
    rp.query()
        .channel_videos_tab("UC2DjFE7Xf11URZqWBigcVOQ", ChannelVideoTab::Live)
        .await
        .unwrap();
}

async fn channel_playlists() {
    let json_path = path!(*TESTFILES_DIR / "channel" / "channel_playlists.json");
    if json_path.exists() {
        return;
    }

    let rp = rp_testfile(&json_path);
    rp.query()
        .channel_playlists("UC2DjFE7Xf11URZqWBigcVOQ")
        .await
        .unwrap();
}

async fn channel_info() {
    let json_path = path!(*TESTFILES_DIR / "channel" / "channel_info.json");
    if json_path.exists() {
        return;
    }

    let rp = rp_testfile(&json_path);
    rp.query()
        .channel_info("UC2DjFE7Xf11URZqWBigcVOQ")
        .await
        .unwrap();
}

async fn channel_videos_cont() {
    let json_path = path!(*TESTFILES_DIR / "channel" / "channel_videos_cont.json");
    if json_path.exists() {
        return;
    }

    let rp = RustyPipe::new();
    let videos = rp
        .query()
        .channel_videos("UC2DjFE7Xf11URZqWBigcVOQ")
        .await
        .unwrap();

    let rp = rp_testfile(&json_path);
    videos.content.next(rp.query()).await.unwrap().unwrap();
}

async fn channel_playlists_cont() {
    let json_path = path!(*TESTFILES_DIR / "channel" / "channel_playlists_cont.json");
    if json_path.exists() {
        return;
    }

    let rp = RustyPipe::new();
    let playlists = rp
        .query()
        .channel_playlists("UC2DjFE7Xf11URZqWBigcVOQ")
        .await
        .unwrap();

    let rp = rp_testfile(&json_path);
    playlists.content.next(rp.query()).await.unwrap().unwrap();
}

async fn search() {
    let json_path = path!(*TESTFILES_DIR / "search" / "default.json");
    if json_path.exists() {
        return;
    }

    let rp = rp_testfile(&json_path);
    rp.query()
        .search::<YouTubeItem, _>("doobydoobap")
        .await
        .unwrap();
}

async fn search_cont() {
    let json_path = path!(*TESTFILES_DIR / "search" / "cont.json");
    if json_path.exists() {
        return;
    }

    let rp = RustyPipe::new();
    let search = rp
        .query()
        .search::<YouTubeItem, _>("doobydoobap")
        .await
        .unwrap();

    let rp = rp_testfile(&json_path);
    search.items.next(rp.query()).await.unwrap().unwrap();
}

async fn search_playlists() {
    let json_path = path!(*TESTFILES_DIR / "search" / "playlists.json");
    if json_path.exists() {
        return;
    }

    let rp = rp_testfile(&json_path);
    rp.query()
        .search_filter::<YouTubeItem, _>("pop", &SearchFilter::new().item_type(ItemType::Playlist))
        .await
        .unwrap();
}

async fn search_empty() {
    let json_path = path!(*TESTFILES_DIR / "search" / "empty.json");
    if json_path.exists() {
        return;
    }

    let rp = rp_testfile(&json_path);
    rp.query()
        .search_filter::<YouTubeItem, _>(
            "test",
            &SearchFilter::new()
                .feature(search_filter::Feature::IsLive)
                .feature(search_filter::Feature::Is3d),
        )
        .await
        .unwrap();
}

async fn trending() {
    let json_path = path!(*TESTFILES_DIR / "trends" / "trending_videos.json");
    if json_path.exists() {
        return;
    }

    let rp = rp_testfile(&json_path);
    rp.query().trending().await.unwrap();
}

async fn history() {
    let json_path = path!(*TESTFILES_DIR / "userdata" / "history.json");
    if json_path.exists() {
        return;
    }

    let rp = rp_testfile(&json_path);
    rp.query().history().await.unwrap();
}

async fn subscriptions() {
    let json_path = path!(*TESTFILES_DIR / "userdata" / "subscriptions.json");
    if json_path.exists() {
        return;
    }

    let rp = rp_testfile(&json_path);
    rp.query().subscriptions().await.unwrap();
}

async fn subscription_feed() {
    let json_path = path!(*TESTFILES_DIR / "userdata" / "subscription_feed.json");
    if json_path.exists() {
        return;
    }

    let rp = rp_testfile(&json_path);
    rp.query().subscription_feed().await.unwrap();
}

async fn music_playlist() {
    for (name, id) in [
        ("short", "RDCLAK5uy_kFQXdnqMaQCVx2wpUM4ZfbsGCDibZtkJk"),
        ("long", "PL5dDx681T4bR7ZF1IuWzOv1omlRbE7PiJ"),
        ("nomusic", "PL1J-6JOckZtE_P9Xx8D3b2O6w0idhuKBe"),
    ] {
        let json_path = path!(*TESTFILES_DIR / "music_playlist" / format!("playlist_{name}.json"));
        if json_path.exists() {
            continue;
        }

        let rp = rp_testfile(&json_path);
        rp.query().music_playlist(id).await.unwrap();
    }
}

async fn music_playlist_cont() {
    let json_path = path!(*TESTFILES_DIR / "music_playlist" / "playlist_cont.json");
    if json_path.exists() {
        return;
    }

    let rp = RustyPipe::new();
    let playlist = rp
        .query()
        .music_playlist("PL5dDx681T4bR7ZF1IuWzOv1omlRbE7PiJ")
        .await
        .unwrap();

    let rp = rp_testfile(&json_path);
    playlist.tracks.next(rp.query()).await.unwrap().unwrap();
}

async fn music_playlist_related() {
    let json_path = path!(*TESTFILES_DIR / "music_playlist" / "playlist_related.json");
    if json_path.exists() {
        return;
    }

    let rp = RustyPipe::new();
    let playlist = rp
        .query()
        .music_playlist("PL5dDx681T4bR7ZF1IuWzOv1omlRbE7PiJ")
        .await
        .unwrap();

    let rp = rp_testfile(&json_path);
    playlist
        .related_playlists
        .next(rp.query())
        .await
        .unwrap()
        .unwrap();
}

async fn music_album() {
    for (name, id) in [
        ("one_artist", "MPREb_nlBWQROfvjo"),
        ("various_artists", "MPREb_8QkDeEIawvX"),
        ("single", "MPREb_bHfHGoy7vuv"),
        ("description", "MPREb_PiyfuVl6aYd"),
        ("unavailable", "MPREb_AzuWg8qAVVl"),
    ] {
        let json_path = path!(*TESTFILES_DIR / "music_playlist" / format!("album_{name}.json"));
        if json_path.exists() {
            continue;
        }

        let rp = rp_testfile(&json_path);
        rp.query().music_album(id).await.unwrap();
    }
}

async fn music_search() {
    for (name, query) in [
        ("default", "black mamba"),
        ("typo", "liblingsmensch"),
        ("radio", "pop radio"),
        ("artist", "taylor swift"),
    ] {
        let json_path = path!(*TESTFILES_DIR / "music_search" / format!("main_{name}.json"));
        if json_path.exists() {
            continue;
        }

        let rp = rp_testfile(&json_path);
        rp.query().music_search_main(query).await.unwrap();
    }
}

async fn music_search_tracks() {
    for (name, query, videos) in [
        ("default", "black mamba", false),
        ("videos", "black mamba", true),
        ("typo", "liblingsmensch", false),
        (
            "no_artist_link",
            "Am sichersten seid ihr im Auto #HURRICANESWIMTEAM",
            false,
        ),
    ] {
        let json_path = path!(*TESTFILES_DIR / "music_search" / format!("tracks_{name}.json"));
        if json_path.exists() {
            continue;
        }

        let rp = rp_testfile(&json_path);
        if videos {
            rp.query().music_search_videos(query).await.unwrap();
        } else {
            rp.query().music_search_tracks(query).await.unwrap();
        }
    }
}

async fn music_search_albums() {
    let json_path = path!(*TESTFILES_DIR / "music_search" / "albums.json");
    if json_path.exists() {
        return;
    }

    let rp = rp_testfile(&json_path);
    rp.query().music_search_albums("black mamba").await.unwrap();
}

async fn music_search_artists() {
    let json_path = path!(*TESTFILES_DIR / "music_search" / "artists.json");
    if json_path.exists() {
        return;
    }

    let rp = rp_testfile(&json_path);
    rp.query()
        .music_search_artists("black mamba")
        .await
        .unwrap();
}

async fn music_search_playlists() {
    for (name, community) in [("ytm", false), ("community", true)] {
        let json_path = path!(*TESTFILES_DIR / "music_search" / format!("playlists_{name}.json"));
        if json_path.exists() {
            continue;
        }

        let rp = rp_testfile(&json_path);
        rp.query()
            .music_search_playlists("pop", community)
            .await
            .unwrap();
    }
}

async fn music_search_cont() {
    let json_path = path!(*TESTFILES_DIR / "music_search" / "tracks_cont.json");
    if json_path.exists() {
        return;
    }

    let rp = RustyPipe::new();
    let res = rp.query().music_search_tracks("black mamba").await.unwrap();

    let rp = rp_testfile(&json_path);
    res.items.next(rp.query()).await.unwrap().unwrap();
}

async fn music_search_suggestion() {
    for (name, query) in [("default", "t"), ("empty", "reujbhevmfndxnjrze")] {
        let json_path = path!(*TESTFILES_DIR / "music_search" / format!("suggestion_{name}.json"));
        if json_path.exists() {
            continue;
        }

        let rp = rp_testfile(&json_path);
        rp.query().music_search_suggestion(query).await.unwrap();
    }
}

async fn music_artist() {
    for (name, id, all_albums) in [
        ("default", "UClmXPfaYhXOYsNn_QUyheWQ", true),
        ("only_singles", "UCfwCE5VhPMGxNPFxtVv7lRw", true),
        ("no_artist", "UCh8gHdtzO2tXd593_bjErWg", true),
        ("only_more_singles", "UC0aXrjVxG5pZr99v77wZdPQ", true),
        ("secondary_channel", "UCC9192yGQD25eBZgFZ84MPw", false),
    ] {
        let json_path = path!(*TESTFILES_DIR / "music_artist" / format!("artist_{name}.json"));
        if json_path.exists() {
            continue;
        }

        let rp = rp_testfile(&json_path);
        rp.query().music_artist(id, all_albums).await.unwrap();
    }
}

async fn music_details() {
    for (name, id) in [("mv", "ZeerrnuLi5E"), ("track", "7nigXQS1Xb0")] {
        let json_path = path!(*TESTFILES_DIR / "music_details" / format!("details_{name}.json"));
        if json_path.exists() {
            continue;
        }

        let rp = rp_testfile(&json_path);
        rp.query().music_details(id).await.unwrap();
    }
}

async fn music_lyrics() {
    let json_path = path!(*TESTFILES_DIR / "music_details" / "lyrics.json");
    if json_path.exists() {
        return;
    }

    let rp = RustyPipe::new();
    let res = rp.query().music_details("n4tK7LYFxI0").await.unwrap();

    let rp = rp_testfile(&json_path);
    rp.query()
        .music_lyrics(&res.lyrics_id.unwrap())
        .await
        .unwrap();
}

async fn music_related() {
    let json_path = path!(*TESTFILES_DIR / "music_details" / "related.json");
    if json_path.exists() {
        return;
    }

    let rp = RustyPipe::new();
    let res = rp.query().music_details("ZeerrnuLi5E").await.unwrap();

    let rp = rp_testfile(&json_path);
    rp.query()
        .music_related(&res.related_id.unwrap())
        .await
        .unwrap();
}

async fn music_radio() {
    for (name, id) in [("mv", "RDAMVMZeerrnuLi5E"), ("track", "RDAMVM7nigXQS1Xb0")] {
        let json_path = path!(*TESTFILES_DIR / "music_details" / format!("radio_{name}.json"));
        if json_path.exists() {
            continue;
        }

        let rp = rp_testfile(&json_path);
        rp.query().music_radio(id).await.unwrap();
    }
}

async fn music_radio_cont() {
    let json_path = path!(*TESTFILES_DIR / "music_details" / "radio_cont.json");
    if json_path.exists() {
        return;
    }

    let rp = RustyPipe::new();
    let res = rp.query().music_radio("RDAMVM7nigXQS1Xb0").await.unwrap();

    let rp = rp_testfile(&json_path);
    res.next(rp.query()).await.unwrap().unwrap();
}

async fn music_new_albums() {
    let json_path = path!(*TESTFILES_DIR / "music_new" / "albums_default.json");
    if json_path.exists() {
        return;
    }

    let rp = rp_testfile(&json_path);
    rp.query().music_new_albums().await.unwrap();
}

async fn music_new_videos() {
    let json_path = path!(*TESTFILES_DIR / "music_new" / "videos_default.json");
    if json_path.exists() {
        return;
    }

    let rp = rp_testfile(&json_path);
    rp.query().music_new_videos().await.unwrap();
}

async fn music_charts() {
    for (name, country) in [("global", Some(Country::Zz)), ("US", Some(Country::Us))] {
        let json_path = path!(*TESTFILES_DIR / "music_charts" / format!("charts_{name}.json"));
        if json_path.exists() {
            continue;
        }

        let rp = rp_testfile(&json_path);
        rp.query().music_charts(country).await.unwrap();
    }
}

async fn music_genres() {
    let json_path = path!(*TESTFILES_DIR / "music_genres" / "genres.json");
    if json_path.exists() {
        return;
    }

    let rp = rp_testfile(&json_path);
    rp.query().music_genres().await.unwrap();
}

async fn music_genre() {
    for (name, id) in [
        ("default", "ggMPOg1uX1lMbVZmbzl6NlJ3"),
        ("mood", "ggMPOg1uX1JOQWZFeDByc2Jm"),
    ] {
        let json_path = path!(*TESTFILES_DIR / "music_genres" / format!("genre_{name}.json"));
        if json_path.exists() {
            continue;
        }

        let rp = rp_testfile(&json_path);
        rp.query().music_genre(id).await.unwrap();
    }
}

async fn music_history() {
    let json_path = path!(*TESTFILES_DIR / "music_userdata" / "music_history.json");
    if json_path.exists() {
        return;
    }

    let rp = rp_testfile(&json_path);
    rp.query().music_history().await.unwrap();
}

async fn music_saved_artists() {
    let json_path = path!(*TESTFILES_DIR / "music_userdata" / "saved_artists.json");
    if json_path.exists() {
        return;
    }

    let rp = rp_testfile(&json_path);
    rp.query().music_saved_artists().await.unwrap();
}

async fn music_saved_albums() {
    let json_path = path!(*TESTFILES_DIR / "music_userdata" / "saved_albums.json");
    if json_path.exists() {
        return;
    }

    let rp = rp_testfile(&json_path);
    rp.query().music_saved_albums().await.unwrap();
}

async fn music_saved_tracks() {
    let json_path = path!(*TESTFILES_DIR / "music_userdata" / "saved_tracks.json");
    if json_path.exists() {
        return;
    }

    let rp = rp_testfile(&json_path);
    rp.query().music_saved_tracks().await.unwrap();
}

async fn music_saved_playlists() {
    let json_path = path!(*TESTFILES_DIR / "music_userdata" / "saved_playlists.json");
    if json_path.exists() {
        return;
    }

    let rp = rp_testfile(&json_path);
    rp.query().music_saved_playlists().await.unwrap();
}
