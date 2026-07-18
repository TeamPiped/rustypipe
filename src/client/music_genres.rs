use std::{borrow::Cow, fmt::Debug};

use crate::{
    error::{Error, ExtractionError},
    json::{yt_music_header_title, yt_single_column_sections, ytq, JsonDoc},
    model::{MusicGenre, MusicGenreItem, MusicGenreSection},
    request_body::ytbody,
    serializer::MapResult,
};

use super::{
    response::{
        music_genres::NavigationButton, music_item::MusicListMapper,
        url_endpoint::NavigationEndpoint,
    },
    ClientType, MapJsonResponse, MapRespCtx, RustyPipeQuery,
};

#[derive(Debug)]
struct MusicGenresJson;
#[derive(Debug)]
struct MusicGenreJson;

impl RustyPipeQuery {
    /// Get a list of moods and genres from YouTube Music
    #[tracing::instrument(skip(self), level = "error")]
    pub async fn music_genres(&self) -> Result<Vec<MusicGenreItem>, Error> {
        let request_body = ytbody!({
            "browseId": "FEmusic_moods_and_genres",
        });

        self.execute_request::<MusicGenresJson, _, _>(
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

        self.execute_request::<MusicGenreJson, _, _>(
            ClientType::DesktopMusic,
            "music_genre",
            genre_id,
            "browse",
            &request_body,
        )
        .await
    }
}

impl MapJsonResponse<Vec<MusicGenreItem>> for MusicGenresJson {
    fn map_json_response(
        json: &JsonDoc,
        _ctx: &MapRespCtx<'_>,
    ) -> Result<MapResult<Vec<MusicGenreItem>>, ExtractionError> {
        json.with_root(|root| {
            let sections = yt_single_column_sections(&root)?;
            let section_items = sections.items();
            let i_start = section_items.len().saturating_sub(2);
            let mut warnings = Vec::new();
            let genres = section_items
                .into_iter()
                .skip(i_start)
                .enumerate()
                .flat_map(|(i, section)| {
                    let Some(grid) = section.query(ytq!(.gridRenderer)) else {
                        return Vec::new();
                    };
                    let Some(contents) = grid.first_of(&[ytq!(.items), ytq!(.contents)]) else {
                        return Vec::new();
                    };
                    let (buttons, mut grid_warnings) =
                        contents.deserialize_items_lossy::<NavigationButton>();
                    warnings.append(&mut grid_warnings);
                    buttons
                        .into_iter()
                        .filter_map(move |section| match section {
                            NavigationButton::MusicNavigationButtonRenderer(btn) => {
                                Some(MusicGenreItem {
                                    id: btn.click_command.browse_endpoint.params,
                                    name: btn.button_text,
                                    is_mood: i == 0,
                                    color: btn.solid.left_stripe_color,
                                })
                            }
                            NavigationButton::None => None,
                        })
                        .collect::<Vec<_>>()
                })
                .collect();

            Ok(MapResult {
                c: genres,
                warnings,
            })
        })
    }
}

impl MapJsonResponse<MusicGenre> for MusicGenreJson {
    fn map_json_response(
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
                    if let Some(shelf) = section.query(ytq!(.musicCarouselShelfRenderer)) {
                        let name = shelf
                            .query(ytq!(.header.musicCarouselShelfBasicHeaderRenderer.title))
                            .and_then(|node| node.text())
                            .unwrap_or_default();
                        let subgenre_id = shelf
                            .query(ytq!(
                                .header.musicCarouselShelfBasicHeaderRenderer.moreContentButton
                                    .buttonRenderer.navigationEndpoint
                            ))
                            .and_then(|endpoint| {
                                if let Ok(NavigationEndpoint::Browse {
                                    browse_endpoint, ..
                                }) = endpoint.deserialize()
                                {
                                    if browse_endpoint.browse_id
                                        == "FEmusic_moods_and_genres_category"
                                    {
                                        Some(browse_endpoint.params)
                                    } else {
                                        None
                                    }
                                } else {
                                    None
                                }
                            });
                        let mut mapper = MusicListMapper::new(ctx.lang);
                        if let Some(contents) = shelf.query(ytq!(.contents)) {
                            mapper.map_response_node(&contents);
                        }
                        let mut mapped = mapper.conv_items();
                        warnings.append(&mut mapped.warnings);
                        return Some(MusicGenreSection {
                            name,
                            subgenre_id,
                            playlists: mapped.c,
                        });
                    }

                    if let Some(grid) = section.query(ytq!(.gridRenderer)) {
                        let name = grid
                            .query(ytq!(.header.gridHeaderRenderer.title))
                            .and_then(|node| node.text())
                            .unwrap_or_default();
                        let mut mapper = MusicListMapper::new(ctx.lang);
                        if let Some(items) = grid.query(ytq!(.items)) {
                            mapper.map_response_node(&items);
                        }
                        let mut mapped = mapper.conv_items();
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
            MusicGenresJson::map_json_response(&json, &MapRespCtx::test("")).unwrap();

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
            MusicGenreJson::map_json_response(&json, &MapRespCtx::test(id)).unwrap();

        assert!(
            map_res.warnings.is_empty(),
            "deserialization/mapping warnings: {:?}",
            map_res.warnings
        );
        insta::assert_ron_snapshot!(format!("map_music_genre_{name}"), map_res.c);
    }
}
