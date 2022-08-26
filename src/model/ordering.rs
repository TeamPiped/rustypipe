use std::cmp::Ordering;

use crate::model::AudioCodec;

use super::{AudioStream, VideoStream};

impl PartialOrd for VideoStream {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(
            match (self.width * self.height).cmp(&(other.width * other.height)) {
                Ordering::Less => Ordering::Less,
                Ordering::Greater => Ordering::Greater,
                Ordering::Equal => match self.codec.cmp(&other.codec) {
                    Ordering::Less => Ordering::Less,
                    Ordering::Greater => Ordering::Greater,
                    Ordering::Equal => self.average_bitrate.cmp(&other.average_bitrate),
                },
            },
        )
    }
}

impl Ord for VideoStream {
    fn cmp(&self, other: &Self) -> Ordering {
        self.partial_cmp(other).unwrap()
    }
}

impl PartialOrd for AudioStream {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        fn cmp_bitrate(s: &AudioStream) -> u32 {
            match s.codec {
                // Opus is more efficient
                AudioCodec::Opus => (s.average_bitrate as f32 * 1.3) as u32,
                _ => s.average_bitrate,
            }
        }

        Some(cmp_bitrate(self).cmp(&cmp_bitrate(other)))
    }
}

impl Ord for AudioStream {
    fn cmp(&self, other: &Self) -> Ordering {
        self.partial_cmp(other).unwrap()
    }
}
