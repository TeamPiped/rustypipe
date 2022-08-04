pub mod player;
pub mod playlist;

pub use player::Player;
pub use playlist::Playlist;

use serde::Deserialize;
use serde_with::{serde_as, VecSkipError};

use crate::serializer::text::TextLink;

#[derive(Default, Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Thumbnails {
    pub thumbnails: Vec<Thumbnail>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Thumbnail {
    pub url: String,
    pub width: u32,
    pub height: u32,
}

// YouTube Music

#[serde_as]
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MusicItem {
    thumbnail: MusicThumbnailRenderer,
    playlist_item_data: PlaylistItemData,
    #[serde(default)]
    #[serde_as(as = "VecSkipError<_>")]
    flex_columns: Vec<MusicColumn>,
    #[serde(default)]
    #[serde_as(as = "VecSkipError<_>")]
    fixed_columns: Vec<MusicColumn>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MusicThumbnailRenderer {
    music_thumbnail_renderer: MusicThumbnailRenderer2,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MusicThumbnailRenderer2 {
    thumbnail: Thumbnails,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaylistItemData {
    video_id: String,
}

#[derive(Clone, Debug, Deserialize)]
pub struct MusicColumn {
    #[serde(
        rename = "musicResponsiveListItemFlexColumnRenderer",
        alias = "musicResponsiveListItemFixedColumnRenderer"
    )]
    renderer: MusicColumnRenderer,
}

#[serde_as]
#[derive(Clone, Debug, Deserialize)]
pub struct MusicColumnRenderer {
    #[serde_as(as = "crate::serializer::text::TextLink")]
    text: TextLink,
}
