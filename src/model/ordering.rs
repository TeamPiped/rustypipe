use std::cmp::Ordering;

use super::{AudioStream, AudioTrackType, VideoStream};
use crate::param::cmp_bitrate;

/// Trait for ordering YouTube video/audio streams by quality
///
/// analogous to [`std::cmp::Ord`]
pub trait QualityOrd {
    /// Compare two streams by quality
    ///
    /// analogous to [`std::cmp::Ord::cmp`]
    fn quality_cmp(&self, other: &Self) -> Ordering;
}

impl QualityOrd for VideoStream {
    fn quality_cmp(&self, other: &Self) -> Ordering {
        self.width
            .min(self.height)
            .cmp(&(other.width.min(other.height)))
            .then_with(|| self.hdr.cmp(&other.hdr))
            .then_with(|| self.fps.cmp(&other.fps))
            .then_with(|| self.codec.cmp(&other.codec))
            .then_with(|| self.average_bitrate.cmp(&other.average_bitrate))
    }
}

impl QualityOrd for AudioStream {
    fn quality_cmp(&self, other: &Self) -> Ordering {
        self.track
            .as_ref()
            .map(|t| track_type_rating(t.track_type))
            .cmp(
                &other
                    .track
                    .as_ref()
                    .map(|t| track_type_rating(t.track_type)),
            )
            .then_with(|| self.channels.cmp(&other.channels))
            .then_with(|| cmp_bitrate(self).cmp(&cmp_bitrate(other)))
    }
}

fn track_type_rating(track_type: Option<AudioTrackType>) -> i8 {
    track_type
        .map(|t| match t {
            AudioTrackType::Original => 2,
            AudioTrackType::Dubbed => 1,
            AudioTrackType::DubbedAuto => -1,
            AudioTrackType::Descriptive => -2,
        })
        .unwrap_or_default()
}
