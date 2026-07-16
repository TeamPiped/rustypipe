pub(crate) mod channel;
pub(crate) mod music_artist;
pub(crate) mod music_details;
pub(crate) mod music_genres;
pub(crate) mod music_item;
pub(crate) mod music_playlist;
pub(crate) mod player;
pub(crate) mod playlist;
pub(crate) mod url_endpoint;
pub(crate) mod video_details;
pub(crate) mod video_item;

// Raw YouTube response shapes live in this module tree. Names that mirror
// Innertube JSON, such as `*Renderer`, `*ViewModel`, and raw `*Data` structs,
// are intentional here and should stay close to serde deserialization.
//
// Endpoint modules should prefer response-local helpers and mappers over
// matching these shapes directly. That keeps YouTube layout churn contained
// while still constructing public model structs at endpoint boundaries.

#[cfg(feature = "rss")]
pub(crate) mod channel_rss;
#[cfg(feature = "rss")]
pub(crate) use channel_rss::ChannelRss;

use std::collections::HashMap;
use std::marker::PhantomData;

use serde::{de::Visitor, Deserialize, Deserializer};
use serde_with::{serde_as, VecSkipError};

use crate::deserialize_through_node;
use crate::serializer::text::{AttributedText, TextComponent};
use crate::yt_string_enum;
use crate::{ytq, FromYtNode};
use crate::{
    error::ExtractionError,
    json::{value_to_json_string, JsonValue},
};

#[derive(Default, Debug)]
pub(crate) struct ImageView {
    pub image: Thumbnails,
}

impl ImageView {
    pub(crate) fn from_node(node: &crate::json::JsonNode<'_>) -> Option<Self> {
        Some(Self {
            image: Thumbnails::from_node(&node.query(crate::json::ytq!(.image))?)?,
        })
    }
}

deserialize_through_node!(ImageView);

/// List of images in different resolutions.
/// Not only used for thumbnails, but also for avatars and banners.
#[derive(Clone, Default, Debug)]
pub(crate) struct Thumbnails {
    pub thumbnails: Vec<Thumbnail>,
}

impl Thumbnails {
    pub(crate) fn from_node(node: &crate::json::JsonNode<'_>) -> Option<Self> {
        // `yt_thumbnails` already understands the `thumbnails` / `sources` alias
        // and the `(root || .thumbnail)` indirection, so reuse it.
        let raw_thumbnails = crate::json::yt_thumbnails(node);
        Some(Self { thumbnails: raw_thumbnails.into_iter().map(Into::into).collect() })
    }
}

deserialize_through_node!(Thumbnails);

#[derive(Clone, Debug)]
pub(crate) struct Thumbnail {
    pub url: String,
    pub width: u32,
    pub height: u32,
}

impl From<crate::model::Thumbnail> for Thumbnail {
    fn from(t: crate::model::Thumbnail) -> Self {
        Self { url: t.url, width: t.width, height: t.height }
    }
}

impl Thumbnail {
    pub(crate) fn from_node(node: &crate::json::JsonNode<'_>) -> Option<Self> {
        Some(Self {
            url: node.query_str(crate::json::ytq!(.url))?,
            width: node.query_u32(crate::json::ytq!(.width)).unwrap_or(0),
            height: node.query_u32(crate::json::ytq!(.height)).unwrap_or(0),
        })
    }
}

deserialize_through_node!(Thumbnail);

#[derive(Debug, Default)]
pub(crate) struct Icon {
    pub icon_type: IconType,
}

impl Icon {
    pub(crate) fn from_node(node: &crate::json::JsonNode<'_>) -> Option<Self> {
        Some(Self {
            icon_type: IconType::from_node(node)?,
        })
    }
}

deserialize_through_node!(Icon);

yt_string_enum! {
    pub(crate) enum IconType {
        /// Checkmark for verified channels
        Check = "CHECK" | "CHECK_CIRCLE_THICK" | "CHECK_CIRCLE_FILLED" | "VERIFIED",
        /// Music note for verified artists
        OfficialArtistBadge = "OFFICIAL_ARTIST_BADGE" | "OFFICIAL_MUSIC_BADGE",
        /// Like button
        Like = "LIKE",
    }
    default: IconType::Like
}

impl IconType {
    pub(crate) fn from_node(node: &crate::json::JsonNode<'_>) -> Option<Self> {
        let s = node.query_str(crate::json::ytq!(.iconType))?;
        Self::from_str(&s)
    }
}

#[derive(Debug)]
pub(crate) struct ChannelBadge {
    pub metadata_badge_renderer: ChannelBadgeRenderer,
}

impl ChannelBadge {
    pub(crate) fn from_node(node: &crate::json::JsonNode<'_>) -> Option<Self> {
        Some(Self {
            metadata_badge_renderer: ChannelBadgeRenderer::from_node(
                &node.query(crate::json::ytq!(.metadataBadgeRenderer))?,
            )?,
        })
    }
}

deserialize_through_node!(ChannelBadge);

#[derive(Debug)]
pub(crate) struct ChannelBadgeRenderer {
    pub style: ChannelBadgeStyle,
}

impl ChannelBadgeRenderer {
    pub(crate) fn from_node(node: &crate::json::JsonNode<'_>) -> Option<Self> {
        Some(Self { style: ChannelBadgeStyle::from_node(node)? })
    }
}

yt_string_enum! {
    pub(crate) enum ChannelBadgeStyle {
        BadgeStyleTypeVerified = "BADGE_STYLE_TYPE_VERIFIED",
        BadgeStyleTypeVerifiedArtist = "BADGE_STYLE_TYPE_VERIFIED_ARTIST",
    }
    default: ChannelBadgeStyle::BadgeStyleTypeVerified
}

impl ChannelBadgeStyle {
    pub(crate) fn from_node(node: &crate::json::JsonNode<'_>) -> Option<Self> {
        let s = node.query_str(crate::json::ytq!(.style))?;
        Self::from_str(&s)
    }
}

#[derive(Debug)]
pub(crate) struct Alert {
    pub alert_renderer: TextBox,
}

impl Alert {
    pub(crate) fn from_node(node: &crate::json::JsonNode<'_>) -> Option<Self> {
        Some(Self {
            alert_renderer: TextBox::from_node(
                &node.query(crate::json::ytq!(.alertRenderer))?,
            )?,
        })
    }
}

deserialize_through_node!(Alert);

#[derive(Debug)]
pub(crate) struct TextBox {
    pub text: String,
}

impl TextBox {
    pub(crate) fn from_node(node: &crate::json::JsonNode<'_>) -> Option<Self> {
        Some(Self {
            text: node.text().unwrap_or_default(),
        })
    }
}

deserialize_through_node!(TextBox);

#[derive(Debug)]
pub(crate) struct SimpleHeaderRenderer {
    pub title: String,
}

impl SimpleHeaderRenderer {
    pub(crate) fn from_node(node: &crate::json::JsonNode<'_>) -> Option<Self> {
        Some(Self {
            title: node_title(node).unwrap_or_default(),
        })
    }
}

deserialize_through_node!(SimpleHeaderRenderer);

fn node_title(node: &crate::json::JsonNode<'_>) -> Option<String> {
    node.query(crate::json::ytq!(.title)).and_then(|n| n.text())
}

#[derive(Debug, Default, FromYtNode)]
pub(crate) struct TextComponentBox {
    #[ytq_attributed_text]
    pub text: TextComponent,
}

#[derive(Debug, Default, FromYtNode)]
pub(crate) struct AttachmentRun {
    pub element: AttachmentRunElement,
}

#[derive(Debug, Default, FromYtNode)]
pub(crate) struct AttachmentRunElement {
    #[ytq(."type")]
    pub typ: AttachmentRunElementType,
}

#[derive(Debug, Default, FromYtNode)]
pub(crate) struct AttachmentRunElementType {
    pub image_type: AttachmentRunElementImageType,
}

#[derive(Debug, Default, FromYtNode)]
pub(crate) struct AttachmentRunElementImageType {
    pub image: AttachmentRunElementImage,
}

#[derive(Debug, Default, FromYtNode)]
pub(crate) struct AttachmentRunElementImage {
    #[ytq_lossy]
    pub sources: Vec<AttachmentRunElementImageSource>,
}

#[derive(Debug, Default, FromYtNode)]
pub(crate) struct AttachmentRunElementImageSource {
    pub client_resource: ClientResource,
}

#[derive(Debug, Default, FromYtNode)]
pub(crate) struct ClientResource {
    pub image_name: IconName,
}

yt_string_enum! {
    pub enum IconName {
        CheckCircleFilled = "CHECK_CIRCLE_FILLED",
        MusicFilled = "MUSIC_FILLED" | "AUDIO_BADGE",
    }
    default: IconName::CheckCircleFilled
}

// CONTINUATION

#[derive(Debug)]
pub(crate) struct MusicContinuationData {
    pub next_continuation_data: MusicContinuationDataInner,
}

impl MusicContinuationData {
    pub(crate) fn from_node(node: &crate::json::JsonNode<'_>) -> Option<Self> {
        Some(Self {
            next_continuation_data: MusicContinuationDataInner::from_node(node)?,
        })
    }
}

deserialize_through_node!(MusicContinuationData);

#[derive(Debug)]
pub(crate) struct MusicContinuationDataInner {
    pub continuation: String,
}

impl MusicContinuationDataInner {
    pub(crate) fn from_node(node: &crate::json::JsonNode<'_>) -> Option<Self> {
        let cont = node
            .query(crate::json::ytq!(
                .nextContinuationData.continuation
                || .nextRadioContinuationData.continuation
            ))
            .and_then(|n| n.as_str())?;
        Some(Self { continuation: cont.to_owned() })
    }
}

deserialize_through_node!(MusicContinuationDataInner);

// ERROR

#[derive(Debug)]
pub(crate) struct ErrorResponse {
    pub error: ErrorResponseContent,
}

impl ErrorResponse {
    pub(crate) fn from_node(node: &crate::json::JsonNode<'_>) -> Option<Self> {
        Some(Self {
            error: ErrorResponseContent::from_node(node)?,
        })
    }
}

deserialize_through_node!(ErrorResponse);

#[derive(Debug)]
pub(crate) struct ErrorResponseContent {
    pub message: String,
}

impl ErrorResponseContent {
    pub(crate) fn from_node(node: &crate::json::JsonNode<'_>) -> Option<Self> {
        Some(Self {
            message: node
                .query(crate::json::ytq!(.error.message))
                .or_else(|| node.query(crate::json::ytq!(.message)))
                .and_then(|n| n.as_str())?
                .to_owned(),
        })
    }
}

deserialize_through_node!(ErrorResponseContent);

// DESERIALIZER

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
            Error(JsonValue),
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
                                value_to_json_string(&value)
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

#[derive(Debug, Default, FromYtNode)]
pub(crate) struct PhMetadataView {
    pub content_metadata_view_model: PhMetadataView2,
}

#[derive(Debug, Default, FromYtNode)]
pub(crate) struct PhMetadataView2 {
    #[ytq_lossy]
    pub metadata_rows: Vec<PhMetadataRow>,
}

#[derive(Debug, Default, FromYtNode)]
pub(crate) struct PhMetadataRow {
    #[ytq_lossy]
    pub metadata_parts: Vec<MetadataPart>,
}

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub(crate) enum MetadataPart {
    Text {
        #[serde_as(as = "AttributedText")]
        text: TextComponent,
    },
    #[serde(rename_all = "camelCase")]
    AvatarStack { avatar_stack: AvatarStackInner },
}

#[derive(Debug, Default, FromYtNode)]
pub(crate) struct AvatarStackInner {
    pub avatar_stack_view_model: TextComponentBox,
}

impl MetadataPart {
    pub fn into_text_component(self) -> TextComponent {
        match self {
            MetadataPart::Text { text } => text,
            MetadataPart::AvatarStack { avatar_stack } => avatar_stack.avatar_stack_view_model.text,
        }
    }

    pub fn as_str(&self) -> &str {
        match self {
            MetadataPart::Text { text } => text.as_str(),
            MetadataPart::AvatarStack { avatar_stack } => {
                avatar_stack.avatar_stack_view_model.text.as_str()
            }
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

#[derive(Debug, Default, FromYtNode)]
pub(crate) struct ThumbnailViewModelWrap {
    pub thumbnail_view_model: ImageViewOl,
}

#[derive(Debug, Default, FromYtNode)]
pub(crate) struct ImageViewOl {
    pub image: Thumbnails,
    #[ytq_lossy]
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
