use std::{
    fs::File,
    ops::Sub,
    path::{Path, PathBuf},
    sync::Mutex,
};

use rustypipe::{
    client::{ClientType, RustyPipe},
    param::search_filter::{self, Entity, SearchFilter},
    report::{Report, Reporter},
};

pub async fn download_testfiles(project_root: &Path) {
    let mut testfiles = project_root.to_path_buf();
    testfiles.push("testfiles");

    player(&testfiles).await;
    player_model(&testfiles).await;
    playlist(&testfiles).await;
    playlist_cont(&testfiles).await;
    video_details(&testfiles).await;
    comments_top(&testfiles).await;
    comments_latest(&testfiles).await;
    recommendations(&testfiles).await;
    channel_videos(&testfiles).await;
    channel_shorts(&testfiles).await;
    channel_livestreams(&testfiles).await;
    channel_playlists(&testfiles).await;
    channel_info(&testfiles).await;
    channel_videos_cont(&testfiles).await;
    channel_playlists_cont(&testfiles).await;
    search(&testfiles).await;
    search_cont(&testfiles).await;
    search_playlists(&testfiles).await;
    search_empty(&testfiles).await;
    startpage(&testfiles).await;
    startpage_cont(&testfiles).await;
    trending(&testfiles).await;

    music_playlist(&testfiles).await;
    music_playlist_cont(&testfiles).await;
    music_album(&testfiles).await;
    music_search(&testfiles).await;
    music_search_tracks(&testfiles).await;
    music_search_albums(&testfiles).await;
    music_search_artists(&testfiles).await;
    music_search_playlists(&testfiles).await;
    music_search_cont(&testfiles).await;
    music_artist(&testfiles).await;
}

const CLIENT_TYPES: [ClientType; 5] = [
    ClientType::Desktop,
    ClientType::DesktopMusic,
    ClientType::TvHtml5Embed,
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
}

async fn player(testfiles: &Path) {
    let video_id = "pPvd8UxmSbQ";

    for client_type in CLIENT_TYPES {
        let mut json_path = testfiles.to_path_buf();
        json_path.push("player");
        json_path.push(format!("{:?}_video.json", client_type).to_lowercase());

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

async fn player_model(testfiles: &Path) {
    let rp = RustyPipe::builder().strict().build();

    for (name, id) in [("multilanguage", "tVWWp1PqDus"), ("hdr", "LXb3EKWsInQ")] {
        let mut json_path = testfiles.to_path_buf();
        json_path.push("player_model");
        json_path.push(format!("{}.json", name).to_lowercase());

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

async fn playlist(testfiles: &Path) {
    for (name, id) in [
        ("short", "RDCLAK5uy_kFQXdnqMaQCVx2wpUM4ZfbsGCDibZtkJk"),
        ("long", "PL5dDx681T4bR7ZF1IuWzOv1omlRbE7PiJ"),
        ("nomusic", "PL1J-6JOckZtE_P9Xx8D3b2O6w0idhuKBe"),
    ] {
        let mut json_path = testfiles.to_path_buf();
        json_path.push("playlist");
        json_path.push(format!("playlist_{}.json", name));
        if json_path.exists() {
            continue;
        }

        let rp = rp_testfile(&json_path);
        rp.query().playlist(id).await.unwrap();
    }
}

async fn playlist_cont(testfiles: &Path) {
    let mut json_path = testfiles.to_path_buf();
    json_path.push("playlist");
    json_path.push("playlist_cont.json");
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
    playlist.videos.next(&rp.query()).await.unwrap().unwrap();
}

async fn video_details(testfiles: &Path) {
    for (name, id) in [
        ("music", "XuM2onMGvTI"),
        ("mv", "ZeerrnuLi5E"),
        ("ccommons", "0rb9CfOvojk"),
        ("chapters", "nFDBxBUfE74"),
        ("live", "86YLFOog4GM"),
        ("agegate", "HRKu0cvrr_o"),
    ] {
        let mut json_path = testfiles.to_path_buf();
        json_path.push("video_details");
        json_path.push(format!("video_details_{}.json", name));
        if json_path.exists() {
            continue;
        }

        let rp = rp_testfile(&json_path);
        rp.query().video_details(id).await.unwrap();
    }
}

async fn comments_top(testfiles: &Path) {
    let mut json_path = testfiles.to_path_buf();
    json_path.push("video_details");
    json_path.push("comments_top.json");
    if json_path.exists() {
        return;
    }

    let rp = RustyPipe::new();
    let details = rp.query().video_details("ZeerrnuLi5E").await.unwrap();

    let rp = rp_testfile(&json_path);
    details
        .top_comments
        .next(&rp.query())
        .await
        .unwrap()
        .unwrap();
}

async fn comments_latest(testfiles: &Path) {
    let mut json_path = testfiles.to_path_buf();
    json_path.push("video_details");
    json_path.push("comments_latest.json");
    if json_path.exists() {
        return;
    }

    let rp = RustyPipe::new();
    let details = rp.query().video_details("ZeerrnuLi5E").await.unwrap();

    let rp = rp_testfile(&json_path);
    details
        .latest_comments
        .next(&rp.query())
        .await
        .unwrap()
        .unwrap();
}

async fn recommendations(testfiles: &Path) {
    let mut json_path = testfiles.to_path_buf();
    json_path.push("video_details");
    json_path.push("recommendations.json");
    if json_path.exists() {
        return;
    }

    let rp = RustyPipe::new();
    let details = rp.query().video_details("ZeerrnuLi5E").await.unwrap();

    let rp = rp_testfile(&json_path);
    details.recommended.next(&rp.query()).await.unwrap();
}

async fn channel_videos(testfiles: &Path) {
    for (name, id) in [
        ("base", "UC2DjFE7Xf11URZqWBigcVOQ"),
        ("music", "UC_vmjW5e1xEHhYjY2a0kK1A"), // YouTube Music channels have no videos
        ("shorts", "UCh8gHdtzO2tXd593_bjErWg"), // shorts and livestreams are rendered differently
        ("live", "UChs0pSaEoNLV4mevBFGaoKA"),
        ("empty", "UCxBa895m48H5idw5li7h-0g"),
        ("upcoming", "UCcvfHa-GHSOHFAjU0-Ie57A"),
    ] {
        let mut json_path = testfiles.to_path_buf();
        json_path.push("channel");
        json_path.push(format!("channel_videos_{}.json", name));
        if json_path.exists() {
            continue;
        }

        let rp = rp_testfile(&json_path);
        rp.query().channel_videos(id).await.unwrap();
    }
}

async fn channel_shorts(testfiles: &Path) {
    let mut json_path = testfiles.to_path_buf();
    json_path.push("channel");
    json_path.push("channel_shorts.json");
    if json_path.exists() {
        return;
    }

    let rp = rp_testfile(&json_path);
    rp.query()
        .channel_shorts("UCh8gHdtzO2tXd593_bjErWg")
        .await
        .unwrap();
}

async fn channel_livestreams(testfiles: &Path) {
    let mut json_path = testfiles.to_path_buf();
    json_path.push("channel");
    json_path.push("channel_livestreams.json");
    if json_path.exists() {
        return;
    }

    let rp = rp_testfile(&json_path);
    rp.query()
        .channel_livestreams("UC2DjFE7Xf11URZqWBigcVOQ")
        .await
        .unwrap();
}

async fn channel_playlists(testfiles: &Path) {
    let mut json_path = testfiles.to_path_buf();
    json_path.push("channel");
    json_path.push("channel_playlists.json");
    if json_path.exists() {
        return;
    }

    let rp = rp_testfile(&json_path);
    rp.query()
        .channel_playlists("UC2DjFE7Xf11URZqWBigcVOQ")
        .await
        .unwrap();
}

async fn channel_info(testfiles: &Path) {
    let mut json_path = testfiles.to_path_buf();
    json_path.push("channel");
    json_path.push("channel_info.json");
    if json_path.exists() {
        return;
    }

    let rp = rp_testfile(&json_path);
    rp.query()
        .channel_info("UC2DjFE7Xf11URZqWBigcVOQ")
        .await
        .unwrap();
}

async fn channel_videos_cont(testfiles: &Path) {
    let mut json_path = testfiles.to_path_buf();
    json_path.push("channel");
    json_path.push("channel_videos_cont.json");
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
    videos.content.next(&rp.query()).await.unwrap().unwrap();
}

async fn channel_playlists_cont(testfiles: &Path) {
    let mut json_path = testfiles.to_path_buf();
    json_path.push("channel");
    json_path.push("channel_playlists_cont.json");
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
    playlists.content.next(&rp.query()).await.unwrap().unwrap();
}

async fn search(testfiles: &Path) {
    let mut json_path = testfiles.to_path_buf();
    json_path.push("search");
    json_path.push("default.json");
    if json_path.exists() {
        return;
    }

    let rp = rp_testfile(&json_path);
    rp.query().search("doobydoobap").await.unwrap();
}

async fn search_cont(testfiles: &Path) {
    let mut json_path = testfiles.to_path_buf();
    json_path.push("search");
    json_path.push("cont.json");
    if json_path.exists() {
        return;
    }

    let rp = RustyPipe::new();
    let search = rp.query().search("doobydoobap").await.unwrap();

    let rp = rp_testfile(&json_path);
    search.items.next(&rp.query()).await.unwrap().unwrap();
}

async fn search_playlists(testfiles: &Path) {
    let mut json_path = testfiles.to_path_buf();
    json_path.push("search");
    json_path.push("playlists.json");
    if json_path.exists() {
        return;
    }

    let rp = rp_testfile(&json_path);
    rp.query()
        .search_filter("pop", &SearchFilter::new().entity(Entity::Playlist))
        .await
        .unwrap();
}

async fn search_empty(testfiles: &Path) {
    let mut json_path = testfiles.to_path_buf();
    json_path.push("search");
    json_path.push("empty.json");
    if json_path.exists() {
        return;
    }

    let rp = rp_testfile(&json_path);
    rp.query()
        .search_filter(
            "test",
            &SearchFilter::new()
                .feature(search_filter::Feature::IsLive)
                .feature(search_filter::Feature::Is3d),
        )
        .await
        .unwrap();
}

async fn startpage(testfiles: &Path) {
    let mut json_path = testfiles.to_path_buf();
    json_path.push("trends");
    json_path.push("startpage.json");
    if json_path.exists() {
        return;
    }

    let rp = rp_testfile(&json_path);
    rp.query().startpage().await.unwrap();
}

async fn startpage_cont(testfiles: &Path) {
    let mut json_path = testfiles.to_path_buf();
    json_path.push("trends");
    json_path.push("startpage_cont.json");
    if json_path.exists() {
        return;
    }

    let rp = RustyPipe::new();
    let startpage = rp.query().startpage().await.unwrap();

    let rp = rp_testfile(&json_path);
    startpage.next(&rp.query()).await.unwrap();
}

async fn trending(testfiles: &Path) {
    let mut json_path = testfiles.to_path_buf();
    json_path.push("trends");
    json_path.push("trending.json");
    if json_path.exists() {
        return;
    }

    let rp = rp_testfile(&json_path);
    rp.query().trending().await.unwrap();
}

async fn music_playlist(testfiles: &Path) {
    for (name, id) in [
        ("short", "RDCLAK5uy_kFQXdnqMaQCVx2wpUM4ZfbsGCDibZtkJk"),
        ("long", "PL5dDx681T4bR7ZF1IuWzOv1omlRbE7PiJ"),
        ("nomusic", "PL1J-6JOckZtE_P9Xx8D3b2O6w0idhuKBe"),
    ] {
        let mut json_path = testfiles.to_path_buf();
        json_path.push("music_playlist");
        json_path.push(format!("playlist_{}.json", name));
        if json_path.exists() {
            continue;
        }

        let rp = rp_testfile(&json_path);
        rp.query().music_playlist(id).await.unwrap();
    }
}

async fn music_playlist_cont(testfiles: &Path) {
    let mut json_path = testfiles.to_path_buf();
    json_path.push("music_playlist");
    json_path.push("playlist_cont.json");
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
    playlist.tracks.next(&rp.query()).await.unwrap().unwrap();
}

async fn music_album(testfiles: &Path) {
    for (name, id) in [
        ("one_artist", "MPREb_nlBWQROfvjo"),
        ("various_artists", "MPREb_8QkDeEIawvX"),
        ("single", "MPREb_bHfHGoy7vuv"),
        ("description", "MPREb_PiyfuVl6aYd"),
    ] {
        let mut json_path = testfiles.to_path_buf();
        json_path.push("music_playlist");
        json_path.push(format!("album_{}.json", name));
        if json_path.exists() {
            continue;
        }

        let rp = rp_testfile(&json_path);
        rp.query().music_album(id).await.unwrap();
    }
}

async fn music_search(testfiles: &Path) {
    for (name, query) in [("default", "black mamba"), ("typo", "liblingsmensch")] {
        let mut json_path = testfiles.to_path_buf();
        json_path.push("music_search");
        json_path.push(format!("{}.json", name));
        if json_path.exists() {
            continue;
        }

        let rp = rp_testfile(&json_path);
        rp.query().music_search(query).await.unwrap();
    }
}

async fn music_search_tracks(testfiles: &Path) {
    for (name, query, videos) in [
        ("default", "black mamba", false),
        ("videos", "black mamba", true),
        ("typo", "liblingsmensch", false),
    ] {
        let mut json_path = testfiles.to_path_buf();
        json_path.push("music_search");
        json_path.push(format!("tracks_{}.json", name));
        if json_path.exists() {
            continue;
        }

        let rp = rp_testfile(&json_path);
        rp.query().music_search_tracks(query, videos).await.unwrap();
    }
}

async fn music_search_albums(testfiles: &Path) {
    let mut json_path = testfiles.to_path_buf();
    json_path.push("music_search");
    json_path.push("albums.json");
    if json_path.exists() {
        return;
    }

    let rp = rp_testfile(&json_path);
    rp.query().music_search_albums("black mamba").await.unwrap();
}

async fn music_search_artists(testfiles: &Path) {
    let mut json_path = testfiles.to_path_buf();
    json_path.push("music_search");
    json_path.push("artists.json");
    if json_path.exists() {
        return;
    }

    let rp = rp_testfile(&json_path);
    rp.query()
        .music_search_artists("black mamba")
        .await
        .unwrap();
}

async fn music_search_playlists(testfiles: &Path) {
    for (name, community) in [("ytm", false), ("community", true)] {
        let mut json_path = testfiles.to_path_buf();
        json_path.push("music_search");
        json_path.push(format!("playlists_{}.json", name));
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

async fn music_search_cont(testfiles: &Path) {
    let mut json_path = testfiles.to_path_buf();
    json_path.push("music_search");
    json_path.push("tracks_cont.json");
    if json_path.exists() {
        return;
    }

    let rp = RustyPipe::new();
    let res = rp
        .query()
        .music_search_tracks("black mamba", false)
        .await
        .unwrap();

    let rp = rp_testfile(&json_path);
    res.items.next(&rp.query()).await.unwrap().unwrap();
}

async fn music_artist(testfiles: &Path) {
    for (name, id) in [
        ("default", "UClmXPfaYhXOYsNn_QUyheWQ"),
        ("no_more_albums", "UC_vmjW5e1xEHhYjY2a0kK1A"),
        ("only_singles", "UCfwCE5VhPMGxNPFxtVv7lRw"),
        ("no_artist", "UCh8gHdtzO2tXd593_bjErWg"),
        ("only_more_singles", "UC0aXrjVxG5pZr99v77wZdPQ"),
    ] {
        let mut json_path = testfiles.to_path_buf();
        json_path.push("music_artist");
        json_path.push(format!("artist_{}.json", name));
        if json_path.exists() {
            continue;
        }

        let rp = rp_testfile(&json_path);
        rp.query().music_artist(id, true).await.unwrap();
    }
}
