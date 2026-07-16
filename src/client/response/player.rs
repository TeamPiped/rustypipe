use std::ops::Range;

use serde::{Deserialize, Deserializer};
use serde_with::serde_as;
use serde_with::{DefaultOnError, DisplayFromStr, VecSkipError};

use super::{Empty, Thumbnails};
use crate::json::{JsonGet, JsonValue};
use crate::serializer::{text::Text, MapResult};
use crate::yt_string_enum;
use crate::FromYtNode;

yt_string_enum! {
    pub(crate) enum Quality {
        Tiny = "tiny",
        Small = "small",
        Medium = "medium",
        Large = "large",
        Highres = "highres",
        Hd720 = "hd720",
        Hd1080 = "hd1080",
        Hd1440 = "hd1440",
        Hd2160 = "hd2160",
    }
    default: Quality::Medium
}

yt_string_enum! {
    pub(crate) enum AudioQuality {
        UltraLow = "AUDIO_QUALITY_ULTRALOW",
        Low = "AUDIO_QUALITY_LOW",
        Medium = "AUDIO_QUALITY_MEDIUM",
        High = "AUDIO_QUALITY_HIGH",
    }
    default: AudioQuality::Medium
}

yt_string_enum! {
    pub(crate) enum FormatType {
        Default = "",
        /// This stream only works via DASH and not via progressive HTTP.
        FormatStreamTypeOtf = "FORMAT_STREAM_TYPE_OTF",
    }
    default: FormatType::Default,
    fallback_to_default
}

yt_string_enum! {
    pub(crate) enum Primaries {
        ColorPrimariesBt709 = "COLOR_PRIMARIES_BT709",
        ColorPrimariesBt2020 = "COLOR_PRIMARIES_BT2020",
    }
    default: Primaries::ColorPrimariesBt709
}

yt_string_enum! {
    #[allow(clippy::enum_variant_names)]
    pub(crate) enum DrmTrackType {
        DrmTrackTypeAudio = "DRM_TRACK_TYPE_AUDIO",
        DrmTrackTypeSd = "DRM_TRACK_TYPE_SD",
        DrmTrackTypeHd = "DRM_TRACK_TYPE_HD",
        DrmTrackTypeUhd1 = "DRM_TRACK_TYPE_UHD1",
    }
    default: DrmTrackType::DrmTrackTypeAudio
}

yt_string_enum! {
    pub(crate) enum DrmFamily {
        Widevine = "WIDEVINE",
        Playready = "PLAYREADY",
        Fairplay = "FAIRPLAY",
    }
    default: DrmFamily::Widevine
}

#[derive(Default, Debug, FromYtNode)]
pub(crate) struct ColorInfo {
    pub primaries: Primaries,
}

#[derive(Debug)]
pub(crate) enum PlayabilityStatus {
    Ok { live_streamability: Option<Empty> },
    /// Video cant be played because of DRM / Geoblock
    Unplayable {
        reason: String,
        error_screen: ErrorScreen,
    },
    /// Age limit / Private video
    LoginRequired {
        reason: String,
        messages: Vec<String>,
    },
    LiveStreamOffline {
        reason: String,
    },
    /// Video was censored / deleted
    Error {
        reason: String,
    },
}

impl<'de> Deserialize<'de> for PlayabilityStatus {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = JsonValue::deserialize(deserializer)?;
        let status = value
            .get_str("status")
            .ok_or_else(|| serde::de::Error::missing_field("status"))?;
        // The `errorScreen`/`playerErrorMessageRenderer` shim uses `Text`
        // (rich text) so we round-trip the entire `value` for that branch
        // through `flexon::from_str`. The other variants are simple enough
        // to extract via ytq!.
        match status.as_str() {
            "OK" => Ok(Self::Ok {
                live_streamability: value
                    .get("liveStreamability")
                    .filter(|v| v.is_object())
                    .map(|_| Empty {}),
            }),
            "UNPLAYABLE" => {
                let raw = crate::json::value_to_json_string(&value);
                let unplayable: Unplayable = flexon::from_str(&raw)
                    .map_err(|e| serde::de::Error::custom(format!("unplayable: {e}")))?;
                Ok(Self::Unplayable {
                    reason: unplayable.reason,
                    error_screen: unplayable.error_screen,
                })
            }
            "LOGIN_REQUIRED" => {
                let raw = crate::json::value_to_json_string(&value);
                let login: LoginRequired = flexon::from_str(&raw)
                    .map_err(|e| serde::de::Error::custom(format!("login_required: {e}")))?;
                Ok(Self::LoginRequired {
                    reason: login.reason,
                    messages: login.messages,
                })
            }
            "LIVE_STREAM_OFFLINE" => Ok(Self::LiveStreamOffline {
                reason: value.get_str("reason").unwrap_or_default(),
            }),
            "ERROR" => Ok(Self::Error {
                reason: value.get_str("reason").unwrap_or_default(),
            }),
            other => Err(serde::de::Error::custom(format!(
                "unknown playability status: {other}"
            ))),
        }
    }
}

#[serde_as]
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Unplayable {
    #[serde(default)]
    reason: String,
    #[serde(default)]
    error_screen: ErrorScreen,
}

#[serde_as]
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct LoginRequired {
    #[serde(default)]
    reason: String,
    #[serde(default)]
    messages: Vec<String>,
}

#[derive(Default, Debug, FromYtNode)]
pub(crate) struct ErrorScreen {
    pub player_error_message_renderer: Option<ErrorMessage>,
    pub player_captcha_view_model: Option<Empty>,
}

#[serde_as]
#[derive(Default, Debug, FromYtNode)]
pub(crate) struct ErrorMessage {
    #[ytq_text]
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
    /// URL pointing to a SABR/UMP stream (returned when SABR is used).
    pub server_abr_streaming_url: Option<String>,
    /// base64-encoded ustreamer config blob (required for SABR).
    pub ustreamer_config: Option<String>,
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

    /// Last-modified timestamp from the stream URL (`lmt` parameter).
    /// Used by SABR to identify the format.
    #[serde_as(as = "Option<DisplayFromStr>")]
    pub last_modified: Option<u64>,
    /// `xtags` from the stream URL. Used by SABR.
    pub xtags: Option<String>,

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

#[derive(Default, Debug, FromYtNode)]
pub(crate) struct AudioTrack {
    #[ytq_default]
    pub id: String,
    #[ytq_default]
    pub display_name: String,
    #[ytq_default]
    pub audio_is_default: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Captions {
    #[serde(rename = "playerCaptionsTracklistRenderer")]
    pub tracklist: PlayerCaptionsTracklistRenderer,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PlayerCaptionsTracklistRenderer {
    pub caption_tracks: Vec<CaptionTrack>,
}

#[derive(Debug, FromYtNode)]
pub(crate) struct CaptionTrack {
    pub base_url: String,
    #[ytq_text]
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
    #[serde(rename = "playerStoryboardSpecRenderer")]
    pub storyboard: StoryboardRenderer,
}

#[derive(Debug)]
pub(crate) struct StoryboardRenderer {
    pub spec: String,
}

impl<'de> Deserialize<'de> for StoryboardRenderer {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = JsonValue::deserialize(deserializer)?;
        Ok(Self {
            spec: value
                .get("spec")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_owned(),
        })
    }
}

#[derive(Default, Debug, FromYtNode)]
pub(crate) struct PlayerConfig {
    pub web_drm_config: Option<WebDrmConfig>,
}

#[derive(Default, Debug, FromYtNode)]
pub(crate) struct WebDrmConfig {
    pub widevine_service_cert: Option<String>,
}

#[derive(Default, Debug, FromYtNode)]
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

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AuthorizedFormat {
    pub track_type: DrmTrackType,
    pub key_id: String,
}
