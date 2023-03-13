use std::borrow::Cow;

use crate::error::{Error, ExtractionError};
use crate::model::{
    paginator::{ContinuationEndpoint, Paginator},
    traits::FromYtItem,
    Comment, MusicItem, PlaylistVideo, YouTubeItem,
};
use crate::serializer::MapResult;
use crate::util::TryRemove;

use super::response::music_item::{map_queue_item, MusicListMapper, PlaylistPanelVideo};
use super::{response, ClientType, MapResponse, QContinuation, RustyPipeQuery};

impl RustyPipeQuery {
    /// Get more YouTube items from the given continuation token and endpoint
    pub async fn continuation<T: FromYtItem, S: AsRef<str>>(
        &self,
        ctoken: S,
        endpoint: ContinuationEndpoint,
        visitor_data: Option<&str>,
    ) -> Result<Paginator<T>, Error> {
        let ctoken = ctoken.as_ref();
        if endpoint.is_music() {
            let context = self
                .get_context(ClientType::DesktopMusic, true, visitor_data)
                .await;
            let request_body = QContinuation {
                context,
                continuation: ctoken,
            };

            let p = self
                .execute_request::<response::MusicContinuation, Paginator<MusicItem>, _>(
                    ClientType::DesktopMusic,
                    "music_continuation",
                    ctoken,
                    endpoint.as_str(),
                    &request_body,
                )
                .await?;

            Ok(map_ytm_paginator(p, visitor_data, endpoint))
        } else {
            let context = self
                .get_context(ClientType::Desktop, true, visitor_data)
                .await;
            let request_body = QContinuation {
                context,
                continuation: ctoken,
            };

            let p = self
                .execute_request::<response::Continuation, Paginator<YouTubeItem>, _>(
                    ClientType::Desktop,
                    "continuation",
                    ctoken,
                    endpoint.as_str(),
                    &request_body,
                )
                .await?;

            Ok(map_yt_paginator(p, visitor_data, endpoint))
        }
    }
}

fn map_yt_paginator<T: FromYtItem>(
    p: Paginator<YouTubeItem>,
    visitor_data: Option<&str>,
    endpoint: ContinuationEndpoint,
) -> Paginator<T> {
    Paginator {
        count: p.count,
        items: p.items.into_iter().filter_map(T::from_yt_item).collect(),
        ctoken: p.ctoken,
        visitor_data: visitor_data.map(str::to_owned),
        endpoint,
    }
}

fn map_ytm_paginator<T: FromYtItem>(
    p: Paginator<MusicItem>,
    visitor_data: Option<&str>,
    endpoint: ContinuationEndpoint,
) -> Paginator<T> {
    Paginator {
        count: p.count,
        items: p.items.into_iter().filter_map(T::from_ytm_item).collect(),
        ctoken: p.ctoken,
        visitor_data: visitor_data.map(str::to_owned),
        endpoint,
    }
}

impl MapResponse<Paginator<YouTubeItem>> for response::Continuation {
    fn map_response(
        self,
        _id: &str,
        lang: crate::param::Language,
        _deobf: Option<&crate::deobfuscate::DeobfData>,
    ) -> Result<MapResult<Paginator<YouTubeItem>>, ExtractionError> {
        let items = self
            .on_response_received_actions
            .and_then(|mut actions| {
                actions
                    .try_swap_remove(0)
                    .map(|action| action.append_continuation_items_action.continuation_items)
            })
            .or_else(|| {
                self.continuation_contents
                    .map(|contents| contents.rich_grid_continuation.contents)
            })
            .ok_or(ExtractionError::InvalidData(Cow::Borrowed(
                "no continuation items",
            )))?;

        let mut mapper = response::YouTubeListMapper::<YouTubeItem>::new(lang);
        mapper.map_response(items);

        Ok(MapResult {
            c: Paginator::new(self.estimated_results, mapper.items, mapper.ctoken),
            warnings: mapper.warnings,
        })
    }
}

impl MapResponse<Paginator<MusicItem>> for response::MusicContinuation {
    fn map_response(
        self,
        _id: &str,
        lang: crate::param::Language,
        _deobf: Option<&crate::deobfuscate::DeobfData>,
    ) -> Result<MapResult<Paginator<MusicItem>>, ExtractionError> {
        let mut mapper = MusicListMapper::new(lang);
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
                        _ => {}
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
                        mapper.add_item(MusicItem::Track(map_queue_item(item, lang)))
                    }
                });
            }
            None => {}
        }

        let map_res = mapper.items();
        let ctoken = continuations
            .try_swap_remove(0)
            .map(|cont| cont.next_continuation_data.continuation);

        Ok(MapResult {
            c: Paginator::new(None, map_res.c, ctoken),
            warnings: map_res.warnings,
        })
    }
}

impl<T: FromYtItem> Paginator<T> {
    /// Get the next page from the paginator (or `None` if the paginator is exhausted)
    pub async fn next<Q: AsRef<RustyPipeQuery>>(&self, query: Q) -> Result<Option<Self>, Error> {
        Ok(match &self.ctoken {
            Some(ctoken) => Some(
                query
                    .as_ref()
                    .continuation(ctoken, self.endpoint, self.visitor_data.as_deref())
                    .await?,
            ),
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

impl Paginator<PlaylistVideo> {
    /// Get the next page from the paginator (or `None` if the paginator is exhausted)
    pub async fn next<Q: AsRef<RustyPipeQuery>>(&self, query: Q) -> Result<Option<Self>, Error> {
        Ok(match &self.ctoken {
            Some(ctoken) => Some(query.as_ref().playlist_continuation(ctoken).await?),
            None => None,
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
        }
    };
}

paginator!(Comment);
paginator!(PlaylistVideo);

#[cfg(test)]
mod tests {
    use std::{fs::File, io::BufReader, path::PathBuf};

    use path_macro::path;
    use rstest::rstest;

    use super::*;
    use crate::model::{MusicPlaylistItem, PlaylistItem, TrackItem};
    use crate::param::Language;

    #[rstest]
    #[case("search", path!("search" / "cont.json"))]
    #[case("startpage", path!("trends" / "startpage_cont.json"))]
    #[case("recommendations", path!("video_details" / "recommendations.json"))]
    fn map_continuation_items(#[case] name: &str, #[case] path: PathBuf) {
        let json_path = path!("testfiles" / path);
        let json_file = File::open(json_path).unwrap();

        let items: response::Continuation =
            serde_json::from_reader(BufReader::new(json_file)).unwrap();
        let map_res: MapResult<Paginator<YouTubeItem>> =
            items.map_response("", Language::En, None).unwrap();

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
    #[case("channel_playlists", path!("channel" / "channel_playlists_cont.json"))]
    fn map_continuation_playlists(#[case] name: &str, #[case] path: PathBuf) {
        let json_path = path!("testfiles" / path);
        let json_file = File::open(json_path).unwrap();

        let items: response::Continuation =
            serde_json::from_reader(BufReader::new(json_file)).unwrap();
        let map_res: MapResult<Paginator<YouTubeItem>> =
            items.map_response("", Language::En, None).unwrap();
        let paginator: Paginator<PlaylistItem> =
            map_yt_paginator(map_res.c, None, ContinuationEndpoint::Browse);

        assert!(
            map_res.warnings.is_empty(),
            "deserialization/mapping warnings: {:?}",
            map_res.warnings
        );
        insta::assert_ron_snapshot!(format!("map_{name}"), paginator);
    }

    #[rstest]
    #[case("playlist_tracks", path!("music_playlist" / "playlist_cont.json"))]
    #[case("search_tracks", path!("music_search" / "tracks_cont.json"))]
    #[case("radio_tracks", path!("music_details" / "radio_cont.json"))]
    fn map_continuation_tracks(#[case] name: &str, #[case] path: PathBuf) {
        let json_path = path!("testfiles" / path);
        let json_file = File::open(json_path).unwrap();

        let items: response::MusicContinuation =
            serde_json::from_reader(BufReader::new(json_file)).unwrap();
        let map_res: MapResult<Paginator<MusicItem>> =
            items.map_response("", Language::En, None).unwrap();
        let paginator: Paginator<TrackItem> =
            map_ytm_paginator(map_res.c, None, ContinuationEndpoint::MusicBrowse);

        assert!(
            map_res.warnings.is_empty(),
            "deserialization/mapping warnings: {:?}",
            map_res.warnings
        );
        insta::assert_ron_snapshot!(format!("map_{name}"), paginator);
    }

    #[rstest]
    #[case("playlist_related", path!("music_playlist" / "playlist_related.json"))]
    fn map_continuation_music_playlists(#[case] name: &str, #[case] path: PathBuf) {
        let json_path = path!("testfiles" / path);
        let json_file = File::open(json_path).unwrap();

        let items: response::MusicContinuation =
            serde_json::from_reader(BufReader::new(json_file)).unwrap();
        let map_res: MapResult<Paginator<MusicItem>> =
            items.map_response("", Language::En, None).unwrap();
        let paginator: Paginator<MusicPlaylistItem> =
            map_ytm_paginator(map_res.c, None, ContinuationEndpoint::MusicBrowse);

        assert!(
            map_res.warnings.is_empty(),
            "deserialization/mapping warnings: {:?}",
            map_res.warnings
        );
        insta::assert_ron_snapshot!(format!("map_{name}"), paginator);
    }
}
