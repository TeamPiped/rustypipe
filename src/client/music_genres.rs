use std::{borrow::Cow, fmt::Debug};

use crate::{
    error::{Error, ExtractionError},
    json::{yt_music_header_title, yt_single_column_sections, ytq, JsonDoc, JsonValue},
    model::{MusicGenre, MusicGenreItem, MusicGenreSection},
    request_body::ytbody,
    serializer::MapResult,
};

use super::{
    response::{
        music_genres::NavigationButtonRenderer,
        music_item::{
            map_music_items, music_carousel_node, music_grid_items, music_grid_node,
            music_item_contents,
        },
        url_endpoint,
    },
    ClientType, MapEndpoint, MapRespCtx, RustyPipeQuery,
};

#[derive(Debug)]
struct MusicGenresEndpoint;
#[derive(Debug)]
struct MusicGenreEndpoint;

impl RustyPipeQuery {
    /// Get a list of moods and genres from YouTube Music
    #[tracing::instrument(skip(self), level = "error")]
    pub async fn music_genres(&self) -> Result<Vec<MusicGenreItem>, Error> {
        let request_body = ytbody!({
            "browseId": "FEmusic_moods_and_genres",
        });

        self.execute_request::<MusicGenresEndpoint, _, _>(
            ClientType::DesktopMusic,
            "music_genres",
            "",
            "browse",
            &request_body,
        )
        .await
    }

    /// Get the playlists from a YouTube Music genre
    #[tracing::instrument(skip(self), level = "error")]
    pub async fn music_genre<S: AsRef<str> + Debug>(
        &self,
        genre_id: S,
    ) -> Result<MusicGenre, Error> {
        let genre_id = genre_id.as_ref();
        let request_body = ytbody!({
            "browseId": "FEmusic_moods_and_genres_category",
            "params": genre_id,
        });

        self.execute_request::<MusicGenreEndpoint, _, _>(
            ClientType::DesktopMusic,
            "music_genre",
            genre_id,
            "browse",
            &request_body,
        )
        .await
    }
}

impl MapEndpoint<Vec<MusicGenreItem>> for MusicGenresEndpoint {
    fn map(
        json: &JsonDoc,
        _ctx: &MapRespCtx<'_>,
    ) -> Result<MapResult<Vec<MusicGenreItem>>, ExtractionError> {
        json.with_root(|root| {
            let sections = yt_single_column_sections(&root)?;
            let section_items = sections.items();
            let i_start = section_items.len().saturating_sub(2);
            let mut warnings = Vec::new();
            let mut genres = Vec::new();
            for (i, section) in section_items.into_iter().skip(i_start).enumerate() {
                let Some(grid) = music_grid_node(&section) else {
                    continue;
                };
                let Some(contents) = music_grid_items(&grid) else {
                    continue;
                };

                for section in contents.items() {
                    let Some(btn) = section.try_deserialize::<NavigationButtonRenderer>(
                        ytq!(.musicNavigationButtonRenderer),
                        &mut warnings,
                    ) else {
                        continue;
                    };
                    genres.push(MusicGenreItem {
                        id: url_endpoint::browse_endpoint(&btn.click_command)
                            .map(|ep| ep.browse_endpoint.params)
                            .unwrap_or_default(),
                        name: btn.button_text,
                        is_mood: i == 0,
                        color: btn.solid.left_stripe_color,
                    });
                }
            }

            Ok(MapResult {
                c: genres,
                warnings,
            })
        })
    }
}

impl MapEndpoint<MusicGenre> for MusicGenreEndpoint {
    fn map(
        json: &JsonDoc,
        ctx: &MapRespCtx<'_>,
    ) -> Result<MapResult<MusicGenre>, ExtractionError> {
        json.with_root(|root| {
            let sections = yt_single_column_sections(&root)?;
            let name = yt_music_header_title(&root).ok_or({
                ExtractionError::InvalidData(Cow::Borrowed("missing genre header title"))
            })?;

            let mut warnings = Vec::new();
            let sections = sections
                .items()
                .into_iter()
                .filter_map(|section| {
                    if let Some(shelf) = music_carousel_node(&section) {
                        let name = shelf
                            .text_at(ytq!(
                                .header.musicCarouselShelfBasicHeaderRenderer.title
                            ))
                            .unwrap_or_default();
                        let subgenre_id = shelf
                            .query(ytq!(
                                .header.musicCarouselShelfBasicHeaderRenderer.moreContentButton
                                    .buttonRenderer.navigationEndpoint
                            ))
                            .and_then(|endpoint| endpoint.deserialize::<JsonValue>().ok())
                            .and_then(|endpoint| url_endpoint::browse_endpoint(&endpoint))
                            .and_then(|endpoint| {
                                (endpoint.browse_endpoint.browse_id
                                    == "FEmusic_moods_and_genres_category")
                                    .then_some(endpoint.browse_endpoint.params)
                            });
                        let mut mapped = music_item_contents(&shelf)
                            .map(|contents| map_music_items(&contents, ctx.lang).0)
                            .unwrap_or_default();
                        warnings.append(&mut mapped.warnings);
                        return Some(MusicGenreSection {
                            name,
                            subgenre_id,
                            playlists: mapped.c,
                        });
                    }

                    if let Some(grid) = music_grid_node(&section) {
                        let name = grid
                            .text_at(ytq!(.header.gridHeaderRenderer.title))
                            .unwrap_or_default();
                        let mut mapped = music_grid_items(&grid)
                            .map(|items| map_music_items(&items, ctx.lang).0)
                            .unwrap_or_default();
                        warnings.append(&mut mapped.warnings);
                        return Some(MusicGenreSection {
                            name,
                            subgenre_id: None,
                            playlists: mapped.c,
                        });
                    }

                    None
                })
                .collect();

            Ok(MapResult {
                c: MusicGenre {
                    id: ctx.id.to_owned(),
                    name,
                    sections,
                },
                warnings,
            })
        })
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use path_macro::path;
    use rstest::rstest;

    use super::*;
    use crate::{model, util::tests::TESTFILES};

    #[test]
    fn map_music_genres() {
        let json_path = path!(*TESTFILES / "music_genres" / "genres.json");
        let json = JsonDoc::new(fs::read_to_string(json_path).unwrap());
        let map_res: MapResult<Vec<model::MusicGenreItem>> =
            MusicGenresEndpoint::map(&json, &MapRespCtx::test("")).unwrap();

        assert!(
            map_res.warnings.is_empty(),
            "deserialization/mapping warnings: {:?}",
            map_res.warnings
        );
        insta::assert_ron_snapshot!("map_music_genres", map_res.c);
    }

    #[rstest]
    #[case::default("default", "ggMPOg1uX1lMbVZmbzl6NlJ3")]
    #[case::mood("mood", "ggMPOg1uX1JOQWZFeDByc2Jm")]
    fn map_music_genre(#[case] name: &str, #[case] id: &str) {
        let json_path = path!(*TESTFILES / "music_genres" / format!("genre_{name}.json"));
        let json = JsonDoc::new(fs::read_to_string(json_path).unwrap());
        let map_res: MapResult<model::MusicGenre> =
            MusicGenreEndpoint::map(&json, &MapRespCtx::test(id)).unwrap();

        assert!(
            map_res.warnings.is_empty(),
            "deserialization/mapping warnings: {:?}",
            map_res.warnings
        );
        insta::assert_ron_snapshot!(format!("map_music_genre_{name}"), map_res.c);
    }
}
