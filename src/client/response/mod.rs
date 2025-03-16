pub(crate) mod channel;
pub(crate) mod music_artist;
pub(crate) mod music_charts;
pub(crate) mod music_details;
pub(crate) mod music_genres;
pub(crate) mod music_item;
pub(crate) mod music_new;
pub(crate) mod music_playlist;
pub(crate) mod music_search;
pub(crate) mod player;
pub(crate) mod playlist;
pub(crate) mod search;
pub(crate) mod trends;
pub(crate) mod url_endpoint;
pub(crate) mod video_details;
pub(crate) mod video_item;

pub(crate) use channel::Channel;
pub(crate) use channel::ChannelAbout;
pub(crate) use music_artist::MusicArtist;
pub(crate) use music_artist::MusicArtistAlbums;
pub(crate) use music_charts::MusicCharts;
pub(crate) use music_details::MusicDetails;
pub(crate) use music_details::MusicLyrics;
pub(crate) use music_details::MusicRelated;
pub(crate) use music_genres::MusicGenre;
pub(crate) use music_genres::MusicGenres;
pub(crate) use music_item::MusicContinuation;
pub(crate) use music_new::MusicNew;
pub(crate) use music_playlist::MusicPlaylist;
pub(crate) use music_search::MusicSearch;
pub(crate) use music_search::MusicSearchSuggestion;
pub(crate) use player::DrmLicense;
pub(crate) use player::Player;
pub(crate) use playlist::Playlist;
pub(crate) use search::Search;
pub(crate) use search::SearchSuggestion;
pub(crate) use trends::Trending;
pub(crate) use url_endpoint::ResolvedUrl;
pub(crate) use video_details::VideoComments;
pub(crate) use video_details::VideoDetails;
pub(crate) use video_item::YouTubeListItem;
pub(crate) use video_item::YouTubeListMapper;

#[cfg(feature = "rss")]
pub(crate) mod channel_rss;
#[cfg(feature = "rss")]
pub(crate) use channel_rss::ChannelRss;

#[cfg(feature = "userdata")]
pub(crate) mod history;
#[cfg(feature = "userdata")]
pub(crate) use history::History;
#[cfg(feature = "userdata")]
pub(crate) mod music_history;
#[cfg(feature = "userdata")]
pub(crate) use music_history::MusicHistory;

use std::borrow::Cow;
use std::collections::HashMap;
use std::marker::PhantomData;

use serde::{
    de::{IgnoredAny, Visitor},
    Deserialize,
};
use serde_with::{serde_as, DisplayFromStr, VecSkipError};

use crate::error::ExtractionError;
use crate::serializer::text::{AttributedText, Text, TextComponent};
use crate::serializer::{MapResult, VecSkipErrorWrap};

use self::video_item::YouTubeListRenderer;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ContentRenderer<T> {
    pub content: T,
}

/// Deserializes any object with an array field named `contents`, `tabs` or `items`.
///
/// Invalid items are skipped
#[derive(Debug)]
pub(crate) struct ContentsRenderer<T> {
    pub contents: Vec<T>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct ContentsRendererLogged<T> {
    #[serde(alias = "items")]
    pub contents: MapResult<Vec<T>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Tab<T> {
    pub tab_renderer: ContentRenderer<T>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SectionList<T> {
    pub section_list_renderer: ContentsRenderer<T>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TwoColumnBrowseResults<T> {
    pub two_column_browse_results_renderer: ContentsRenderer<T>,
}

#[derive(Default, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ThumbnailsWrap {
    #[serde(default)]
    pub thumbnail: Thumbnails,
}

#[derive(Default, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ImageView {
    pub image: Thumbnails,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AvatarViewModel {
    pub avatar_view_model: ImageView,
}

/// List of images in different resolutions.
/// Not only used for thumbnails, but also for avatars and banners.
#[derive(Default, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Thumbnails {
    #[serde(default, alias = "sources")]
    pub thumbnails: Vec<Thumbnail>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Thumbnail {
    pub url: String,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ContinuationItemRenderer {
    pub continuation_endpoint: ContinuationEndpoint,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub(crate) enum ContinuationEndpoint {
    ContinuationCommand(ContinuationCommandWrap),
    CommandExecutorCommand(CommandExecutorCommandWrap),
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ContinuationCommandWrap {
    pub continuation_command: ContinuationCommand,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ContinuationCommand {
    pub token: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CommandExecutorCommandWrap {
    pub command_executor_command: CommandExecutorCommand,
}

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CommandExecutorCommand {
    #[serde_as(as = "VecSkipError<_>")]
    commands: Vec<ContinuationCommandWrap>,
}

impl ContinuationEndpoint {
    pub fn into_token(self) -> Option<String> {
        match self {
            Self::ContinuationCommand(cmd) => Some(cmd.continuation_command.token),
            Self::CommandExecutorCommand(cmd) => cmd
                .command_executor_command
                .commands
                .into_iter()
                .next()
                .map(|c| c.continuation_command.token),
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Icon {
    pub icon_type: IconType,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub(crate) enum IconType {
    /// Checkmark for verified channels
    Check,
    /// Music note for verified artists
    OfficialArtistBadge,
    /// Like button
    Like,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ChannelBadge {
    pub metadata_badge_renderer: ChannelBadgeRenderer,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ChannelBadgeRenderer {
    pub style: ChannelBadgeStyle,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub(crate) enum ChannelBadgeStyle {
    BadgeStyleTypeVerified,
    BadgeStyleTypeVerifiedArtist,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Alert {
    pub alert_renderer: TextBox,
}

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TextBox {
    #[serde_as(as = "Text")]
    pub text: String,
}

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SimpleHeaderRenderer {
    #[serde_as(as = "Text")]
    pub title: String,
}

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TextComponentBox {
    #[serde_as(as = "AttributedText")]
    pub text: TextComponent,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ResponseContext {
    pub visitor_data: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AttachmentRun {
    pub element: AttachmentRunElement,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AttachmentRunElement {
    #[serde(rename = "type")]
    pub typ: AttachmentRunElementType,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AttachmentRunElementType {
    pub image_type: AttachmentRunElementImageType,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AttachmentRunElementImageType {
    pub image: AttachmentRunElementImage,
}

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AttachmentRunElementImage {
    #[serde_as(as = "VecSkipError<_>")]
    pub sources: Vec<AttachmentRunElementImageSource>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AttachmentRunElementImageSource {
    pub client_resource: ClientResource,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ClientResource {
    pub image_name: IconName,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum IconName {
    CheckCircleFilled,
    #[serde(alias = "AUDIO_BADGE")]
    MusicFilled,
}

// CONTINUATION

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Continuation {
    /// Number of search results
    #[serde_as(as = "Option<DisplayFromStr>")]
    pub estimated_results: Option<u64>,
    #[serde(
        alias = "onResponseReceivedCommands",
        alias = "onResponseReceivedEndpoints"
    )]
    #[serde_as(as = "Option<VecSkipError<_>>")]
    pub on_response_received_actions: Option<Vec<ContinuationActionWrap<YouTubeListItem>>>,
    /// Used for channel video rich grid renderer
    ///
    /// A/B test seen on 19.10.2022
    pub continuation_contents: Option<RichGridContinuationContents>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ContinuationActionWrap<T> {
    #[serde(alias = "reloadContinuationItemsCommand")]
    pub append_continuation_items_action: ContinuationAction<T>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ContinuationAction<T> {
    pub continuation_items: MapResult<Vec<T>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RichGridContinuationContents {
    pub rich_grid_continuation: YouTubeListRenderer,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MusicContinuationData {
    #[serde(alias = "nextRadioContinuationData")]
    pub next_continuation_data: MusicContinuationDataInner,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MusicContinuationDataInner {
    pub continuation: String,
}

// ERROR

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ErrorResponse {
    pub error: ErrorResponseContent,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ErrorResponseContent {
    pub message: String,
}

// DESERIALIZER

impl<'de, T> Deserialize<'de> for ContentsRenderer<T>
where
    T: Deserialize<'de>,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct ItemVisitor<T>(PhantomData<T>);

        impl<'de, T> Visitor<'de> for ItemVisitor<T>
        where
            T: Deserialize<'de>,
        {
            type Value = ContentsRenderer<T>;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("map")
            }

            fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
            where
                A: serde::de::MapAccess<'de>,
            {
                let mut contents = None;

                while let Some(k) = map.next_key::<Cow<'de, str>>()? {
                    if k == "contents" || k == "tabs" || k == "items" {
                        contents = Some(ContentsRenderer {
                            contents: map.next_value::<VecSkipErrorWrap<T>>()?.0,
                        });
                    } else {
                        map.next_value::<IgnoredAny>()?;
                    }
                }

                contents.ok_or(serde::de::Error::missing_field("contents"))
            }
        }

        deserializer.deserialize_map(ItemVisitor(PhantomData::<T>))
    }
}

// MAPPING

impl From<Thumbnail> for crate::model::Thumbnail {
    fn from(tn: Thumbnail) -> Self {
        crate::model::Thumbnail {
            url: tn.url,
            width: tn.width,
            height: tn.height,
        }
    }
}

impl From<Thumbnails> for Vec<crate::model::Thumbnail> {
    fn from(ts: Thumbnails) -> Self {
        ts.thumbnails
            .into_iter()
            .map(|t| crate::model::Thumbnail {
                url: t.url,
                width: t.width,
                height: t.height,
            })
            .collect()
    }
}

impl ContentImage {
    pub(crate) fn into_image(self) -> ImageViewOl {
        match self {
            ContentImage::ThumbnailViewModel(image) => image,
            ContentImage::CollectionThumbnailViewModel { primary_thumbnail } => {
                primary_thumbnail.thumbnail_view_model
            }
        }
    }
}

impl From<Vec<ChannelBadge>> for crate::model::Verification {
    fn from(badges: Vec<ChannelBadge>) -> Self {
        badges
            .first()
            .map_or(crate::model::Verification::None, |b| {
                match b.metadata_badge_renderer.style {
                    ChannelBadgeStyle::BadgeStyleTypeVerified => Self::Verified,
                    ChannelBadgeStyle::BadgeStyleTypeVerifiedArtist => Self::Artist,
                }
            })
    }
}

impl From<Icon> for crate::model::Verification {
    fn from(icon: Icon) -> Self {
        match icon.icon_type {
            IconType::Check => Self::Verified,
            IconType::OfficialArtistBadge => Self::Artist,
            IconType::Like => Self::None,
        }
    }
}

impl From<AttachmentRun> for crate::model::Verification {
    fn from(value: AttachmentRun) -> Self {
        match value
            .element
            .typ
            .image_type
            .image
            .sources
            .into_iter()
            .next()
            .map(|s| s.client_resource.image_name)
        {
            Some(IconName::CheckCircleFilled) => Self::Verified,
            Some(IconName::MusicFilled) => Self::Artist,
            None => Self::None,
        }
    }
}

pub(crate) fn alerts_to_err(id: &str, alerts: Option<Vec<Alert>>) -> ExtractionError {
    ExtractionError::NotFound {
        id: id.to_owned(),
        msg: alerts
            .map(|alerts| {
                alerts
                    .into_iter()
                    .map(|a| a.alert_renderer.text)
                    .collect::<Vec<_>>()
                    .join(" ")
                    .into()
            })
            .unwrap_or_default(),
    }
}

// FRAMEWORK UPDATES

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct FrameworkUpdates<T> {
    pub entity_batch_update: EntityBatchUpdate<T>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct EntityBatchUpdate<T> {
    pub mutations: FrameworkUpdateMutations<T>,
}

/// List of update mutations that deserializes into a HashMap (entity_key => payload)
#[derive(Debug)]
pub(crate) struct FrameworkUpdateMutations<T> {
    pub items: HashMap<String, T>,
    pub warnings: Vec<String>,
}

impl<'de, T> Deserialize<'de> for FrameworkUpdateMutations<T>
where
    T: Deserialize<'de>,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct SeqVisitor<T>(PhantomData<T>);

        #[derive(serde::Deserialize)]
        #[serde(untagged)]
        enum MutationOrError<T> {
            #[serde(rename_all = "camelCase")]
            Good {
                entity_key: String,
                payload: T,
            },
            Error(serde_json::Value),
        }

        impl<'de, T> Visitor<'de> for SeqVisitor<T>
        where
            T: Deserialize<'de>,
        {
            type Value = FrameworkUpdateMutations<T>;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("sequence of entity mutations")
            }

            fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
            where
                A: serde::de::SeqAccess<'de>,
            {
                let mut items = HashMap::with_capacity(seq.size_hint().unwrap_or_default());
                let mut warnings = Vec::new();

                while let Some(value) = seq.next_element::<MutationOrError<T>>()? {
                    match value {
                        MutationOrError::Good {
                            entity_key,
                            payload,
                        } => {
                            items.insert(entity_key, payload);
                        }
                        MutationOrError::Error(value) => {
                            warnings.push(format!(
                                "error deserializing item: {}",
                                serde_json::to_string(&value).unwrap_or_default()
                            ));
                        }
                    }
                }

                Ok(FrameworkUpdateMutations { items, warnings })
            }
        }

        deserializer.deserialize_seq(SeqVisitor(PhantomData::<T>))
    }
}

// PAGE HEADER

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PageHeaderRendererContent<T> {
    pub page_header_view_model: T,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PhMetadataView {
    pub content_metadata_view_model: PhMetadataView2,
}

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PhMetadataView2 {
    #[serde_as(as = "VecSkipError<_>")]
    pub metadata_rows: Vec<PhMetadataRow>,
}

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PhMetadataRow {
    #[serde_as(as = "VecSkipError<_>")]
    pub metadata_parts: Vec<MetadataPart>,
}

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum MetadataPart {
    Text(#[serde_as(as = "AttributedText")] TextComponent),
    #[serde(rename_all = "camelCase")]
    AvatarStack {
        avatar_stack_view_model: TextComponentBox,
    },
}

impl MetadataPart {
    pub fn into_text_component(self) -> TextComponent {
        match self {
            MetadataPart::Text(text_component) => text_component,
            MetadataPart::AvatarStack {
                avatar_stack_view_model,
            } => avatar_stack_view_model.text,
        }
    }

    pub fn as_str(&self) -> &str {
        match self {
            MetadataPart::Text(s) => s.as_str(),
            MetadataPart::AvatarStack {
                avatar_stack_view_model,
            } => avatar_stack_view_model.text.as_str(),
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum ContentImage {
    ThumbnailViewModel(ImageViewOl),
    #[serde(rename_all = "camelCase")]
    CollectionThumbnailViewModel {
        primary_thumbnail: ThumbnailViewModelWrap,
    },
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ThumbnailViewModelWrap {
    pub thumbnail_view_model: ImageViewOl,
}

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ImageViewOl {
    pub image: Thumbnails,
    #[serde_as(as = "VecSkipError<_>")]
    pub overlays: Vec<ImageViewOverlay>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ImageViewOverlay {
    pub thumbnail_overlay_badge_view_model: ThumbnailOverlayBadgeViewModel,
}

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ThumbnailOverlayBadgeViewModel {
    #[serde_as(as = "VecSkipError<_>")]
    pub thumbnail_badges: Vec<ThumbnailBadges>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ThumbnailBadges {
    pub thumbnail_badge_view_model: TextBox,
}

#[derive(Debug, Deserialize)]
pub(crate) struct Empty {}
