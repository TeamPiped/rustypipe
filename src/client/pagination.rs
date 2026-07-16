use std::fmt::Debug;

use crate::error::{Error, ExtractionError};
use crate::json::{
    value_from_json_value, yt_continuation, yt_estimated_results, ytq, JsonDoc, JsonNode,
};
use crate::model::{
    paginator::{ContinuationEndpoint, Paginator},
    traits::FromYtItem,
    Comment, MusicItem, YouTubeItem,
};
use crate::request_body::ytbody;
use crate::serializer::{ItemsAccumulator, MapResult};

#[cfg(feature = "userdata")]
use crate::model::{HistoryItem, TrackItem, VideoItem};

#[cfg(feature = "userdata")]
use super::response::music_item::map_music_items_value;
use super::response::music_item::{
    map_music_continuation_items, map_queue_item, music_carousel_node, music_grid_items,
    music_grid_node, music_section_list_continuation_node, music_shelf_continuation_node,
    music_shelf_node,
};
#[cfg(feature = "userdata")]
use super::response::{music_item::MusicShelf, MusicContinuationData};
use super::{response, ClientType, MapEndpoint, MapRespCtx, MapRespOptions, RustyPipeQuery};

#[derive(Debug)]
pub(crate) struct ContinuationMarker;

#[derive(Debug)]
pub(crate) struct MusicContinuationMarker;

impl RustyPipeQuery {
    /// Get more YouTube items from the given continuation token and endpoint
    #[tracing::instrument(skip(self), level = "error")]
    pub async fn continuation<T: FromYtItem, S: AsRef<str> + Debug>(
        &self,
        ctoken: S,
        endpoint: ContinuationEndpoint,
        visitor_data: Option<&str>,
    ) -> Result<Paginator<T>, Error> {
        let ctoken = ctoken.as_ref();
        if endpoint.is_music() {
            let request_body = ytbody!({
                "continuation": ctoken,
            });

            let p = self
                .execute_request_ctx::<MusicContinuationMarker, Paginator<MusicItem>, _>(
                    ClientType::DesktopMusic,
                    "music_continuation",
                    ctoken,
                    endpoint.as_str(),
                    &request_body,
                    MapRespOptions {
                        visitor_data,
                        ..Default::default()
                    },
                )
                .await?;

            Ok(map_ytm_paginator(p, endpoint))
        } else {
            let request_body = ytbody!({
                "continuation": ctoken,
            });

            let p = self
                .execute_request_ctx::<ContinuationMarker, Paginator<YouTubeItem>, _>(
                    ClientType::Desktop,
                    "continuation",
                    ctoken,
                    endpoint.as_str(),
                    &request_body,
                    MapRespOptions {
                        visitor_data,
                        ..Default::default()
                    },
                )
                .await?;

            Ok(map_yt_paginator(p, endpoint))
        }
    }
}

fn map_yt_paginator<T: FromYtItem>(
    p: Paginator<YouTubeItem>,
    endpoint: ContinuationEndpoint,
) -> Paginator<T> {
    Paginator {
        count: p.count,
        items: p.items.into_iter().filter_map(T::from_yt_item).collect(),
        ctoken: p.ctoken,
        visitor_data: p.visitor_data,
        endpoint,
        authenticated: p.authenticated,
    }
}

fn map_ytm_paginator<T: FromYtItem>(
    p: Paginator<MusicItem>,
    endpoint: ContinuationEndpoint,
) -> Paginator<T> {
    Paginator {
        count: p.count,
        items: p.items.into_iter().filter_map(T::from_ytm_item).collect(),
        ctoken: p.ctoken,
        visitor_data: p.visitor_data,
        endpoint,
        authenticated: p.authenticated,
    }
}

fn yt_continuation_yt_items<'a>(root: &JsonNode<'a>) -> Vec<JsonNode<'a>> {
    for actions in [
        root.query(ytq!(.onResponseReceivedActions)),
        root.query(ytq!(.onResponseReceivedCommands)),
        root.query(ytq!(.onResponseReceivedEndpoints)),
    ]
    .into_iter()
    .flatten()
    {
        let mut merged = Vec::new();
        for action in actions.items() {
            let Some(items) = action.query(ytq!(
                .(.appendContinuationItemsAction || .reloadContinuationItemsCommand).continuationItems
            )) else {
                continue;
            };
            merged.extend(items.items());
        }
        if !merged.is_empty() {
            return merged;
        }
    }

    if let Some(items) = root.query(ytq!(.continuationContents.richGridContinuation.contents)) {
        return items.items();
    }

    Vec::new()
}

fn map_music_shelf_node<'a>(
    shelf: &JsonNode<'a>,
    continuations: &mut Vec<JsonNode<'a>>,
) -> Option<JsonNode<'a>> {
    if let Some(cont) = shelf.query(ytq!(.continuations)) {
        continuations.extend(cont.items());
    }
    shelf.query(ytq!(.contents))
}

fn map_music_continuation_contents<'a>(
    root: &JsonNode<'a>,
    ctx: &MapRespCtx<'_>,
    continuations: &mut Vec<JsonNode<'a>>,
) -> (Vec<JsonNode<'a>>, Vec<MapResult<MusicItem>>) {
    let mut item_nodes = Vec::new();
    let mut extra_items = Vec::new();
    let Some(contents) = root.query(ytq!(.continuationContents)) else {
        return (item_nodes, extra_items);
    };

    if let Some(shelf) = music_shelf_continuation_node(&contents) {
        if let Some(items) = map_music_shelf_node(&shelf, continuations) {
            item_nodes.push(items);
        }
    } else if let Some(section_list) = music_section_list_continuation_node(&contents) {
        if let Some(sections) = section_list.query(ytq!(.contents)) {
            for section in sections.items() {
                if let Some(shelf) = music_shelf_node(&section) {
                    if let Some(items) = map_music_shelf_node(&shelf, continuations) {
                        item_nodes.push(items);
                    }
                } else if let Some(shelf) = music_carousel_node(&section) {
                    if let Some(items) = shelf.query(ytq!(.contents)) {
                        item_nodes.push(items);
                    }
                } else if let Some(grid) = music_grid_node(&section) {
                    if let Some(items) = music_grid_items(&grid) {
                        item_nodes.push(items);
                    }
                    if let Some(cont) = grid.query(ytq!(.continuations)) {
                        continuations.extend(cont.items());
                    }
                }
            }
        }
    } else if let Some(panel) = contents.query(ytq!(.playlistPanelContinuation)) {
        if let Some(cont) = panel.query(ytq!(.continuations)) {
            continuations.extend(cont.items());
        }
        if let Ok(panel) = panel.deserialize::<response::music_item::PlaylistPanelRenderer>() {
            for item in panel.contents {
                if let Some(value) = item.get("playlistPanelVideoRenderer") {
                    if let Some(item) =
                        value_from_json_value::<response::music_item::QueueMusicItem>(value)
                    {
                        let track = map_queue_item(item, ctx.lang);
                        extra_items.push(MapResult {
                            c: MusicItem::Track(track.c),
                            warnings: track.warnings,
                        });
                    }
                }
            }
        }
    } else if let Some(grid) = contents.query(ytq!(.gridContinuation)) {
        if let Some(items) = grid.query(ytq!(.items)) {
            item_nodes.push(items);
        }
        if let Some(cont) = grid.query(ytq!(.continuations)) {
            continuations.extend(cont.items());
        }
    }
    (item_nodes, extra_items)
}

fn music_continuation_token(
    continuations: &[JsonNode<'_>],
    ctoken: Option<String>,
) -> Option<String> {
    ctoken.or_else(|| continuations.first().and_then(|cont| yt_continuation(cont)))
}

impl MapEndpoint<Paginator<YouTubeItem>> for ContinuationMarker {
    fn map(
        json: &JsonDoc,
        ctx: &MapRespCtx<'_>,
    ) -> Result<MapResult<Paginator<YouTubeItem>>, ExtractionError> {
        json.with_root(|root| {
            let estimated_results = yt_estimated_results(&root);
            let items = yt_continuation_yt_items(&root);

            let mut acc = ItemsAccumulator::<YouTubeItem>::new();
            for item in items {
                let (item_res, item_ctoken, _) =
                    response::video_item::map_youtube_item(&item, ctx.lang);
                acc.add_mapped_vec(item_res, item_ctoken);
            }
            let (mapped, ctoken) = acc.finish();

            Ok(MapResult {
                c: Paginator::new_ext(
                    estimated_results,
                    mapped.c,
                    ctoken,
                    ctx.visitor_data.map(str::to_owned),
                    ContinuationEndpoint::Browse,
                    ctx.authenticated,
                ),
                warnings: mapped.warnings,
            })
        })
    }
}

impl MapEndpoint<Paginator<MusicItem>> for MusicContinuationMarker {
    fn map(
        json: &JsonDoc,
        ctx: &MapRespCtx<'_>,
    ) -> Result<MapResult<Paginator<MusicItem>>, ExtractionError> {
        json.with_root(|root| {
            let mut continuations = Vec::new();

            let (item_nodes, extra_items) =
                map_music_continuation_contents(&root, ctx, &mut continuations);
            let (map_res, mapped_ctoken) = map_music_continuation_items(
                &root,
                ctx.lang,
                ctx.artist.clone(),
                Vec::<crate::json::JsonValue>::new(),
                item_nodes,
                extra_items,
            );
            let ctoken = music_continuation_token(&continuations, mapped_ctoken);

            Ok(MapResult {
                c: Paginator::new_ext(
                    None,
                    map_res.c,
                    ctoken,
                    ctx.visitor_data.map(str::to_owned),
                    ContinuationEndpoint::MusicBrowse,
                    ctx.authenticated,
                ),
                warnings: map_res.warnings,
            })
        })
    }
}

#[cfg(feature = "userdata")]
impl MapEndpoint<Paginator<HistoryItem<VideoItem>>> for ContinuationMarker {
    fn map(
        json: &JsonDoc,
        ctx: &MapRespCtx<'_>,
    ) -> Result<MapResult<Paginator<HistoryItem<VideoItem>>>, ExtractionError> {
        json.with_root(|root| {
            let mut map_res: MapResult<Vec<HistoryItem<VideoItem>>> = MapResult::default();
            let mut ctoken = None;

            let items = yt_continuation_yt_items(&root);
            for item in items {
                if let Some(contents) = item.query(ytq!(
                    .itemSectionRenderer.contents || .expandedShelfContentsRenderer.items
                )) {
                    let date_txt = item.text_at(ytq!(
                        .itemSectionRenderer.header.itemSectionHeaderRenderer.title
                    ));
                    response::video_item::extend_video_history_items(
                        &contents,
                        ctx.lang,
                        date_txt,
                        ctx.utc_offset,
                        &mut map_res,
                    );
                } else if ctoken.is_none() {
                    ctoken = item
                        .query(ytq!(.continuationItemRenderer.continuationEndpoint))
                        .and_then(|node| node.deserialize::<crate::json::JsonValue>().ok())
                        .and_then(|endpoint| crate::json::yt_continuation_value(&endpoint));
                }
            }

            Ok(MapResult {
                c: Paginator::new_ext(
                    None,
                    map_res.c,
                    ctoken,
                    ctx.visitor_data.map(str::to_owned),
                    ContinuationEndpoint::Browse,
                    ctx.authenticated,
                ),
                warnings: map_res.warnings,
            })
        })
    }
}

#[cfg(feature = "userdata")]
impl MapEndpoint<Paginator<HistoryItem<TrackItem>>> for MusicContinuationMarker {
    fn map(
        json: &JsonDoc,
        ctx: &MapRespCtx<'_>,
    ) -> Result<MapResult<Paginator<HistoryItem<TrackItem>>>, ExtractionError> {
        json.with_root(|root| {
            let mut map_res: MapResult<Vec<HistoryItem<TrackItem>>> = MapResult::default();
            let mut continuations: Vec<MusicContinuationData> = Vec::new();

            let mut map_shelf = |shelf: MusicShelf| {
                let (mut mapped_items, _) = map_music_items_value(shelf.contents, ctx.lang);
                let playback_date = shelf.title.as_deref().and_then(|s| {
                    crate::util::timeago::parse_textual_date_to_d(
                        ctx.lang,
                        ctx.utc_offset,
                        s,
                        &mut map_res.warnings,
                    )
                });
                map_res.warnings.append(&mut mapped_items.warnings);
                map_res.c.extend(
                    mapped_items
                        .c
                        .into_iter()
                        .filter_map(TrackItem::from_ytm_item)
                        .map(|item| HistoryItem {
                            item,
                            playback_date,
                            playback_date_txt: shelf.title.clone(),
                        }),
                );
                continuations.extend(shelf.continuations);
            };

            if let Some(contents) = root.query(ytq!(.continuationContents)) {
                if let Some(shelf_node) = music_shelf_continuation_node(&contents) {
                    if let Ok(shelf) = shelf_node.deserialize::<MusicShelf>() {
                        map_shelf(shelf);
                    }
                } else if let Some(section_list) = music_section_list_continuation_node(&contents) {
                    if let Some(sections) = section_list.query(ytq!(.contents)) {
                        for section in sections.items() {
                            if let Some(shelf_node) = music_shelf_node(&section) {
                                if let Ok(shelf) = shelf_node.deserialize::<MusicShelf>() {
                                    map_shelf(shelf);
                                }
                            }
                        }
                    }
                }
            }

            let ctoken = continuations
                .into_iter()
                .next()
                .map(|cont| cont.next_continuation_data.continuation);

            Ok(MapResult {
                c: Paginator::new_ext(
                    None,
                    map_res.c,
                    ctoken,
                    ctx.visitor_data.map(str::to_owned),
                    ContinuationEndpoint::MusicBrowse,
                    ctx.authenticated,
                ),
                warnings: map_res.warnings,
            })
        })
    }
}

impl<T: FromYtItem> Paginator<T> {
    /// Get the next page from the paginator (or `None` if the paginator is exhausted)
    pub async fn next<Q: AsRef<RustyPipeQuery>>(&self, query: Q) -> Result<Option<Self>, Error> {
        Ok(match &self.ctoken {
            Some(ctoken) => {
                let q = if self.authenticated {
                    &query.as_ref().clone().authenticated()
                } else {
                    query.as_ref()
                };

                Some(
                    q.continuation(ctoken, self.endpoint, self.visitor_data.as_deref())
                        .await?,
                )
            }
            _ => None,
        })
    }

    /// Extend the items of the paginator by the next page
    ///
    /// Returns false if the paginator is exhausted.
    pub async fn extend<Q: AsRef<RustyPipeQuery>>(&mut self, query: Q) -> Result<bool, Error> {
        match self.next(query).await {
            Ok(Some(paginator)) => {
                let mut items = paginator.items;
                self.items.append(&mut items);
                self.ctoken = paginator.ctoken;
                if paginator.visitor_data.is_some() {
                    self.visitor_data = paginator.visitor_data;
                }
                Ok(true)
            }
            Ok(None) => Ok(false),
            Err(e) => Err(e),
        }
    }

    /// Extend the items of the paginator by the given amount of pages
    /// or until the paginator is exhausted.
    pub async fn extend_pages<Q: AsRef<RustyPipeQuery>>(
        &mut self,
        query: Q,
        n_pages: usize,
    ) -> Result<(), Error> {
        let query = query.as_ref();
        for _ in 0..n_pages {
            match self.extend(query).await {
                Ok(false) => break,
                Err(e) => return Err(e),
                _ => {}
            }
        }
        Ok(())
    }

    /// Extend the items of the paginator until the given amount of items
    /// is reached or the paginator is exhausted.
    pub async fn extend_limit<Q: AsRef<RustyPipeQuery>>(
        &mut self,
        query: Q,
        n_items: usize,
    ) -> Result<(), Error> {
        let query = query.as_ref();
        while self.items.len() < n_items {
            match self.extend(query).await {
                Ok(false) => break,
                Err(e) => return Err(e),
                _ => {}
            }
        }
        Ok(())
    }

    /// Extend the items of the paginator until the paginator is exhausted.
    pub async fn extend_all<Q: AsRef<RustyPipeQuery>>(&mut self, query: Q) -> Result<(), Error> {
        let query = query.as_ref();
        loop {
            match self.extend(query).await {
                Ok(false) => break,
                Err(e) => return Err(e),
                _ => {}
            }
        }
        Ok(())
    }
}

impl Paginator<Comment> {
    /// Get the next page from the paginator (or `None` if the paginator is exhausted)
    pub async fn next<Q: AsRef<RustyPipeQuery>>(&self, query: Q) -> Result<Option<Self>, Error> {
        Ok(match &self.ctoken {
            Some(ctoken) => Some(
                query
                    .as_ref()
                    .video_comments(ctoken, self.visitor_data.as_deref())
                    .await?,
            ),
            _ => None,
        })
    }
}

#[cfg(feature = "userdata")]
#[cfg_attr(docsrs, doc(cfg(feature = "userdata")))]
impl Paginator<HistoryItem<VideoItem>> {
    /// Get the next page from the paginator (or `None` if the paginator is exhausted)
    pub async fn next<Q: AsRef<RustyPipeQuery>>(&self, query: Q) -> Result<Option<Self>, Error> {
        Ok(match &self.ctoken {
            Some(ctoken) => Some(
                query
                    .as_ref()
                    .history_continuation(ctoken, self.visitor_data.as_deref())
                    .await?,
            ),
            _ => None,
        })
    }
}

#[cfg(feature = "userdata")]
#[cfg_attr(docsrs, doc(cfg(feature = "userdata")))]
impl Paginator<HistoryItem<TrackItem>> {
    /// Get the next page from the paginator (or `None` if the paginator is exhausted)
    pub async fn next<Q: AsRef<RustyPipeQuery>>(&self, query: Q) -> Result<Option<Self>, Error> {
        Ok(match &self.ctoken {
            Some(ctoken) => Some(
                query
                    .as_ref()
                    .music_history_continuation(ctoken, self.visitor_data.as_deref())
                    .await?,
            ),
            _ => None,
        })
    }
}

macro_rules! paginator {
    ($entity_type:ty) => {
        impl Paginator<$entity_type> {
            /// Extend the items of the paginator by the next page
            ///
            /// Returns false if the paginator is exhausted.
            pub async fn extend<Q: AsRef<RustyPipeQuery>>(
                &mut self,
                query: Q,
            ) -> Result<bool, Error> {
                match self.next(query).await {
                    Ok(Some(paginator)) => {
                        let mut items = paginator.items;
                        self.items.append(&mut items);
                        self.ctoken = paginator.ctoken;
                        if paginator.visitor_data.is_some() {
                            self.visitor_data = paginator.visitor_data;
                        }
                        Ok(true)
                    }
                    Ok(None) => Ok(false),
                    Err(e) => Err(e),
                }
            }

            /// Extend the items of the paginator by the given amount of pages
            /// or until the paginator is exhausted.
            pub async fn extend_pages<Q: AsRef<RustyPipeQuery>>(
                &mut self,
                query: Q,
                n_pages: usize,
            ) -> Result<(), Error> {
                let query = query.as_ref();
                for _ in 0..n_pages {
                    match self.extend(query).await {
                        Ok(false) => break,
                        Err(e) => return Err(e),
                        _ => {}
                    }
                }
                Ok(())
            }

            /// Extend the items of the paginator until the given amount of items
            /// is reached or the paginator is exhausted.
            pub async fn extend_limit<Q: AsRef<RustyPipeQuery>>(
                &mut self,
                query: Q,
                n_items: usize,
            ) -> Result<(), Error> {
                let query = query.as_ref();
                while self.items.len() < n_items {
                    match self.extend(query).await {
                        Ok(false) => break,
                        Err(e) => return Err(e),
                        _ => {}
                    }
                }
                Ok(())
            }

            /// Extend the items of the paginator until the paginator is exhausted.
            pub async fn extend_all<Q: AsRef<RustyPipeQuery>>(
                &mut self,
                query: Q,
            ) -> Result<(), Error> {
                let query = query.as_ref();
                loop {
                    match self.extend(query).await {
                        Ok(false) => break,
                        Err(e) => return Err(e),
                        _ => {}
                    }
                }
                Ok(())
            }
        }
    };
}

paginator!(Comment);
#[cfg(feature = "userdata")]
#[cfg_attr(docsrs, doc(cfg(feature = "userdata")))]
paginator!(HistoryItem<VideoItem>);
#[cfg(feature = "userdata")]
#[cfg_attr(docsrs, doc(cfg(feature = "userdata")))]
paginator!(HistoryItem<TrackItem>);

#[cfg(test)]
mod tests {
    use std::{fs, path::PathBuf};

    use path_macro::path;
    use rstest::rstest;

    use super::*;
    use crate::{
        model::{
            AlbumItem, ArtistItem, ChannelItem, MusicPlaylistItem, PlaylistItem, TrackItem,
            VideoItem,
        },
        util::tests::TESTFILES,
    };

    #[rstest]
    #[case::search("search", path!("search" / "cont.json"))]
    #[case::recommendations("recommendations", path!("video_details" / "recommendations.json"))]
    fn map_continuation_items(#[case] name: &str, #[case] path: PathBuf) {
        let json_path = path!(*TESTFILES / path);
        let json = JsonDoc::new(fs::read_to_string(json_path).unwrap());
        let map_res: MapResult<Paginator<YouTubeItem>> =
            ContinuationMarker::map(&json, &MapRespCtx::test("")).unwrap();

        assert!(
            map_res.warnings.is_empty(),
            "deserialization/mapping warnings: {:?}",
            map_res.warnings
        );
        insta::assert_ron_snapshot!(format!("map_{name}"), map_res.c, {
            ".items.*.publish_date" => "[date]",
        });
    }

    #[rstest]
    #[case::channel_videos("channel_videos", path!("channel" / "channel_videos_cont.json"))]
    #[case::playlist("playlist", path!("playlist" / "playlist_cont.json"))]
    fn map_continuation_videos(#[case] name: &str, #[case] path: PathBuf) {
        let json_path = path!(*TESTFILES / path);
        let json = JsonDoc::new(fs::read_to_string(json_path).unwrap());
        let map_res: MapResult<Paginator<YouTubeItem>> =
            ContinuationMarker::map(&json, &MapRespCtx::test("")).unwrap();
        let paginator: Paginator<VideoItem> =
            map_yt_paginator(map_res.c, ContinuationEndpoint::Browse);

        assert!(
            map_res.warnings.is_empty(),
            "deserialization/mapping warnings: {:?}",
            map_res.warnings
        );
        insta::assert_ron_snapshot!(format!("map_{name}"), paginator, {
            ".items[].publish_date" => "[date]",
        });
    }

    #[rstest]
    #[case::channel_playlists("channel_playlists", path!("channel" / "channel_playlists_cont.json"))]
    fn map_continuation_playlists(#[case] name: &str, #[case] path: PathBuf) {
        let json_path = path!(*TESTFILES / path);
        let json = JsonDoc::new(fs::read_to_string(json_path).unwrap());
        let map_res: MapResult<Paginator<YouTubeItem>> =
            ContinuationMarker::map(&json, &MapRespCtx::test("")).unwrap();
        let paginator: Paginator<PlaylistItem> =
            map_yt_paginator(map_res.c, ContinuationEndpoint::Browse);

        assert!(
            map_res.warnings.is_empty(),
            "deserialization/mapping warnings: {:?}",
            map_res.warnings
        );
        insta::assert_ron_snapshot!(format!("map_{name}"), paginator);
    }

    #[rstest]
    #[case::subscriptions("subscriptions", path!("userdata" / "subscriptions.json"))]
    fn map_continuation_channels(#[case] name: &str, #[case] path: PathBuf) {
        let json_path = path!(*TESTFILES / path);
        let json = JsonDoc::new(fs::read_to_string(json_path).unwrap());
        let map_res: MapResult<Paginator<YouTubeItem>> =
            ContinuationMarker::map(&json, &MapRespCtx::test("")).unwrap();
        let paginator: Paginator<ChannelItem> =
            map_yt_paginator(map_res.c, ContinuationEndpoint::Browse);

        assert!(
            map_res.warnings.is_empty(),
            "deserialization/mapping warnings: {:?}",
            map_res.warnings
        );
        insta::assert_ron_snapshot!(format!("map_{name}"), paginator);
    }

    #[rstest]
    #[case::playlist_tracks("playlist_tracks", path!("music_playlist" / "playlist_cont.json"))]
    #[case::search_tracks("search_tracks", path!("music_search" / "tracks_cont.json"))]
    #[case::radio_tracks("radio_tracks", path!("music_details" / "radio_cont.json"))]
    #[case::saved_tracks("saved_tracks", path!("music_userdata" / "saved_tracks.json"))]
    fn map_continuation_tracks(#[case] name: &str, #[case] path: PathBuf) {
        let json_path = path!(*TESTFILES / path);
        let json = JsonDoc::new(fs::read_to_string(json_path).unwrap());
        let map_res: MapResult<Paginator<MusicItem>> =
            MusicContinuationMarker::map(&json, &MapRespCtx::test("")).unwrap();
        let paginator: Paginator<TrackItem> =
            map_ytm_paginator(map_res.c, ContinuationEndpoint::MusicBrowse);

        assert!(
            map_res.warnings.is_empty(),
            "deserialization/mapping warnings: {:?}",
            map_res.warnings
        );
        insta::assert_ron_snapshot!(format!("map_{name}"), paginator);
    }

    #[rstest]
    #[case::saved_artists("saved_artists", path!("music_userdata" / "saved_artists.json"))]
    fn map_continuation_artists(#[case] name: &str, #[case] path: PathBuf) {
        let json_path = path!(*TESTFILES / path);
        let json = JsonDoc::new(fs::read_to_string(json_path).unwrap());
        let map_res: MapResult<Paginator<MusicItem>> =
            MusicContinuationMarker::map(&json, &MapRespCtx::test("")).unwrap();
        let paginator: Paginator<ArtistItem> =
            map_ytm_paginator(map_res.c, ContinuationEndpoint::MusicBrowse);

        assert!(
            map_res.warnings.is_empty(),
            "deserialization/mapping warnings: {:?}",
            map_res.warnings
        );
        insta::assert_ron_snapshot!(format!("map_{name}"), paginator);
    }

    #[rstest]
    #[case::saved_albums("saved_albums", path!("music_userdata" / "saved_albums.json"))]
    fn map_continuation_albums(#[case] name: &str, #[case] path: PathBuf) {
        let json_path = path!(*TESTFILES / path);
        let json = JsonDoc::new(fs::read_to_string(json_path).unwrap());
        let map_res: MapResult<Paginator<MusicItem>> =
            MusicContinuationMarker::map(&json, &MapRespCtx::test("")).unwrap();
        let paginator: Paginator<AlbumItem> =
            map_ytm_paginator(map_res.c, ContinuationEndpoint::MusicBrowse);

        assert!(
            map_res.warnings.is_empty(),
            "deserialization/mapping warnings: {:?}",
            map_res.warnings
        );
        insta::assert_ron_snapshot!(format!("map_{name}"), paginator);
    }

    #[rstest]
    #[case::playlist_related("playlist_related", path!("music_playlist" / "playlist_related.json"))]
    #[case::saved_playlists("saved_playlists", path!("music_userdata" / "saved_playlists.json"))]
    fn map_continuation_music_playlists(#[case] name: &str, #[case] path: PathBuf) {
        let json_path = path!(*TESTFILES / path);
        let json = JsonDoc::new(fs::read_to_string(json_path).unwrap());
        let map_res: MapResult<Paginator<MusicItem>> =
            MusicContinuationMarker::map(&json, &MapRespCtx::test("")).unwrap();
        let paginator: Paginator<MusicPlaylistItem> =
            map_ytm_paginator(map_res.c, ContinuationEndpoint::MusicBrowse);

        assert!(
            map_res.warnings.is_empty(),
            "deserialization/mapping warnings: {:?}",
            map_res.warnings
        );
        insta::assert_ron_snapshot!(format!("map_{name}"), paginator);
    }
}
