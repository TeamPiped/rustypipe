use std::{borrow::Cow, convert::TryFrom, fmt::Debug};

use time::OffsetDateTime;

use crate::{
    error::{Error, ExtractionError},
    model::{
        paginator::{ContinuationEndpoint, Paginator},
        richtext::RichText,
        ChannelId, Playlist, VideoItem,
    },
    serializer::text::{TextComponent, TextComponents},
    util::{self, timeago, TryRemove},
};

use super::{response, ClientType, MapResponse, MapResult, QBrowse, RustyPipeQuery};

impl RustyPipeQuery {
    /// Get a YouTube playlist
    #[tracing::instrument(skip(self))]
    pub async fn playlist<S: AsRef<str> + Debug>(&self, playlist_id: S) -> Result<Playlist, Error> {
        let playlist_id = playlist_id.as_ref();
        // YTM playlists require visitor data for continuations to work
        let visitor_data: Option<String> = if playlist_id.starts_with("RD") {
            Some(self.get_visitor_data().await?)
        } else {
            None
        };
        let context = self
            .get_context(ClientType::Desktop, true, visitor_data.as_deref())
            .await;
        let request_body = QBrowse {
            context,
            browse_id: &format!("VL{playlist_id}"),
        };

        self.execute_request_vdata::<response::Playlist, _, _>(
            ClientType::Desktop,
            "playlist",
            playlist_id,
            "browse",
            &request_body,
            visitor_data.as_deref(),
        )
        .await
    }
}

impl MapResponse<Playlist> for response::Playlist {
    fn map_response(
        self,
        id: &str,
        lang: crate::param::Language,
        _deobf: Option<&crate::deobfuscate::DeobfData>,
        vdata: Option<&str>,
    ) -> Result<MapResult<Playlist>, ExtractionError> {
        let (Some(contents), Some(header)) = (self.contents, self.header) else {
            return Err(response::alerts_to_err(id, self.alerts));
        };

        let video_items = contents
            .two_column_browse_results_renderer
            .contents
            .into_iter()
            .next()
            .ok_or(ExtractionError::InvalidData(Cow::Borrowed(
                "twoColumnBrowseResultsRenderer empty",
            )))?
            .tab_renderer
            .content
            .section_list_renderer
            .contents
            .into_iter()
            .next()
            .ok_or(ExtractionError::InvalidData(Cow::Borrowed(
                "sectionListRenderer empty",
            )))?
            .item_section_renderer
            .contents
            .into_iter()
            .next()
            .ok_or(ExtractionError::InvalidData(Cow::Borrowed(
                "itemSectionRenderer empty",
            )))?
            .playlist_video_list_renderer
            .contents;

        let mut mapper = response::YouTubeListMapper::<VideoItem>::new(lang);
        mapper.map_response(video_items);

        let (description, thumbnails, last_update_txt) = match self.sidebar {
            Some(sidebar) => {
                let sidebar_items = sidebar.playlist_sidebar_renderer.contents;
                let mut primary =
                    sidebar_items
                        .into_iter()
                        .next()
                        .ok_or(ExtractionError::InvalidData(Cow::Borrowed(
                            "no primary sidebar",
                        )))?;

                (
                    primary
                        .playlist_sidebar_primary_info_renderer
                        .description
                        .filter(|d| !d.0.is_empty()),
                    primary
                        .playlist_sidebar_primary_info_renderer
                        .thumbnail_renderer
                        .playlist_video_thumbnail_renderer
                        .thumbnail,
                    primary
                        .playlist_sidebar_primary_info_renderer
                        .stats
                        .try_swap_remove(2),
                )
            }
            None => {
                let header_banner = header
                    .playlist_header_renderer
                    .playlist_header_banner
                    .ok_or(ExtractionError::InvalidData(Cow::Borrowed(
                        "no thumbnail found",
                    )))?;

                let mut byline = header.playlist_header_renderer.byline;
                let last_update_txt = byline
                    .try_swap_remove(1)
                    .map(|b| b.playlist_byline_renderer.text);

                (
                    None,
                    header_banner.hero_playlist_thumbnail_renderer.thumbnail,
                    last_update_txt,
                )
            }
        };

        let n_videos = if mapper.ctoken.is_some() {
            util::parse_numeric(&header.playlist_header_renderer.num_videos_text)
                .map_err(|_| ExtractionError::InvalidData(Cow::Borrowed("no video count")))?
        } else {
            mapper.items.len() as u64
        };

        let playlist_id = header.playlist_header_renderer.playlist_id;
        if playlist_id != id {
            return Err(ExtractionError::WrongResult(format!(
                "got wrong playlist id {playlist_id}, expected {id}"
            )));
        }

        let name = header.playlist_header_renderer.title;
        let description = description
            .or_else(|| {
                header
                    .playlist_header_renderer
                    .description_text
                    .map(|text| TextComponents(vec![TextComponent::Text { text }]))
            })
            .map(RichText::from);
        let channel = header
            .playlist_header_renderer
            .owner_text
            .and_then(|link| ChannelId::try_from(link).ok());

        let last_update = last_update_txt.as_ref().and_then(|txt| {
            timeago::parse_textual_date_or_warn(lang, txt, &mut mapper.warnings)
                .map(OffsetDateTime::date)
        });

        Ok(MapResult {
            c: Playlist {
                id: playlist_id,
                name,
                videos: Paginator::new_ext(
                    Some(n_videos),
                    mapper.items,
                    mapper.ctoken,
                    vdata.map(str::to_owned),
                    ContinuationEndpoint::Browse,
                ),
                video_count: n_videos,
                thumbnail: thumbnails.into(),
                description,
                channel,
                last_update,
                last_update_txt,
                visitor_data: self
                    .response_context
                    .visitor_data
                    .or_else(|| vdata.map(str::to_owned)),
            },
            warnings: mapper.warnings,
        })
    }
}

#[cfg(test)]
mod tests {
    use std::{fs::File, io::BufReader};

    use path_macro::path;
    use rstest::rstest;

    use crate::{param::Language, util::tests::TESTFILES};

    use super::*;

    #[rstest]
    #[case::short("short", "RDCLAK5uy_kFQXdnqMaQCVx2wpUM4ZfbsGCDibZtkJk")]
    #[case::long("long", "PL5dDx681T4bR7ZF1IuWzOv1omlRbE7PiJ")]
    #[case::nomusic("nomusic", "PL1J-6JOckZtE_P9Xx8D3b2O6w0idhuKBe")]
    #[case::live("live", "UULVvqRdlKsE5Q8mf8YXbdIJLw")]
    fn map_playlist_data(#[case] name: &str, #[case] id: &str) {
        let json_path = path!(*TESTFILES / "playlist" / format!("playlist_{name}.json"));
        let json_file = File::open(json_path).unwrap();

        let playlist: response::Playlist =
            serde_json::from_reader(BufReader::new(json_file)).unwrap();
        let map_res = playlist.map_response(id, Language::En, None, None).unwrap();

        assert!(
            map_res.warnings.is_empty(),
            "deserialization/mapping warnings: {:?}",
            map_res.warnings
        );
        insta::assert_ron_snapshot!(format!("map_playlist_data_{name}"), map_res.c, {
            ".last_update" => "[date]",
            ".videos.items[].publish_date" => "[date]",
        });
    }
}
