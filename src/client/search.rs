use serde::Serialize;

use crate::{
    deobfuscate::Deobfuscator,
    error::{Error, ExtractionError},
    model::{
        ChannelId, ChannelTag, Paginator, SearchChannel, SearchItem, SearchPlaylist,
        SearchPlaylistVideo, SearchResult, SearchVideo,
    },
    param::{search_filter::SearchFilter, Language},
    timeago,
    util::{self, TryRemove},
};

use super::{
    response::{self, IsLive, IsShort},
    ClientType, MapResponse, MapResult, QContinuation, RustyPipeQuery, YTContext,
};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct QSearch<'a> {
    context: YTContext,
    query: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    params: Option<String>,
}

impl RustyPipeQuery {
    pub async fn search(self, query: &str) -> Result<SearchResult, Error> {
        let context = self.get_context(ClientType::Desktop, true).await;
        let request_body = QSearch {
            context,
            query,
            params: None,
        };

        self.execute_request::<response::Search, _, _>(
            ClientType::Desktop,
            "search",
            query,
            "search",
            &request_body,
        )
        .await
    }

    pub async fn search_filter(
        self,
        query: &str,
        filter: &SearchFilter,
    ) -> Result<SearchResult, Error> {
        let context = self.get_context(ClientType::Desktop, true).await;
        let request_body = QSearch {
            context,
            query,
            params: Some(filter.encode()),
        };

        self.execute_request::<response::Search, _, _>(
            ClientType::Desktop,
            "search_filter",
            query,
            "search",
            &request_body,
        )
        .await
    }

    pub async fn search_continuation(self, ctoken: &str) -> Result<Paginator<SearchItem>, Error> {
        let context = self.get_context(ClientType::Desktop, true).await;
        let request_body = QContinuation {
            context,
            continuation: ctoken,
        };

        self.execute_request::<response::SearchCont, _, _>(
            ClientType::Desktop,
            "search_continuation",
            ctoken,
            "search",
            &request_body,
        )
        .await
    }
}

impl MapResponse<SearchResult> for response::Search {
    fn map_response(
        self,
        _id: &str,
        lang: Language,
        _deobf: Option<&Deobfuscator>,
    ) -> Result<MapResult<SearchResult>, ExtractionError> {
        let section_list_items = self
            .contents
            .two_column_search_results_renderer
            .primary_contents
            .section_list_renderer
            .contents;

        let (items, ctoken) = map_section_list_items(section_list_items)?;

        let mut warnings = items.warnings;
        let (mut mapped, corrected_query) = map_search_items(items.c, lang);
        warnings.append(&mut mapped.warnings);

        Ok(MapResult {
            c: SearchResult {
                items: Paginator::new(self.estimated_results, mapped.c, ctoken),
                corrected_query,
            },
            warnings,
        })
    }
}

impl MapResponse<Paginator<SearchItem>> for response::SearchCont {
    fn map_response(
        self,
        _id: &str,
        lang: Language,
        _deobf: Option<&Deobfuscator>,
    ) -> Result<MapResult<Paginator<SearchItem>>, ExtractionError> {
        let mut commands = self.on_response_received_commands;
        let cont_command = some_or_bail!(
            commands.try_swap_remove(0),
            Err(ExtractionError::InvalidData(
                "no item section renderer".into()
            ))
        );

        let (items, ctoken) = map_section_list_items(
            cont_command
                .append_continuation_items_action
                .continuation_items,
        )?;

        let mut warnings = items.warnings;
        let (mut mapped, _) = map_search_items(items.c, lang);
        warnings.append(&mut mapped.warnings);

        Ok(MapResult {
            c: Paginator::new(self.estimated_results, mapped.c, ctoken),
            warnings,
        })
    }
}

fn map_section_list_items(
    section_list_items: Vec<response::search::SectionListItem>,
) -> Result<(MapResult<Vec<response::search::SearchItem>>, Option<String>), ExtractionError> {
    let mut items = None;
    let mut ctoken = None;
    section_list_items.into_iter().for_each(|item| match item {
        response::search::SectionListItem::ItemSectionRenderer { contents } => {
            items = Some(contents);
        }
        response::search::SectionListItem::ContinuationItemRenderer {
            continuation_endpoint,
        } => {
            ctoken = Some(continuation_endpoint.continuation_command.token);
        }
    });

    let items = some_or_bail!(
        items,
        Err(ExtractionError::InvalidData(
            "no item section renderer".into()
        ))
    );

    Ok((items, ctoken))
}

fn map_search_items(
    items: Vec<response::search::SearchItem>,
    lang: Language,
) -> (MapResult<Vec<SearchItem>>, Option<String>) {
    let mut warnings = Vec::new();

    let mut c_query = None;
    let mapped_items = items
        .into_iter()
        .filter_map(|item| match item {
            response::search::SearchItem::VideoRenderer(mut video) => {
                match ChannelId::try_from(video.channel) {
                    Ok(channel) => Some(SearchItem::Video(SearchVideo {
                        id: video.video_id,
                        title: video.title,
                        length: video
                            .length_text
                            .and_then(|txt| util::parse_video_length_or_warn(&txt, &mut warnings)),
                        thumbnail: video.thumbnail.into(),
                        channel: ChannelTag {
                            id: channel.id,
                            name: channel.name,
                            avatar: video
                                .channel_thumbnail_supported_renderers
                                .channel_thumbnail_with_link_renderer
                                .thumbnail
                                .into(),
                            verification: video.owner_badges.into(),
                            subscriber_count: None,
                        },
                        publish_date: video.published_time_text.as_ref().and_then(|txt| {
                            timeago::parse_timeago_or_warn(lang, txt, &mut warnings)
                        }),
                        publish_date_txt: video.published_time_text,
                        view_count: video
                            .view_count_text
                            .map(|txt| util::parse_numeric(&txt).unwrap_or_default()),
                        is_live: video.thumbnail_overlays.is_live(),
                        is_short: video.thumbnail_overlays.is_short(),
                        short_description: video
                            .detailed_metadata_snippets
                            .try_swap_remove(0)
                            .map(|s| s.snippet_text)
                            .unwrap_or_default(),
                    })),
                    Err(e) => {
                        warnings.push(e.to_string());
                        None
                    }
                }
            }
            response::search::SearchItem::PlaylistRenderer(mut playlist) => {
                Some(SearchItem::Playlist(SearchPlaylist {
                    id: playlist.playlist_id,
                    name: playlist.title,
                    thumbnail: playlist
                        .thumbnails
                        .try_swap_remove(0)
                        .unwrap_or_default()
                        .into(),
                    video_count: playlist.video_count,
                    first_videos: playlist
                        .videos
                        .into_iter()
                        .map(|v| SearchPlaylistVideo {
                            id: v.child_video_renderer.video_id,
                            title: v.child_video_renderer.title,
                            length: v.child_video_renderer.length_text.and_then(|txt| {
                                util::parse_video_length_or_warn(&txt, &mut warnings)
                            }),
                        })
                        .collect(),
                }))
            }
            response::search::SearchItem::ChannelRenderer(channel) => {
                Some(SearchItem::Channel(SearchChannel {
                    id: channel.channel_id,
                    name: channel.title,
                    avatar: channel.thumbnail.into(),
                    verification: channel.owner_badges.into(),
                    subscriber_count: channel
                        .subscriber_count_text
                        .and_then(|txt| util::parse_numeric_or_warn(&txt, &mut warnings)),
                    short_description: channel.description_snippet,
                }))
            }
            response::search::SearchItem::ShowingResultsForRenderer { corrected_query } => {
                c_query = Some(corrected_query);
                None
            }
            response::search::SearchItem::None => None,
        })
        .collect();
    (
        MapResult {
            c: mapped_items,
            warnings,
        },
        c_query,
    )
}

#[cfg(test)]
mod tests {
    use std::{fs::File, io::BufReader, path::Path};

    use crate::{
        client::{response, MapResponse, RustyPipe},
        model::{Paginator, SearchItem, SearchResult},
        param::Language,
        serializer::MapResult,
    };

    use rstest::rstest;

    #[tokio::test]
    async fn t1() {
        let rp = RustyPipe::builder().strict().build();
        let result = rp
            .query()
            .search("grewhbtrjlrbnerwhlbvuwrkeghurzueg")
            .await
            .unwrap();
        dbg!(&result);
    }

    #[rstest]
    #[case::default("default")]
    #[case::playlists("playlists")]
    #[case::playlists("empty")]
    fn t_map_search(#[case] name: &str) {
        let filename = format!("testfiles/search/{}.json", name);
        let json_path = Path::new(&filename);
        let json_file = File::open(json_path).unwrap();

        let search: response::Search = serde_json::from_reader(BufReader::new(json_file)).unwrap();
        let map_res: MapResult<SearchResult> = search.map_response("", Language::En, None).unwrap();

        assert!(
            map_res.warnings.is_empty(),
            "deserialization/mapping warnings: {:?}",
            map_res.warnings
        );

        insta::assert_ron_snapshot!(format!("map_search_{}", name), map_res.c, {
            ".items.items.*.publish_date" => "[date]",
        });
    }

    #[test]
    fn t_map_search_cont() {
        let filename = format!("testfiles/search/cont.json");
        let json_path = Path::new(&filename);
        let json_file = File::open(json_path).unwrap();

        let search_cont: response::SearchCont =
            serde_json::from_reader(BufReader::new(json_file)).unwrap();
        let map_res: MapResult<Paginator<SearchItem>> =
            search_cont.map_response("", Language::En, None).unwrap();

        assert!(
            map_res.warnings.is_empty(),
            "deserialization/mapping warnings: {:?}",
            map_res.warnings
        );

        insta::assert_ron_snapshot!("map_search_cont", map_res.c, {
            ".items.*.publish_date" => "[date]",
        });
    }
}
