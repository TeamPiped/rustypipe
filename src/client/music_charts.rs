use crate::{
    error::{Error, ExtractionError},
    json::{yt_single_column_sections, ytq, JsonDoc, JsonValue},
    model::{MusicCharts, TrackItem},
    param::Country,
    request_body::ytbody,
    serializer::MapResult,
};

use super::{
    response::{
        self,
        music_item::{map_music_items, music_carousel_node, music_item_contents},
        url_endpoint::MusicPageType,
    },
    ClientType, MapEndpoint, MapRespCtx, RustyPipeQuery,
};

#[derive(Debug)]
struct MusicChartsEndpoint;

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

        self.execute_request::<MusicChartsEndpoint, _, _>(
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

impl MapEndpoint<MusicCharts> for MusicChartsEndpoint {
    fn map(
        json: &JsonDoc,
        ctx: &MapRespCtx<'_>,
    ) -> Result<MapResult<MusicCharts>, ExtractionError> {
        json.with_root(|root| {
            let countries = map_charts_countries(&root);
            let sections = yt_single_column_sections(&root)?;

            let mut top_playlist_id = None;
            let mut trending_playlist_id = None;
            let mut mapped_top: MapResult<Vec<TrackItem>> = MapResult::default();
            let mut mapped_trending: MapResult<Vec<TrackItem>> = MapResult::default();
            let mut other_items = Vec::new();
            let mut other_warnings = Vec::new();

            for section in sections.items() {
                let Some(shelf) = music_carousel_node(&section) else {
                    continue;
                };
                let page = shelf
                    .query(ytq!(
                        .header.musicCarouselShelfBasicHeaderRenderer.moreContentButton
                            .buttonRenderer.navigationEndpoint
                    ))
                    .and_then(|endpoint| endpoint.deserialize::<JsonValue>().ok())
                    .and_then(|endpoint| {
                        response::url_endpoint::music_page(&endpoint).map(|mp| (mp.typ, mp.id))
                    });

                let Some(contents) = music_item_contents(&shelf) else {
                    continue;
                };

                match page {
                    Some((MusicPageType::Playlist { .. }, id)) if top_playlist_id.is_none() => {
                        mapped_top.extend_vec(map_music_items(&contents, ctx.lang).0);
                        top_playlist_id = Some(id);
                    }
                    Some((MusicPageType::Playlist { .. }, id))
                        if trending_playlist_id.is_none() =>
                    {
                        mapped_trending.extend_vec(map_music_items(&contents, ctx.lang).0);
                        trending_playlist_id = Some(id);
                    }
                    _ => {
                        let mut mapped: MapResult<Vec<crate::model::MusicItem>> =
                            map_music_items(&contents, ctx.lang).0;
                        other_items.append(&mut mapped.c);
                        other_warnings.append(&mut mapped.warnings);
                    }
                }
            }

            let mut artists = Vec::new();
            let mut playlists = Vec::new();
            for item in other_items {
                match item {
                    crate::model::MusicItem::Artist(artist) => artists.push(artist),
                    crate::model::MusicItem::Playlist(playlist) => playlists.push(playlist),
                    _ => {}
                }
            }

            let mut warnings = mapped_top.warnings;
            warnings.extend(mapped_trending.warnings);
            warnings.extend(other_warnings);

            Ok(MapResult {
                c: MusicCharts {
                    top_tracks: mapped_top.c,
                    trending_tracks: mapped_trending.c,
                    artists,
                    playlists,
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
            MusicChartsEndpoint::map(&json, &MapRespCtx::test("")).unwrap();

        assert!(
            map_res.warnings.is_empty(),
            "deserialization/mapping warnings: {:?}",
            map_res.warnings
        );
        insta::assert_ron_snapshot!(format!("map_music_charts_{name}"), map_res.c);
    }
}
