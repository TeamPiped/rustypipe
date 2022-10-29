use serde::Deserialize;
use serde_with::{serde_as, DefaultOnError};

use crate::{
    model::{self, ChannelId},
    serializer::{text::TextComponents, MapResult},
    util::{self, TryRemove},
};

use super::ThumbnailsWrap;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MusicItem {
    pub music_responsive_list_item_renderer: InnerMusicItem,
}

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct InnerMusicItem {
    pub thumbnail: MusicThumbnailRenderer,
    #[serde(default)]
    #[serde_as(deserialize_as = "DefaultOnError")]
    pub playlist_item_data: Option<PlaylistItemData>,
    pub flex_columns: Vec<MusicColumn>,
    pub fixed_columns: Vec<MusicColumn>,
}

#[derive(Default, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MusicThumbnailRenderer {
    #[serde(alias = "croppedSquareThumbnailRenderer")]
    pub music_thumbnail_renderer: ThumbnailsWrap,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PlaylistItemData {
    pub video_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MusicContentsRenderer<T> {
    pub contents: Vec<T>,
    /*
    /// Continuation token for fetching recommended items
    #[serde(default)]
    #[serde_as(as = "VecSkipError<_>")]
    pub continuations: Vec<MusicContinuation>,
    */
}

#[derive(Debug, Deserialize)]
pub(crate) struct MusicColumn {
    #[serde(
        rename = "musicResponsiveListItemFlexColumnRenderer",
        alias = "musicResponsiveListItemFixedColumnRenderer"
    )]
    pub renderer: MusicColumnRenderer,
}

#[serde_as]
#[derive(Debug, Deserialize)]
pub(crate) struct MusicColumnRenderer {
    pub text: TextComponents,
}

impl From<MusicThumbnailRenderer> for Vec<model::Thumbnail> {
    fn from(tr: MusicThumbnailRenderer) -> Self {
        tr.music_thumbnail_renderer.thumbnail.into()
    }
}

/*
#MAPPER
*/

#[derive(Debug)]
pub(crate) struct MusicListMapper<T> {
    artists: Option<Vec<ChannelId>>,

    pub items: Vec<T>,
    pub warnings: Vec<String>,
}

impl<T> MusicListMapper<T> {
    pub fn new() -> Self {
        Self {
            artists: None,
            items: Vec::new(),
            warnings: Vec::new(),
        }
    }

    pub fn with_artists(artists: Vec<ChannelId>) -> Self {
        Self {
            artists: Some(artists),
            items: Vec::new(),
            warnings: Vec::new(),
        }
    }

    fn map_music_item(&mut self, item: MusicItem) -> Option<model::YouTubeMusicItem> {
        let item = item.music_responsive_list_item_renderer;

        let first_tn = item
            .thumbnail
            .music_thumbnail_renderer
            .thumbnail
            .thumbnails
            .first();

        let id = some_or_bail!(
            item.playlist_item_data
                .map(|d| d.video_id)
                .or_else(|| first_tn.and_then(|tn| util::video_id_from_thumbnail_url(&tn.url))),
            None
        );

        let is_video = !first_tn.map(|tn| tn.height == tn.width).unwrap_or_default();

        let duration = item.fixed_columns.first().and_then(|col| {
            col.renderer
                .text
                .0
                .first()
                .and_then(|txt| util::parse_video_length(txt.as_str()))
        });

        let mut columns = item.flex_columns;

        let album = columns.try_swap_remove(2).and_then(|col| {
            col.renderer
                .text
                .0
                .into_iter()
                .find_map(|c| model::AlbumId::try_from(c).ok())
        });

        let artists_col = columns.try_swap_remove(1);
        let artists_txt = artists_col
            .as_ref()
            .map(|col| col.renderer.text.to_string());
        let mut artists = artists_col
            .map(|col| {
                col.renderer
                    .text
                    .0
                    .into_iter()
                    .filter_map(|c| ChannelId::try_from(c).ok())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        if let Some(a) = &self.artists {
            if artists.is_empty() {
                artists = a.clone();
            }
        }

        let title = columns
            .try_swap_remove(0)
            .map(|col| col.renderer.text.to_string());

        match (title, duration) {
            (Some(title), Some(duration)) => {
                Some(model::YouTubeMusicItem::Track(model::TrackItem {
                    id,
                    title,
                    duration,
                    cover: item.thumbnail.into(),
                    artists,
                    artists_txt,
                    album,
                    view_count: None,
                    is_video,
                }))
            }
            (None, _) => {
                self.warnings
                    .push(format!("track {}: could not get title", id));
                None
            }
            (_, None) => {
                self.warnings
                    .push(format!("track {}: could not parse duration", id));
                None
            }
        }
    }
}

/*
impl MusicListMapper<model::YouTubeMusicItem> {
    fn map_item(&mut self, item: MusicItem) {
        if let Some(mapped) = self.map_music_item(item) {
            self.items.push(mapped);
        }
    }

    pub(crate) fn map_response(&mut self, mut res: MapResult<Vec<MusicItem>>) {
        self.warnings.append(&mut res.warnings);
        res.c.into_iter().for_each(|item| self.map_item(item));
    }
}
*/

impl MusicListMapper<model::TrackItem> {
    fn map_item(&mut self, item: MusicItem) {
        if let Some(model::YouTubeMusicItem::Track(track)) = self.map_music_item(item) {
            self.items.push(track);
        }
    }

    pub(crate) fn map_response(&mut self, mut res: MapResult<Vec<MusicItem>>) {
        self.warnings.append(&mut res.warnings);
        res.c.into_iter().for_each(|item| self.map_item(item));
    }
}
