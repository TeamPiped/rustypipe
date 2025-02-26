use std::{borrow::Cow, fmt::Debug};

use crate::{
    error::{Error, ExtractionError},
    model::{MusicGenre, MusicGenreItem, MusicGenreSection},
    serializer::MapResult,
};

use super::{
    response::{self, music_item::MusicListMapper, url_endpoint::NavigationEndpoint},
    ClientType, MapRespCtx, MapResponse, QBrowse, QBrowseParams, RustyPipeQuery,
};

impl RustyPipeQuery {
    /// Get a list of moods and genres from YouTube Music
    #[tracing::instrument(skip(self), level = "error")]
    pub async fn music_genres(&self) -> Result<Vec<MusicGenreItem>, Error> {
        let request_body = QBrowse {
            browse_id: "FEmusic_moods_and_genres",
        };

        self.execute_request::<response::MusicGenres, _, _>(
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
        let request_body = QBrowseParams {
            browse_id: "FEmusic_moods_and_genres_category",
            params: genre_id,
        };

        self.execute_request::<response::MusicGenre, _, _>(
            ClientType::DesktopMusic,
            "music_genre",
            genre_id,
            "browse",
            &request_body,
        )
        .await
    }
}

impl MapResponse<Vec<MusicGenreItem>> for response::MusicGenres {
    fn map_response(
        self,
        _ctx: &MapRespCtx<'_>,
    ) -> Result<MapResult<Vec<MusicGenreItem>>, ExtractionError> {
        let content = self
            .contents
            .single_column_browse_results_renderer
            .contents
            .into_iter()
            .next()
            .ok_or(ExtractionError::InvalidData(Cow::Borrowed("no content")))?
            .tab_renderer
            .content
            .section_list_renderer
            .contents;

        // Skip over the first section (which contains a personalized genre/mood list)
        let i_start = content.len().saturating_sub(2);
        let mut content_iter = content.into_iter();
        for _ in 0..i_start {
            content_iter.next();
        }

        let mut warnings = Vec::new();
        let genres = content_iter
            .enumerate()
            .flat_map(|(i, grid)| {
                let mut grid = grid.grid_renderer.contents;
                warnings.append(&mut grid.warnings);
                grid.c.into_iter().filter_map(move |section| match section {
                    response::music_genres::NavigationButton::MusicNavigationButtonRenderer(
                        btn,
                    ) => Some(MusicGenreItem {
                        id: btn.click_command.browse_endpoint.params,
                        name: btn.button_text,
                        is_mood: i == 0,
                        color: btn.solid.left_stripe_color,
                    }),
                    response::music_genres::NavigationButton::None => None,
                })
            })
            .collect();

        Ok(MapResult {
            c: genres,
            warnings,
        })
    }
}

impl MapResponse<MusicGenre> for response::MusicGenre {
    fn map_response(self, ctx: &MapRespCtx<'_>) -> Result<MapResult<MusicGenre>, ExtractionError> {
        let content = self
            .contents
            .single_column_browse_results_renderer
            .contents
            .into_iter()
            .next()
            .ok_or(ExtractionError::InvalidData(Cow::Borrowed("no content")))?
            .tab_renderer
            .content
            .section_list_renderer
            .contents;

        let mut warnings = Vec::new();
        let sections = content
            .into_iter()
            .filter_map(|section| {
                let (name, subgenre_id, items) = match section {
                    response::music_item::ItemSection::MusicCarouselShelfRenderer(shelf) => (
                        shelf
                            .header
                            .as_ref()
                            .map(|h| {
                                h.music_carousel_shelf_basic_header_renderer
                                    .title
                                    .to_string()
                            })
                            .unwrap_or_default(),
                        shelf.header.and_then(|h| {
                            h.music_carousel_shelf_basic_header_renderer
                                .more_content_button
                                .and_then(|btn| {
                                    if let NavigationEndpoint::Browse {
                                        browse_endpoint, ..
                                    } = btn.button_renderer.navigation_endpoint
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
                                })
                        }),
                        shelf.contents,
                    ),
                    response::music_item::ItemSection::GridRenderer(grid) => (
                        grid.header
                            .map(|h| h.grid_header_renderer.title)
                            .unwrap_or_default(),
                        None,
                        grid.items,
                    ),
                    _ => return None,
                };

                let mut mapper = MusicListMapper::new(ctx.lang);
                mapper.map_response(items);
                let mut mapped = mapper.conv_items();
                warnings.append(&mut mapped.warnings);

                Some(MusicGenreSection {
                    name,
                    subgenre_id,
                    playlists: mapped.c,
                })
            })
            .collect();

        Ok(MapResult {
            c: MusicGenre {
                id: ctx.id.to_owned(),
                name: self.header.music_header_renderer.title,
                sections,
            },
            warnings: Vec::new(),
        })
    }
}

#[cfg(test)]
mod tests {
    use std::{fs::File, io::BufReader};

    use path_macro::path;
    use rstest::rstest;

    use super::*;
    use crate::{model, util::tests::TESTFILES};

    #[test]
    fn map_music_genres() {
        let json_path = path!(*TESTFILES / "music_genres" / "genres.json");
        let json_file = File::open(json_path).unwrap();

        let playlist: response::MusicGenres =
            serde_json::from_reader(BufReader::new(json_file)).unwrap();
        let map_res: MapResult<Vec<model::MusicGenreItem>> =
            playlist.map_response(&MapRespCtx::test("")).unwrap();

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
        let json_file = File::open(json_path).unwrap();

        let playlist: response::MusicGenre =
            serde_json::from_reader(BufReader::new(json_file)).unwrap();
        let map_res: MapResult<model::MusicGenre> =
            playlist.map_response(&MapRespCtx::test(id)).unwrap();

        assert!(
            map_res.warnings.is_empty(),
            "deserialization/mapping warnings: {:?}",
            map_res.warnings
        );
        insta::assert_ron_snapshot!(format!("map_music_genre_{name}"), map_res.c);
    }
}
