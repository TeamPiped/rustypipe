use std::cmp::Ordering;

use crate::model::AudioCodec;

use super::{AudioStream, VideoStream};

pub trait QualityOrd {
    fn quality_cmp(&self, other: &Self) -> Ordering;
}

impl QualityOrd for VideoStream {
    fn quality_cmp(&self, other: &Self) -> Ordering {
        match (self.width * self.height).cmp(&(other.width * other.height)) {
            Ordering::Less => Ordering::Less,
            Ordering::Greater => Ordering::Greater,
            Ordering::Equal => match self.codec.cmp(&other.codec) {
                Ordering::Less => Ordering::Less,
                Ordering::Greater => Ordering::Greater,
                Ordering::Equal => self.average_bitrate.cmp(&other.average_bitrate),
            },
        }
    }
}

impl QualityOrd for AudioStream {
    fn quality_cmp(&self, other: &Self) -> Ordering {
        fn cmp_bitrate(s: &AudioStream) -> u32 {
            match s.codec {
                // Opus is more efficient
                AudioCodec::Opus => (s.average_bitrate as f32 * 1.3) as u32,
                _ => s.average_bitrate,
            }
        }

        cmp_bitrate(self).cmp(&cmp_bitrate(other))
    }
}
