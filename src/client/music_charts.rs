use std::borrow::Cow;

use serde::Serialize;

use crate::{
    error::{Error, ExtractionError},
    model::{MusicCharts, TrackItem},
    param::Country,
    serializer::MapResult,
};

use super::{
    response::{self, music_item::MusicListMapper, url_endpoint::MusicPageType},
    ClientType, MapRespCtx, MapResponse, RustyPipeQuery,
};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct QCharts<'a> {
    browse_id: &'a str,
    params: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    form_data: Option<FormData>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct FormData {
    pub selected_values: [Country; 1],
}

impl RustyPipeQuery {
    /// Get the YouTube Music charts for a given country
    #[tracing::instrument(skip(self), level = "error")]
    pub async fn music_charts(&self, country: Option<Country>) -> Result<MusicCharts, Error> {
        let request_body = QCharts {
            browse_id: "FEmusic_charts",
            params: "sgYPRkVtdXNpY19leHBsb3Jl",
            form_data: country.map(|c| FormData {
                selected_values: [c],
            }),
        };

        self.execute_request::<response::MusicCharts, _, _>(
            ClientType::DesktopMusic,
            "music_charts",
            "",
            "browse",
            &request_body,
        )
        .await
    }
}

impl MapResponse<MusicCharts> for response::MusicCharts {
    fn map_response(self, ctx: &MapRespCtx<'_>) -> Result<MapResult<MusicCharts>, ExtractionError> {
        let countries = self
            .framework_updates
            .map(|fwu| {
                fwu.entity_batch_update
                    .mutations
                    .into_iter()
                    .map(|x| x.payload.music_form_boolean_choice.opaque_token)
                    .collect()
            })
            .unwrap_or_default();

        let mut top_playlist_id = None;
        let mut trending_playlist_id = None;

        let mut mapper_top = MusicListMapper::new(ctx.lang);
        let mut mapper_trending = MusicListMapper::new(ctx.lang);
        let mut mapper_other = MusicListMapper::new(ctx.lang);

        self.contents
            .single_column_browse_results_renderer
            .contents
            .into_iter()
            .next()
            .ok_or(ExtractionError::InvalidData(Cow::Borrowed("no content")))?
            .tab_renderer
            .content
            .section_list_renderer
            .contents
            .into_iter()
            .for_each(|s| match s {
                response::music_charts::ItemSection::MusicCarouselShelfRenderer(shelf) => {
                    match shelf.header.and_then(|h| {
                        h.music_carousel_shelf_basic_header_renderer
                            .more_content_button
                            .and_then(|btn| btn.button_renderer.navigation_endpoint.music_page())
                            .map(|mp| (mp.typ, mp.id))
                    }) {
                        Some((MusicPageType::Playlist { .. }, id)) => {
                            // Top music videos (first shelf with associated playlist)
                            if top_playlist_id.is_none() {
                                mapper_top.map_response(shelf.contents);
                                top_playlist_id = Some(id);
                            }
                            // Trending (second shelf with associated playlist)
                            else if trending_playlist_id.is_none() {
                                mapper_trending.map_response(shelf.contents);
                                trending_playlist_id = Some(id);
                            }
                        }
                        // Other sections (artists, playlists)
                        _ => {
                            mapper_other.map_response(shelf.contents);
                        }
                    }
                }
                response::music_charts::ItemSection::None => {}
            });

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
    }
}

#[cfg(test)]
mod tests {
    use std::{fs::File, io::BufReader, path::Path};

    use rstest::rstest;

    use super::*;

    #[rstest]
    #[case::default("global")]
    #[case::us("US")]
    fn map_music_charts(#[case] name: &str) {
        let filename = format!("testfiles/music_charts/charts_{name}.json");
        let json_path = Path::new(&filename);
        let json_file = File::open(json_path).unwrap();

        let charts: response::MusicCharts =
            serde_json::from_reader(BufReader::new(json_file)).unwrap();
        let map_res: MapResult<MusicCharts> = charts.map_response(&MapRespCtx::test("")).unwrap();

        assert!(
            map_res.warnings.is_empty(),
            "deserialization/mapping warnings: {:?}",
            map_res.warnings
        );
        insta::assert_ron_snapshot!(format!("map_music_charts_{name}"), map_res.c);
    }
}
