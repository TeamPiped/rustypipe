use std::ops::Range;

use serde::Deserialize;
use serde_with::serde_as;
use serde_with::{json::JsonString, DefaultOnError};

use super::Thumbnails;
use crate::serializer::{text::Text, MapResult, VecLogError};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Player {
    pub playability_status: PlayabilityStatus,
    pub streaming_data: Option<StreamingData>,
    pub captions: Option<Captions>,
    pub video_details: Option<VideoDetails>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "status", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PlayabilityStatus {
    #[serde(rename_all = "camelCase")]
    Ok { live_streamability: Option<Empty> },
    /// Video cant be played because of DRM / Geoblock
    #[serde(rename_all = "camelCase")]
    Unplayable { reason: String },
    /// Age limit / Private video
    #[serde(rename_all = "camelCase")]
    LoginRequired { reason: String },
    #[serde(rename_all = "camelCase")]
    LiveStreamOffline { reason: String },
    /// Video was censored / deleted
    #[serde(rename_all = "camelCase")]
    Error { reason: String },
}

#[derive(Debug, Deserialize)]
pub struct Empty {}

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StreamingData {
    #[serde_as(as = "JsonString")]
    pub expires_in_seconds: u32,
    #[serde(default)]
    #[serde_as(as = "VecLogError<_>")]
    pub formats: MapResult<Vec<Format>>,
    #[serde(default)]
    #[serde_as(as = "VecLogError<_>")]
    pub adaptive_formats: MapResult<Vec<Format>>,
    /// Only on livestreams
    pub dash_manifest_url: Option<String>,
    /// Only on livestreams
    pub hls_manifest_url: Option<String>,
}

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Format {
    pub itag: u32,
    pub url: Option<String>,

    #[serde(default, rename = "type")]
    pub format_type: FormatType,

    pub mime_type: String,

    pub bitrate: u32,

    pub width: Option<u32>,
    pub height: Option<u32>,

    #[serde_as(as = "Option<crate::serializer::Range>")]
    pub index_range: Option<Range<u32>>,
    #[serde_as(as = "Option<crate::serializer::Range>")]
    pub init_range: Option<Range<u32>>,

    #[serde_as(as = "Option<JsonString>")]
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
    #[serde_as(as = "Option<JsonString>")]
    pub audio_sample_rate: Option<u32>,
    pub audio_channels: Option<u8>,
    pub loudness_db: Option<f64>,
    pub audio_track: Option<AudioTrack>,

    pub signature_cipher: Option<String>,
}

impl Format {
    pub fn is_audio(&self) -> bool {
        self.content_length.is_some()
            && self.audio_quality.is_some()
            && self.audio_sample_rate.is_some()
    }

    pub fn is_video(&self) -> bool {
        self.quality.is_some()
            && self.quality_label.is_some()
            && self.fps.is_some()
            && self.height.is_some()
            && self.width.is_some()
    }
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[serde(rename_all = "lowercase")]
pub enum Quality {
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

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum AudioQuality {
    #[serde(rename = "AUDIO_QUALITY_LOW", alias = "low")]
    Low,
    #[serde(rename = "AUDIO_QUALITY_MEDIUM", alias = "medium")]
    Medium,
    #[serde(rename = "AUDIO_QUALITY_HIGH", alias = "high")]
    High,
}

#[derive(Default, Clone, Copy, Debug, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum FormatType {
    #[default]
    Default,
    /// This stream only works via DASH and not via progressive HTTP.
    FormatStreamTypeOtf,
}

#[derive(Default, Debug, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ColorInfo {
    pub primaries: Primaries,
}

#[derive(Default, Clone, Copy, Debug, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Primaries {
    #[default]
    ColorPrimariesBt709,
    ColorPrimariesBt2020,
}

#[derive(Default, Debug, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct AudioTrack {
    pub id: String,
    pub display_name: String,
    pub audio_is_default: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Captions {
    pub player_captions_tracklist_renderer: PlayerCaptionsTracklistRenderer,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlayerCaptionsTracklistRenderer {
    pub caption_tracks: Vec<CaptionTrack>,
}

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CaptionTrack {
    pub base_url: String,
    #[serde_as(as = "Text")]
    pub name: String,
    pub language_code: String,
}

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VideoDetails {
    pub video_id: String,
    pub title: String,
    #[serde_as(as = "JsonString")]
    pub length_seconds: u32,
    #[serde(default)]
    pub keywords: Vec<String>,
    pub channel_id: String,
    pub short_description: Option<String>,
    #[serde(default)]
    pub thumbnail: Thumbnails,
    #[serde_as(as = "JsonString")]
    pub view_count: u64,
    pub author: String,
    pub is_live_content: bool,
}
