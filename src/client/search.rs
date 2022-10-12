use serde::{de::IgnoredAny, Serialize};

use crate::{
    deobfuscate::Deobfuscator,
    error::{Error, ExtractionError},
    model::{Paginator, SearchItem, SearchResult, SearchVideo},
    param::{search_filter::SearchFilter, Language},
    util::TryRemove,
};

use super::{
    response::{self, TryFromWLang},
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

    pub async fn search_suggestion(self, query: &str) -> Result<Vec<String>, Error> {
        let url = url::Url::parse_with_params("https://suggestqueries-clients6.youtube.com/complete/search?client=youtube&gs_rn=64&gs_ri=youtube&ds=yt&cp=1&gs_id=4&xhr=t&xssi=t",
            &[("hl", self.opts.lang.to_string()), ("gl", self.opts.country.to_string()), ("q", query.to_string())]
        ).map_err(|_| Error::Other("could not build url".into()))?;

        let response = self
            .client
            .http_request_txt(self.client.inner.http.get(url).build()?)
            .await?;

        let trimmed = response.get(5..).ok_or_else(|| {
            Error::Extraction(ExtractionError::InvalidData(
                "could not get string slice".into(),
            ))
        })?;

        let parsed = serde_json::from_str::<(
            IgnoredAny,
            Vec<(String, IgnoredAny, IgnoredAny)>,
            IgnoredAny,
        )>(trimmed)
        .map_err(|e| Error::Extraction(ExtractionError::InvalidData(e.to_string().into())))?;

        Ok(parsed.1.into_iter().map(|item| item.0).collect())
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
            response::search::SearchItem::VideoRenderer(video) => {
                match SearchVideo::from_w_lang(video, lang) {
                    Ok(video) => Some(SearchItem::Video(video)),
                    Err(e) => {
                        warnings.push(e.to_string());
                        None
                    }
                }
            }
            response::search::SearchItem::PlaylistRenderer(playlist) => {
                Some(SearchItem::Playlist(playlist.into()))
            }
            response::search::SearchItem::ChannelRenderer(channel) => {
                Some(SearchItem::Channel(channel.into()))
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
        client::{response, MapResponse},
        model::{Paginator, SearchItem, SearchResult},
        param::Language,
        serializer::MapResult,
    };

    use rstest::rstest;

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
