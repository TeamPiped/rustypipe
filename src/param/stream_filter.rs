//! Filters for selecting audio/video streams

use std::cmp::Ordering;

use crate::model::{
    traits::QualityOrd, AudioCodec, AudioFormat, AudioStream, AudioTrackType, DrmSystem,
    DrmTrackType, VideoCodec, VideoFormat, VideoPlayer, VideoStream,
};

/// The StreamFilter is used for selecting audio/video streams from an extracted video
#[derive(Debug, Default, Clone)]
pub struct StreamFilter {
    audio_max_bitrate: Option<u32>,
    audio_formats: Option<Vec<AudioFormat>>,
    audio_codecs: Option<Vec<AudioCodec>>,
    audio_max_channels: Option<u8>,
    audio_languages: Vec<String>,
    audio_autodub: bool,
    audio_descriptive: bool,
    video_max_res: Option<u32>,
    video_max_fps: Option<u8>,
    video_formats: Option<Vec<VideoFormat>>,
    video_codecs: Option<Vec<VideoCodec>>,
    video_hdr: bool,
    video_none: bool,
    drm_track_types: Vec<DrmTrackType>,
    drm_system: Option<DrmSystem>,
}

const N_RES_AUDIO: usize = 4;
const N_RES_VIDEO: usize = 5;
type AudioRes = Option<[i64; N_RES_AUDIO]>;
type VideoRes = Option<[i64; N_RES_VIDEO]>;

impl StreamFilter {
    /// Create a new [`StreamFilter`]
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the maximum audio bitrate in bits per second.
    ///
    /// This is a soft filter, so if there is no stream with a bitrate
    /// <= the limit, the stream with the next higher bitrate is returned.
    #[must_use]
    pub fn audio_max_bitrate(mut self, max_bitrate: u32) -> Self {
        self.audio_max_bitrate = Some(max_bitrate);
        self
    }

    /// Set the supported audio container formats
    #[must_use]
    pub fn audio_formats<F: Into<Vec<AudioFormat>>>(mut self, formats: F) -> Self {
        self.audio_formats = Some(formats.into());
        self
    }

    /// Set the supported audio codecs
    #[must_use]
    pub fn audio_codecs<C: Into<Vec<AudioCodec>>>(mut self, codecs: C) -> Self {
        self.audio_codecs = Some(codecs.into());
        self
    }

    /// Set the maximum number of audio channels
    #[must_use]
    pub fn audio_max_channels(mut self, max_channels: u8) -> Self {
        self.audio_max_channels = Some(max_channels);
        self
    }

    /// Set the preferred audio languages
    /// Some YouTube videos feature multiple audio streams in
    /// different languages (e.g. <https://www.youtube.com/watch?v=tVWWp1PqDus>).
    ///
    /// If this filter is unset or no stream matches,
    /// the filter returns the default audio stream.
    #[must_use]
    pub fn audio_languages<S: Into<Vec<String>>>(mut self, languages: S) -> Self {
        self.audio_languages = languages.into();
        self
    }
    /// Set the preferred audio language
    /// Some YouTube videos feature multiple audio streams in
    /// different languages (e.g. <https://www.youtube.com/watch?v=tVWWp1PqDus>).
    ///
    /// If this filter is unset or no stream matches,
    /// the filter returns the default audio stream.
    #[must_use]
    pub fn audio_language<S: Into<String>>(mut self, language: S) -> Self {
        self.audio_languages = vec![language.into()];
        self
    }

    /// Select the descriptive audio track for visually impaired people if available
    #[must_use]
    pub fn audio_descriptive(mut self) -> Self {
        self.audio_descriptive = true;
        self
    }

    /// Allow audio tracks that are AI-dubbed by YouTube
    ///
    /// By default these are never selected.
    #[must_use]
    pub fn audio_autodub(mut self) -> Self {
        self.audio_autodub = true;
        self
    }

    /// Set the maximum video resolution. Resolution is determined by the
    /// pixel count of the shorter edge (e.g. 1080p).
    ///
    /// This is a soft filter, so if there is no stream with a resolution
    /// <= the limit, the stream with the next higher resolution is returned.
    #[must_use]
    pub fn video_max_res(mut self, max_res: u32) -> Self {
        self.video_max_res = Some(max_res);
        self
    }

    /// Set the maximum video framerate.
    ///
    /// This is a soft filter, so if there is no stream with a framerate
    /// <= the limit, the stream with the next higher framerate is returned.
    #[must_use]
    pub fn video_max_fps(mut self, max_fps: u8) -> Self {
        self.video_max_fps = Some(max_fps);
        self
    }

    /// Set the supported video container formats
    #[must_use]
    pub fn video_formats<F: Into<Vec<VideoFormat>>>(mut self, formats: F) -> Self {
        self.video_formats = Some(formats.into());
        self
    }

    /// Set the supported video codecs
    #[must_use]
    pub fn video_codecs<C: Into<Vec<VideoCodec>>>(mut self, codecs: C) -> Self {
        self.video_codecs = Some(codecs.into());
        self
    }

    /// Allow HDR videos
    #[must_use]
    pub fn video_hdr(mut self) -> Self {
        self.video_hdr = true;
        self
    }

    /// Output no video stream (audio only)
    #[must_use]
    pub fn no_video(mut self) -> Self {
        self.video_none = true;
        self
    }

    /// Allow DRM protected streams of the given track types
    ///
    /// By default no DRM-protected streams are returned
    #[must_use]
    pub fn drm_track_types<T: Into<Vec<DrmTrackType>>>(mut self, track_types: T) -> Self {
        self.drm_track_types = track_types.into();
        self
    }

    /// Allow DRM protected streams that can be played back with the given DRM system
    ///
    /// By default no DRM-protected streams are returned
    #[must_use]
    pub fn drm_system(mut self, drm_system: DrmSystem) -> Self {
        self.drm_system = Some(drm_system);
        self
    }

    fn check_drm(&self, track_type: Option<DrmTrackType>, drm_systems: &[DrmSystem]) -> Option<()> {
        if let Some(track_type) = track_type {
            if !self.drm_track_types.contains(&track_type) {
                return None;
            }
            if !drm_systems.contains(&self.drm_system?) {
                return None;
            }
        }
        Some(())
    }

    fn apply_audio(&self, stream: &AudioStream) -> AudioRes {
        let bitrate = match self.audio_max_bitrate {
            Some(max) => {
                if stream.average_bitrate > max {
                    i64::from(max).wrapping_sub(stream.average_bitrate.into())
                } else {
                    i64::from(cmp_bitrate(stream))
                }
            }
            None => i64::from(cmp_bitrate(stream)),
        };

        if let Some(formats) = &self.audio_formats {
            if !formats.contains(&stream.format) {
                return None;
            }
        }

        if let Some(codecs) = &self.audio_codecs {
            if !codecs.contains(&stream.codec) {
                return None;
            }
        }

        let language = if self.audio_languages.is_empty() {
            0
        } else {
            match &stream.track {
                Some(track) => match &track.lang {
                    Some(lang) => {
                        if self.audio_languages.contains(lang) {
                            10
                        } else if let Some((lang, _)) = lang.split_once('-') {
                            if self.audio_languages.contains(&lang.to_owned()) {
                                5
                            } else {
                                -1
                            }
                        } else {
                            -10
                        }
                    }
                    None => 0,
                },
                None => 0,
            }
        };

        let track_type = match &stream.track {
            Some(track) => match track.track_type {
                Some(AudioTrackType::Original) => 5,
                Some(AudioTrackType::Descriptive) => {
                    if self.audio_descriptive {
                        10
                    } else {
                        return None;
                    }
                }
                Some(AudioTrackType::Dubbed) => 0,
                Some(AudioTrackType::DubbedAuto) => {
                    if self.audio_autodub {
                        -1
                    } else {
                        return None;
                    }
                }
                None => 0,
            },
            None => 0,
        };

        let channels = stream.channels.unwrap_or_default();
        if let Some(max_channels) = self.audio_max_channels {
            if channels > max_channels {
                return None;
            }
        }

        self.check_drm(stream.drm_track_type, &stream.drm_systems)?;

        Some([language, track_type, channels.into(), bitrate])
    }

    fn apply_video(&self, stream: &VideoStream) -> VideoRes {
        let vres = stream.height.min(stream.width);
        let res = match self.video_max_res {
            Some(max) => filter_max(vres, max),
            None => vres.into(),
        };

        let fps = match self.video_max_fps {
            Some(max) => filter_max(stream.fps.into(), max.into()),
            None => i64::from(stream.fps),
        };

        if let Some(formats) = &self.video_formats {
            if !formats.contains(&stream.format) {
                return None;
            }
        }

        if let Some(codecs) = &self.video_codecs {
            if !codecs.contains(&stream.codec) {
                return None;
            }
        }
        let codecs = match stream.codec {
            VideoCodec::Unknown => -1,
            VideoCodec::Mp4v => 1,
            VideoCodec::Avc1 => 2,
            VideoCodec::Vp9 => 3,
            VideoCodec::Av01 => 4,
        };

        let hdr = if self.video_hdr {
            if stream.hdr {
                10
            } else {
                0
            }
        } else if stream.hdr {
            return None;
        } else {
            0
        };

        self.check_drm(stream.drm_track_type, &stream.drm_systems)?;

        Some([res, hdr, fps, codecs, stream.average_bitrate.into()])
    }

    /// Return true if no video stream should be selected
    pub fn is_video_none(&self) -> bool {
        self.video_none
    }
}

fn filter_max(val: u32, max: u32) -> i64 {
    if val > max {
        i64::from(max).wrapping_sub(val.into())
    } else {
        val.into()
    }
}

impl VideoPlayer {
    /// Select the audio stream which is the best match for the given [`StreamFilter`]
    #[must_use]
    pub fn select_audio_stream(&self, filter: &StreamFilter) -> Option<&AudioStream> {
        self.audio_streams
            .iter()
            .filter_map(|s| filter.apply_audio(s).map(|r| (s, r)))
            .max_by_key(|(_, r)| *r)
            .map(|(s, _)| s)
    }

    fn _select_video_stream<'a>(
        streams: &'a [VideoStream],
        filter: &StreamFilter,
    ) -> Option<&'a VideoStream> {
        if filter.video_none {
            return None;
        }

        streams
            .iter()
            .filter_map(|s| filter.apply_video(s).map(|r| (s, r)))
            .max_by_key(|(_, r)| *r)
            .map(|(s, _)| s)
    }

    /// Select the video stream which is the best match for the given [`StreamFilter`]
    pub fn select_video_stream(&self, filter: &StreamFilter) -> Option<&VideoStream> {
        Self::_select_video_stream(&self.video_streams, filter)
    }

    /// Select the video-only stream which is the best match for the given [`StreamFilter`]
    pub fn select_video_only_stream(&self, filter: &StreamFilter) -> Option<&VideoStream> {
        Self::_select_video_stream(&self.video_only_streams, filter)
    }

    /// Select a video and audio stream which is the best match for the given [`StreamFilter`]
    pub fn select_video_audio_stream(
        &self,
        filter: &StreamFilter,
    ) -> (Option<&VideoStream>, Option<&AudioStream>) {
        let video_stream = self.select_video_stream(filter);
        let video_only_stream = self.select_video_only_stream(filter);

        match (video_stream, video_only_stream) {
            (None, None) => (None, self.select_audio_stream(filter)),
            (None, Some(video_only_stream)) => {
                (Some(video_only_stream), self.select_audio_stream(filter))
            }
            (Some(video_stream), None) => (Some(video_stream), None),
            (Some(video_stream), Some(video_only_stream)) => {
                match video_only_stream.quality_cmp(video_stream) {
                    Ordering::Greater => match self.select_audio_stream(filter) {
                        Some(audio_stream) => (Some(video_only_stream), Some(audio_stream)),
                        None => (Some(video_stream), None),
                    },
                    _ => (Some(video_stream), None),
                }
            }
        }
    }
}

pub(crate) fn cmp_bitrate(s: &AudioStream) -> u32 {
    match s.codec {
        // Opus is more efficient
        AudioCodec::Opus | AudioCodec::Ac3 => (s.average_bitrate as f32 * 1.3) as u32,
        // Dolby audio should be preferred
        AudioCodec::Ec3 => (s.average_bitrate as f32 * 1.5) as u32,
        _ => s.average_bitrate,
    }
}

#[cfg(test)]
mod tests {
    use std::{fs::File, io::BufReader};

    use once_cell::sync::Lazy;
    use path_macro::path;
    use rstest::rstest;

    use super::*;
    use crate::util::tests::TESTFILES;

    static PLAYER_ML: Lazy<VideoPlayer> = Lazy::new(|| {
        let json_path = path!(*TESTFILES / "player_model" / "multilanguage.json");
        let json_file = File::open(json_path).unwrap();

        serde_json::from_reader(BufReader::new(json_file)).unwrap()
    });

    static PLAYER_HDR: Lazy<VideoPlayer> = Lazy::new(|| {
        let json_path = path!(*TESTFILES / "player_model" / "hdr.json");
        let json_file = File::open(json_path).unwrap();

        serde_json::from_reader(BufReader::new(json_file)).unwrap()
    });

    static PLAYER_SURROUND: Lazy<VideoPlayer> = Lazy::new(|| {
        let json_path = path!(*TESTFILES / "player_model" / "surround.json");
        let json_file = File::open(json_path).unwrap();

        serde_json::from_reader(BufReader::new(json_file)).unwrap()
    });

    static PLAYER_DRM: Lazy<VideoPlayer> = Lazy::new(|| {
        let json_path = path!(*TESTFILES / "player_model" / "drm.json");
        let json_file = File::open(json_path).unwrap();

        serde_json::from_reader(BufReader::new(json_file)).unwrap()
    });

    #[rstest]
    #[case::default(&PLAYER_ML, StreamFilter::new(), Some("https://rr4---sn-h0jeener.googlevideo.com/videoplayback?c=WEB&clen=16104136&dur=1012.661&ei=6OtcZNqtBdOi7gP1upHYCQ&expire=1683832904&fexp=24007246&fvip=2&gir=yes&id=o-ABVtPh3j24hkJeXp8igjvreyODn-oV0CacOqb7pDjJoG&initcwndbps=1720000&ip=2003%3Ade%3Aaf31%3A5200%3A791a%3A897%3Ac15c%3Aae59&itag=251&keepalive=yes&lmt=1683782301237288&lsig=AG3C_xAwRQIgC7HZtYuc6dI92m6wCcoXYpdzSpVtPTIbO7jBKGpUrYMCIQCc0WNtFvN8Awqx9uuRVp5SUSe3rOt2D7M-rCKpgVv_0A%3D%3D&lsparams=mh%2Cmm%2Cmn%2Cms%2Cmv%2Cmvi%2Cpl%2Cinitcwndbps&mh=wB&mime=audio%2Fwebm&mm=31%2C29&mn=sn-h0jeener%2Csn-h0jeln7l&ms=au%2Crdu&mt=1683811031&mv=m&mvi=4&n=U8mCOo4eYD4n0A&ns=LToEdXWVFHcH53e3aTe1N7kN&pl=37&requiressl=yes&sig=AOq0QJ8wRQIhAPcUhhfkNVA_JcdU6KLTOFjRCnNl6n8gamJA-Q0PgCpIAiBTMV2k2JfHzbHBtsHxuNW7zHvSaYaUbz-dEIQC45o1eA%3D%3D&source=youtube&sparams=expire%2Cei%2Cip%2Cid%2Citag%2Csource%2Crequiressl%2Cspc%2Cvprv%2Csvpuc%2Cxtags%2Cmime%2Cns%2Cgir%2Cclen%2Cdur%2Clmt&spc=qEK7B81AP536F3aOi5JzMyLCUDiktWigtEpf9nI2xg&svpuc=1&txp=4532434&vprv=1&xtags=acont%3Doriginal%3Alang%3Den-US"))]
    #[case::bitrate(&PLAYER_ML, StreamFilter::new().audio_max_bitrate(100_000), Some("https://rr4---sn-h0jeener.googlevideo.com/videoplayback?c=WEB&clen=8217508&dur=1012.661&ei=6OtcZNqtBdOi7gP1upHYCQ&expire=1683832904&fexp=24007246&fvip=2&gir=yes&id=o-ABVtPh3j24hkJeXp8igjvreyODn-oV0CacOqb7pDjJoG&initcwndbps=1720000&ip=2003%3Ade%3Aaf31%3A5200%3A791a%3A897%3Ac15c%3Aae59&itag=250&keepalive=yes&lmt=1683782195315620&lsig=AG3C_xAwRQIgC7HZtYuc6dI92m6wCcoXYpdzSpVtPTIbO7jBKGpUrYMCIQCc0WNtFvN8Awqx9uuRVp5SUSe3rOt2D7M-rCKpgVv_0A%3D%3D&lsparams=mh%2Cmm%2Cmn%2Cms%2Cmv%2Cmvi%2Cpl%2Cinitcwndbps&mh=wB&mime=audio%2Fwebm&mm=31%2C29&mn=sn-h0jeener%2Csn-h0jeln7l&ms=au%2Crdu&mt=1683811031&mv=m&mvi=4&n=U8mCOo4eYD4n0A&ns=LToEdXWVFHcH53e3aTe1N7kN&pl=37&requiressl=yes&sig=AOq0QJ8wRQIga2iMQsToMxO7hTOx0gNAzhYoV1lL5PpE9lkAuBXt1nkCIQCuFuQXWNixIquEugtkT1C9khuKRP_C-wzSOiUmRp1DRg%3D%3D&source=youtube&sparams=expire%2Cei%2Cip%2Cid%2Citag%2Csource%2Crequiressl%2Cspc%2Cvprv%2Csvpuc%2Cxtags%2Cmime%2Cns%2Cgir%2Cclen%2Cdur%2Clmt&spc=qEK7B81AP536F3aOi5JzMyLCUDiktWigtEpf9nI2xg&svpuc=1&txp=4532434&vprv=1&xtags=acont%3Doriginal%3Alang%3Den-US"))]
    #[case::m4a_format(&PLAYER_ML, StreamFilter::new().audio_formats([AudioFormat::M4a]), Some("https://rr4---sn-h0jeener.googlevideo.com/videoplayback?c=WEB&clen=16390508&dur=1012.691&ei=6OtcZNqtBdOi7gP1upHYCQ&expire=1683832904&fexp=24007246&fvip=2&gir=yes&id=o-ABVtPh3j24hkJeXp8igjvreyODn-oV0CacOqb7pDjJoG&initcwndbps=1720000&ip=2003%3Ade%3Aaf31%3A5200%3A791a%3A897%3Ac15c%3Aae59&itag=140&keepalive=yes&lmt=1683782363698612&lsig=AG3C_xAwRQIgC7HZtYuc6dI92m6wCcoXYpdzSpVtPTIbO7jBKGpUrYMCIQCc0WNtFvN8Awqx9uuRVp5SUSe3rOt2D7M-rCKpgVv_0A%3D%3D&lsparams=mh%2Cmm%2Cmn%2Cms%2Cmv%2Cmvi%2Cpl%2Cinitcwndbps&mh=wB&mime=audio%2Fmp4&mm=31%2C29&mn=sn-h0jeener%2Csn-h0jeln7l&ms=au%2Crdu&mt=1683811031&mv=m&mvi=4&n=U8mCOo4eYD4n0A&ns=LToEdXWVFHcH53e3aTe1N7kN&pl=37&requiressl=yes&sig=AOq0QJ8wRgIhAMgM470I-QXq4lTRuPtXf5UInHB_tG0tTGXRhVZ6nwImAiEAn0JYRknq5dtTwcmzZheekxVOZKhZ2Rpxc_UyvX2CMRY%3D&source=youtube&sparams=expire%2Cei%2Cip%2Cid%2Citag%2Csource%2Crequiressl%2Cspc%2Cvprv%2Csvpuc%2Cxtags%2Cmime%2Cns%2Cgir%2Cclen%2Cdur%2Clmt&spc=qEK7B81AP536F3aOi5JzMyLCUDiktWigtEpf9nI2xg&svpuc=1&txp=4532434&vprv=1&xtags=acont%3Doriginal%3Alang%3Den-US"))]
    #[case::m4a_codec(&PLAYER_ML, StreamFilter::new().audio_codecs([AudioCodec::Mp4a]), Some("https://rr4---sn-h0jeener.googlevideo.com/videoplayback?c=WEB&clen=16390508&dur=1012.691&ei=6OtcZNqtBdOi7gP1upHYCQ&expire=1683832904&fexp=24007246&fvip=2&gir=yes&id=o-ABVtPh3j24hkJeXp8igjvreyODn-oV0CacOqb7pDjJoG&initcwndbps=1720000&ip=2003%3Ade%3Aaf31%3A5200%3A791a%3A897%3Ac15c%3Aae59&itag=140&keepalive=yes&lmt=1683782363698612&lsig=AG3C_xAwRQIgC7HZtYuc6dI92m6wCcoXYpdzSpVtPTIbO7jBKGpUrYMCIQCc0WNtFvN8Awqx9uuRVp5SUSe3rOt2D7M-rCKpgVv_0A%3D%3D&lsparams=mh%2Cmm%2Cmn%2Cms%2Cmv%2Cmvi%2Cpl%2Cinitcwndbps&mh=wB&mime=audio%2Fmp4&mm=31%2C29&mn=sn-h0jeener%2Csn-h0jeln7l&ms=au%2Crdu&mt=1683811031&mv=m&mvi=4&n=U8mCOo4eYD4n0A&ns=LToEdXWVFHcH53e3aTe1N7kN&pl=37&requiressl=yes&sig=AOq0QJ8wRgIhAMgM470I-QXq4lTRuPtXf5UInHB_tG0tTGXRhVZ6nwImAiEAn0JYRknq5dtTwcmzZheekxVOZKhZ2Rpxc_UyvX2CMRY%3D&source=youtube&sparams=expire%2Cei%2Cip%2Cid%2Citag%2Csource%2Crequiressl%2Cspc%2Cvprv%2Csvpuc%2Cxtags%2Cmime%2Cns%2Cgir%2Cclen%2Cdur%2Clmt&spc=qEK7B81AP536F3aOi5JzMyLCUDiktWigtEpf9nI2xg&svpuc=1&txp=4532434&vprv=1&xtags=acont%3Doriginal%3Alang%3Den-US"))]
    #[case::french(&PLAYER_ML, StreamFilter::new().audio_language("fr"), Some("https://rr4---sn-h0jeener.googlevideo.com/videoplayback?c=WEB&clen=940286&dur=60.101&ei=6OtcZNqtBdOi7gP1upHYCQ&expire=1683832904&fexp=24007246&fvip=2&gir=yes&id=o-ABVtPh3j24hkJeXp8igjvreyODn-oV0CacOqb7pDjJoG&initcwndbps=1720000&ip=2003%3Ade%3Aaf31%3A5200%3A791a%3A897%3Ac15c%3Aae59&itag=251&keepalive=yes&lmt=1683774002236584&lsig=AG3C_xAwRQIgC7HZtYuc6dI92m6wCcoXYpdzSpVtPTIbO7jBKGpUrYMCIQCc0WNtFvN8Awqx9uuRVp5SUSe3rOt2D7M-rCKpgVv_0A%3D%3D&lsparams=mh%2Cmm%2Cmn%2Cms%2Cmv%2Cmvi%2Cpl%2Cinitcwndbps&mh=wB&mime=audio%2Fwebm&mm=31%2C29&mn=sn-h0jeener%2Csn-h0jeln7l&ms=au%2Crdu&mt=1683811031&mv=m&mvi=4&n=U8mCOo4eYD4n0A&ns=LToEdXWVFHcH53e3aTe1N7kN&pl=37&requiressl=yes&sig=AOq0QJ8wRQIhAIUUin7WZBnoVDb2p0wuTPc7HZwbF8I5sxzLrVN9WeBwAiBQTZwhxCQ1IdrUkkD1-cSGYBtMF1aKkjPZ-LWeie0aZA%3D%3D&source=youtube&sparams=expire%2Cei%2Cip%2Cid%2Citag%2Csource%2Crequiressl%2Cspc%2Cvprv%2Csvpuc%2Cxtags%2Cmime%2Cns%2Cgir%2Cclen%2Cdur%2Clmt&spc=qEK7B81AP536F3aOi5JzMyLCUDiktWigtEpf9nI2xg&svpuc=1&txp=4532434&vprv=1&xtags=acont%3Ddubbed%3Alang%3Dfr"))]
    #[case::br_fallback(&PLAYER_ML, StreamFilter::new().audio_max_bitrate(0), Some("https://rr4---sn-h0jeener.googlevideo.com/videoplayback?c=WEB&clen=6306327&dur=1012.661&ei=6OtcZNqtBdOi7gP1upHYCQ&expire=1683832904&fexp=24007246&fvip=2&gir=yes&id=o-ABVtPh3j24hkJeXp8igjvreyODn-oV0CacOqb7pDjJoG&initcwndbps=1720000&ip=2003%3Ade%3Aaf31%3A5200%3A791a%3A897%3Ac15c%3Aae59&itag=249&keepalive=yes&lmt=1683782187865292&lsig=AG3C_xAwRQIgC7HZtYuc6dI92m6wCcoXYpdzSpVtPTIbO7jBKGpUrYMCIQCc0WNtFvN8Awqx9uuRVp5SUSe3rOt2D7M-rCKpgVv_0A%3D%3D&lsparams=mh%2Cmm%2Cmn%2Cms%2Cmv%2Cmvi%2Cpl%2Cinitcwndbps&mh=wB&mime=audio%2Fwebm&mm=31%2C29&mn=sn-h0jeener%2Csn-h0jeln7l&ms=au%2Crdu&mt=1683811031&mv=m&mvi=4&n=U8mCOo4eYD4n0A&ns=LToEdXWVFHcH53e3aTe1N7kN&pl=37&requiressl=yes&sig=AOq0QJ8wRAIgW1DTCrLV_GyEM1rdjScgyceZE1llb73KJMFXmPm5Y04CIAYOLZuuzFX4ba5720kMOcQ1-Ld1DULs85nLxJglitCl&source=youtube&sparams=expire%2Cei%2Cip%2Cid%2Citag%2Csource%2Crequiressl%2Cspc%2Cvprv%2Csvpuc%2Cxtags%2Cmime%2Cns%2Cgir%2Cclen%2Cdur%2Clmt&spc=qEK7B81AP536F3aOi5JzMyLCUDiktWigtEpf9nI2xg&svpuc=1&txp=4532434&vprv=1&xtags=acont%3Doriginal%3Alang%3Den-US"))]
    #[case::lang_fallback(&PLAYER_ML, StreamFilter::new().audio_language("xx"), Some("https://rr4---sn-h0jeener.googlevideo.com/videoplayback?c=WEB&clen=16104136&dur=1012.661&ei=6OtcZNqtBdOi7gP1upHYCQ&expire=1683832904&fexp=24007246&fvip=2&gir=yes&id=o-ABVtPh3j24hkJeXp8igjvreyODn-oV0CacOqb7pDjJoG&initcwndbps=1720000&ip=2003%3Ade%3Aaf31%3A5200%3A791a%3A897%3Ac15c%3Aae59&itag=251&keepalive=yes&lmt=1683782301237288&lsig=AG3C_xAwRQIgC7HZtYuc6dI92m6wCcoXYpdzSpVtPTIbO7jBKGpUrYMCIQCc0WNtFvN8Awqx9uuRVp5SUSe3rOt2D7M-rCKpgVv_0A%3D%3D&lsparams=mh%2Cmm%2Cmn%2Cms%2Cmv%2Cmvi%2Cpl%2Cinitcwndbps&mh=wB&mime=audio%2Fwebm&mm=31%2C29&mn=sn-h0jeener%2Csn-h0jeln7l&ms=au%2Crdu&mt=1683811031&mv=m&mvi=4&n=U8mCOo4eYD4n0A&ns=LToEdXWVFHcH53e3aTe1N7kN&pl=37&requiressl=yes&sig=AOq0QJ8wRQIhAPcUhhfkNVA_JcdU6KLTOFjRCnNl6n8gamJA-Q0PgCpIAiBTMV2k2JfHzbHBtsHxuNW7zHvSaYaUbz-dEIQC45o1eA%3D%3D&source=youtube&sparams=expire%2Cei%2Cip%2Cid%2Citag%2Csource%2Crequiressl%2Cspc%2Cvprv%2Csvpuc%2Cxtags%2Cmime%2Cns%2Cgir%2Cclen%2Cdur%2Clmt&spc=qEK7B81AP536F3aOi5JzMyLCUDiktWigtEpf9nI2xg&svpuc=1&txp=4532434&vprv=1&xtags=acont%3Doriginal%3Alang%3Den-US"))]
    #[case::noformat(&PLAYER_ML, StreamFilter::new().audio_formats([]), None)]
    #[case::nocodec(&PLAYER_ML, StreamFilter::new().audio_codecs([]), None)]
    #[case::surround(&PLAYER_SURROUND, StreamFilter::new(), Some("https://rr2---sn-h0jeenek.googlevideo.com/videoplayback?bui=AY2Et-P2auEfQCfvN1IA3yW9ExwyKiCcsFBtuaw0RQncjLVWDPJaVQYgtPEU0ihKmuokmZ_B80uh1lEO&c=TVHTML5&clen=73404667&dur=1528.864&ei=5yaIZ-P4Ea6I6dsPq7zx8Q0&expire=1736997703&fexp=51326932%2C51335594%2C51353498%2C51371294%2C51384460&fvip=2&gir=yes&id=o-AMht44pplOHlN5qaf_EH_YF6UFNUwZBORrpu9IH3kXJI&initcwndbps=2547500&ip=93.235.184.108&itag=328&keepalive=yes&lmt=1728213526028394&lmw=1&lsig=AGluJ3MwRQIhALUHsNUsYW-Gzp2bi2VB2xd_58iwzBMS77zfVLvvFq6RAiBS2vSPvOJReYr7OLk5jad2YNhkw22jHeD9Gv5tHDBgOQ%3D%3D&lsparams=met%2Cmh%2Cmm%2Cmn%2Cms%2Cmv%2Cmvi%2Cpl%2Crms%2Cinitcwndbps&met=1736976103%2C&mh=QX&mime=audio%2Fmp4&mm=31%2C26&mn=sn-h0jeenek%2Csn-4g5ednsk&ms=au%2Conr&mt=1736975835&mv=m&mvi=2&n=tNYdd0DuA4-wSg&ns=T9fKjV9jVlWEIaRxjvaOYZAQ&pl=26&requiressl=yes&rms=au%2Cau&rqh=1&sefc=1&sig=AJfQdSswRAIgWzmlzm0Po3ervktgNwWpuFCrXT8sr1wxYrj2j8XQx58CIC8vqHqPEgSS7LYOXLXlWeHiCJsB6FbIgv9JYsBwC-pB&source=youtube&sparams=expire%2Cei%2Cip%2Cid%2Citag%2Csource%2Crequiressl%2Cxpc%2Cbui%2Cvprv%2Csvpuc%2Cmime%2Cns%2Crqh%2Cgir%2Cclen%2Cdur%2Clmt&svpuc=1&txp=5308224&vprv=1&xpc=EgVo2aDSNQ%3D%3D"))]
    #[case::drm_none(&PLAYER_DRM, StreamFilter::new(), None)]
    #[case::drm_widevine(&PLAYER_DRM, StreamFilter::new().drm_system(DrmSystem::Widevine).drm_track_types([DrmTrackType::Audio, DrmTrackType::Hd]), Some("https://rr5---sn-h0jeener.googlevideo.com/videoplayback?aid=5c0488f533287530&asource=youtube&bui=AY2Et-NkyUGB6drHIkFCr0ToP8t9AOS64up0-Owwh4Yf-O6qvnjjBFdz7Fs6Grqo6Ki-GHFcFw&c=WEB&clen=469250041&ctier=A&dur=9722.240&ei=Dj6IZ_uSLs3l6dsPjd_aWQ&expire=1737003630&fexp=51326932%2C51335594%2C51353498%2C51355912%2C51384461&fvip=4&gcr=de&gir=yes&hightc=yes&id=b98fb7e4443ca114&initcwndbps=2801250&ip=93.235.184.108&itag=329&keepalive=yes&lmt=1687683252651200&lsig=AGluJ3MwRgIhAJHpDe-OyDAhm5uIGnacZ1NBH8woFM0noBJtngRPnn5mAiEAsAmkoTzrVeLQ8q58XCi7Z895Q1mb5t4fN_AfUoWd2fU%3D&lsparams=met%2Cmh%2Cmm%2Cmn%2Cms%2Cmv%2Cmvi%2Cpl%2Crms%2Cinitcwndbps&met=1736982030%2C&mh=3d&mime=audio%2Fmp4&mm=31%2C29&mn=sn-h0jeener%2Csn-h0jelnes&ms=au%2Crdu&mt=1736981586&mv=m&mvi=5&n=P31qX6QwZPWNtA&ns=Y9nsajsoC2zMRwwsmAQSUMMQ&pfa=5&pl=26&requiressl=yes&rms=au%2Cau&rqh=1&sefc=1&sig=AJfQdSswRQIhAILjbszDNz0ese6Cb02T8WRudpVkIlsCQDizjrMeByN3AiAjW8WSTC7AYQsoQrbBmBqb5U15Jz3RNAww352BUg8vmQ%3D%3D&siu=1&source=yt_media&sparams=expire%2Cei%2Cip%2Cid%2Citag%2Csource%2Crequiressl%2Cxpc%2Cctier%2Cpfa%2Cgcr%2Chightc%2Csiu%2Cbui%2Cspc%2Cvprv%2Csvpuc%2Cxtags%2Cmime%2Cns%2Crqh%2Caid%2Casource%2Cgir%2Cclen%2Cdur%2Clmt&spc=9kzgDTo16Q_mO7TFjJcMOcNa4IBGqdJV3_zJD2blPLtGQWHzV12Pjt9HGSUEzE5EuxsT3KGLQTHgHKI&svpuc=1&txp=0000224&vprv=1&xpc=EgVo2aDSNQ%3D%3D&xtags=acont%3Doriginal%3Alang%3Den"))]
    fn t_select_audio_stream(
        #[case] player: &VideoPlayer,
        #[case] filter: StreamFilter,
        #[case] expect_url: Option<&str>,
    ) {
        let selection = player.select_audio_stream(&filter);

        match expect_url {
            Some(expect_url) => assert_eq!(selection.unwrap().url, expect_url),
            None => assert_eq!(selection, None),
        }
    }

    #[rstest]
    #[case::default(&PLAYER_HDR, StreamFilter::new(), Some("https://rr5---sn-h0jelne7.googlevideo.com/videoplayback?aitags=133%2C134%2C135%2C136%2C160%2C242%2C243%2C244%2C247%2C278%2C298%2C299%2C302%2C303%2C308%2C315%2C330%2C331%2C332%2C333%2C334%2C335%2C336%2C337%2C394%2C395%2C396%2C397%2C398%2C399%2C400%2C401%2C694%2C695%2C696%2C697%2C698%2C699%2C700%2C701&c=WEB&clen=998696577&dur=313.780&ei=eckIY72IKcGZ8gOMt6CwDg&expire=1661541849&fexp=24001373%2C24007246&fvip=2&gir=yes&id=o-AOqXE9lVS424yszv6LN5V_gaevdHxenJl-tYNy3Drs6g&initcwndbps=1428750&ip=2003%3Ade%3Aaf05%3A2500%3A5dad%3A319b%3Aca30%3Ae212&itag=315&keepalive=yes&lmt=1647476955807851&lsig=AG3C_xAwRQIhAMioKyc-dqs-6uvAwLViCcCTXKHn9sIbo0cbSSBXGG4kAiBQNsRBAvQrbWdOjZIsQXYrfPEb1KDpE_AlSEGQZXB9uA%3D%3D&lsparams=mh%2Cmm%2Cmn%2Cms%2Cmv%2Cmvi%2Cpl%2Cinitcwndbps&mh=NH&mime=video%2Fwebm&mm=31%2C29&mn=sn-h0jelne7%2Csn-h0jeenl6&ms=au%2Crdu&mt=1661519833&mv=m&mvi=5&n=Zd7nrOM1B2C6PA&ns=426LxLap5MonJD_YWdS4lSYH&pl=37&rbqsm=fr&requiressl=yes&sig=AOq0QJ8wRAIfP4IVSo-00_kq_JIkuh032hcLoJzNEhYjvwgLiDpEzQIhALPVrvDBjRwiFddXiAyADmRtYygte4HvlJ3XOrkOf_TR&source=youtube&sparams=expire%2Cei%2Cip%2Cid%2Caitags%2Csource%2Crequiressl%2Cspc%2Cvprv%2Cmime%2Cns%2Cgir%2Cclen%2Cdur%2Clmt&spc=lT-KhuPtxVzL5-QbZ7S9zNeOHsWTdms&txp=4532434&vprv=1"))]
    #[case::hdr(&PLAYER_HDR, StreamFilter::new().video_hdr(), Some("https://rr5---sn-h0jelne7.googlevideo.com/videoplayback?aitags=133%2C134%2C135%2C136%2C160%2C242%2C243%2C244%2C247%2C278%2C298%2C299%2C302%2C303%2C308%2C315%2C330%2C331%2C332%2C333%2C334%2C335%2C336%2C337%2C394%2C395%2C396%2C397%2C398%2C399%2C400%2C401%2C694%2C695%2C696%2C697%2C698%2C699%2C700%2C701&c=WEB&clen=976824147&dur=313.780&ei=eckIY72IKcGZ8gOMt6CwDg&expire=1661541849&fexp=24001373%2C24007246&fvip=2&gir=yes&id=o-AOqXE9lVS424yszv6LN5V_gaevdHxenJl-tYNy3Drs6g&initcwndbps=1428750&ip=2003%3Ade%3Aaf05%3A2500%3A5dad%3A319b%3Aca30%3Ae212&itag=701&keepalive=yes&lmt=1647469891607029&lsig=AG3C_xAwRQIhAMioKyc-dqs-6uvAwLViCcCTXKHn9sIbo0cbSSBXGG4kAiBQNsRBAvQrbWdOjZIsQXYrfPEb1KDpE_AlSEGQZXB9uA%3D%3D&lsparams=mh%2Cmm%2Cmn%2Cms%2Cmv%2Cmvi%2Cpl%2Cinitcwndbps&mh=NH&mime=video%2Fmp4&mm=31%2C29&mn=sn-h0jelne7%2Csn-h0jeenl6&ms=au%2Crdu&mt=1661519833&mv=m&mvi=5&n=Zd7nrOM1B2C6PA&ns=426LxLap5MonJD_YWdS4lSYH&pl=37&rbqsm=fr&requiressl=yes&sig=AOq0QJ8wRgIhAOax_lAWCW5ENOYxe3gZfBHgHA5oZJPyMlYQFy73t7-pAiEA46J7dsT-1pv9smuoP3Kx5T7c_IJ6cEZN4U9UkSNuT7o%3D&source=youtube&sparams=expire%2Cei%2Cip%2Cid%2Caitags%2Csource%2Crequiressl%2Cspc%2Cvprv%2Cmime%2Cns%2Cgir%2Cclen%2Cdur%2Clmt&spc=lT-KhuPtxVzL5-QbZ7S9zNeOHsWTdms&txp=4532434&vprv=1"))]
    #[case::resolution(&PLAYER_HDR, StreamFilter::new().video_max_res(720), Some("https://rr5---sn-h0jelne7.googlevideo.com/videoplayback?aitags=133%2C134%2C135%2C136%2C160%2C242%2C243%2C244%2C247%2C278%2C298%2C299%2C302%2C303%2C308%2C315%2C330%2C331%2C332%2C333%2C334%2C335%2C336%2C337%2C394%2C395%2C396%2C397%2C398%2C399%2C400%2C401%2C694%2C695%2C696%2C697%2C698%2C699%2C700%2C701&c=WEB&clen=76313586&dur=313.780&ei=eckIY72IKcGZ8gOMt6CwDg&expire=1661541849&fexp=24001373%2C24007246&fvip=2&gir=yes&id=o-AOqXE9lVS424yszv6LN5V_gaevdHxenJl-tYNy3Drs6g&initcwndbps=1428750&ip=2003%3Ade%3Aaf05%3A2500%3A5dad%3A319b%3Aca30%3Ae212&itag=302&keepalive=yes&lmt=1647455155369524&lsig=AG3C_xAwRQIhAMioKyc-dqs-6uvAwLViCcCTXKHn9sIbo0cbSSBXGG4kAiBQNsRBAvQrbWdOjZIsQXYrfPEb1KDpE_AlSEGQZXB9uA%3D%3D&lsparams=mh%2Cmm%2Cmn%2Cms%2Cmv%2Cmvi%2Cpl%2Cinitcwndbps&mh=NH&mime=video%2Fwebm&mm=31%2C29&mn=sn-h0jelne7%2Csn-h0jeenl6&ms=au%2Crdu&mt=1661519833&mv=m&mvi=5&n=Zd7nrOM1B2C6PA&ns=426LxLap5MonJD_YWdS4lSYH&pl=37&rbqsm=fr&requiressl=yes&sig=AOq0QJ8wRAIgW0H1434eh9Axw6zw95qezJB0D2aVd2bxEIs4T5bcfFACIDOjha9WLycp0L188FZyFGa1RBkLPoGrrJOppsaXqwDR&source=youtube&sparams=expire%2Cei%2Cip%2Cid%2Caitags%2Csource%2Crequiressl%2Cspc%2Cvprv%2Cmime%2Cns%2Cgir%2Cclen%2Cdur%2Clmt&spc=lT-KhuPtxVzL5-QbZ7S9zNeOHsWTdms&txp=4532434&vprv=1"))]
    #[case::resolution_fps(&PLAYER_HDR, StreamFilter::new().video_max_res(720).video_max_fps(30), Some("https://rr5---sn-h0jelne7.googlevideo.com/videoplayback?aitags=133%2C134%2C135%2C136%2C160%2C242%2C243%2C244%2C247%2C278%2C298%2C299%2C302%2C303%2C308%2C315%2C330%2C331%2C332%2C333%2C334%2C335%2C336%2C337%2C394%2C395%2C396%2C397%2C398%2C399%2C400%2C401%2C694%2C695%2C696%2C697%2C698%2C699%2C700%2C701&c=WEB&clen=47531179&dur=313.780&ei=eckIY72IKcGZ8gOMt6CwDg&expire=1661541849&fexp=24001373%2C24007246&fvip=2&gir=yes&id=o-AOqXE9lVS424yszv6LN5V_gaevdHxenJl-tYNy3Drs6g&initcwndbps=1428750&ip=2003%3Ade%3Aaf05%3A2500%3A5dad%3A319b%3Aca30%3Ae212&itag=247&keepalive=yes&lmt=1647458657499381&lsig=AG3C_xAwRQIhAMioKyc-dqs-6uvAwLViCcCTXKHn9sIbo0cbSSBXGG4kAiBQNsRBAvQrbWdOjZIsQXYrfPEb1KDpE_AlSEGQZXB9uA%3D%3D&lsparams=mh%2Cmm%2Cmn%2Cms%2Cmv%2Cmvi%2Cpl%2Cinitcwndbps&mh=NH&mime=video%2Fwebm&mm=31%2C29&mn=sn-h0jelne7%2Csn-h0jeenl6&ms=au%2Crdu&mt=1661519833&mv=m&mvi=5&n=Zd7nrOM1B2C6PA&ns=426LxLap5MonJD_YWdS4lSYH&pl=37&rbqsm=fr&requiressl=yes&sig=AOq0QJ8wRgIhAMUsmcl1zgbr3YQranPWNV1kcxT5IdEoLL7FTFEDdHHPAiEAhQnrfYMU0A9xZ69MfBujWA4pXtCOQCg2Jn6ve9J_vBQ%3D&source=youtube&sparams=expire%2Cei%2Cip%2Cid%2Caitags%2Csource%2Crequiressl%2Cspc%2Cvprv%2Cmime%2Cns%2Cgir%2Cclen%2Cdur%2Clmt&spc=lT-KhuPtxVzL5-QbZ7S9zNeOHsWTdms&txp=4532434&vprv=1"))]
    #[case::res_fallback(&PLAYER_HDR, StreamFilter::new().video_max_res(100), Some("https://rr5---sn-h0jelne7.googlevideo.com/videoplayback?aitags=133%2C134%2C135%2C136%2C160%2C242%2C243%2C244%2C247%2C278%2C298%2C299%2C302%2C303%2C308%2C315%2C330%2C331%2C332%2C333%2C334%2C335%2C336%2C337%2C394%2C395%2C396%2C397%2C398%2C399%2C400%2C401%2C694%2C695%2C696%2C697%2C698%2C699%2C700%2C701&c=WEB&clen=3182932&dur=313.780&ei=eckIY72IKcGZ8gOMt6CwDg&expire=1661541849&fexp=24001373%2C24007246&fvip=2&gir=yes&id=o-AOqXE9lVS424yszv6LN5V_gaevdHxenJl-tYNy3Drs6g&initcwndbps=1428750&ip=2003%3Ade%3Aaf05%3A2500%3A5dad%3A319b%3Aca30%3Ae212&itag=278&keepalive=yes&lmt=1647458650479323&lsig=AG3C_xAwRQIhAMioKyc-dqs-6uvAwLViCcCTXKHn9sIbo0cbSSBXGG4kAiBQNsRBAvQrbWdOjZIsQXYrfPEb1KDpE_AlSEGQZXB9uA%3D%3D&lsparams=mh%2Cmm%2Cmn%2Cms%2Cmv%2Cmvi%2Cpl%2Cinitcwndbps&mh=NH&mime=video%2Fwebm&mm=31%2C29&mn=sn-h0jelne7%2Csn-h0jeenl6&ms=au%2Crdu&mt=1661519833&mv=m&mvi=5&n=Zd7nrOM1B2C6PA&ns=426LxLap5MonJD_YWdS4lSYH&pl=37&rbqsm=fr&requiressl=yes&sig=AOq0QJ8wRQIhAKcXzSIMQGA4R_rvoVg3ONpXOjpbaNZ5y9WJHLiQDTTVAiA6ePO9vuh5_zYE3Dw-QoRfqhT0CBDkg6w4dIo0MEfWnA%3D%3D&source=youtube&sparams=expire%2Cei%2Cip%2Cid%2Caitags%2Csource%2Crequiressl%2Cspc%2Cvprv%2Cmime%2Cns%2Cgir%2Cclen%2Cdur%2Clmt&spc=lT-KhuPtxVzL5-QbZ7S9zNeOHsWTdms&txp=4532434&vprv=1"))]
    #[case::webm_format(&PLAYER_HDR, StreamFilter::new().video_formats([VideoFormat::Webm]), Some("https://rr5---sn-h0jelne7.googlevideo.com/videoplayback?aitags=133%2C134%2C135%2C136%2C160%2C242%2C243%2C244%2C247%2C278%2C298%2C299%2C302%2C303%2C308%2C315%2C330%2C331%2C332%2C333%2C334%2C335%2C336%2C337%2C394%2C395%2C396%2C397%2C398%2C399%2C400%2C401%2C694%2C695%2C696%2C697%2C698%2C699%2C700%2C701&c=WEB&clen=998696577&dur=313.780&ei=eckIY72IKcGZ8gOMt6CwDg&expire=1661541849&fexp=24001373%2C24007246&fvip=2&gir=yes&id=o-AOqXE9lVS424yszv6LN5V_gaevdHxenJl-tYNy3Drs6g&initcwndbps=1428750&ip=2003%3Ade%3Aaf05%3A2500%3A5dad%3A319b%3Aca30%3Ae212&itag=315&keepalive=yes&lmt=1647476955807851&lsig=AG3C_xAwRQIhAMioKyc-dqs-6uvAwLViCcCTXKHn9sIbo0cbSSBXGG4kAiBQNsRBAvQrbWdOjZIsQXYrfPEb1KDpE_AlSEGQZXB9uA%3D%3D&lsparams=mh%2Cmm%2Cmn%2Cms%2Cmv%2Cmvi%2Cpl%2Cinitcwndbps&mh=NH&mime=video%2Fwebm&mm=31%2C29&mn=sn-h0jelne7%2Csn-h0jeenl6&ms=au%2Crdu&mt=1661519833&mv=m&mvi=5&n=Zd7nrOM1B2C6PA&ns=426LxLap5MonJD_YWdS4lSYH&pl=37&rbqsm=fr&requiressl=yes&sig=AOq0QJ8wRAIfP4IVSo-00_kq_JIkuh032hcLoJzNEhYjvwgLiDpEzQIhALPVrvDBjRwiFddXiAyADmRtYygte4HvlJ3XOrkOf_TR&source=youtube&sparams=expire%2Cei%2Cip%2Cid%2Caitags%2Csource%2Crequiressl%2Cspc%2Cvprv%2Cmime%2Cns%2Cgir%2Cclen%2Cdur%2Clmt&spc=lT-KhuPtxVzL5-QbZ7S9zNeOHsWTdms&txp=4532434&vprv=1"))]
    #[case::vp9_codec(&PLAYER_HDR, StreamFilter::new().video_codecs([VideoCodec::Vp9]), Some("https://rr5---sn-h0jelne7.googlevideo.com/videoplayback?aitags=133%2C134%2C135%2C136%2C160%2C242%2C243%2C244%2C247%2C278%2C298%2C299%2C302%2C303%2C308%2C315%2C330%2C331%2C332%2C333%2C334%2C335%2C336%2C337%2C394%2C395%2C396%2C397%2C398%2C399%2C400%2C401%2C694%2C695%2C696%2C697%2C698%2C699%2C700%2C701&c=WEB&clen=998696577&dur=313.780&ei=eckIY72IKcGZ8gOMt6CwDg&expire=1661541849&fexp=24001373%2C24007246&fvip=2&gir=yes&id=o-AOqXE9lVS424yszv6LN5V_gaevdHxenJl-tYNy3Drs6g&initcwndbps=1428750&ip=2003%3Ade%3Aaf05%3A2500%3A5dad%3A319b%3Aca30%3Ae212&itag=315&keepalive=yes&lmt=1647476955807851&lsig=AG3C_xAwRQIhAMioKyc-dqs-6uvAwLViCcCTXKHn9sIbo0cbSSBXGG4kAiBQNsRBAvQrbWdOjZIsQXYrfPEb1KDpE_AlSEGQZXB9uA%3D%3D&lsparams=mh%2Cmm%2Cmn%2Cms%2Cmv%2Cmvi%2Cpl%2Cinitcwndbps&mh=NH&mime=video%2Fwebm&mm=31%2C29&mn=sn-h0jelne7%2Csn-h0jeenl6&ms=au%2Crdu&mt=1661519833&mv=m&mvi=5&n=Zd7nrOM1B2C6PA&ns=426LxLap5MonJD_YWdS4lSYH&pl=37&rbqsm=fr&requiressl=yes&sig=AOq0QJ8wRAIfP4IVSo-00_kq_JIkuh032hcLoJzNEhYjvwgLiDpEzQIhALPVrvDBjRwiFddXiAyADmRtYygte4HvlJ3XOrkOf_TR&source=youtube&sparams=expire%2Cei%2Cip%2Cid%2Caitags%2Csource%2Crequiressl%2Cspc%2Cvprv%2Cmime%2Cns%2Cgir%2Cclen%2Cdur%2Clmt&spc=lT-KhuPtxVzL5-QbZ7S9zNeOHsWTdms&txp=4532434&vprv=1"))]
    #[case::noformat(&PLAYER_HDR, StreamFilter::new().video_formats([]), None)]
    #[case::nocodec(&PLAYER_HDR, StreamFilter::new().video_codecs([]), None)]
    #[case::drm_none(&PLAYER_DRM, StreamFilter::new(), None)]
    #[case::drm_widevine(&PLAYER_DRM, StreamFilter::new().drm_system(DrmSystem::Widevine).drm_track_types([DrmTrackType::Audio, DrmTrackType::Hd]), Some("https://rr5---sn-h0jeener.googlevideo.com/videoplayback?aid=5c0488f533287530&aitags=142%2C143%2C144%2C145%2C146%2C161%2C222%2C223%2C224%2C225%2C226%2C227%2C273%2C274%2C275%2C276%2C279%2C280%2C314%2C317%2C318%2C357%2C358%2C359%2C360%2C561%2C583%2C584%2C585%2C647%2C648%2C649%2C650%2C651%2C652%2C653%2C654%2C657%2C658%2C659%2C663%2C664%2C665%2C668%2C669%2C670&asource=youtube&bui=AY2Et-NkyUGB6drHIkFCr0ToP8t9AOS64up0-Owwh4Yf-O6qvnjjBFdz7Fs6Grqo6Ki-GHFcFw&c=WEB&clen=6316012989&ctier=A&dur=9722.170&ei=Dj6IZ_uSLs3l6dsPjd_aWQ&expire=1737003630&fexp=51326932%2C51335594%2C51353498%2C51355912%2C51384461&fvip=4&gcr=de&gir=yes&hightc=yes&id=b98fb7e4443ca114&initcwndbps=2801250&ip=93.235.184.108&itag=360&keepalive=yes&lmt=1687684175871682&lsig=AGluJ3MwRgIhAJHpDe-OyDAhm5uIGnacZ1NBH8woFM0noBJtngRPnn5mAiEAsAmkoTzrVeLQ8q58XCi7Z895Q1mb5t4fN_AfUoWd2fU%3D&lsparams=met%2Cmh%2Cmm%2Cmn%2Cms%2Cmv%2Cmvi%2Cpl%2Crms%2Cinitcwndbps&met=1736982030%2C&mh=3d&mime=video%2Fwebm&mm=31%2C29&mn=sn-h0jeener%2Csn-h0jelnes&ms=au%2Crdu&mt=1736981586&mv=m&mvi=5&n=P31qX6QwZPWNtA&ns=Y9nsajsoC2zMRwwsmAQSUMMQ&pfa=5&pl=26&requiressl=yes&rms=au%2Cau&rqh=1&sefc=1&sig=AJfQdSswRAIgXpaSy6Bm-MMSD_sEZpIzwvOzV9F8l0ydWul08VTpigcCIFEwz3HufM7FXciR_AeUEet0J6Y-GUwI4YEEFIa3BLwi&siu=1&source=yt_media&sparams=expire%2Cei%2Cip%2Cid%2Caitags%2Csource%2Crequiressl%2Cxpc%2Cctier%2Cpfa%2Cgcr%2Chightc%2Csiu%2Cbui%2Cspc%2Cvprv%2Csvpuc%2Cmime%2Cns%2Crqh%2Caid%2Casource%2Cgir%2Cclen%2Cdur%2Clmt&spc=9kzgDTo16Q_mO7TFjJcMOcNa4IBGqdJV3_zJD2blPLtGQWHzV12Pjt9HGSUEzE5EuxsT3KGLQTHgHKI&svpuc=1&txp=0000224&vprv=1&xpc=EgVo2aDSNQ%3D%3D"))]
    fn t_select_video_only_stream(
        #[case] player: &VideoPlayer,
        #[case] filter: StreamFilter,
        #[case] expect_url: Option<&str>,
    ) {
        let selection = player.select_video_only_stream(&filter);

        match expect_url {
            Some(expect_url) => assert_eq!(selection.unwrap().url, expect_url),
            None => assert_eq!(selection, None),
        }
    }

    #[rstest]
    #[case::default(
        StreamFilter::new(),
        Some("https://rr5---sn-h0jelne7.googlevideo.com/videoplayback?aitags=133%2C134%2C135%2C136%2C160%2C242%2C243%2C244%2C247%2C278%2C298%2C299%2C302%2C303%2C308%2C315%2C330%2C331%2C332%2C333%2C334%2C335%2C336%2C337%2C394%2C395%2C396%2C397%2C398%2C399%2C400%2C401%2C694%2C695%2C696%2C697%2C698%2C699%2C700%2C701&c=WEB&clen=998696577&dur=313.780&ei=eckIY72IKcGZ8gOMt6CwDg&expire=1661541849&fexp=24001373%2C24007246&fvip=2&gir=yes&id=o-AOqXE9lVS424yszv6LN5V_gaevdHxenJl-tYNy3Drs6g&initcwndbps=1428750&ip=2003%3Ade%3Aaf05%3A2500%3A5dad%3A319b%3Aca30%3Ae212&itag=315&keepalive=yes&lmt=1647476955807851&lsig=AG3C_xAwRQIhAMioKyc-dqs-6uvAwLViCcCTXKHn9sIbo0cbSSBXGG4kAiBQNsRBAvQrbWdOjZIsQXYrfPEb1KDpE_AlSEGQZXB9uA%3D%3D&lsparams=mh%2Cmm%2Cmn%2Cms%2Cmv%2Cmvi%2Cpl%2Cinitcwndbps&mh=NH&mime=video%2Fwebm&mm=31%2C29&mn=sn-h0jelne7%2Csn-h0jeenl6&ms=au%2Crdu&mt=1661519833&mv=m&mvi=5&n=Zd7nrOM1B2C6PA&ns=426LxLap5MonJD_YWdS4lSYH&pl=37&rbqsm=fr&requiressl=yes&sig=AOq0QJ8wRAIfP4IVSo-00_kq_JIkuh032hcLoJzNEhYjvwgLiDpEzQIhALPVrvDBjRwiFddXiAyADmRtYygte4HvlJ3XOrkOf_TR&source=youtube&sparams=expire%2Cei%2Cip%2Cid%2Caitags%2Csource%2Crequiressl%2Cspc%2Cvprv%2Cmime%2Cns%2Cgir%2Cclen%2Cdur%2Clmt&spc=lT-KhuPtxVzL5-QbZ7S9zNeOHsWTdms&txp=4532434&vprv=1"),
        Some("https://rr5---sn-h0jelne7.googlevideo.com/videoplayback?c=WEB&clen=5199784&dur=313.801&ei=eckIY72IKcGZ8gOMt6CwDg&expire=1661541849&fexp=24001373%2C24007246&fvip=2&gir=yes&id=o-AOqXE9lVS424yszv6LN5V_gaevdHxenJl-tYNy3Drs6g&initcwndbps=1428750&ip=2003%3Ade%3Aaf05%3A2500%3A5dad%3A319b%3Aca30%3Ae212&itag=251&keepalive=yes&lmt=1647453650291076&lsig=AG3C_xAwRQIhAMioKyc-dqs-6uvAwLViCcCTXKHn9sIbo0cbSSBXGG4kAiBQNsRBAvQrbWdOjZIsQXYrfPEb1KDpE_AlSEGQZXB9uA%3D%3D&lsparams=mh%2Cmm%2Cmn%2Cms%2Cmv%2Cmvi%2Cpl%2Cinitcwndbps&mh=NH&mime=audio%2Fwebm&mm=31%2C29&mn=sn-h0jelne7%2Csn-h0jeenl6&ms=au%2Crdu&mt=1661519833&mv=m&mvi=5&n=Zd7nrOM1B2C6PA&ns=426LxLap5MonJD_YWdS4lSYH&pl=37&rbqsm=fr&requiressl=yes&sig=AOq0QJ8wRQIhALtI3j8ZChpNb0LcyDZ3yosbWnSpqaO0-jKAe_UM_RQyAiAMwrpdeNbJEnQn3q1eveaAcRcNIwy5iJ4fIjeBW_MUfg%3D%3D&source=youtube&sparams=expire%2Cei%2Cip%2Cid%2Citag%2Csource%2Crequiressl%2Cspc%2Cvprv%2Cmime%2Cns%2Cgir%2Cclen%2Cdur%2Clmt&spc=lT-KhuPtxVzL5-QbZ7S9zNeOHsWTdms&txp=4532434&vprv=1")
    )]
    #[case::webm(
        StreamFilter::new().video_formats([VideoFormat::Webm]),
        Some("https://rr5---sn-h0jelne7.googlevideo.com/videoplayback?aitags=133%2C134%2C135%2C136%2C160%2C242%2C243%2C244%2C247%2C278%2C298%2C299%2C302%2C303%2C308%2C315%2C330%2C331%2C332%2C333%2C334%2C335%2C336%2C337%2C394%2C395%2C396%2C397%2C398%2C399%2C400%2C401%2C694%2C695%2C696%2C697%2C698%2C699%2C700%2C701&c=WEB&clen=998696577&dur=313.780&ei=eckIY72IKcGZ8gOMt6CwDg&expire=1661541849&fexp=24001373%2C24007246&fvip=2&gir=yes&id=o-AOqXE9lVS424yszv6LN5V_gaevdHxenJl-tYNy3Drs6g&initcwndbps=1428750&ip=2003%3Ade%3Aaf05%3A2500%3A5dad%3A319b%3Aca30%3Ae212&itag=315&keepalive=yes&lmt=1647476955807851&lsig=AG3C_xAwRQIhAMioKyc-dqs-6uvAwLViCcCTXKHn9sIbo0cbSSBXGG4kAiBQNsRBAvQrbWdOjZIsQXYrfPEb1KDpE_AlSEGQZXB9uA%3D%3D&lsparams=mh%2Cmm%2Cmn%2Cms%2Cmv%2Cmvi%2Cpl%2Cinitcwndbps&mh=NH&mime=video%2Fwebm&mm=31%2C29&mn=sn-h0jelne7%2Csn-h0jeenl6&ms=au%2Crdu&mt=1661519833&mv=m&mvi=5&n=Zd7nrOM1B2C6PA&ns=426LxLap5MonJD_YWdS4lSYH&pl=37&rbqsm=fr&requiressl=yes&sig=AOq0QJ8wRAIfP4IVSo-00_kq_JIkuh032hcLoJzNEhYjvwgLiDpEzQIhALPVrvDBjRwiFddXiAyADmRtYygte4HvlJ3XOrkOf_TR&source=youtube&sparams=expire%2Cei%2Cip%2Cid%2Caitags%2Csource%2Crequiressl%2Cspc%2Cvprv%2Cmime%2Cns%2Cgir%2Cclen%2Cdur%2Clmt&spc=lT-KhuPtxVzL5-QbZ7S9zNeOHsWTdms&txp=4532434&vprv=1"),
        Some("https://rr5---sn-h0jelne7.googlevideo.com/videoplayback?c=WEB&clen=5199784&dur=313.801&ei=eckIY72IKcGZ8gOMt6CwDg&expire=1661541849&fexp=24001373%2C24007246&fvip=2&gir=yes&id=o-AOqXE9lVS424yszv6LN5V_gaevdHxenJl-tYNy3Drs6g&initcwndbps=1428750&ip=2003%3Ade%3Aaf05%3A2500%3A5dad%3A319b%3Aca30%3Ae212&itag=251&keepalive=yes&lmt=1647453650291076&lsig=AG3C_xAwRQIhAMioKyc-dqs-6uvAwLViCcCTXKHn9sIbo0cbSSBXGG4kAiBQNsRBAvQrbWdOjZIsQXYrfPEb1KDpE_AlSEGQZXB9uA%3D%3D&lsparams=mh%2Cmm%2Cmn%2Cms%2Cmv%2Cmvi%2Cpl%2Cinitcwndbps&mh=NH&mime=audio%2Fwebm&mm=31%2C29&mn=sn-h0jelne7%2Csn-h0jeenl6&ms=au%2Crdu&mt=1661519833&mv=m&mvi=5&n=Zd7nrOM1B2C6PA&ns=426LxLap5MonJD_YWdS4lSYH&pl=37&rbqsm=fr&requiressl=yes&sig=AOq0QJ8wRQIhALtI3j8ZChpNb0LcyDZ3yosbWnSpqaO0-jKAe_UM_RQyAiAMwrpdeNbJEnQn3q1eveaAcRcNIwy5iJ4fIjeBW_MUfg%3D%3D&source=youtube&sparams=expire%2Cei%2Cip%2Cid%2Citag%2Csource%2Crequiressl%2Cspc%2Cvprv%2Cmime%2Cns%2Cgir%2Cclen%2Cdur%2Clmt&spc=lT-KhuPtxVzL5-QbZ7S9zNeOHsWTdms&txp=4532434&vprv=1")
    )]
    #[case::noaudio(
        StreamFilter::new().audio_formats([]),
        Some("https://rr5---sn-h0jelne7.googlevideo.com/videoplayback?c=WEB&clen=23544588&dur=313.834&ei=eckIY72IKcGZ8gOMt6CwDg&expire=1661541849&fexp=24001373%2C24007246&fvip=2&gir=yes&id=o-AOqXE9lVS424yszv6LN5V_gaevdHxenJl-tYNy3Drs6g&initcwndbps=1428750&ip=2003%3Ade%3Aaf05%3A2500%3A5dad%3A319b%3Aca30%3Ae212&itag=18&lmt=1647456546485912&lsig=AG3C_xAwRQIhAMioKyc-dqs-6uvAwLViCcCTXKHn9sIbo0cbSSBXGG4kAiBQNsRBAvQrbWdOjZIsQXYrfPEb1KDpE_AlSEGQZXB9uA%3D%3D&lsparams=mh%2Cmm%2Cmn%2Cms%2Cmv%2Cmvi%2Cpl%2Cinitcwndbps&mh=NH&mime=video%2Fmp4&mm=31%2C29&mn=sn-h0jelne7%2Csn-h0jeenl6&ms=au%2Crdu&mt=1661519833&mv=m&mvi=5&n=HWZNhARNT_nJgg&ns=pLFQxzhiCbZ9F2HJmDLveKoH&pl=37&ratebypass=yes&rbqsm=fr&requiressl=yes&sig=AOq0QJ8wRQIgeCEjusAq6p33rH0NHyTAbPIRaaEkjDE32AXBFzDvR-ICIQD0LI8hQVH8oCMWu6OuADzc1FSQhIqYs5RLkxBmObIdsw%3D%3D&source=youtube&sparams=expire%2Cei%2Cip%2Cid%2Citag%2Csource%2Crequiressl%2Cspc%2Cvprv%2Cmime%2Cns%2Cgir%2Cclen%2Cratebypass%2Cdur%2Clmt&spc=lT-KhuPtxVzL5-QbZ7S9zNeOHsWTdms&txp=4530434&vprv=1"),
        None
    )]
    #[case::novideo(
        StreamFilter::new().no_video(),
        None,
        Some("https://rr5---sn-h0jelne7.googlevideo.com/videoplayback?c=WEB&clen=5199784&dur=313.801&ei=eckIY72IKcGZ8gOMt6CwDg&expire=1661541849&fexp=24001373%2C24007246&fvip=2&gir=yes&id=o-AOqXE9lVS424yszv6LN5V_gaevdHxenJl-tYNy3Drs6g&initcwndbps=1428750&ip=2003%3Ade%3Aaf05%3A2500%3A5dad%3A319b%3Aca30%3Ae212&itag=251&keepalive=yes&lmt=1647453650291076&lsig=AG3C_xAwRQIhAMioKyc-dqs-6uvAwLViCcCTXKHn9sIbo0cbSSBXGG4kAiBQNsRBAvQrbWdOjZIsQXYrfPEb1KDpE_AlSEGQZXB9uA%3D%3D&lsparams=mh%2Cmm%2Cmn%2Cms%2Cmv%2Cmvi%2Cpl%2Cinitcwndbps&mh=NH&mime=audio%2Fwebm&mm=31%2C29&mn=sn-h0jelne7%2Csn-h0jeenl6&ms=au%2Crdu&mt=1661519833&mv=m&mvi=5&n=Zd7nrOM1B2C6PA&ns=426LxLap5MonJD_YWdS4lSYH&pl=37&rbqsm=fr&requiressl=yes&sig=AOq0QJ8wRQIhALtI3j8ZChpNb0LcyDZ3yosbWnSpqaO0-jKAe_UM_RQyAiAMwrpdeNbJEnQn3q1eveaAcRcNIwy5iJ4fIjeBW_MUfg%3D%3D&source=youtube&sparams=expire%2Cei%2Cip%2Cid%2Citag%2Csource%2Crequiressl%2Cspc%2Cvprv%2Cmime%2Cns%2Cgir%2Cclen%2Cdur%2Clmt&spc=lT-KhuPtxVzL5-QbZ7S9zNeOHsWTdms&txp=4532434&vprv=1")
    )]
    #[case::noformat(StreamFilter::new().audio_formats([]).video_formats([]), None, None)]
    fn t_select_video_audio_stream(
        #[case] filter: StreamFilter,
        #[case] expect_video_url: Option<&str>,
        #[case] expect_audio_url: Option<&str>,
    ) {
        let (video, audio) = PLAYER_HDR.select_video_audio_stream(&filter);

        match expect_video_url {
            Some(expect_url) => assert_eq!(video.unwrap().url, expect_url),
            None => assert_eq!(video, None),
        }

        match expect_audio_url {
            Some(expect_url) => assert_eq!(audio.unwrap().url, expect_url),
            None => assert_eq!(audio, None),
        }
    }
}
