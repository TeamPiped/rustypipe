use crate::{
    error::{Error, ExtractionError},
    json::{JsonDoc, yt_single_column_sections, ytq},
    model::{MusicCharts, TrackItem},
    param::Country,
    request_body::ytbody,
    serializer::MapResult,
};

use super::{
    response::{self, music_item::MusicListMapper, url_endpoint::MusicPageType},
    ClientType, MapJsonResponse, MapRespCtx, RustyPipeQuery,
};

#[derive(Debug)]
struct MusicChartsJson;

impl RustyPipeQuery {
    /// Get the YouTube Music charts for a given country
    #[tracing::instrument(skip(self), level = "error")]
    pub async fn music_charts(&self, country: Option<Country>) -> Result<MusicCharts, Error> {
        let request_body = ytbody!({
            "browseId": "FEmusic_charts",
            "params": "sgYPRkVtdXNpY19leHBsb3Jl",
            ? "formData": country.map(|c| ytbody!({
                "selectedValues": [c],
            })),
        });

        self.execute_request::<MusicChartsJson, _, _>(
            ClientType::DesktopMusic,
            "music_charts",
            "",
            "browse",
            &request_body,
        )
        .await
    }
}

fn map_charts_countries(root: &crate::json::JsonNode<'_>) -> std::collections::BTreeSet<Country> {
    root.query(ytq!(.frameworkUpdates.entityBatchUpdate.mutations))
        .map(|mutations| {
            mutations
                .items()
                .into_iter()
                .filter_map(|mutation| {
                    mutation
                        .query(ytq!(.payload.musicFormBooleanChoice.opaqueToken))
                        .and_then(|node| node.deserialize::<Country>().ok())
                })
                .collect()
        })
        .unwrap_or_default()
}

impl MapJsonResponse<MusicCharts> for MusicChartsJson {
    fn map_json_response(
        json: &JsonDoc,
        ctx: &MapRespCtx<'_>,
    ) -> Result<MapResult<MusicCharts>, ExtractionError> {
        json.with_root(|root| {
            let countries = map_charts_countries(&root);
            let sections = yt_single_column_sections(&root)?;

            let mut top_playlist_id = None;
            let mut trending_playlist_id = None;
            let mut mapper_top = MusicListMapper::new(ctx.lang);
            let mut mapper_trending = MusicListMapper::new(ctx.lang);
            let mut mapper_other = MusicListMapper::new(ctx.lang);

            for section in sections.items() {
                let Some(shelf) = section.query(ytq!(.musicCarouselShelfRenderer)) else {
                    continue;
                };
                let page = shelf
                    .query(ytq!(
                        .header.musicCarouselShelfBasicHeaderRenderer.moreContentButton
                            .buttonRenderer.navigationEndpoint
                    ))
                    .and_then(|endpoint| {
                        endpoint
                            .deserialize::<response::url_endpoint::NavigationEndpoint>()
                            .ok()
                            .and_then(|ep| ep.music_page())
                            .map(|mp| (mp.typ, mp.id))
                    });

                let Some(contents) = shelf.query(ytq!(.contents)) else {
                    continue;
                };

                match page {
                    Some((MusicPageType::Playlist { .. }, id)) if top_playlist_id.is_none() => {
                        mapper_top.map_response_node(&contents);
                        top_playlist_id = Some(id);
                    }
                    Some((MusicPageType::Playlist { .. }, id)) if trending_playlist_id.is_none() => {
                        mapper_trending.map_response_node(&contents);
                        trending_playlist_id = Some(id);
                    }
                    _ => {
                        mapper_other.map_response_node(&contents);
                    }
                }
            }

            let mapped_top = mapper_top.conv_items::<TrackItem>();
            let mapped_trending = mapper_trending.conv_items::<TrackItem>();
            let mapped_other = mapper_other.group_items();

            let mut warnings = mapped_top.warnings;
            warnings.extend(mapped_trending.warnings);
            warnings.extend(mapped_other.warnings);

            Ok(MapResult {
                c: MusicCharts {
                    top_tracks: mapped_top.c,
                    trending_tracks: mapped_trending.c,
                    artists: mapped_other.c.artists,
                    playlists: mapped_other.c.playlists,
                    top_playlist_id,
                    trending_playlist_id,
                    available_countries: countries,
                },
                warnings,
            })
        })
    }
}

#[cfg(test)]
mod tests {
    use std::{fs, path::Path};

    use rstest::rstest;

    use super::*;

    #[rstest]
    #[case::default("global")]
    #[case::us("US")]
    fn map_music_charts(#[case] name: &str) {
        let filename = format!("testfiles/music_charts/charts_{name}.json");
        let json_path = Path::new(&filename);
        let json = JsonDoc::new(fs::read_to_string(json_path).unwrap());
        let map_res: MapResult<MusicCharts> =
            MusicChartsJson::map_json_response(&json, &MapRespCtx::test("")).unwrap();

        assert!(
            map_res.warnings.is_empty(),
            "deserialization/mapping warnings: {:?}",
            map_res.warnings
        );
        insta::assert_ron_snapshot!(format!("map_music_charts_{name}"), map_res.c);
    }
}
