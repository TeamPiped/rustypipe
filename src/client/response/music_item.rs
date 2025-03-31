use serde::Deserialize;
use serde_with::{rust::deserialize_ignore_any, serde_as, DefaultOnError, VecSkipError};

use crate::{
    model::{
        self, traits::FromYtItem, AlbumId, AlbumItem, AlbumType, ArtistId, ArtistItem, ChannelId,
        MusicItem, MusicItemType, MusicPlaylistItem, TrackItem, UserItem,
    },
    param::Language,
    serializer::{
        text::{Text, TextComponent, TextComponents},
        MapResult,
    },
    util::{self, dictionary, timeago},
};

use super::{
    url_endpoint::{
        BrowseEndpointWrap, MusicPage, MusicPageType, MusicVideoType, NavigationEndpoint, PageType,
    },
    ContentsRenderer, ContinuationActionWrap, ContinuationEndpoint, MusicContinuationData,
    SimpleHeaderRenderer, Thumbnails, ThumbnailsWrap,
};

#[cfg(feature = "userdata")]
use crate::model::HistoryItem;
#[cfg(feature = "userdata")]
use time::UtcOffset;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum ItemSection {
    #[serde(alias = "musicPlaylistShelfRenderer")]
    MusicShelfRenderer(MusicShelf),
    MusicCarouselShelfRenderer(MusicCarouselShelf),
    GridRenderer(GridRenderer),
    #[serde(other, deserialize_with = "deserialize_ignore_any")]
    None,
}

/// MusicShelf represents the standard, vertical list of music items
/// (used in search results, playlist, album).
#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MusicShelf {
    #[cfg(feature = "userdata")]
    #[serde_as(as = "Option<Text>")]
    pub title: Option<String>,
    /// Playlist ID (only for playlists)
    pub playlist_id: Option<String>,
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

/// MusicCarouselShelf represents a horizontal list of music items displayed with
/// large covers.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MusicCarouselShelf {
    pub header: Option<MusicCarouselShelfHeader>,
    pub contents: MapResult<Vec<MusicResponseItem>>,
}

/// MusicCardShelf is used to display the top search result. It contains
/// one main item and optionally a list of sub-items (like an artist + top tracks).
#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MusicCardShelf {
    #[serde_as(as = "Text")]
    pub title: String,
    pub on_tap: NavigationEndpoint,
    #[serde(default)]
    pub subtitle: TextComponents,
    #[serde(default)]
    pub thumbnail: MusicThumbnailRenderer,
    #[serde(default)]
    pub contents: MapResult<Vec<MusicResponseItem>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(clippy::enum_variant_names)]
pub(crate) enum MusicResponseItem {
    MusicResponsiveListItemRenderer(ListMusicItem),
    MusicTwoRowItemRenderer(CoverMusicItem),
    MessageRenderer(serde::de::IgnoredAny),
    #[serde(rename_all = "camelCase")]
    ContinuationItemRenderer {
        continuation_endpoint: ContinuationEndpoint,
    },
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
    #[serde(default)]
    pub music_item_renderer_display_policy: DisplayPolicy,
    /// Album track number
    #[serde_as(as = "Option<Text>")]
    pub index: Option<String>,
    pub menu: Option<MusicItemMenu>,
    #[serde(default)]
    #[serde_as(deserialize_as = "VecSkipError<_>")]
    pub badges: Vec<TrackBadge>,
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

#[derive(Default, Debug, Copy, Clone, PartialEq, Eq, Deserialize)]
pub(crate) enum DisplayPolicy {
    #[serde(rename = "MUSIC_ITEM_RENDERER_DISPLAY_POLICY_GREY_OUT")]
    GreyOut,
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
    /// `"Playlist", " • ", <"YouTube Music"> " • ", "53 songs"`
    ///
    /// `"Playlist", " • ", <"Vevo Playlists"> " • ", "13M views"`
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
    #[serde(default, alias = "croppedSquareThumbnailRenderer")]
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
#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MusicContinuation {
    pub continuation_contents: Option<ContinuationContents>,
    #[serde(default)]
    #[serde_as(as = "VecSkipError<_>")]
    pub on_response_received_actions: Vec<ContinuationActionWrap<MusicResponseItem>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(clippy::enum_variant_names)]
pub(crate) enum ContinuationContents {
    #[serde(alias = "musicPlaylistShelfContinuation")]
    MusicShelfContinuation(MusicShelf),
    SectionListContinuation(ContentsRenderer<ItemSection>),
    PlaylistPanelContinuation(PlaylistPanelRenderer),
    GridContinuation(GridRenderer),
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MusicCarouselShelfHeader {
    pub music_carousel_shelf_basic_header_renderer: MusicCarouselShelfHeaderRenderer,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MusicCarouselShelfHeaderRenderer {
    pub more_content_button: Option<Button>,
    #[serde(default)]
    pub title: TextComponents,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Button {
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
    pub menu_renderer: ContentsRenderer<MusicItemMenuEntry>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MusicItemMenuEntry {
    pub menu_navigation_item_renderer: ButtonRenderer,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Grid {
    pub grid_renderer: GridRenderer,
}

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GridRenderer {
    pub items: MapResult<Vec<MusicResponseItem>>,
    pub header: Option<GridHeader>,
    #[serde(default)]
    #[serde_as(as = "VecSkipError<_>")]
    pub continuations: Vec<MusicContinuationData>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GridHeader {
    pub grid_header_renderer: SimpleHeaderRenderer,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SingleColumnBrowseResult<T> {
    pub single_column_browse_results_renderer: ContentsRenderer<T>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SimpleHeader {
    pub music_header_renderer: SimpleHeaderRenderer,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum TrackBadge {
    LiveBadgeRenderer {},
}

#[serde_as]
#[derive(Default, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MusicMicroformat {
    #[serde_as(as = "DefaultOnError")]
    pub microformat_data_renderer: MicroformatData,
}

#[derive(Default, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MicroformatData {
    pub url_canonical: Option<String>,
    #[serde(default)]
    pub noindex: bool,
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
    /// Default album type in case an album is unlabeled
    pub album_type: AlbumType,
    artist_page: bool,
    search_suggestion: bool,
    items: Vec<MusicItem>,
    warnings: Vec<String>,
    pub ctoken: Option<String>,
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
            album_type: AlbumType::Single,
            artist_page: false,
            search_suggestion: false,
            items: Vec::new(),
            warnings: Vec::new(),
            ctoken: None,
        }
    }

    pub fn new_search_suggest(lang: Language) -> Self {
        Self {
            lang,
            artists: None,
            album: None,
            album_type: AlbumType::Single,
            artist_page: false,
            search_suggestion: true,
            items: Vec::new(),
            warnings: Vec::new(),
            ctoken: None,
        }
    }

    /// Create a new MusicListMapper for an artist page
    pub fn with_artist(lang: Language, artist: ArtistId) -> Self {
        Self {
            lang,
            artists: Some((vec![artist], false)),
            album: None,
            album_type: AlbumType::Single,
            artist_page: true,
            search_suggestion: false,
            items: Vec::new(),
            warnings: Vec::new(),
            ctoken: None,
        }
    }

    /// Create a new MusicListMapper for an album page
    pub fn with_album(lang: Language, artists: Vec<ArtistId>, by_va: bool, album: AlbumId) -> Self {
        Self {
            lang,
            artists: Some((artists, by_va)),
            album: Some(album),
            album_type: AlbumType::Single,
            artist_page: false,
            search_suggestion: false,
            items: Vec::new(),
            warnings: Vec::new(),
            ctoken: None,
        }
    }

    /// Map a MusicResponseItem (list item or tile)
    fn map_item(&mut self, item: MusicResponseItem) -> Result<Option<MusicItemType>, String> {
        match item {
            // List item
            MusicResponseItem::MusicResponsiveListItemRenderer(item) => self.map_list_item(item),
            // Tile
            MusicResponseItem::MusicTwoRowItemRenderer(item) => self.map_tile(item),
            MusicResponseItem::MessageRenderer(_) => Ok(None),
            MusicResponseItem::ContinuationItemRenderer {
                continuation_endpoint,
            } => {
                if self.ctoken.is_none() {
                    self.ctoken = continuation_endpoint.into_token();
                }
                Ok(None)
            }
        }
    }

    pub fn map_response(
        &mut self,
        mut res: MapResult<Vec<MusicResponseItem>>,
    ) -> Option<MusicItemType> {
        let mut etype = None;
        self.warnings.append(&mut res.warnings);
        res.c.into_iter().for_each(|item| {
            if let Some(et) = self.add_response_item(item) {
                if etype.is_none() {
                    etype = Some(et);
                }
            }
        });
        etype
    }

    /// Map a ListMusicItem (album/playlist item, search result)
    fn map_list_item(&mut self, item: ListMusicItem) -> Result<Option<MusicItemType>, String> {
        let mut columns = item.flex_columns.into_iter();
        let c1 = columns.next();
        let c2 = columns.next();
        let c3 = columns.next();
        let c4 = columns.next();

        let title = c1.as_ref().map(|col| col.renderer.text.to_string());

        let first_tn = item
            .thumbnail
            .music_thumbnail_renderer
            .thumbnail
            .thumbnails
            .first();

        let music_page = item
            .navigation_endpoint
            .and_then(NavigationEndpoint::music_page)
            .or_else(|| {
                c1.and_then(|c1| {
                    c1.renderer
                        .text
                        .0
                        .into_iter()
                        .next()
                        .and_then(TextComponent::music_page)
                })
            })
            .or_else(|| {
                item.playlist_item_data.map(|d| MusicPage {
                    id: d.video_id,
                    typ: MusicPageType::Track {
                        vtype: MusicVideoType::from_is_video(
                            self.album.is_none()
                                && !first_tn.map(|tn| tn.height == tn.width).unwrap_or_default(),
                        ),
                    },
                })
            })
            .or_else(|| {
                first_tn.and_then(|tn| {
                    util::video_id_from_thumbnail_url(&tn.url).map(|id| MusicPage {
                        id,
                        typ: MusicPageType::Track {
                            vtype: MusicVideoType::from_is_video(
                                self.album.is_none() && tn.width != tn.height,
                            ),
                        },
                    })
                })
            });

        match music_page.map(|mp| (mp.typ, mp.id)) {
            // Track
            Some((MusicPageType::Track { vtype }, id)) => {
                let title = title.ok_or_else(|| format!("track {id}: could not get title"))?;

                #[derive(Default)]
                struct Parsed {
                    artists: Option<TextComponents>,
                    album: Option<TextComponents>,
                    duration: Option<TextComponents>,
                    view_count: Option<TextComponents>,
                }

                // Dont map music livestreams
                if item
                    .badges
                    .iter()
                    .any(|b| matches!(b, TrackBadge::LiveBadgeRenderer {}))
                {
                    return Ok(None);
                }

                let p = match item.flex_column_display_style {
                    // Search result
                    FlexColumnDisplayStyle::TwoLines => {
                        // Is this a related track (from the "similar titles" tab in the player)?
                        if vtype != MusicVideoType::Video && item.item_height == ItemHeight::Compact
                        {
                            Parsed {
                                artists: c2.map(TextComponents::from),
                                album: c3.map(TextComponents::from),
                                ..Default::default()
                            }
                        } else {
                            let mut subtitle_parts = c2
                                .ok_or_else(|| format!("track {id}: could not get subtitle"))?
                                .renderer
                                .text
                                .split(util::DOT_SEPARATOR)
                                .into_iter();

                            // Is this a related video?
                            if item.item_height == ItemHeight::Compact {
                                Parsed {
                                    artists: subtitle_parts.next(),
                                    view_count: subtitle_parts.next(),
                                    ..Default::default()
                                }
                            }
                            // Is this an item from search suggestion?
                            else if self.search_suggestion {
                                // Skip first part (track type)
                                subtitle_parts.next();
                                Parsed {
                                    artists: subtitle_parts.next(),
                                    album: c3.map(TextComponents::from),
                                    view_count: subtitle_parts.next(),
                                    ..Default::default()
                                }
                            }
                            // Is it a podcast episode?
                            else if vtype == MusicVideoType::Episode {
                                Parsed {
                                    artists: subtitle_parts.next_back(),
                                    ..Default::default()
                                }
                            } else {
                                // Skip first part (track type)
                                if subtitle_parts.len() > 3
                                    || (vtype == MusicVideoType::Video && subtitle_parts.len() == 2)
                                {
                                    subtitle_parts.next();
                                }

                                match vtype {
                                    MusicVideoType::Video => Parsed {
                                        artists: subtitle_parts.next(),
                                        view_count: subtitle_parts.next(),
                                        duration: subtitle_parts.next(),
                                        ..Default::default()
                                    },
                                    _ => Parsed {
                                        artists: subtitle_parts.next(),
                                        album: subtitle_parts.next(),
                                        duration: subtitle_parts.next(),
                                        view_count: c3.map(TextComponents::from),
                                    },
                                }
                            }
                        }
                    }
                    // Playlist item
                    FlexColumnDisplayStyle::Default => {
                        let artists = c2.map(TextComponents::from);
                        let duration = item
                            .fixed_columns
                            .into_iter()
                            .next()
                            .map(TextComponents::from);
                        if self.album.is_some() {
                            Parsed {
                                artists,
                                view_count: c3.map(TextComponents::from),
                                duration,
                                ..Default::default()
                            }
                        } else if self.artist_page && c4.is_some() {
                            Parsed {
                                artists,
                                view_count: c3.map(TextComponents::from),
                                album: c4.map(TextComponents::from),
                                duration,
                            }
                        } else {
                            Parsed {
                                artists,
                                album: c3.map(TextComponents::from),
                                duration,
                                ..Default::default()
                            }
                        }
                    }
                };

                let duration = p
                    .duration
                    .and_then(|p| util::parse_video_length(p.first_str()));
                let album = p
                    .album
                    .and_then(|p| p.0.into_iter().find_map(|c| AlbumId::try_from(c).ok()))
                    .or_else(|| self.album.clone());
                let view_count = p.view_count.and_then(|p| {
                    util::parse_large_numstr_or_warn(p.first_str(), self.lang, &mut self.warnings)
                });
                let (mut artists, by_va) = map_artists(p.artists);

                // Extract artist id from dropdown menu
                let artist_id = map_artist_id_fallback(item.menu, artists.first());

                // Fall back to the artist given when constructing the mapper.
                // This is used for extracting artist pages.
                // On some albums, the artist name of the tracks is not given but different
                // from the album artist. In this case dont copy the album artist.
                if let Some((fb_artists, _)) = &self.artists {
                    if artists.is_empty()
                        && (self.artist_page
                            || artist_id.is_none()
                            || fb_artists.iter().any(|fb_id| {
                                fb_id
                                    .id
                                    .as_deref()
                                    .map(|aid| artist_id.as_deref() == Some(aid))
                                    .unwrap_or_default()
                            }))
                    {
                        artists.clone_from(fb_artists);
                    }
                }

                let track_nr = item.index.and_then(|txt| util::parse_numeric(&txt).ok());

                self.items.push(MusicItem::Track(TrackItem {
                    id,
                    name: title,
                    duration,
                    cover: item.thumbnail.into(),
                    artists,
                    artist_id,
                    album,
                    view_count,
                    track_type: vtype.into(),
                    track_nr,
                    by_va,
                }));
                Ok(Some(MusicItemType::Track))
            }
            // Artist / Album / Playlist
            Some((page_type, id)) => {
                // Ignore "Shuffle all" button and builtin "Liked music" and "Saved episodes" playlists
                if page_type == MusicPageType::None
                    || (page_type == (MusicPageType::Playlist { is_podcast: false })
                        && matches!(id.as_str(), "MLCT" | "LM" | "SE"))
                {
                    return Ok(None);
                }

                let mut subtitle_parts = c2
                    .ok_or_else(|| format!("{id}: could not get subtitle"))?
                    .renderer
                    .text
                    .split(util::DOT_SEPARATOR)
                    .into_iter();

                let title = title.ok_or_else(|| format!("track {id}: could not get title"))?;

                let subtitle_p1 = subtitle_parts.next();
                let subtitle_p2 = subtitle_parts.next();
                let subtitle_p3 = subtitle_parts.next();

                match page_type {
                    MusicPageType::Artist => {
                        let subscriber_count = subtitle_p2.and_then(|p| {
                            util::parse_large_numstr_or_warn(
                                p.first_str(),
                                self.lang,
                                &mut self.warnings,
                            )
                        });

                        self.items.push(MusicItem::Artist(ArtistItem {
                            id,
                            name: title,
                            avatar: item.thumbnail.into(),
                            subscriber_count,
                        }));
                        Ok(Some(MusicItemType::Artist))
                    }
                    MusicPageType::Album => {
                        let album_type = subtitle_p1
                            .map(|st| map_album_type(st.first_str(), self.lang))
                            .unwrap_or_default();

                        let (mut artists, by_va) = map_artists(subtitle_p2);
                        let artist_id = map_artist_id_fallback(item.menu, artists.first());

                        // Album artist links may be invisible on the search page, so
                        // fall back to menu data
                        if let Some(a1) = artists.first_mut() {
                            if a1.id.is_none() {
                                a1.id.clone_from(&artist_id);
                            }
                        }

                        let year =
                            subtitle_p3.and_then(|st| util::parse_numeric(st.first_str()).ok());

                        self.items.push(MusicItem::Album(AlbumItem {
                            id,
                            name: title,
                            cover: item.thumbnail.into(),
                            artists,
                            artist_id,
                            album_type,
                            year,
                            by_va,
                        }));
                        Ok(Some(MusicItemType::Album))
                    }
                    MusicPageType::Playlist { is_podcast } => {
                        // Part 1 may be the "Playlist" label
                        let (channel_p, tcount_p) = match subtitle_p3 {
                            Some(_) => (subtitle_p2, subtitle_p3),
                            None => (subtitle_p1, subtitle_p2),
                        };

                        let from_ytm = channel_p
                            .as_ref()
                            .and_then(|p| p.0.first())
                            .map(util::is_ytm)
                            .unwrap_or_default();
                        let channel = channel_p.and_then(|p| {
                            p.0.into_iter().find_map(|c| ChannelId::try_from(c).ok())
                        });
                        let track_count = tcount_p
                            .filter(|_| from_ytm)
                            .and_then(|p| util::parse_numeric(p.first_str()).ok());

                        self.items.push(MusicItem::Playlist(MusicPlaylistItem {
                            id,
                            name: title,
                            thumbnail: item.thumbnail.into(),
                            channel,
                            track_count,
                            from_ytm,
                            is_podcast,
                        }));
                        Ok(Some(MusicItemType::Playlist))
                    }
                    MusicPageType::User => {
                        // Part 1 may be the "Profile" label
                        let handle = map_channel_handle(subtitle_p2.as_ref())
                            .or_else(|| map_channel_handle(subtitle_p1.as_ref()));

                        self.items.push(MusicItem::User(UserItem {
                            id,
                            name: title,
                            handle,
                            avatar: item.thumbnail.into(),
                        }));
                        Ok(Some(MusicItemType::User))
                    }
                    MusicPageType::None => {
                        // There may be broken YT channels from the artist search. They can be skipped.
                        Ok(None)
                    }
                    // Tracks were already handled above
                    MusicPageType::Track { .. } => unreachable!(),
                }
            }
            None => {
                if item.music_item_renderer_display_policy == DisplayPolicy::GreyOut {
                    Ok(None)
                } else {
                    Err("could not determine item type".to_owned())
                }
            }
        }
    }

    /// Map a CoverMusicItem (album/playlist tile)
    fn map_tile(&mut self, item: CoverMusicItem) -> Result<Option<MusicItemType>, String> {
        let mut subtitle_parts = item.subtitle.split(util::DOT_SEPARATOR).into_iter();
        let subtitle_p1 = subtitle_parts.next();
        let subtitle_p2 = subtitle_parts.next();

        match item.navigation_endpoint.music_page() {
            Some(music_page) => match music_page.typ {
                MusicPageType::Track { vtype } => {
                    let (artists, by_va, view_count, duration) = if vtype == MusicVideoType::Episode
                    {
                        let (artists, by_va) = map_artists(subtitle_p2);
                        let duration = subtitle_p1.and_then(|s| {
                            timeago::parse_video_duration_or_warn(
                                self.lang,
                                s.first_str(),
                                &mut self.warnings,
                            )
                        });
                        (artists, by_va, None, duration)
                    } else {
                        let (artists, by_va) = map_artists(subtitle_p1);
                        let view_count = subtitle_p2.and_then(|c| {
                            util::parse_large_numstr_or_warn(
                                c.first_str(),
                                self.lang,
                                &mut self.warnings,
                            )
                        });
                        (artists, by_va, view_count, None)
                    };

                    self.items.push(MusicItem::Track(TrackItem {
                        id: music_page.id,
                        name: item.title,
                        duration,
                        cover: item.thumbnail_renderer.into(),
                        artist_id: artists.first().and_then(|a| a.id.clone()),
                        artists,
                        album: None,
                        view_count,
                        track_type: vtype.into(),
                        track_nr: None,
                        by_va,
                    }));
                    Ok(Some(MusicItemType::Track))
                }
                MusicPageType::Artist => {
                    let subscriber_count = subtitle_p1.and_then(|p| {
                        util::parse_large_numstr_or_warn(
                            p.first_str(),
                            self.lang,
                            &mut self.warnings,
                        )
                    });

                    self.items.push(MusicItem::Artist(ArtistItem {
                        id: music_page.id,
                        name: item.title,
                        avatar: item.thumbnail_renderer.into(),
                        subscriber_count,
                    }));
                    Ok(Some(MusicItemType::Artist))
                }
                MusicPageType::Album => {
                    let mut year = None;
                    let mut album_type = self.album_type;

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
                            // Album on artist page with unknown year
                            (None, None, Some(artists), true) => artists.clone(),
                            // "Album", <"Oonagh"> (Album variants, new releases)
                            (Some(atype_txt), Some(p2), _, false) => {
                                album_type = map_album_type(atype_txt.first_str(), self.lang);
                                map_artists(Some(p2))
                            }
                            // "Album" (Album variants, no artist)
                            (Some(atype_txt), None, _, false) => {
                                album_type = map_album_type(atype_txt.first_str(), self.lang);
                                (Vec::new(), true)
                            }
                            _ => {
                                return Err(format!(
                                    "could not parse subtitle of album {}",
                                    music_page.id
                                ));
                            }
                        };

                    self.items.push(MusicItem::Album(AlbumItem {
                        id: music_page.id,
                        name: item.title,
                        cover: item.thumbnail_renderer.into(),
                        artist_id: artists.first().and_then(|a| a.id.clone()),
                        artists,
                        album_type,
                        year,
                        by_va,
                    }));
                    Ok(Some(MusicItemType::Album))
                }
                MusicPageType::Playlist { is_podcast } => {
                    // When the playlist subtitle has only 1 part, it is a playlist from YT Music
                    // (featured on the startpage or in genres)
                    let from_ytm = subtitle_p2
                        .as_ref()
                        .and_then(|p| p.0.first())
                        .map_or(true, util::is_ytm);
                    let channel = subtitle_p2
                        .and_then(|p| p.0.into_iter().find_map(|c| ChannelId::try_from(c).ok()));

                    self.items.push(MusicItem::Playlist(MusicPlaylistItem {
                        id: music_page.id,
                        name: item.title,
                        thumbnail: item.thumbnail_renderer.into(),
                        channel,
                        track_count: None,
                        from_ytm,
                        is_podcast,
                    }));
                    Ok(Some(MusicItemType::Playlist))
                }
                MusicPageType::None | MusicPageType::User => Ok(None),
            },
            None => Err("could not determine item type".to_owned()),
        }
    }

    /// Map a MusicCardShelf (used for the top search result)
    pub fn map_card(&mut self, card: MusicCardShelf) -> Option<MusicItemType> {
        /*
        "Artist" " • " "<subscriber count>"
        "Album" " • " "<artist>"
        "Song" " • " "<artist>" " • " "<album>" " • " "<duration>"
        "Video" " • " "<artist>" " • " "<view count>" " • " "<duration>"
        "Playlist" " • " "<author>" " • " "<track count>" (guessed)
        */
        let mut subtitle_parts = card.subtitle.split(util::DOT_SEPARATOR).into_iter();
        let subtitle_p1 = subtitle_parts.next();
        let subtitle_p2 = subtitle_parts.next();
        let subtitle_p3 = subtitle_parts.next();
        let subtitle_p4 = subtitle_parts.next();

        let item_type = match card.on_tap.music_page() {
            Some(music_page) => match music_page.typ {
                MusicPageType::Artist => {
                    let subscriber_count = subtitle_p2.and_then(|p| {
                        util::parse_large_numstr_or_warn(
                            p.first_str(),
                            self.lang,
                            &mut self.warnings,
                        )
                    });

                    self.items.push(MusicItem::Artist(ArtistItem {
                        id: music_page.id,
                        name: card.title,
                        avatar: card.thumbnail.into(),
                        subscriber_count,
                    }));
                    Some(MusicItemType::Artist)
                }
                MusicPageType::Album => {
                    let (artists, by_va) = map_artists(subtitle_p2);
                    let album_type = subtitle_p1
                        .map(|p| map_album_type(p.first_str(), self.lang))
                        .unwrap_or_default();

                    self.items.push(MusicItem::Album(AlbumItem {
                        id: music_page.id,
                        name: card.title,
                        cover: card.thumbnail.into(),
                        artist_id: artists.first().and_then(|a| a.id.clone()),
                        artists,
                        album_type,
                        year: subtitle_p3.and_then(|y| util::parse_numeric(y.first_str()).ok()),
                        by_va,
                    }));
                    Some(MusicItemType::Album)
                }
                MusicPageType::Track { vtype } => {
                    if vtype == MusicVideoType::Episode {
                        let (artists, by_va) = map_artists(subtitle_p3);

                        self.items.push(MusicItem::Track(TrackItem {
                            id: music_page.id,
                            name: card.title,
                            duration: None,
                            cover: card.thumbnail.into(),
                            artist_id: artists.first().and_then(|a| a.id.clone()),
                            artists,
                            album: None,
                            view_count: None,
                            track_type: vtype.into(),
                            track_nr: None,
                            by_va,
                        }));
                    } else {
                        let (artists, by_va) = map_artists(subtitle_p2);
                        let duration =
                            subtitle_p4.and_then(|p| util::parse_video_length(p.first_str()));
                        let (album, view_count) = if vtype.is_video() {
                            (
                                None,
                                subtitle_p3.and_then(|p| {
                                    util::parse_large_numstr_or_warn(
                                        p.first_str(),
                                        self.lang,
                                        &mut self.warnings,
                                    )
                                }),
                            )
                        } else {
                            (
                                subtitle_p3.and_then(|p| {
                                    p.0.into_iter().find_map(|c| AlbumId::try_from(c).ok())
                                }),
                                None,
                            )
                        };

                        self.items.push(MusicItem::Track(TrackItem {
                            id: music_page.id,
                            name: card.title,
                            duration,
                            cover: card.thumbnail.into(),
                            artist_id: artists.first().and_then(|a| a.id.clone()),
                            artists,
                            album,
                            view_count,
                            track_type: vtype.into(),
                            track_nr: None,
                            by_va,
                        }));
                    }
                    Some(MusicItemType::Track)
                }
                MusicPageType::Playlist { is_podcast } => {
                    let from_ytm = subtitle_p2
                        .as_ref()
                        .and_then(|p| p.0.first())
                        .map_or(true, util::is_ytm);
                    let channel = subtitle_p2
                        .and_then(|p| p.0.into_iter().find_map(|c| ChannelId::try_from(c).ok()));
                    let track_count =
                        subtitle_p3.and_then(|p| util::parse_numeric(p.first_str()).ok());

                    self.items.push(MusicItem::Playlist(MusicPlaylistItem {
                        id: music_page.id,
                        name: card.title,
                        thumbnail: card.thumbnail.into(),
                        channel,
                        track_count,
                        from_ytm,
                        is_podcast,
                    }));
                    Some(MusicItemType::Playlist)
                }
                MusicPageType::User => {
                    // Part 1 may be the "Profile" label
                    let handle = map_channel_handle(subtitle_p2.as_ref())
                        .or_else(|| map_channel_handle(subtitle_p1.as_ref()));

                    self.items.push(MusicItem::User(UserItem {
                        id: music_page.id,
                        name: card.title,
                        handle,
                        avatar: card.thumbnail.into(),
                    }));
                    Some(MusicItemType::User)
                }
                MusicPageType::None => None,
            },
            None => {
                self.warnings
                    .push("could not determine item type".to_owned());
                None
            }
        };

        self.map_response(card.contents);

        item_type
    }

    pub fn add_item(&mut self, item: MusicItem) {
        self.items.push(item);
    }

    pub fn add_response_item(&mut self, item: MusicResponseItem) -> Option<MusicItemType> {
        match self.map_item(item) {
            Ok(et) => et,
            Err(e) => {
                self.warnings.push(e);
                None
            }
        }
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
                MusicItem::User(_) => {}
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

    #[cfg(feature = "userdata")]
    pub fn conv_history_items(
        self,
        date_txt: Option<String>,
        utc_offset: UtcOffset,
        res: &mut MapResult<Vec<HistoryItem<TrackItem>>>,
    ) {
        res.warnings.extend(self.warnings);
        res.c.extend(
            self.items
                .into_iter()
                .filter_map(TrackItem::from_ytm_item)
                .map(|item| HistoryItem {
                    item,
                    playback_date: date_txt.as_deref().and_then(|s| {
                        timeago::parse_textual_date_to_d(
                            self.lang,
                            utc_offset,
                            s,
                            &mut res.warnings,
                        )
                    }),
                    playback_date_txt: date_txt.clone(),
                }),
        );
    }
}

/// Map TextComponents containing artist names to a list of artists and a 'Various Artists' flag
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

fn map_artist_id_fallback(
    menu: Option<MusicItemMenu>,
    fallback_artist: Option<&ArtistId>,
) -> Option<String> {
    menu.and_then(|m| map_artist_id(m.menu_renderer.contents))
        .or_else(|| fallback_artist.and_then(|a| a.id.clone()))
}

fn map_channel_handle(st: Option<&TextComponents>) -> Option<String> {
    st.map(|t| t.first_str())
        .filter(|t| t.starts_with('@'))
        .map(str::to_owned)
}

pub(crate) fn map_artist_id(entries: Vec<MusicItemMenuEntry>) -> Option<String> {
    entries.into_iter().find_map(|i| {
        if let NavigationEndpoint::Browse {
            browse_endpoint, ..
        } = i.menu_navigation_item_renderer.navigation_endpoint
        {
            browse_endpoint
                .browse_endpoint_context_supported_configs
                .and_then(|cfg| {
                    if cfg.browse_endpoint_context_music_config.page_type == PageType::Artist {
                        Some(browse_endpoint.browse_id)
                    } else {
                        None
                    }
                })
        } else {
            None
        }
    })
}

pub(crate) fn map_album_type(txt: &str, lang: Language) -> AlbumType {
    dictionary::entry(lang)
        .album_types
        .get(txt.to_lowercase().trim())
        .copied()
        .unwrap_or_default()
}

pub(crate) fn map_queue_item(item: QueueMusicItem, lang: Language) -> MapResult<TrackItem> {
    let mut warnings = Vec::new();
    let mut subtitle_parts = item.long_byline_text.split(util::DOT_SEPARATOR).into_iter();

    let is_video = !item
        .thumbnail
        .thumbnails
        .first()
        .map(|tn| tn.height == tn.width)
        .unwrap_or_default();

    let artist_p = subtitle_parts.next();
    let (artists, by_va) = map_artists(artist_p);
    let artist_id = map_artist_id_fallback(item.menu, artists.first());

    let subtitle_p2 = subtitle_parts.next();
    let (album, view_count) = if is_video {
        (
            None,
            subtitle_p2
                .and_then(|p| util::parse_large_numstr_or_warn(p.first_str(), lang, &mut warnings)),
        )
    } else {
        (
            subtitle_p2.and_then(|p| p.0.into_iter().find_map(|c| AlbumId::try_from(c).ok())),
            None,
        )
    };

    MapResult {
        c: TrackItem {
            id: item.video_id,
            name: item.title,
            duration: item
                .length_text
                .and_then(|txt| util::parse_video_length(&txt)),
            cover: item.thumbnail.into(),
            artists,
            artist_id,
            album,
            view_count,
            track_type: MusicVideoType::from_is_video(is_video).into(),
            track_nr: None,
            by_va,
        },
        warnings,
    }
}

#[cfg(test)]
mod tests {
    use std::{collections::BTreeMap, fs::File, io::BufReader};

    use path_macro::path;

    use super::*;
    use crate::util::tests::TESTFILES;

    #[test]
    fn map_album_type_samples() {
        let json_path = path!(*TESTFILES / "dict" / "album_type_samples.json");
        let json_file = File::open(json_path).unwrap();
        let atype_samples: BTreeMap<Language, BTreeMap<String, String>> =
            serde_json::from_reader(BufReader::new(json_file)).unwrap();

        for (lang, entry) in &atype_samples {
            for (album_type_str, txt) in entry {
                let album_type_n = album_type_str.split('_').next().unwrap();
                let album_type = serde_plain::from_str::<AlbumType>(album_type_n).unwrap();
                let res = map_album_type(txt, *lang);
                assert_eq!(
                    res, album_type,
                    "{album_type_str}: lang: {lang}, txt: {txt}"
                );
            }
        }
    }
}
