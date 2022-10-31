use serde::Deserialize;
use serde_with::{serde_as, DefaultOnError, VecSkipError};

use crate::{
    model::{self, AlbumItem, AlbumType, ArtistItem, ChannelId, MusicPlaylistItem, TrackItem},
    param::Language,
    serializer::{
        text::{Text, TextComponents},
        MapResult, VecLogError,
    },
    util::{self, TryRemove},
};

use super::{
    url_endpoint::{NavigationEndpoint, PageType},
    MusicContinuation, ThumbnailsWrap,
};

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MusicShelf {
    /// Playlist ID (only for playlists)
    pub playlist_id: Option<String>,
    #[serde_as(as = "VecLogError<_>")]
    pub contents: MapResult<Vec<MusicItem>>,
    /// Continuation token for fetching more (>100) playlist items
    #[serde(default)]
    #[serde_as(as = "VecSkipError<_>")]
    pub continuations: Vec<MusicContinuation>,
}

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
    /// `[<"Das Beste">], [<"Silbermond">], [<"Laut Gedacht (Re-Edition)">]`
    /// Playlist track (title, artist, album)
    ///
    /// `[<"Der Himmel reißt auf">]` Album track (title)
    ///
    /// `[<"Girls">], ["Song", " • ", <"aespa">, " • ", <"Girls - The 2nd Mini Album">, " • ", "4:01"]`
    /// Search track (title, artist, album, duration)
    ///
    /// `[<"Black Mamba">], ["Video", " • ", <"aespa">, " • ", "235M views", " • ", "3:50"]`
    /// Search video (title, artist, view count, duration)
    ///
    /// `["Next Level"], ["Single", " • ", <"aespa">, " • ", "2021"]`
    /// Search album (title, type, artist, year)
    ///
    /// `["Test Shot Starfish"], ["Artist", " • ", "1660 subscribers"]` Search artist
    ///
    /// `["aespa - All Songs & MV"], ["Playlist", " • ", <"Jerwen">, " • ", "49 songs"]`
    /// Search playlist (title, creator, track count)
    pub flex_columns: Vec<MusicColumn>,
    /// Track duration (playlist/album tracks)
    ///
    /// `"3:32"`
    #[serde(default)]
    pub fixed_columns: Vec<MusicColumn>,
    /// Content type + ID (for non-track search items)
    pub navigation_endpoint: Option<NavigationEndpoint>,
    #[serde(default)]
    pub flex_column_display_style: FlexColumnDisplayStyle,
}

#[derive(Default, Debug, Deserialize)]
pub(crate) enum FlexColumnDisplayStyle {
    #[serde(rename = "MUSIC_RESPONSIVE_LIST_ITEM_FLEX_COLUMN_DISPLAY_STYLE_TWO_LINE_STACK")]
    TwoLines,
    #[default]
    #[serde(other)]
    Default,
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

impl From<MusicColumn> for TextComponents {
    fn from(col: MusicColumn) -> Self {
        col.renderer.text
    }
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
                let mut columns = item.flex_columns.into_iter();
                let title = columns.next().map(|col| col.renderer.text.to_string());
                let c2 = columns.next();
                let c3 = columns.next();

                match item.navigation_endpoint {
                    // Artist / Album / Playlist
                    Some(ne) => {
                        let (page_type, id) = ne
                            .music_page()
                            .ok_or_else(|| "could not get navigation endpoint".to_owned())?;

                        let title =
                            title.ok_or_else(|| format!("track {}: could not get title", id))?;

                        let mut subtitle_parts = c2
                            .ok_or_else(|| format!("item {}: could not get subtitle", id))?
                            .renderer
                            .text
                            .split(util::DOT_SEPARATOR)
                            .into_iter();
                        let subtitle_p1 = subtitle_parts.next();
                        let subtitle_p2 = subtitle_parts.next();
                        let subtitle_p3 = subtitle_parts.next();

                        match page_type {
                            PageType::Artist => {
                                let subscriber_count = subtitle_p2.and_then(|p| {
                                    util::parse_large_numstr(&p.to_string(), self.lang)
                                });

                                self.artists.push(ArtistItem {
                                    id,
                                    name: title,
                                    avatar: item.thumbnail.into(),
                                    subscriber_count,
                                });
                                Ok(())
                            }
                            PageType::Album => {
                                let album_type = subtitle_p1
                                    .map(|st| map_album_type(&st.to_string()))
                                    .unwrap_or_default();

                                let (artists, artists_txt) = map_artists(subtitle_p2);

                                let year = subtitle_p3
                                    .and_then(|st| util::parse_numeric(&st.to_string()).ok());

                                self.albums.push(AlbumItem {
                                    id,
                                    name: title,
                                    cover: item.thumbnail.into(),
                                    artists,
                                    artists_txt,
                                    album_type,
                                    year,
                                });
                                Ok(())
                            }
                            PageType::Playlist => {
                                let from_ytm = subtitle_p2
                                    .as_ref()
                                    .and_then(|p| {
                                        p.0.first().map(|txt| txt.as_str() == util::YT_MUSIC_NAME)
                                    })
                                    .unwrap_or_default();
                                let channel = subtitle_p2.and_then(|p| {
                                    p.0.into_iter().find_map(|c| ChannelId::try_from(c).ok())
                                });
                                let track_count = subtitle_p3
                                    .and_then(|p| util::parse_numeric(&p.to_string()).ok());

                                self.playlists.push(MusicPlaylistItem {
                                    id,
                                    name: title,
                                    thumbnail: item.thumbnail.into(),
                                    channel,
                                    track_count,
                                    from_ytm,
                                });
                                Ok(())
                            }
                            PageType::Channel => {
                                Err(format!("channel items unsupported. id: {}", id))
                            }
                        }
                    }
                    // Track
                    None => {
                        let first_tn = item
                            .thumbnail
                            .music_thumbnail_renderer
                            .thumbnail
                            .thumbnails
                            .first();

                        let id = item
                            .playlist_item_data
                            .map(|d| d.video_id)
                            .or_else(|| {
                                first_tn.and_then(|tn| util::video_id_from_thumbnail_url(&tn.url))
                            })
                            .ok_or_else(|| "no video id".to_owned())?;

                        let title =
                            title.ok_or_else(|| format!("track {}: could not get title", id))?;

                        let is_video =
                            !first_tn.map(|tn| tn.height == tn.width).unwrap_or_default();

                        let (artists_p, album_p, duration_p) = match item.flex_column_display_style
                        {
                            FlexColumnDisplayStyle::TwoLines => {
                                let mut subtitle_parts = c2
                                    .ok_or_else(|| format!("track {}: could not get subtitle", id))?
                                    .renderer
                                    .text
                                    .split(util::DOT_SEPARATOR)
                                    .into_iter();
                                // Skip first part (track type)
                                subtitle_parts.next();
                                (
                                    subtitle_parts.next(),
                                    subtitle_parts.next(),
                                    subtitle_parts.next(),
                                )
                            }
                            FlexColumnDisplayStyle::Default => {
                                let mut fixed_columns = item.fixed_columns;
                                (
                                    c2.map(TextComponents::from),
                                    c3.map(TextComponents::from),
                                    fixed_columns.try_swap_remove(0).map(TextComponents::from),
                                )
                            }
                        };

                        let duration = duration_p
                            .and_then(|p| util::parse_video_length(&p.to_string()))
                            .ok_or_else(|| format!("track {}: could not parse duration", id))?;

                        // The album field contains the track count for search videos
                        let (album, view_count) = match (item.flex_column_display_style, is_video) {
                            (FlexColumnDisplayStyle::TwoLines, true) => (
                                None,
                                album_p.and_then(|p| {
                                    util::parse_large_numstr(&p.to_string(), self.lang)
                                }),
                            ),
                            (_, false) => (
                                album_p.and_then(|p| {
                                    p.0.into_iter()
                                        .find_map(|c| model::AlbumId::try_from(c).ok())
                                }),
                                None,
                            ),
                            (FlexColumnDisplayStyle::Default, true) => (None, None),
                        };

                        let mut artists_txt =
                            artists_p.as_ref().and_then(TextComponents::to_opt_string);
                        let mut artists = artists_p
                            .map(|p| {
                                p.0.into_iter()
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

                        self.tracks.push(TrackItem {
                            id,
                            title,
                            duration,
                            cover: item.thumbnail.into(),
                            artists,
                            artists_txt,
                            album,
                            view_count,
                            is_video,
                        });
                        Ok(())
                    }
                }
            }
            MusicItem::MusicTwoRowItemRenderer(item) => {
                let mut subtitle_parts = item.subtitle.split(util::DOT_SEPARATOR).into_iter();
                let subtitle_p1 = subtitle_parts.next();
                let subtitle_p2 = subtitle_parts.next();
                let subtitle_p3 = subtitle_parts.next();

                let (page_type, id) = item
                    .navigation_endpoint
                    .music_page()
                    .ok_or_else(|| "could not get navigation endpoint".to_owned())?;

                match page_type {
                    PageType::Album => {
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
                                        id
                                    ));
                                }
                            };

                        self.albums.push(AlbumItem {
                            id,
                            name: item.title,
                            cover: item.thumbnail_renderer.into(),
                            artists,
                            artists_txt,
                            year,
                            album_type,
                        });
                        Ok(())
                    }
                    PageType::Playlist => {
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
                        let track_count =
                            subtitle_p3.and_then(|p| util::parse_numeric(&p.to_string()).ok());

                        self.playlists.push(MusicPlaylistItem {
                            id,
                            name: item.title,
                            thumbnail: item.thumbnail_renderer.into(),
                            channel,
                            track_count,
                            from_ytm,
                        });
                        Ok(())
                    }
                    PageType::Artist => {
                        let subscriber_count = subtitle_p1
                            .and_then(|p| util::parse_large_numstr(&p.to_string(), self.lang));

                        self.artists.push(ArtistItem {
                            id,
                            name: item.title,
                            avatar: item.thumbnail_renderer.into(),
                            subscriber_count,
                        });
                        Ok(())
                    }
                    PageType::Channel => Err(format!("channel items unsupported. id: {}", id)),
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
