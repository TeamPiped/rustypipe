use serde::Deserialize;
use serde_with::{rust::deserialize_ignore_any, serde_as, DefaultOnError, VecSkipError};

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
    url_endpoint::{BrowseEndpointWrap, NavigationEndpoint, PageType},
    ContentsRenderer, MusicContinuationData, Thumbnails, ThumbnailsWrap,
};

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum ItemSection {
    #[serde(alias = "musicPlaylistShelfRenderer")]
    MusicShelfRenderer(MusicShelf),
    MusicCarouselShelfRenderer {
        header: Option<MusicCarouselShelfHeader>,
        #[serde_as(as = "VecLogError<_>")]
        contents: MapResult<Vec<MusicResponseItem>>,
    },
    #[serde(other, deserialize_with = "deserialize_ignore_any")]
    None,
}

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
    /// "More" button at the bottom (artist pages)
    #[serde(default)]
    #[serde_as(as = "DefaultOnError")]
    pub bottom_endpoint: Option<BrowseEndpointWrap>,
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
    /// ### Playlist track
    ///
    /// `[<"Das Beste">], [<"Silbermond">], [<"Laut Gedacht (Re-Edition)">]`
    ///
    /// (title, artist, album)
    ///
    /// ### Album track
    ///
    /// `[<"Der Himmel reißt auf">]`
    ///
    /// (title)
    ///
    /// ### Search track
    ///
    /// `[<"Girls">], ["Song", " • ", <"aespa">, " • ", <"Girls - The 2nd Mini Album">, " • ", "4:01"]`
    ///
    /// (title, artist, album, duration)
    ///
    /// Info: "Song" label is missing in the "Songs" tab
    ///
    /// ### Search video
    ///
    /// `[<"Black Mamba">], ["Video", " • ", <"aespa">, " • ", "235M views", " • ", "3:50"]`
    ///
    /// (title, artist, view count, duration)
    ///
    /// Info: "Video" label is missing in the "Videos" tab
    ///
    /// ### Search podcast episode
    ///
    /// `["Blond - Da muss man dabei..."], ["Episode", " • ", "Dec 24, 2020", " • ", <"BLOND_OFFICIAL">], ["Dec 24, 2020"]`
    ///
    /// (title, date, artist, date again?)
    ///
    /// Info: "Episode" label is missing in the "Videos" tab
    ///
    /// ### Search album
    ///
    /// `["Next Level"], ["Single", " • ", <"aespa">, " • ", "2021"]`
    ///
    /// (title, type, artist, year)
    ///
    /// ### Search artist
    ///
    /// `["Test Shot Starfish"], ["Artist", " • ", "1660 subscribers"]`
    ///
    /// (subscriber count)
    ///
    /// ### Search playlist
    ///
    /// `["aespa - All Songs & MV"], ["Playlist", " • ", <"Jerwen">, " • ", "49 songs"]`
    ///
    /// (title, creator, track count)
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
    #[serde(default)]
    pub item_height: ItemHeight,
    /// Album track number
    #[serde_as(as = "Option<Text>")]
    pub index: Option<String>,
    pub menu: Option<MusicItemMenu>,
}

#[derive(Default, Debug, Copy, Clone, Deserialize)]
pub(crate) enum FlexColumnDisplayStyle {
    #[serde(rename = "MUSIC_RESPONSIVE_LIST_ITEM_FLEX_COLUMN_DISPLAY_STYLE_TWO_LINE_STACK")]
    TwoLines,
    #[default]
    #[serde(other)]
    Default,
}

#[derive(Default, Debug, Copy, Clone, PartialEq, Eq, Deserialize)]
pub(crate) enum ItemHeight {
    #[serde(rename = "MUSIC_RESPONSIVE_LIST_ITEM_HEIGHT_MEDIUM_COMPACT")]
    Compact,
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

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PlaylistPanelRenderer {
    #[serde_as(as = "VecLogError<_>")]
    pub contents: MapResult<Vec<PlaylistPanelVideo>>,
    /// Continuation token for fetching more radio items
    #[serde(default)]
    #[serde_as(as = "VecSkipError<_>")]
    pub continuations: Vec<MusicContinuationData>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum PlaylistPanelVideo {
    PlaylistPanelVideoRenderer(QueueMusicItem),
    #[serde(other, deserialize_with = "deserialize_ignore_any")]
    None,
}

/// Music item from a playback queue (`playlistPanelVideoRenderer`)
#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct QueueMusicItem {
    pub video_id: String,
    #[serde_as(as = "Text")]
    pub title: String,
    #[serde_as(as = "Option<Text>")]
    pub length_text: Option<String>,
    /// Artist + Album + Year (for tracks)
    /// `<"IVE">, " • ", <"LOVE DIVE (LOVE DIVE)">, " • ", "2022"`
    ///
    /// Artist + view count + like count (for videos)
    /// `<"aespa">, " • ", "250M views", " • ", "3.6M likes"`
    #[serde(default)]
    pub long_byline_text: TextComponents,
    #[serde(default)]
    pub thumbnail: Thumbnails,
    pub menu: Option<MusicItemMenu>,
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

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MusicContentsRenderer<T> {
    pub contents: Vec<T>,
    /// Continuation token for fetching recommended items
    #[serde(default)]
    #[serde_as(as = "VecSkipError<_>")]
    pub continuations: Vec<MusicContinuationData>,
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
#[allow(clippy::enum_variant_names)]
pub(crate) enum ContinuationContents {
    #[serde(alias = "musicPlaylistShelfContinuation")]
    MusicShelfContinuation(MusicShelf),
    SectionListContinuation(ContentsRenderer<ItemSection>),
    PlaylistPanelContinuation(PlaylistPanelRenderer),
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MusicCarouselShelfHeader {
    pub music_carousel_shelf_basic_header_renderer: MusicCarouselShelfHeaderRenderer,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MusicCarouselShelfHeaderRenderer {
    pub more_content_button: Option<MoreContentButton>,
    #[serde(default)]
    pub title: TextComponents,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MoreContentButton {
    pub button_renderer: ButtonRenderer,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ButtonRenderer {
    pub navigation_endpoint: NavigationEndpoint,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MusicItemMenu {
    pub menu_renderer: MusicItemMenuRenderer,
}

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MusicItemMenuRenderer {
    #[serde_as(as = "VecSkipError<_>")]
    pub items: Vec<MusicItemMenuEntry>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MusicItemMenuEntry {
    pub menu_navigation_item_renderer: ButtonRenderer,
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

    /// Create a new MusicListMapper for an artist page
    pub fn with_artist(lang: Language, artist: ArtistId) -> Self {
        Self {
            lang,
            artists: Some((vec![artist], false)),
            album: None,
            artist_page: true,
            items: Vec::new(),
            warnings: Vec::new(),
        }
    }

    /// Create a new MusicListMapper for an album page
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

    fn map_item(&mut self, item: MusicResponseItem) -> Result<Option<MusicEntityType>, String> {
        match item {
            // List item
            MusicResponseItem::MusicResponsiveListItemRenderer(item) => {
                let mut columns = item.flex_columns.into_iter();
                let title = columns.next().map(|col| col.renderer.text.to_string());
                let c2 = columns.next();
                let c3 = columns.next();

                match item.navigation_endpoint {
                    // Artist / Album / Playlist
                    Some(ne) => {
                        let mut subtitle_parts = c2
                            .ok_or_else(|| "could not get subtitle".to_owned())?
                            .renderer
                            .text
                            .split(util::DOT_SEPARATOR)
                            .into_iter();

                        let (page_type, id) = match ne.music_page() {
                            Some(music_page) => music_page,
                            None => {
                                // Ignore radio items
                                if subtitle_parts.len() == 1 {
                                    return Ok(None);
                                }
                                return Err("invalid navigation endpoint".to_string());
                            }
                        };

                        let title =
                            title.ok_or_else(|| format!("track {}: could not get title", id))?;

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
                                Ok(Some(MusicEntityType::Artist))
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
                                Ok(Some(MusicEntityType::Album))
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
                                Ok(Some(MusicEntityType::Playlist))
                            }
                            PageType::Channel => {
                                // There may be broken YT channels from the artist search. They can be skipped.
                                Ok(None)
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
                            // Search result
                            FlexColumnDisplayStyle::TwoLines => {
                                // Is this a related track?
                                if !is_video && item.item_height == ItemHeight::Compact {
                                    (
                                        c2.map(TextComponents::from),
                                        c3.map(TextComponents::from),
                                        None,
                                    )
                                } else {
                                    let mut subtitle_parts = c2
                                        .ok_or_else(|| {
                                            format!("track {}: could not get subtitle", id)
                                        })?
                                        .renderer
                                        .text
                                        .split(util::DOT_SEPARATOR)
                                        .into_iter();

                                    // Is this a related video?
                                    if item.item_height == ItemHeight::Compact {
                                        (subtitle_parts.next(), subtitle_parts.next(), None)
                                    }
                                    // Is it a podcast episode?
                                    else if subtitle_parts.len() <= 3 && c3.is_some() {
                                        (subtitle_parts.rev().next(), None, None)
                                    } else {
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
                                }
                            }
                            // Playlist item
                            FlexColumnDisplayStyle::Default => {
                                let mut fixed_columns = item.fixed_columns;
                                (
                                    c2.map(TextComponents::from),
                                    c3.map(TextComponents::from),
                                    fixed_columns.try_swap_remove(0).map(TextComponents::from),
                                )
                            }
                        };

                        let duration =
                            duration_p.and_then(|p| util::parse_video_length(p.first_str()));

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

                        // Extract artist id from dropdown menu
                        let artist_id = map_artist_id(item.menu, artists.first());

                        let track_nr = item.index.and_then(|txt| util::parse_numeric(&txt).ok());

                        self.items.push(MusicItem::Track(TrackItem {
                            id,
                            title,
                            duration,
                            cover: item.thumbnail.into(),
                            artists,
                            artist_id,
                            album,
                            view_count,
                            is_video,
                            track_nr,
                        }));
                        Ok(Some(MusicEntityType::Track))
                    }
                }
            }
            // Tile
            MusicResponseItem::MusicTwoRowItemRenderer(item) => {
                let mut subtitle_parts = item.subtitle.split(util::DOT_SEPARATOR).into_iter();
                let subtitle_p1 = subtitle_parts.next();
                let subtitle_p2 = subtitle_parts.next();
                let subtitle_p3 = subtitle_parts.next();

                match item.navigation_endpoint.watch_endpoint {
                    // Music video
                    Some(wep) => {
                        let artists = map_artists(subtitle_p1).0;

                        self.items.push(MusicItem::Track(TrackItem {
                            id: wep.video_id,
                            title: item.title,
                            duration: None,
                            cover: item.thumbnail_renderer.into(),
                            artist_id: artists.first().and_then(|a| a.id.to_owned()),
                            artists,
                            album: None,
                            view_count: subtitle_p2
                                .and_then(|c| util::parse_large_numstr(c.first_str(), self.lang)),
                            is_video: true,
                            track_nr: None,
                        }));
                        Ok(Some(MusicEntityType::Track))
                    }
                    // Artist / Album / Playlist
                    None => {
                        let (page_type, id) = item
                            .navigation_endpoint
                            .music_page()
                            .ok_or_else(|| "could not get navigation endpoint".to_owned())?;

                        match page_type {
                            PageType::Album => {
                                let mut year = None;
                                let mut album_type = AlbumType::Single;

                                let (artists, by_va) = match (
                                    subtitle_p1,
                                    subtitle_p2,
                                    &self.artists,
                                    self.artist_page,
                                ) {
                                    // "2022" (Artist singles)
                                    (Some(year_txt), None, Some(artists), true) => {
                                        year = util::parse_numeric(year_txt.first_str()).ok();
                                        artists.clone()
                                    }
                                    // "Album", "2022" (Artist albums)
                                    (Some(atype_txt), Some(year_txt), Some(artists), true) => {
                                        year = util::parse_numeric(year_txt.first_str()).ok();
                                        album_type =
                                            map_album_type(atype_txt.first_str(), self.lang);
                                        artists.clone()
                                    }
                                    // "Album", <"Oonagh"> (Album variants, new releases)
                                    (Some(atype_txt), Some(p2), _, false) => {
                                        album_type =
                                            map_album_type(atype_txt.first_str(), self.lang);
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
                                Ok(Some(MusicEntityType::Album))
                            }
                            PageType::Playlist => {
                                let from_ytm = subtitle_p2
                                    .as_ref()
                                    .map(|p| p.first_str() == util::YT_MUSIC_NAME)
                                    .unwrap_or_default();
                                let channel = subtitle_p2.and_then(|p| {
                                    p.0.into_iter().find_map(|c| ChannelId::try_from(c).ok())
                                });
                                let track_count = subtitle_p3
                                    .and_then(|p| util::parse_numeric(p.first_str()).ok());

                                self.items.push(MusicItem::Playlist(MusicPlaylistItem {
                                    id,
                                    name: item.title,
                                    thumbnail: item.thumbnail_renderer.into(),
                                    channel,
                                    track_count,
                                    from_ytm,
                                }));
                                Ok(Some(MusicEntityType::Playlist))
                            }
                            PageType::Artist => {
                                let subscriber_count = subtitle_p1.and_then(|p| {
                                    util::parse_large_numstr(p.first_str(), self.lang)
                                });

                                self.items.push(MusicItem::Artist(ArtistItem {
                                    id,
                                    name: item.title,
                                    avatar: item.thumbnail_renderer.into(),
                                    subscriber_count,
                                }));
                                Ok(Some(MusicEntityType::Artist))
                            }
                            PageType::Channel => {
                                Err(format!("channel items unsupported. id: {}", id))
                            }
                        }
                    }
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
                Ok(Some(et)) => {
                    if etype.is_none() {
                        etype = Some(et);
                    }
                }
                Ok(None) => {}
                Err(e) => self.warnings.push(e),
            });
        etype
    }

    pub fn add_item(&mut self, item: MusicItem) {
        self.items.push(item);
    }

    pub fn add_warnings(&mut self, warnings: &mut Vec<String>) {
        self.warnings.append(warnings);
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

pub(crate) fn map_artist_id(
    menu: Option<MusicItemMenu>,
    fallback_artist: Option<&ArtistId>,
) -> Option<String> {
    menu.and_then(|m| {
        m.menu_renderer.items.into_iter().find_map(|i| {
            let ep = i
                .menu_navigation_item_renderer
                .navigation_endpoint
                .browse_endpoint;
            ep.and_then(|ep| {
                ep.browse_endpoint_context_supported_configs
                    .and_then(|cfg| {
                        if cfg.browse_endpoint_context_music_config.page_type == PageType::Artist {
                            Some(ep.browse_id)
                        } else {
                            None
                        }
                    })
            })
        })
    })
    .or_else(|| fallback_artist.and_then(|a| a.id.to_owned()))
}

pub(crate) fn map_album_type(txt: &str, lang: Language) -> AlbumType {
    dictionary::entry(lang)
        .album_types
        .get(txt.to_lowercase().trim())
        .copied()
        .unwrap_or_default()
}

pub(crate) fn map_queue_item(item: QueueMusicItem, lang: Language) -> TrackItem {
    let mut subtitle_parts = item.long_byline_text.split(util::DOT_SEPARATOR).into_iter();

    let is_video = !item
        .thumbnail
        .thumbnails
        .first()
        .map(|tn| tn.height == tn.width)
        .unwrap_or_default();

    let artist_p = subtitle_parts.next();
    let (artists, _) = map_artists(artist_p);
    let artist_id = map_artist_id(item.menu, artists.first());

    let subtitle_p2 = subtitle_parts.next();
    let (album, view_count) = if is_video {
        (
            None,
            subtitle_p2.and_then(|p| util::parse_large_numstr(p.first_str(), lang)),
        )
    } else {
        (
            subtitle_p2.and_then(|p| p.0.into_iter().find_map(|c| AlbumId::try_from(c).ok())),
            None,
        )
    };

    TrackItem {
        id: item.video_id,
        title: item.title,
        duration: item
            .length_text
            .and_then(|txt| util::parse_video_length(&txt)),
        cover: item.thumbnail.into(),
        artists,
        artist_id,
        album,
        view_count,
        is_video,
        track_nr: None,
    }
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
