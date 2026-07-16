use serde::{Deserialize, Deserializer};
use serde_with::{serde_as, DefaultOnError, VecSkipError};

use crate::{
    json::{
        json_null, value_from_json_value_owned, value_to_json_string, yt_continuation_value, ytq,
        JsonDoc, JsonNode, JsonValue,
    },
    model::{
        self, traits::FromYtItem, AlbumId, AlbumItem, AlbumType, ArtistId, ArtistItem, ChannelId,
        MusicItem, MusicItemType, MusicPlaylistItem, TrackItem,
    },
    param::Language,
    serializer::{
        text::{Text, TextComponent, TextComponents},
        MapResult,
    },
    util::{self, dictionary, timeago},
    yt_string_enum,
    FromYtNode,
};

use super::{
    url_endpoint::{self, MusicPage, MusicPageType, MusicVideoType, PageType},
    MusicContinuationData, SimpleHeaderRenderer, Thumbnails,
};

fn continuation_token(endpoint: &JsonValue) -> Option<String> {
    yt_continuation_value(endpoint)
}

#[cfg(feature = "userdata")]
use crate::model::HistoryItem;
#[cfg(feature = "userdata")]
use time::UtcOffset;

pub(crate) fn music_shelf_node<'a>(section: &JsonNode<'a>) -> Option<JsonNode<'a>> {
    section.query(ytq!(.musicShelfRenderer || .musicPlaylistShelfRenderer))
}

pub(crate) fn music_carousel_node<'a>(section: &JsonNode<'a>) -> Option<JsonNode<'a>> {
    section.query(ytq!(.musicCarouselShelfRenderer))
}

pub(crate) fn music_grid_node<'a>(section: &JsonNode<'a>) -> Option<JsonNode<'a>> {
    section.query(ytq!(.gridRenderer))
}

pub(crate) fn music_shelf_from_value(value: &JsonValue) -> Option<MusicShelf> {
    value
        .get("musicShelfRenderer")
        .or_else(|| value.get("musicPlaylistShelfRenderer"))
        .cloned()
        .and_then(value_from_json_value_owned)
}

pub(crate) fn music_carousel_from_value(value: &JsonValue) -> Option<MusicCarouselShelf> {
    value
        .get("musicCarouselShelfRenderer")
        .cloned()
        .and_then(value_from_json_value_owned)
}

pub(crate) fn music_grid_items<'a>(grid: &JsonNode<'a>) -> Option<JsonNode<'a>> {
    grid.query(ytq!(.items || .contents))
}

pub(crate) fn music_item_contents<'a>(node: &JsonNode<'a>) -> Option<JsonNode<'a>> {
    node.query(ytq!(.contents || .items))
}

pub(crate) fn music_shelf_continuation_node<'a>(
    continuation_contents: &JsonNode<'a>,
) -> Option<JsonNode<'a>> {
    continuation_contents
        .query(ytq!(.musicShelfContinuation || .musicPlaylistShelfContinuation))
}

pub(crate) fn music_section_list_continuation_node<'a>(
    continuation_contents: &JsonNode<'a>,
) -> Option<JsonNode<'a>> {
    continuation_contents.query(ytq!(.sectionListContinuation))
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
    pub contents: JsonValue,
    /// Continuation token for fetching more (>100) playlist items
    #[serde(default)]
    #[serde_as(as = "VecSkipError<_>")]
    pub continuations: Vec<MusicContinuationData>,
    /// "More" button at the bottom (artist pages)
    #[serde(default)]
    #[serde_as(as = "DefaultOnError")]
    pub bottom_endpoint: Option<JsonValue>,
}

/// MusicCarouselShelf represents a horizontal list of music items displayed with
/// large covers.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MusicCarouselShelf {
    #[serde(default)]
    #[serde(deserialize_with = "deserialize_music_carousel_header")]
    pub header: Option<MusicCarouselShelfHeaderRenderer>,
    pub contents: JsonValue,
}

fn deserialize_music_carousel_header<'de, D>(
    deserializer: D,
) -> Result<Option<MusicCarouselShelfHeaderRenderer>, D::Error>
where
    D: Deserializer<'de>,
{
    let value = Option::<JsonValue>::deserialize(deserializer)?;
    Ok(value.and_then(|value| {
        value
            .get("musicCarouselShelfBasicHeaderRenderer")
            .cloned()
            .or(Some(value))
            .and_then(value_from_json_value_owned)
    }))
}

/// MusicCardShelf is used to display the top search result. It contains
/// one main item and optionally a list of sub-items (like an artist + top tracks).
#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MusicCardShelf {
    #[serde_as(as = "Text")]
    pub title: String,
    pub on_tap: JsonValue,
    #[serde(default)]
    pub subtitle: TextComponents,
    #[serde(default)]
    pub thumbnail: MusicThumbnailRenderer,
    #[serde(default = "json_null")]
    pub contents: JsonValue,
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
    pub navigation_endpoint: Option<JsonValue>,
    #[serde(default)]
    pub flex_column_display_style: FlexColumnDisplayStyle,
    #[serde(default)]
    pub item_height: ItemHeight,
    #[serde(default)]
    pub music_item_renderer_display_policy: DisplayPolicy,
    /// Album track number
    #[serde_as(as = "Option<Text>")]
    pub index: Option<String>,
    pub menu: Option<JsonValue>,
    #[serde(default)]
    #[serde_as(deserialize_as = "VecSkipError<_>")]
    pub badges: Vec<TrackBadge>,
}

yt_string_enum! {
    pub(crate) enum FlexColumnDisplayStyle {
        TwoLines = "MUSIC_RESPONSIVE_LIST_ITEM_FLEX_COLUMN_DISPLAY_STYLE_TWO_LINE_STACK",
        Default = "",
    }
    default: FlexColumnDisplayStyle::Default,
    fallback_to_default
}

yt_string_enum! {
    pub(crate) enum ItemHeight {
        Compact = "MUSIC_RESPONSIVE_LIST_ITEM_HEIGHT_MEDIUM_COMPACT",
        Default = "",
    }
    default: ItemHeight::Default,
    fallback_to_default
}

yt_string_enum! {
    pub(crate) enum DisplayPolicy {
        GreyOut = "MUSIC_ITEM_RENDERER_DISPLAY_POLICY_GREY_OUT",
        Default = "",
    }
    default: DisplayPolicy::Default,
    fallback_to_default
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
    pub navigation_endpoint: JsonValue,
}

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PlaylistPanelRenderer {
    pub contents: Vec<JsonValue>,
    /// Continuation token for fetching more radio items
    #[serde(default)]
    #[serde_as(as = "VecSkipError<_>")]
    #[allow(dead_code)]
    pub continuations: Vec<MusicContinuationData>,
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
    pub menu: Option<JsonValue>,
}

#[derive(Default, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MusicThumbnailRenderer {
    #[serde(default, alias = "croppedSquareThumbnailRenderer")]
    #[serde(deserialize_with = "deserialize_music_thumbnail")]
    pub music_thumbnail_renderer: Thumbnails,
}

fn deserialize_music_thumbnail<'de, D>(deserializer: D) -> Result<Thumbnails, D::Error>
where
    D: Deserializer<'de>,
{
    let value = JsonValue::deserialize(deserializer)?;
    Ok(value
        .get("thumbnail")
        .cloned()
        .and_then(value_from_json_value_owned)
        .unwrap_or_default())
}

#[derive(Debug, FromYtNode)]
pub(crate) struct PlaylistItemData {
    pub video_id: String,
}

#[derive(Debug)]
pub(crate) struct MusicColumn {
    pub renderer: MusicColumnRenderer,
}

impl<'de> Deserialize<'de> for MusicColumn {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = JsonValue::deserialize(deserializer)?;
        let renderer_value = value
            .get("musicResponsiveListItemFlexColumnRenderer")
            .or_else(|| value.get("musicResponsiveListItemFixedColumnRenderer"))
            .ok_or_else(|| serde::de::Error::missing_field("musicResponsiveListItem*ColumnRenderer"))?;
        let raw = crate::json::value_to_json_string(renderer_value);
        let renderer: MusicColumnRenderer = flexon::from_str(&raw)
            .map_err(|e| serde::de::Error::custom(format!("column renderer: {e}")))?;
        Ok(Self { renderer })
    }
}

#[derive(Debug)]
pub(crate) struct MusicColumnRenderer {
    pub text: TextComponents,
}

impl<'de> Deserialize<'de> for MusicColumnRenderer {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = JsonValue::deserialize(deserializer)?;
        let raw = crate::json::value_to_json_string(
            value
                .get("text")
                .ok_or_else(|| serde::de::Error::missing_field("text"))?,
        );
        let text: TextComponents = flexon::from_str(&raw)
            .map_err(|e| serde::de::Error::custom(format!("column text: {e}")))?;
        Ok(Self { text })
    }
}

impl From<MusicColumn> for TextComponents {
    fn from(col: MusicColumn) -> Self {
        col.renderer.text
    }
}

impl From<MusicThumbnailRenderer> for Vec<model::Thumbnail> {
    fn from(tr: MusicThumbnailRenderer) -> Self {
        tr.music_thumbnail_renderer.into()
    }
}

#[derive(Debug, Default)]
pub(crate) struct MusicCarouselShelfHeaderRenderer {
    pub more_content_button: Option<JsonValue>,
    pub title: TextComponents,
}

impl<'de> Deserialize<'de> for MusicCarouselShelfHeaderRenderer {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = JsonValue::deserialize(deserializer)?;
        let title = match value.get("title") {
            Some(v) => {
                let raw = crate::json::value_to_json_string(v);
                match flexon::from_str::<TextComponents>(&raw) {
                    Ok(t) => t,
                    Err(e) => return Err(serde::de::Error::custom(format!("carousel title: {e}"))),
                }
            }
            None => TextComponents::default(),
        };
        Ok(Self {
            more_content_button: value.get("moreContentButton").cloned(),
            title,
        })
    }
}

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GridRenderer {
    pub items: JsonValue,
    #[allow(dead_code)]
    pub header: Option<GridHeader>,
    #[serde(default)]
    #[serde_as(as = "VecSkipError<_>")]
    pub continuations: Vec<MusicContinuationData>,
}

#[derive(Debug)]
pub(crate) struct GridHeader {
    #[allow(dead_code)]
    pub grid_header_renderer: SimpleHeaderRenderer,
}

impl<'de> Deserialize<'de> for GridHeader {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = JsonValue::deserialize(deserializer)?;
        let raw = crate::json::value_to_json_string(
            value
                .get("gridHeaderRenderer")
                .ok_or_else(|| serde::de::Error::missing_field("gridHeaderRenderer"))?,
        );
        let inner: SimpleHeaderRenderer = flexon::from_str(&raw)
            .map_err(|e| serde::de::Error::custom(format!("grid header: {e}")))?;
        Ok(Self {
            grid_header_renderer: inner,
        })
    }
}

#[derive(Debug)]
pub(crate) enum TrackBadge {
    LiveBadgeRenderer {},
}

impl<'de> Deserialize<'de> for TrackBadge {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = JsonValue::deserialize(deserializer)?;
        if value.get("liveBadgeRenderer").is_some() {
            Ok(TrackBadge::LiveBadgeRenderer {})
        } else {
            Err(serde::de::Error::custom("unknown track badge"))
        }
    }
}

#[serde_as]
#[derive(Default, Debug, FromYtNode)]
pub(crate) struct MusicMicroformat {
    #[ytq_default]
    pub microformat_data_renderer: MicroformatData,
}

#[derive(Default, Debug, FromYtNode)]
pub(crate) struct MicroformatData {
    pub url_canonical: Option<String>,
    #[ytq_default]
    pub noindex: bool,
}

/*
#MAPPER
*/

#[derive(Debug)]
struct MusicItemParser {
    lang: Language,
    /// Artists list + various artists flag
    artists: Option<(Vec<ArtistId>, bool)>,
    album: Option<AlbumId>,
    /// Default album type in case an album is unlabeled
    album_type: AlbumType,
    artist_page: bool,
    search_suggestion: bool,
    items: Vec<MusicItem>,
    warnings: Vec<String>,
    ctoken: Option<String>,
}

#[derive(Debug)]
pub(crate) struct GroupedMusicItems {
    pub tracks: Vec<TrackItem>,
    pub albums: Vec<AlbumItem>,
    pub artists: Vec<ArtistItem>,
    pub playlists: Vec<MusicPlaylistItem>,
}

impl MusicItemParser {
    fn new(lang: Language) -> Self {
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

    fn new_search_suggest(lang: Language) -> Self {
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

    /// Create parser context for an artist page
    fn with_artist(lang: Language, artist: ArtistId) -> Self {
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

    /// Create parser context for an album page
    fn with_album(lang: Language, artists: Vec<ArtistId>, by_va: bool, album: AlbumId) -> Self {
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

    fn map_response_value(&mut self, value: JsonValue) -> Option<MusicItemType> {
        let doc = JsonDoc::new(value_to_json_string(&value));
        doc.with_root(|root| Ok(self.map_response_node(&root)))
            .ok()
            .flatten()
    }

    fn map_response_items(&mut self, items_node: &JsonNode<'_>) -> Option<MusicItemType> {
        let mut etype = None;
        items_node.items().into_iter().for_each(|item| {
            if let Some(et) = self.add_response_item_node(&item) {
                if etype.is_none() {
                    etype = Some(et);
                }
            }
        });
        etype
    }

    fn map_response_node(&mut self, node: &JsonNode<'_>) -> Option<MusicItemType> {
        let items_node = match node.query(ytq!(.contents || .items)) {
            Some(items) => items,
            None => node.clone(),
        };
        self.map_response_items(&items_node)
    }

    /// Map a ListMusicItem (album/playlist item, search result)
    fn map_list_item(&mut self, item: ListMusicItem) -> Result<Option<MusicItemType>, String> {
        let mut columns = item.flex_columns.into_iter();
        let c1 = columns.next();
        let c2 = columns.next();
        let c3 = columns.next();
        let c4 = columns.next();

        let title = c1.as_ref().map(|col| col.renderer.text.to_string());

        let first_tn = item.thumbnail.music_thumbnail_renderer.thumbnails.first();

        let music_page = item
            .navigation_endpoint
            .and_then(|endpoint| url_endpoint::music_page(&endpoint))
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
                    unavailable: item.music_item_renderer_display_policy == DisplayPolicy::GreyOut,
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

                        self.items.push(MusicItem::User(model::UserItem {
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

        match url_endpoint::music_page(&item.navigation_endpoint) {
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
                        unavailable: false,
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
                        .is_none_or(util::is_ytm);
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
    fn map_card(&mut self, card: MusicCardShelf) -> Option<MusicItemType> {
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

        let item_type = match url_endpoint::music_page(&card.on_tap) {
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
                            unavailable: false,
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
                            unavailable: false,
                        }));
                    }
                    Some(MusicItemType::Track)
                }
                MusicPageType::Playlist { is_podcast } => {
                    let from_ytm = subtitle_p2
                        .as_ref()
                        .and_then(|p| p.0.first())
                        .is_none_or(util::is_ytm);
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

                    self.items.push(MusicItem::User(model::UserItem {
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

        self.map_response_value(card.contents);

        item_type
    }

    fn add_item(&mut self, item: MusicItem) {
        self.items.push(item);
    }

    fn add_response_item_node(&mut self, item: &JsonNode<'_>) -> Option<MusicItemType> {
        let result = if let Some(item) = item
            .query(ytq!(.musicResponsiveListItemRenderer))
            .and_then(|node| node.deserialize::<ListMusicItem>().ok())
        {
            self.map_list_item(item)
        } else if let Some(item) = item
            .query(ytq!(.musicTwoRowItemRenderer))
            .and_then(|node| node.deserialize::<CoverMusicItem>().ok())
        {
            self.map_tile(item)
        } else if let Some(endpoint) = item
            .query(ytq!(.continuationItemRenderer.continuationEndpoint))
            .and_then(|node| node.deserialize::<JsonValue>().ok())
        {
            if self.ctoken.is_none() {
                self.ctoken = continuation_token(&endpoint);
            }
            Ok(None)
        } else {
            Ok(None)
        };

        match result {
            Ok(et) => et,
            Err(e) => {
                self.warnings.push(e);
                None
            }
        }
    }

    fn add_warnings(&mut self, warnings: &mut Vec<String>) {
        self.warnings.append(warnings);
    }

    fn items(self) -> MapResult<Vec<MusicItem>> {
        MapResult {
            c: self.items,
            warnings: self.warnings,
        }
    }

    fn conv_items<T: FromYtItem>(self) -> MapResult<Vec<T>> {
        MapResult {
            c: self
                .items
                .into_iter()
                .filter_map(T::from_ytm_item)
                .collect(),
            warnings: self.warnings,
        }
    }

    fn group_items(self) -> MapResult<GroupedMusicItems> {
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
    fn conv_history_items(
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

pub(crate) fn map_music_items<T: FromYtItem>(
    node: &JsonNode<'_>,
    lang: Language,
) -> (MapResult<Vec<T>>, Option<String>) {
    let mut mapper = MusicItemParser::new(lang);
    mapper.map_response_node(node);
    let ctoken = mapper.ctoken.clone();
    (mapper.conv_items(), ctoken)
}

pub(crate) fn map_music_items_value<T: FromYtItem>(
    value: JsonValue,
    lang: Language,
) -> (MapResult<Vec<T>>, Option<String>) {
    let mut mapper = MusicItemParser::new(lang);
    mapper.map_response_value(value);
    let ctoken = mapper.ctoken.clone();
    (mapper.conv_items(), ctoken)
}

pub(crate) fn map_album_track_items(
    value: JsonValue,
    lang: Language,
    artists: Vec<ArtistId>,
    by_va: bool,
    album: AlbumId,
) -> (MapResult<Vec<TrackItem>>, Option<String>) {
    let mut mapper = MusicItemParser::with_album(lang, artists, by_va, album);
    mapper.map_response_value(value);
    let ctoken = mapper.ctoken.clone();
    (mapper.conv_items(), ctoken)
}

pub(crate) fn map_grouped_music_items_values(
    values: impl IntoIterator<Item = (JsonValue, AlbumType)>,
    lang: Language,
    artist: Option<ArtistId>,
) -> (MapResult<GroupedMusicItems>, Option<String>) {
    let mut mapper = match artist {
        Some(artist) => MusicItemParser::with_artist(lang, artist),
        None => MusicItemParser::new(lang),
    };
    for (value, album_type) in values {
        mapper.album_type = album_type;
        mapper.map_response_value(value);
    }
    let ctoken = mapper.ctoken.clone();
    (mapper.group_items(), ctoken)
}

pub(crate) fn map_music_item_card(
    card: MusicCardShelf,
    lang: Language,
) -> (
    MapResult<Vec<MusicItem>>,
    Option<String>,
    Option<MusicItemType>,
) {
    let mut mapper = MusicItemParser::new(lang);
    let item_type = mapper.map_card(card);
    let ctoken = mapper.ctoken.clone();
    (mapper.items(), ctoken, item_type)
}

pub(crate) fn map_search_suggestion_item(
    item: &JsonNode<'_>,
    lang: Language,
) -> (
    MapResult<Vec<MusicItem>>,
    Option<String>,
    Option<MusicItemType>,
) {
    let mut mapper = MusicItemParser::new_search_suggest(lang);
    let item_type = mapper.add_response_item_node(item);
    let ctoken = mapper.ctoken.clone();
    (mapper.items(), ctoken, item_type)
}

pub(crate) fn map_music_continuation_items<'a>(
    root: &JsonNode<'_>,
    lang: Language,
    artist: Option<ArtistId>,
    values: impl IntoIterator<Item = JsonValue>,
    items_nodes: impl IntoIterator<Item = JsonNode<'a>>,
    extra_items: impl IntoIterator<Item = MapResult<MusicItem>>,
) -> (MapResult<Vec<MusicItem>>, Option<String>) {
    let mut mapper = match artist {
        Some(artist) => MusicItemParser::with_artist(lang, artist),
        None => MusicItemParser::new(lang),
    };
    for value in values {
        mapper.map_response_value(value);
    }
    for items in items_nodes {
        mapper.map_response_node(&items);
    }
    for mut item in extra_items {
        mapper.add_item(item.c);
        mapper.add_warnings(&mut item.warnings);
    }
    if let Some(actions) = root.query(ytq!(.onResponseReceivedActions)) {
        for action in actions.items() {
            if let Some(items) = action.query(ytq!(
                .(.appendContinuationItemsAction || .reloadContinuationItemsCommand).continuationItems
            )) {
                mapper.map_response_node(&items);
            }
        }
    }
    let ctoken = mapper.ctoken.clone();
    (mapper.items(), ctoken)
}

#[cfg(feature = "userdata")]
pub(crate) fn extend_music_history_items_value(
    value: JsonValue,
    lang: Language,
    date_txt: Option<String>,
    utc_offset: UtcOffset,
    res: &mut MapResult<Vec<HistoryItem<TrackItem>>>,
) {
    let mut mapper = MusicItemParser::new(lang);
    mapper.map_response_value(value);
    mapper.conv_history_items(date_txt, utc_offset, res);
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
    menu: Option<JsonValue>,
    fallback_artist: Option<&ArtistId>,
) -> Option<String> {
    menu.as_ref()
        .and_then(map_artist_id)
        .or_else(|| fallback_artist.and_then(|a| a.id.clone()))
}

fn map_channel_handle(st: Option<&TextComponents>) -> Option<String> {
    st.map(|t| t.first_str())
        .filter(|t| t.starts_with('@'))
        .map(str::to_owned)
}

pub(crate) fn map_artist_id(menu: &JsonValue) -> Option<String> {
    menu.get("menuRenderer")
        .and_then(|renderer| renderer.get("items"))
        .and_then(|items| items.as_array())
        .into_iter()
        .flat_map(|items| items.iter())
        .find_map(|item| {
            item.get("menuNavigationItemRenderer")
                .and_then(|item| item.get("navigationEndpoint"))
                .and_then(url_endpoint::browse_endpoint)
                .and_then(|ep| {
                    let browse_endpoint = ep.browse_endpoint;
                    browse_endpoint
                        .browse_endpoint_context_supported_configs
                        .and_then(|cfg| {
                            if cfg.browse_endpoint_context_music_config.page_type
                                == PageType::Artist
                            {
                                Some(browse_endpoint.browse_id)
                            } else {
                                None
                            }
                        })
                })
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
            unavailable: false,
        },
        warnings,
    }
}

/// Resolves the music header renderer from a top-level header node.
pub(crate) fn music_header_node<'a>(node: &JsonNode<'a>) -> Option<JsonNode<'a>> {
    node.query(ytq!(.musicDetailHeaderRenderer || .musicResponsiveHeaderRenderer))
}

/// Resolve a text field at the given `path` of a music header node.
pub(crate) fn music_header_text(
    header: &JsonNode<'_>,
    path: crate::json::Query,
) -> Option<String> {
    header.text_at(path)
}

/// Resolve an attributed text (`TextComponents`) field at the given `path` of
/// a music header node.
pub(crate) fn music_header_components(
    header: &JsonNode<'_>,
    path: crate::json::Query,
) -> Option<TextComponents> {
    header
        .query(path)
        .and_then(|node| node.deserialize::<TextComponents>().ok())
}

/// Resolve the description (`TextComponents`) of a music header node.
pub(crate) fn music_header_description(header: &JsonNode<'_>) -> Option<TextComponents> {
    let description = header.query(ytq!(.description))?;
    description
        .query(ytq!(.musicDescriptionShelfRenderer.description))
        .unwrap_or(description)
        .deserialize()
        .ok()
}

/// Resolve the `secondSubtitle` of a music header as a `Vec<String>` of plain
/// text components.
pub(crate) fn music_header_second_subtitle(header: &JsonNode<'_>) -> Vec<String> {
    header
        .query(ytq!(.secondSubtitle))
        .and_then(|node| node.deserialize::<TextComponents>().ok())
        .map(|components| {
            components
                .0
                .into_iter()
                .map(|component| component.as_str().to_owned())
                .collect()
        })
        .unwrap_or_default()
}

/// Resolve the `menu` JSON value of a music header, falling back to a button
/// under `buttons` if no menu is present.
pub(crate) fn music_header_menu(header: &JsonNode<'_>) -> Option<JsonValue> {
    header
        .query(ytq!(.menu))
        .and_then(|node| node.deserialize::<JsonValue>().ok())
        .or_else(|| {
            header.query(ytq!(.buttons)).and_then(|buttons| {
                buttons
                    .items()
                    .into_iter()
                    .find_map(|button| button.deserialize::<JsonValue>().ok())
            })
        })
}

/// Resolve the cover thumbnail of a music header.
pub(crate) fn music_header_thumbnail(header: &JsonNode<'_>) -> Vec<crate::model::Thumbnail> {
    header.query_thumbnails(ytq!(
        .thumbnail.(.musicThumbnailRenderer || .croppedSquareThumbnailRenderer).thumbnail
    ))
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
            flexon::from_reader(BufReader::new(json_file)).unwrap();

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
