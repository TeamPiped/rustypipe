use std::ops::Range;

use serde::Deserialize;
use serde_with::serde_as;
use serde_with::{DefaultOnError, DisplayFromStr, VecSkipError};

use super::{Empty, ResponseContext, Thumbnails};
use crate::serializer::{text::Text, MapResult};

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Player {
    pub playability_status: PlayabilityStatus,
    pub streaming_data: Option<StreamingData>,
    pub captions: Option<Captions>,
    pub video_details: Option<VideoDetails>,
    #[serde(default)]
    #[serde_as(deserialize_as = "DefaultOnError")]
    pub storyboards: Option<Storyboards>,
    pub response_context: ResponseContext,
    #[serde(default)]
    pub player_config: PlayerConfig,
    #[serde(default)]
    pub heartbeat_params: HeartbeatParams,
}

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(tag = "status", rename_all = "SCREAMING_SNAKE_CASE")]
pub(crate) enum PlayabilityStatus {
    #[serde(rename_all = "camelCase")]
    Ok { live_streamability: Option<Empty> },
    /// Video cant be played because of DRM / Geoblock
    #[serde(rename_all = "camelCase")]
    Unplayable {
        #[serde(default)]
        reason: String,
        #[serde(default)]
        error_screen: ErrorScreen,
    },
    /// Age limit / Private video
    #[serde(rename_all = "camelCase")]
    LoginRequired {
        #[serde(default)]
        reason: String,
        #[serde(default)]
        messages: Vec<String>,
    },
    #[serde(rename_all = "camelCase")]
    LiveStreamOffline {
        #[serde(default)]
        reason: String,
    },
    /// Video was censored / deleted
    #[serde(rename_all = "camelCase")]
    Error {
        #[serde(default)]
        reason: String,
    },
}

#[serde_as]
#[derive(Default, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ErrorScreen {
    #[serde(default)]
    #[serde_as(deserialize_as = "DefaultOnError")]
    pub player_error_message_renderer: Option<ErrorMessage>,
    pub player_captcha_view_model: Option<Empty>,
}

#[serde_as]
#[derive(Default, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ErrorMessage {
    #[serde_as(as = "Text")]
    pub subreason: String,
}

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StreamingData {
    #[serde_as(as = "DisplayFromStr")]
    pub expires_in_seconds: u32,
    #[serde(default)]
    pub formats: MapResult<Vec<Format>>,
    #[serde(default)]
    pub adaptive_formats: MapResult<Vec<Format>>,
    /// Only on livestreams
    pub dash_manifest_url: Option<String>,
    /// Only on livestreams
    pub hls_manifest_url: Option<String>,
    pub drm_params: Option<String>,
    #[serde(default)]
    #[serde_as(deserialize_as = "VecSkipError<_>")]
    pub initial_authorized_drm_track_types: Vec<DrmTrackType>,
}

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Format {
    pub itag: u32,
    pub url: Option<String>,

    #[serde(default, rename = "type")]
    pub format_type: FormatType,

    pub mime_type: String,

    pub bitrate: u32,

    pub width: Option<u32>,
    pub height: Option<u32>,
    #[serde_as(as = "Option<DisplayFromStr>")]
    pub approx_duration_ms: Option<u32>,

    #[serde_as(as = "Option<crate::serializer::Range>")]
    pub index_range: Option<Range<u32>>,
    #[serde_as(as = "Option<crate::serializer::Range>")]
    pub init_range: Option<Range<u32>>,

    #[serde_as(as = "Option<DisplayFromStr>")]
    pub content_length: Option<u64>,

    #[serde(default)]
    #[serde_as(deserialize_as = "DefaultOnError")]
    pub quality: Option<Quality>,
    pub fps: Option<u8>,
    pub quality_label: Option<String>,
    pub average_bitrate: Option<u32>,
    pub color_info: Option<ColorInfo>,

    // Audio only
    #[serde(default)]
    #[serde_as(deserialize_as = "DefaultOnError")]
    pub audio_quality: Option<AudioQuality>,
    #[serde_as(as = "Option<DisplayFromStr>")]
    pub audio_sample_rate: Option<u32>,
    pub audio_channels: Option<u8>,
    pub loudness_db: Option<f32>,
    pub audio_track: Option<AudioTrack>,

    pub signature_cipher: Option<String>,

    #[serde(default)]
    #[serde_as(deserialize_as = "VecSkipError<_>")]
    pub drm_families: Vec<DrmFamily>,
    pub drm_track_type: Option<DrmTrackType>,
}

impl Format {
    pub fn is_audio(&self) -> bool {
        self.audio_quality.is_some() && self.audio_sample_rate.is_some()
    }

    pub fn is_video(&self) -> bool {
        self.quality.is_some()
            && self.quality_label.is_some()
            && self.fps.is_some()
            && self.height.is_some()
            && self.width.is_some()
    }
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "lowercase")]
pub(crate) enum Quality {
    Tiny,
    Small,
    Medium,
    Large,
    Highres,
    Hd720,
    Hd1080,
    Hd1440,
    Hd2160,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum AudioQuality {
    #[serde(rename = "AUDIO_QUALITY_ULTRALOW")]
    UltraLow,
    #[serde(rename = "AUDIO_QUALITY_LOW")]
    Low,
    #[serde(rename = "AUDIO_QUALITY_MEDIUM")]
    Medium,
    #[serde(rename = "AUDIO_QUALITY_HIGH")]
    High,
}

#[derive(Default, Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub(crate) enum FormatType {
    #[default]
    Default,
    /// This stream only works via DASH and not via progressive HTTP.
    FormatStreamTypeOtf,
}

#[derive(Default, Debug, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub(crate) struct ColorInfo {
    pub primaries: Primaries,
}

#[derive(Default, Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub(crate) enum Primaries {
    #[default]
    ColorPrimariesBt709,
    ColorPrimariesBt2020,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[allow(clippy::enum_variant_names)]
pub(crate) enum DrmTrackType {
    DrmTrackTypeAudio,
    DrmTrackTypeSd,
    DrmTrackTypeHd,
    DrmTrackTypeUhd1,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub(crate) enum DrmFamily {
    Widevine,
    Playready,
    Fairplay,
}

#[derive(Default, Debug, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub(crate) struct AudioTrack {
    pub id: String,
    pub display_name: String,
    pub audio_is_default: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Captions {
    pub player_captions_tracklist_renderer: PlayerCaptionsTracklistRenderer,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PlayerCaptionsTracklistRenderer {
    pub caption_tracks: Vec<CaptionTrack>,
}

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CaptionTrack {
    pub base_url: String,
    #[serde_as(as = "Text")]
    pub name: String,
    pub language_code: String,
}

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct VideoDetails {
    pub video_id: String,
    pub title: Option<String>,
    #[serde_as(as = "DisplayFromStr")]
    pub length_seconds: u32,
    #[serde(default)]
    pub keywords: Vec<String>,
    pub channel_id: String,
    pub short_description: Option<String>,
    #[serde(default)]
    pub thumbnail: Thumbnails,
    #[serde_as(as = "Option<DisplayFromStr>")]
    pub view_count: Option<u64>,
    pub author: Option<String>,
    pub is_live_content: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Storyboards {
    pub player_storyboard_spec_renderer: StoryboardRenderer,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StoryboardRenderer {
    pub spec: String,
}

#[derive(Default, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PlayerConfig {
    pub web_drm_config: Option<WebDrmConfig>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WebDrmConfig {
    pub widevine_service_cert: Option<String>,
}

#[derive(Default, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct HeartbeatParams {
    pub drm_session_id: Option<String>,
}

impl From<DrmTrackType> for crate::model::DrmTrackType {
    fn from(value: DrmTrackType) -> Self {
        match value {
            DrmTrackType::DrmTrackTypeAudio => Self::Audio,
            DrmTrackType::DrmTrackTypeSd => Self::Sd,
            DrmTrackType::DrmTrackTypeHd => Self::Hd,
            DrmTrackType::DrmTrackTypeUhd1 => Self::Uhd1,
        }
    }
}

impl From<DrmFamily> for crate::model::DrmSystem {
    fn from(value: DrmFamily) -> Self {
        match value {
            DrmFamily::Widevine => Self::Widevine,
            DrmFamily::Playready => Self::Playready,
            DrmFamily::Fairplay => Self::Fairplay,
        }
    }
}

#[derive(Default, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DrmLicense {
    pub status: String,
    pub license: String,
    pub authorized_formats: Vec<AuthorizedFormat>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AuthorizedFormat {
    pub track_type: DrmTrackType,
    pub key_id: String,
}
