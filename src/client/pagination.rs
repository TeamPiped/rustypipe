use std::fmt::Debug;

use crate::error::{Error, ExtractionError};
use crate::model::{
    paginator::{ContinuationEndpoint, Paginator},
    traits::FromYtItem,
    Comment, MusicItem, YouTubeItem,
};
use crate::serializer::MapResult;

#[cfg(feature = "userdata")]
use crate::model::{HistoryItem, TrackItem, VideoItem};

use super::response::{
    music_item::{map_queue_item, MusicListMapper, PlaylistPanelVideo},
    YouTubeListItem,
};
use super::{
    response, ClientType, MapRespCtx, MapRespOptions, MapResponse, QContinuation, RustyPipeQuery,
};

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
            let request_body = QContinuation {
                continuation: ctoken,
            };

            let p = self
                .execute_request_ctx::<response::MusicContinuation, Paginator<MusicItem>, _>(
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
            let request_body = QContinuation {
                continuation: ctoken,
            };

            let p = self
                .execute_request_ctx::<response::Continuation, Paginator<YouTubeItem>, _>(
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

fn continuation_items(response: response::Continuation) -> MapResult<Vec<YouTubeListItem>> {
    response
        .on_response_received_actions
        .and_then(|actions| {
            actions
                .into_iter()
                .map(|action| action.append_continuation_items_action.continuation_items)
                .reduce(|mut acc, mut items| {
                    acc.c.append(&mut items.c);
                    acc.warnings.append(&mut items.warnings);
                    acc
                })
        })
        .or_else(|| {
            response
                .continuation_contents
                .map(|contents| contents.rich_grid_continuation.contents)
        })
        .unwrap_or_default()
}

impl MapResponse<Paginator<YouTubeItem>> for response::Continuation {
    fn map_response(
        self,
        ctx: &MapRespCtx<'_>,
    ) -> Result<MapResult<Paginator<YouTubeItem>>, ExtractionError> {
        let estimated_results = self.estimated_results;
        let items = continuation_items(self);

        let mut mapper = response::YouTubeListMapper::<YouTubeItem>::new(ctx.lang);
        mapper.map_response(items);

        Ok(MapResult {
            c: Paginator::new_ext(
                estimated_results,
                mapper.items,
                mapper.ctoken,
                ctx.visitor_data.map(str::to_owned),
                ContinuationEndpoint::Browse,
                ctx.authenticated,
            ),
            warnings: mapper.warnings,
        })
    }
}

impl MapResponse<Paginator<MusicItem>> for response::MusicContinuation {
    fn map_response(
        self,
        ctx: &MapRespCtx<'_>,
    ) -> Result<MapResult<Paginator<MusicItem>>, ExtractionError> {
        let mut mapper = if let Some(artist) = &ctx.artist {
            MusicListMapper::with_artist(ctx.lang, artist.clone())
        } else {
            MusicListMapper::new(ctx.lang)
        };
        let mut continuations = Vec::new();

        match self.continuation_contents {
            Some(response::music_item::ContinuationContents::MusicShelfContinuation(mut shelf)) => {
                mapper.map_response(shelf.contents);
                continuations.append(&mut shelf.continuations);
            }
            Some(response::music_item::ContinuationContents::SectionListContinuation(contents)) => {
                for c in contents.contents {
                    match c {
                        response::music_item::ItemSection::MusicShelfRenderer(mut shelf) => {
                            mapper.map_response(shelf.contents);
                            continuations.append(&mut shelf.continuations);
                        }
                        response::music_item::ItemSection::MusicCarouselShelfRenderer(shelf) => {
                            mapper.map_response(shelf.contents);
                        }
                        response::music_item::ItemSection::GridRenderer(mut grid) => {
                            mapper.map_response(grid.items);
                            continuations.append(&mut grid.continuations);
                        }
                        response::music_item::ItemSection::None => {}
                    }
                }
            }
            Some(response::music_item::ContinuationContents::PlaylistPanelContinuation(
                mut panel,
            )) => {
                continuations.append(&mut panel.continuations);
                mapper.add_warnings(&mut panel.contents.warnings);
                panel.contents.c.into_iter().for_each(|item| {
                    if let PlaylistPanelVideo::PlaylistPanelVideoRenderer(item) = item {
                        let mut track = map_queue_item(item, ctx.lang);
                        mapper.add_item(MusicItem::Track(track.c));
                        mapper.add_warnings(&mut track.warnings);
                    }
                });
            }
            Some(response::music_item::ContinuationContents::GridContinuation(mut grid)) => {
                mapper.map_response(grid.items);
                continuations.append(&mut grid.continuations);
            }
            None => {}
        }

        for a in self.on_response_received_actions {
            mapper.map_response(a.append_continuation_items_action.continuation_items);
        }

        let ctoken = mapper.ctoken.clone().or_else(|| {
            continuations
                .into_iter()
                .next()
                .map(|cont| cont.next_continuation_data.continuation)
        });
        let map_res = mapper.items();

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
    }
}

#[cfg(feature = "userdata")]
impl MapResponse<Paginator<HistoryItem<VideoItem>>> for response::Continuation {
    fn map_response(
        self,
        ctx: &MapRespCtx<'_>,
    ) -> Result<MapResult<Paginator<HistoryItem<VideoItem>>>, ExtractionError> {
        let mut map_res = MapResult::default();
        let mut ctoken = None;

        let items = continuation_items(self);
        for item in items.c {
            match item {
                response::YouTubeListItem::ItemSectionRenderer { header, contents } => {
                    let mut mapper = response::YouTubeListMapper::<VideoItem>::new(ctx.lang);
                    mapper.map_response(contents);
                    mapper.conv_history_items(
                        header.map(|h| h.item_section_header_renderer.title),
                        ctx.utc_offset,
                        &mut map_res,
                    );
                }
                response::YouTubeListItem::ContinuationItemRenderer(ep) => {
                    if ctoken.is_none() {
                        ctoken = ep.continuation_endpoint.into_token();
                    }
                }
                _ => {}
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
    }
}

#[cfg(feature = "userdata")]
impl MapResponse<Paginator<HistoryItem<TrackItem>>> for response::MusicContinuation {
    fn map_response(
        self,
        ctx: &MapRespCtx<'_>,
    ) -> Result<MapResult<Paginator<HistoryItem<TrackItem>>>, ExtractionError> {
        let mut map_res = MapResult::default();
        let mut continuations = Vec::new();

        let mut map_shelf = |shelf: response::music_item::MusicShelf| {
            let mut mapper = MusicListMapper::new(ctx.lang);
            mapper.map_response(shelf.contents);
            mapper.conv_history_items(shelf.title, ctx.utc_offset, &mut map_res);
            continuations.extend(shelf.continuations);
        };

        match self.continuation_contents {
            Some(response::music_item::ContinuationContents::MusicShelfContinuation(shelf)) => {
                map_shelf(shelf);
            }
            Some(response::music_item::ContinuationContents::SectionListContinuation(contents)) => {
                for c in contents.contents {
                    if let response::music_item::ItemSection::MusicShelfRenderer(shelf) = c {
                        map_shelf(shelf);
                    }
                }
            }
            _ => {}
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
    use std::{fs::File, io::BufReader, path::PathBuf};

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
        let json_file = File::open(json_path).unwrap();

        let items: response::Continuation =
            serde_json::from_reader(BufReader::new(json_file)).unwrap();
        let map_res: MapResult<Paginator<YouTubeItem>> =
            items.map_response(&MapRespCtx::test("")).unwrap();

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
        let json_file = File::open(json_path).unwrap();

        let items: response::Continuation =
            serde_json::from_reader(BufReader::new(json_file)).unwrap();
        let map_res: MapResult<Paginator<YouTubeItem>> =
            items.map_response(&MapRespCtx::test("")).unwrap();
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
        let json_file = File::open(json_path).unwrap();

        let items: response::Continuation =
            serde_json::from_reader(BufReader::new(json_file)).unwrap();
        let map_res: MapResult<Paginator<YouTubeItem>> =
            items.map_response(&MapRespCtx::test("")).unwrap();
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
        let json_file = File::open(json_path).unwrap();

        let items: response::Continuation =
            serde_json::from_reader(BufReader::new(json_file)).unwrap();
        let map_res: MapResult<Paginator<YouTubeItem>> =
            items.map_response(&MapRespCtx::test("")).unwrap();
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
        let json_file = File::open(json_path).unwrap();

        let items: response::MusicContinuation =
            serde_json::from_reader(BufReader::new(json_file)).unwrap();
        let map_res: MapResult<Paginator<MusicItem>> =
            items.map_response(&MapRespCtx::test("")).unwrap();
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
        let json_file = File::open(json_path).unwrap();

        let items: response::MusicContinuation =
            serde_json::from_reader(BufReader::new(json_file)).unwrap();
        let map_res: MapResult<Paginator<MusicItem>> =
            items.map_response(&MapRespCtx::test("")).unwrap();
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
        let json_file = File::open(json_path).unwrap();

        let items: response::MusicContinuation =
            serde_json::from_reader(BufReader::new(json_file)).unwrap();
        let map_res: MapResult<Paginator<MusicItem>> =
            items.map_response(&MapRespCtx::test("")).unwrap();
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
        let json_file = File::open(json_path).unwrap();

        let items: response::MusicContinuation =
            serde_json::from_reader(BufReader::new(json_file)).unwrap();
        let map_res: MapResult<Paginator<MusicItem>> =
            items.map_response(&MapRespCtx::test("")).unwrap();
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
