use serde::Deserialize;
use serde_with::{serde_as, DefaultOnError, VecSkipError};

use crate::{
    model::{
        self, AlbumId, AlbumItem, AlbumType, ArtistId, ArtistItem, ChannelId, FromYtItem,
        MusicEntityType, MusicItem, MusicPlaylistItem, TrackItem,
    },
    param::Language,
    serializer::{
        text::{Text, TextComponents},
        MapResult, VecLogError,
    },
    util::{self, dictionary, TryRemove},
};

use super::{
    url_endpoint::{NavigationEndpoint, PageType},
    MusicContinuationData, ThumbnailsWrap,
};

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MusicShelf {
    /// Playlist ID (only for playlists)
    pub playlist_id: Option<String>,
    #[serde_as(as = "VecLogError<_>")]
    pub contents: MapResult<Vec<MusicResponseItem>>,
    /// Continuation token for fetching more (>100) playlist items
    #[serde(default)]
    #[serde_as(as = "VecSkipError<_>")]
    pub continuations: Vec<MusicContinuationData>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum MusicResponseItem {
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
    /// Search track (title, artist, album, duration).
    ///
    /// Info: "Song" label is missing in the "Songs" tab
    ///
    /// `[<"Black Mamba">], ["Video", " • ", <"aespa">, " • ", "235M views", " • ", "3:50"]`
    /// Search video (title, artist, view count, duration)
    ///
    /// Info: "Video" label is missing in the "Videos" tab
    ///
    /// `["Next Level"], ["Single", " • ", <"aespa">, " • ", "2021"]`
    /// Search album (title, type, artist, year)
    ///
    /// `["Test Shot Starfish"], ["Artist", " • ", "1660 subscribers"]` Search artist
    ///
    /// `["aespa - All Songs & MV"], ["Playlist", " • ", <"Jerwen">, " • ", "49 songs"]`
    /// Search playlist (title, creator, track count)
    ///
    /// Info: "Playlist" label is missing in the "Playlists" tab
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
    #[serde_as(as = "Option<Text>")]
    pub index: Option<String>,
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

/// Music list continuation response model
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MusicContinuation {
    pub continuation_contents: ContinuationContents,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ContinuationContents {
    #[serde(alias = "musicPlaylistShelfContinuation")]
    pub music_shelf_continuation: MusicShelf,
}

/*
#MAPPER
*/

#[derive(Debug)]
pub(crate) struct MusicListMapper {
    lang: Language,
    /// Artists list + various artists flag
    artists: Option<(Vec<ArtistId>, bool)>,
    album: Option<AlbumId>,
    artist_page: bool,
    items: Vec<MusicItem>,
    warnings: Vec<String>,
}

#[derive(Debug)]
pub(crate) struct GroupedMusicItems {
    pub tracks: Vec<TrackItem>,
    pub albums: Vec<AlbumItem>,
    pub artists: Vec<ArtistItem>,
    pub playlists: Vec<MusicPlaylistItem>,
}

impl MusicListMapper {
    pub fn new(lang: Language) -> Self {
        Self {
            lang,
            artists: None,
            album: None,
            artist_page: false,
            items: Vec::new(),
            warnings: Vec::new(),
        }
    }

    /*
    pub fn with_artists(
        lang: Language,
        artists: Vec<ArtistId>,
        by_va: bool,
        artist_page: bool,
    ) -> Self {
        Self {
            lang,
            artists: Some((artists, by_va)),
            album: None,
            artist_page,
            items: Vec::new(),
            warnings: Vec::new(),
        }
    }*/

    pub fn with_album(lang: Language, artists: Vec<ArtistId>, by_va: bool, album: AlbumId) -> Self {
        Self {
            lang,
            artists: Some((artists, by_va)),
            album: Some(album),
            artist_page: false,
            items: Vec::new(),
            warnings: Vec::new(),
        }
    }

    fn map_item(&mut self, item: MusicResponseItem) -> Result<MusicEntityType, String> {
        match item {
            MusicResponseItem::MusicResponsiveListItemRenderer(item) => {
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
                                    util::parse_large_numstr(p.first_str(), self.lang)
                                });

                                self.items.push(MusicItem::Artist(ArtistItem {
                                    id,
                                    name: title,
                                    avatar: item.thumbnail.into(),
                                    subscriber_count,
                                }));
                                Ok(MusicEntityType::Artist)
                            }
                            PageType::Album => {
                                let album_type = subtitle_p1
                                    .map(|st| map_album_type(st.first_str(), self.lang))
                                    .unwrap_or_default();

                                let (artists, by_va) = map_artists(subtitle_p2);

                                let year = subtitle_p3
                                    .and_then(|st| util::parse_numeric(st.first_str()).ok());

                                self.items.push(MusicItem::Album(AlbumItem {
                                    id,
                                    name: title,
                                    cover: item.thumbnail.into(),
                                    artists,
                                    album_type,
                                    year,
                                    by_va,
                                }));
                                Ok(MusicEntityType::Album)
                            }
                            PageType::Playlist => {
                                // Part 1 may be the "Playlist" label
                                let (channel_p, tcount_p) = match subtitle_p3 {
                                    Some(_) => (subtitle_p2, subtitle_p3),
                                    None => (subtitle_p1, subtitle_p2),
                                };

                                let from_ytm = channel_p
                                    .as_ref()
                                    .map(|p| p.first_str() == util::YT_MUSIC_NAME)
                                    .unwrap_or_default();
                                let channel = channel_p.and_then(|p| {
                                    p.0.into_iter().find_map(|c| ChannelId::try_from(c).ok())
                                });
                                let track_count =
                                    tcount_p.and_then(|p| util::parse_numeric(p.first_str()).ok());

                                self.items.push(MusicItem::Playlist(MusicPlaylistItem {
                                    id,
                                    name: title,
                                    thumbnail: item.thumbnail.into(),
                                    channel,
                                    track_count,
                                    from_ytm,
                                }));
                                Ok(MusicEntityType::Playlist)
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

                        // Videos have rectangular thumbnails, YTM tracks have square covers
                        // Exception: there are no thumbnails on album items
                        let is_video = self.album.is_none()
                            && !first_tn.map(|tn| tn.height == tn.width).unwrap_or_default();

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
                                if subtitle_parts.len() > 3 {
                                    subtitle_parts.next();
                                }
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
                            .and_then(|p| util::parse_video_length(p.first_str()))
                            .ok_or_else(|| format!("track {}: could not parse duration", id))?;

                        let (album, view_count) = match (item.flex_column_display_style, is_video) {
                            // The album field contains the view count for search videos
                            (FlexColumnDisplayStyle::TwoLines, true) => (
                                None,
                                album_p.and_then(|p| {
                                    util::parse_large_numstr(p.first_str(), self.lang)
                                }),
                            ),
                            (_, false) => (
                                album_p
                                    .and_then(|p| {
                                        p.0.into_iter().find_map(|c| AlbumId::try_from(c).ok())
                                    })
                                    .or_else(|| self.album.clone()),
                                None,
                            ),
                            (FlexColumnDisplayStyle::Default, true) => (None, None),
                        };

                        let (mut artists, _) = map_artists(artists_p);

                        // Fall back to the artist given when constructing the mapper.
                        // This is used for extracting artist pages.
                        if let Some(a) = &self.artists {
                            if artists.is_empty() {
                                artists = a.0.clone();
                            }
                        }

                        let track_nr = item.index.and_then(|txt| util::parse_numeric(&txt).ok());

                        self.items.push(MusicItem::Track(TrackItem {
                            id,
                            title,
                            duration,
                            cover: item.thumbnail.into(),
                            artists,
                            album,
                            view_count,
                            is_video,
                            track_nr,
                        }));
                        Ok(MusicEntityType::Track)
                    }
                }
            }
            MusicResponseItem::MusicTwoRowItemRenderer(item) => {
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

                        let (artists, by_va) =
                            match (subtitle_p1, subtitle_p2, &self.artists, self.artist_page) {
                                // "2022" (Artist singles)
                                (Some(year_txt), None, Some(artists), true) => {
                                    year = util::parse_numeric(year_txt.first_str()).ok();
                                    artists.clone()
                                }
                                // "Album", "2022" (Artist albums)
                                (Some(atype_txt), Some(year_txt), Some(artists), true) => {
                                    year = util::parse_numeric(year_txt.first_str()).ok();
                                    album_type = map_album_type(atype_txt.first_str(), self.lang);
                                    artists.clone()
                                }
                                // "Album", <"Oonagh"> (Album variants, new releases)
                                (Some(atype_txt), Some(p2), _, false) => {
                                    album_type = map_album_type(atype_txt.first_str(), self.lang);
                                    map_artists(Some(p2))
                                }
                                _ => {
                                    return Err(format!(
                                        "could not parse subtitle of album {}",
                                        id
                                    ));
                                }
                            };

                        self.items.push(MusicItem::Album(AlbumItem {
                            id,
                            name: item.title,
                            cover: item.thumbnail_renderer.into(),
                            artists,
                            album_type,
                            year,
                            by_va,
                        }));
                        Ok(MusicEntityType::Album)
                    }
                    PageType::Playlist => {
                        let from_ytm = subtitle_p2
                            .as_ref()
                            .map(|p| p.first_str() == util::YT_MUSIC_NAME)
                            .unwrap_or_default();
                        let channel = subtitle_p2.and_then(|p| {
                            p.0.into_iter().find_map(|c| ChannelId::try_from(c).ok())
                        });
                        let track_count =
                            subtitle_p3.and_then(|p| util::parse_numeric(p.first_str()).ok());

                        self.items.push(MusicItem::Playlist(MusicPlaylistItem {
                            id,
                            name: item.title,
                            thumbnail: item.thumbnail_renderer.into(),
                            channel,
                            track_count,
                            from_ytm,
                        }));
                        Ok(MusicEntityType::Playlist)
                    }
                    PageType::Artist => {
                        let subscriber_count = subtitle_p1
                            .and_then(|p| util::parse_large_numstr(p.first_str(), self.lang));

                        self.items.push(MusicItem::Artist(ArtistItem {
                            id,
                            name: item.title,
                            avatar: item.thumbnail_renderer.into(),
                            subscriber_count,
                        }));
                        Ok(MusicEntityType::Artist)
                    }
                    PageType::Channel => Err(format!("channel items unsupported. id: {}", id)),
                }
            }
        }
    }

    pub fn map_response(
        &mut self,
        mut res: MapResult<Vec<MusicResponseItem>>,
    ) -> Option<MusicEntityType> {
        let mut etype = None;
        self.warnings.append(&mut res.warnings);
        res.c
            .into_iter()
            .for_each(|item| match self.map_item(item) {
                Ok(t) => etype = Some(t),
                Err(e) => self.warnings.push(e),
            });
        etype
    }

    pub fn items(self) -> MapResult<Vec<MusicItem>> {
        MapResult {
            c: self.items,
            warnings: self.warnings,
        }
    }

    pub fn conv_items<T: FromYtItem>(self) -> MapResult<Vec<T>> {
        MapResult {
            c: self
                .items
                .into_iter()
                .filter_map(T::from_ytm_item)
                .collect(),
            warnings: self.warnings,
        }
    }

    pub fn group_items(self) -> MapResult<GroupedMusicItems> {
        let mut tracks = Vec::new();
        let mut albums = Vec::new();
        let mut artists = Vec::new();
        let mut playlists = Vec::new();

        for item in self.items {
            match item {
                MusicItem::Track(track) => tracks.push(track),
                MusicItem::Album(album) => albums.push(album),
                MusicItem::Artist(artist) => artists.push(artist),
                MusicItem::Playlist(playlist) => playlists.push(playlist),
            }
        }

        MapResult {
            c: GroupedMusicItems {
                tracks,
                albums,
                artists,
                playlists,
            },
            warnings: self.warnings,
        }
    }
}

pub(crate) fn map_artists(artists_p: Option<TextComponents>) -> (Vec<ArtistId>, bool) {
    let mut by_va = false;
    let artists = artists_p
        .map(|part| {
            part.0
                .into_iter()
                .enumerate()
                .filter_map(|(i, c)| {
                    let artist = ArtistId::from(c);
                    // Filter out text components with no links that are at
                    // odd positions (conjunctions)
                    if artist.id.is_none() && i % 2 == 1 {
                        None
                    } else if artist.id.is_none() && artist.name == util::VARIOUS_ARTISTS {
                        by_va = true;
                        None
                    } else {
                        Some(artist)
                    }
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    (artists, by_va)
}

pub(crate) fn map_album_type(txt: &str, lang: Language) -> AlbumType {
    dictionary::entry(lang)
        .album_types
        .get(txt.to_lowercase().trim())
        .copied()
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use std::{collections::BTreeMap, fs::File, io::BufReader, path::Path};

    use super::*;

    #[test]
    fn map_album_type_samples() {
        let json_path = Path::new("testfiles/dict/album_type_samples.json");
        let json_file = File::open(json_path).unwrap();
        let atype_samples: BTreeMap<Language, BTreeMap<AlbumType, String>> =
            serde_json::from_reader(BufReader::new(json_file)).unwrap();

        atype_samples.iter().for_each(|(lang, entry)| {
            entry.iter().for_each(|(album_type, txt)| {
                let res = map_album_type(txt, *lang);
                assert_eq!(res, *album_type, "lang: {}, txt: {}", lang, txt);
            });
        });
    }
}
