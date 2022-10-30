use serde::Deserialize;
use serde_with::{serde_as, DefaultOnError};

use crate::{
    model::{self, AlbumItem, AlbumType, ArtistItem, ChannelId, MusicPlaylistItem, TrackItem},
    param::Language,
    serializer::{
        text::{Text, TextComponents},
        MapResult,
    },
    util::{self, TryRemove},
};

use super::{url_endpoint::NavigationEndpoint, ThumbnailsWrap};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum MusicItem {
    MusicResponsiveListItemRenderer(ListMusicItem),
    MusicTwoRowItemRenderer(CoverMusicItem),
}

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ListMusicItem {
    #[serde(default)]
    pub thumbnail: MusicThumbnailRenderer,
    #[serde(default)]
    #[serde_as(deserialize_as = "DefaultOnError")]
    pub playlist_item_data: Option<PlaylistItemData>,
    pub flex_columns: Vec<MusicColumn>,
    pub fixed_columns: Vec<MusicColumn>,
}

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CoverMusicItem {
    #[serde_as(as = "Text")]
    pub title: String,
    /// Content type + Channel/Artist
    ///
    /// `"Album", " • ", <"Oonagh">` Album variants, new releases
    ///
    /// `"Album", " • ", "2022"` Artist albums
    ///
    /// `"2022"` Artist singles
    ///
    /// `"Playlist", " • ", <"ThetaDev"> " • ", "26 songs"`
    ///
    /// `"Playlist", " • ", "YouTube Music" Featured on
    #[serde(default)]
    pub subtitle: TextComponents,
    #[serde(default)]
    pub thumbnail_renderer: MusicThumbnailRenderer,
    /// Content type + ID
    pub navigation_endpoint: NavigationEndpoint,
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
pub(crate) struct MusicListMapper {
    lang: Language,
    o_artists: Option<(Vec<ChannelId>, String)>,
    artist_page: bool,

    pub tracks: Vec<TrackItem>,
    pub albums: Vec<AlbumItem>,
    pub artists: Vec<ArtistItem>,
    pub playlists: Vec<MusicPlaylistItem>,

    pub warnings: Vec<String>,
}

impl MusicListMapper {
    pub fn new(lang: Language) -> Self {
        Self {
            lang,
            o_artists: None,
            artist_page: false,
            tracks: Vec::new(),
            albums: Vec::new(),
            artists: Vec::new(),
            playlists: Vec::new(),
            warnings: Vec::new(),
        }
    }

    pub fn with_artists(
        lang: Language,
        artists: Vec<ChannelId>,
        artists_txt: String,
        artist_page: bool,
    ) -> Self {
        Self {
            lang,
            o_artists: Some((artists, artists_txt)),
            artist_page,
            tracks: Vec::new(),
            albums: Vec::new(),
            artists: Vec::new(),
            playlists: Vec::new(),
            warnings: Vec::new(),
        }
    }

    fn map_item(&mut self, item: MusicItem) -> Result<(), String> {
        match item {
            MusicItem::MusicResponsiveListItemRenderer(item) => {
                let first_tn = item
                    .thumbnail
                    .music_thumbnail_renderer
                    .thumbnail
                    .thumbnails
                    .first();

                let id = item
                    .playlist_item_data
                    .map(|d| d.video_id)
                    .or_else(|| first_tn.and_then(|tn| util::video_id_from_thumbnail_url(&tn.url)))
                    .ok_or_else(|| "no video id".to_owned())?;

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
                let mut artists_txt = artists_col
                    .as_ref()
                    .and_then(|col| col.renderer.text.to_opt_string());
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
                if let Some(a) = &self.o_artists {
                    if artists.is_empty() && artists_txt.is_none() {
                        let xa = a.clone();
                        artists = xa.0;
                        artists_txt = Some(xa.1);
                    }
                }

                let title = columns
                    .try_swap_remove(0)
                    .map(|col| col.renderer.text.to_string());

                match (title, duration) {
                    (Some(title), Some(duration)) => {
                        self.tracks.push(TrackItem {
                            id,
                            title,
                            duration,
                            cover: item.thumbnail.into(),
                            artists,
                            artists_txt,
                            album,
                            view_count: None,
                            is_video,
                        });
                        Ok(())
                    }
                    (None, _) => Err(format!("track {}: could not get title", id)),
                    (_, None) => Err(format!("track {}: could not parse duration", id)),
                }
            }
            MusicItem::MusicTwoRowItemRenderer(item) => {
                let mut subtitle_parts = item.subtitle.split(util::DOT_SEPARATOR).into_iter();
                let subtitle_p1 = subtitle_parts.next();
                let subtitle_p2 = subtitle_parts.next();
                let subtitle_p3 = subtitle_parts.next();

                let (page_type, browse_id) = item
                    .navigation_endpoint
                    .music_page()
                    .ok_or_else(|| "could not get navigation endpoint".to_owned())?;

                match page_type {
                    super::url_endpoint::PageType::Album => {
                        let mut year = None;
                        let mut album_type = AlbumType::Single;

                        let (artists, artists_txt) =
                            match (subtitle_p1, subtitle_p2, &self.o_artists, self.artist_page) {
                                // "2022" (Artist singles)
                                (Some(year_txt), None, Some((artists, artists_txt)), true) => {
                                    year = util::parse_numeric(&year_txt.to_string()).ok();
                                    (artists.clone(), artists_txt.clone())
                                }
                                // "Album", "2022" (Artist albums)
                                (
                                    Some(atype_txt),
                                    Some(year_txt),
                                    Some((artists, artists_txt)),
                                    true,
                                ) => {
                                    year = util::parse_numeric(&year_txt.to_string()).ok();
                                    album_type = map_album_type(&atype_txt.to_string());
                                    (artists.clone(), artists_txt.clone())
                                }
                                // "Album", <"Oonagh"> (Album variants, new releases)
                                (Some(atype_txt), Some(p2), _, false) => {
                                    album_type = map_album_type(&atype_txt.to_string());
                                    map_artists(Some(p2))
                                }
                                _ => {
                                    return Err(format!(
                                        "could not parse subtitle of album {}",
                                        browse_id
                                    ));
                                }
                            };

                        self.albums.push(AlbumItem {
                            id: browse_id,
                            name: item.title,
                            cover: item.thumbnail_renderer.into(),
                            artists,
                            artists_txt,
                            year,
                            album_type,
                        });
                        Ok(())
                    }
                    super::url_endpoint::PageType::Playlist => {
                        // TODO: make component to string zero-copy if len=1
                        let from_ytm = subtitle_p2
                            .as_ref()
                            .and_then(|p| {
                                p.0.first().map(|txt| txt.as_str() == util::YT_MUSIC_NAME)
                            })
                            .unwrap_or_default();
                        let channel = subtitle_p2.and_then(|p| {
                            p.0.into_iter().find_map(|c| ChannelId::try_from(c).ok())
                        });

                        self.playlists.push(MusicPlaylistItem {
                            id: browse_id,
                            name: item.title,
                            thumbnail: item.thumbnail_renderer.into(),
                            channel,
                            track_count: subtitle_p3
                                .and_then(|p| util::parse_numeric(&p.to_string()).ok()),
                            from_ytm,
                        });
                        Ok(())
                    }
                    super::url_endpoint::PageType::Artist => {
                        let subscriber_count = subtitle_p1
                            .and_then(|p| util::parse_large_numstr(&p.to_string(), self.lang));

                        self.artists.push(ArtistItem {
                            id: browse_id,
                            name: item.title,
                            avatar: item.thumbnail_renderer.into(),
                            subscriber_count,
                        });
                        Ok(())
                    }
                    super::url_endpoint::PageType::Channel => {
                        Err(format!("channel items unsupported. id: {}", browse_id))
                    }
                }
            }
        }
    }

    pub fn map_response(&mut self, mut res: MapResult<Vec<MusicItem>>) {
        self.warnings.append(&mut res.warnings);
        res.c.into_iter().for_each(|item| {
            if let Err(e) = self.map_item(item) {
                self.warnings.push(e);
            }
        });
    }
}

pub(crate) fn map_artists(artists_p: Option<TextComponents>) -> (Vec<ChannelId>, String) {
    let artists_txt = artists_p
        .as_ref()
        .map(|p| p.to_string())
        .unwrap_or_default();
    let artists = artists_p
        .map(|part| {
            part.0
                .into_iter()
                .filter_map(|c| ChannelId::try_from(c).ok())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    (artists, artists_txt)
}

pub(crate) fn map_album_type(txt: &str) -> AlbumType {
    // TODO: add support for different languages
    match txt {
        "Single" => AlbumType::Single,
        "EP" => AlbumType::Ep,
        _ => AlbumType::Album,
    }
}
