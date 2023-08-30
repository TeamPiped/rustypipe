//! Filters for selecting audio/video streams

use std::cmp::Ordering;

use crate::model::{
    traits::QualityOrd, AudioCodec, AudioFormat, AudioStream, VideoCodec, VideoFormat, VideoPlayer,
    VideoStream,
};

/// The StreamFilter is used for selecting audio/video streams from an extracted video
#[derive(Debug, Default, Clone)]
pub struct StreamFilter<'a> {
    audio_max_bitrate: Option<u32>,
    audio_formats: Option<&'a [AudioFormat]>,
    audio_codecs: Option<&'a [AudioCodec]>,
    audio_language: Option<&'a str>,
    video_max_res: Option<u32>,
    video_max_fps: Option<u8>,
    video_formats: Option<&'a [VideoFormat]>,
    video_codecs: Option<&'a [VideoCodec]>,
    video_hdr: bool,
    video_none: bool,
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
enum FilterResult {
    Deny,
    AllowLowest,
    Allow,
    Match,
}

impl FilterResult {
    fn hard(val: bool) -> Self {
        if val {
            Self::Match
        } else {
            Self::Deny
        }
    }

    fn soft(val: bool) -> Self {
        if val {
            Self::Match
        } else {
            Self::AllowLowest
        }
    }

    fn allow(val: bool) -> Self {
        if val {
            Self::Allow
        } else {
            Self::Deny
        }
    }

    fn join(self, other: Self) -> Self {
        if self == Self::Deny {
            Self::Deny
        } else {
            self.min(other)
        }
    }
}

impl<'a> StreamFilter<'a> {
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

    fn apply_audio_max_bitrate(&self, stream: &AudioStream) -> FilterResult {
        match self.audio_max_bitrate {
            Some(max_bitrate) => FilterResult::soft(stream.average_bitrate <= max_bitrate),
            None => FilterResult::Match,
        }
    }

    /// Set the supported audio container formats
    #[must_use]
    pub fn audio_formats(mut self, formats: &'a [AudioFormat]) -> Self {
        self.audio_formats = Some(formats);
        self
    }

    fn apply_audio_formats(&self, stream: &AudioStream) -> FilterResult {
        match &self.audio_formats {
            Some(formats) => FilterResult::hard(formats.contains(&stream.format)),
            None => FilterResult::Match,
        }
    }

    /// Set the supported audio codecs
    #[must_use]
    pub fn audio_codecs(mut self, codecs: &'a [AudioCodec]) -> Self {
        self.audio_codecs = Some(codecs);
        self
    }

    fn apply_audio_codecs(&self, stream: &AudioStream) -> FilterResult {
        match &self.audio_codecs {
            Some(codecs) => FilterResult::hard(codecs.contains(&stream.codec)),
            None => FilterResult::Match,
        }
    }

    /// Set the preferred audio language (2 letter ISO 639-1 code, e.g. `en`, `fr`).
    /// Some YouTube videos feature multiple audio streams in
    /// different languages (e.g. <https://www.youtube.com/watch?v=tVWWp1PqDus>).
    ///
    /// If this filter is unset or no stream matches,
    /// the filter returns the default audio stream.
    #[must_use]
    pub fn audio_language(mut self, language: &'a str) -> Self {
        self.audio_language = Some(language);
        self
    }

    fn apply_audio_language(&self, stream: &AudioStream) -> FilterResult {
        match &self.audio_language {
            Some(language) => match &stream.track {
                Some(track) => match &track.lang {
                    Some(track_lang) => {
                        if track_lang == language {
                            FilterResult::Match
                        } else {
                            FilterResult::allow(track.is_default)
                        }
                    }
                    None => FilterResult::allow(track.is_default),
                },
                None => FilterResult::Match,
            },
            None => FilterResult::hard(stream.track.as_ref().map_or(true, |t| t.is_default)),
        }
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

    fn apply_video_max_res(&self, stream: &VideoStream) -> FilterResult {
        match self.video_max_res {
            Some(max_res) => FilterResult::soft(stream.height.min(stream.width) <= max_res),
            None => FilterResult::Match,
        }
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

    fn apply_video_max_fps(&self, stream: &VideoStream) -> FilterResult {
        match self.video_max_fps {
            Some(max_fps) => FilterResult::soft(stream.fps <= max_fps),
            None => FilterResult::Match,
        }
    }

    /// Set the supported video container formats
    #[must_use]
    pub fn video_formats(mut self, formats: &'a [VideoFormat]) -> Self {
        self.video_formats = Some(formats);
        self
    }

    fn apply_video_formats(&self, stream: &VideoStream) -> FilterResult {
        match &self.video_formats {
            Some(formats) => FilterResult::hard(formats.contains(&stream.format)),
            None => FilterResult::Match,
        }
    }

    /// Set the supported video codecs
    #[must_use]
    pub fn video_codecs(mut self, codecs: &'a [VideoCodec]) -> Self {
        self.video_codecs = Some(codecs);
        self
    }

    fn apply_video_codecs(&self, stream: &VideoStream) -> FilterResult {
        match &self.video_codecs {
            Some(codecs) => FilterResult::hard(codecs.contains(&stream.codec)),
            None => FilterResult::Match,
        }
    }

    /// Allow HDR videos
    #[must_use]
    pub fn video_hdr(mut self) -> Self {
        self.video_hdr = true;
        self
    }

    fn apply_video_hdr(&self, stream: &VideoStream) -> FilterResult {
        match &self.video_hdr {
            true => FilterResult::Match,
            false => FilterResult::hard(!&stream.hdr),
        }
    }

    /// Output no video stream (audio only)
    #[must_use]
    pub fn no_video(mut self) -> Self {
        self.video_none = true;
        self
    }

    fn apply_audio(&self, stream: &AudioStream) -> FilterResult {
        self.apply_audio_max_bitrate(stream).join(
            self.apply_audio_formats(stream).join(
                self.apply_audio_codecs(stream)
                    .join(self.apply_audio_language(stream)),
            ),
        )
    }

    fn apply_video(&self, stream: &VideoStream) -> FilterResult {
        self.apply_video_max_res(stream).join(
            self.apply_video_formats(stream).join(
                self.apply_video_codecs(stream).join(
                    self.apply_video_hdr(stream)
                        .join(self.apply_video_max_fps(stream)),
                ),
            ),
        )
    }
}

impl VideoPlayer {
    /// Select the audio stream which is the best match for the given [`StreamFilter`]
    #[must_use]
    pub fn select_audio_stream(&self, filter: &StreamFilter) -> Option<&AudioStream> {
        let mut fallback: Option<&AudioStream> = None;

        self.audio_streams
            .iter()
            .rev()
            .find(|s| match filter.apply_audio(s) {
                FilterResult::Deny => false,
                FilterResult::AllowLowest => {
                    fallback = Some(s);
                    false
                }
                FilterResult::Allow => {
                    if fallback.is_none() {
                        fallback = Some(s);
                    }
                    false
                }
                FilterResult::Match => true,
            })
            .or(fallback)
    }

    fn _select_video_stream<'a>(
        streams: &'a [VideoStream],
        filter: &StreamFilter,
    ) -> Option<&'a VideoStream> {
        if filter.video_none {
            return None;
        }

        let mut fallback: Option<&VideoStream> = None;

        streams
            .iter()
            .rev()
            .find(|s| match filter.apply_video(s) {
                FilterResult::Deny => false,
                FilterResult::AllowLowest => {
                    fallback = Some(s);
                    false
                }
                FilterResult::Allow => {
                    if fallback.is_none() {
                        fallback = Some(s);
                    }
                    false
                }
                FilterResult::Match => true,
            })
            .or(fallback)
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

    #[rstest]
    #[case::default(StreamFilter::default(), Some("https://rr4---sn-h0jeener.googlevideo.com/videoplayback?c=WEB&clen=16104136&dur=1012.661&ei=6OtcZNqtBdOi7gP1upHYCQ&expire=1683832904&fexp=24007246&fvip=2&gir=yes&id=o-ABVtPh3j24hkJeXp8igjvreyODn-oV0CacOqb7pDjJoG&initcwndbps=1720000&ip=2003%3Ade%3Aaf31%3A5200%3A791a%3A897%3Ac15c%3Aae59&itag=251&keepalive=yes&lmt=1683782301237288&lsig=AG3C_xAwRQIgC7HZtYuc6dI92m6wCcoXYpdzSpVtPTIbO7jBKGpUrYMCIQCc0WNtFvN8Awqx9uuRVp5SUSe3rOt2D7M-rCKpgVv_0A%3D%3D&lsparams=mh%2Cmm%2Cmn%2Cms%2Cmv%2Cmvi%2Cpl%2Cinitcwndbps&mh=wB&mime=audio%2Fwebm&mm=31%2C29&mn=sn-h0jeener%2Csn-h0jeln7l&ms=au%2Crdu&mt=1683811031&mv=m&mvi=4&n=U8mCOo4eYD4n0A&ns=LToEdXWVFHcH53e3aTe1N7kN&pl=37&requiressl=yes&sig=AOq0QJ8wRQIhAPcUhhfkNVA_JcdU6KLTOFjRCnNl6n8gamJA-Q0PgCpIAiBTMV2k2JfHzbHBtsHxuNW7zHvSaYaUbz-dEIQC45o1eA%3D%3D&source=youtube&sparams=expire%2Cei%2Cip%2Cid%2Citag%2Csource%2Crequiressl%2Cspc%2Cvprv%2Csvpuc%2Cxtags%2Cmime%2Cns%2Cgir%2Cclen%2Cdur%2Clmt&spc=qEK7B81AP536F3aOi5JzMyLCUDiktWigtEpf9nI2xg&svpuc=1&txp=4532434&vprv=1&xtags=acont%3Doriginal%3Alang%3Den-US"))]
    #[case::bitrate(StreamFilter::default().audio_max_bitrate(100_000).clone(), Some("https://rr4---sn-h0jeener.googlevideo.com/videoplayback?c=WEB&clen=8217508&dur=1012.661&ei=6OtcZNqtBdOi7gP1upHYCQ&expire=1683832904&fexp=24007246&fvip=2&gir=yes&id=o-ABVtPh3j24hkJeXp8igjvreyODn-oV0CacOqb7pDjJoG&initcwndbps=1720000&ip=2003%3Ade%3Aaf31%3A5200%3A791a%3A897%3Ac15c%3Aae59&itag=250&keepalive=yes&lmt=1683782195315620&lsig=AG3C_xAwRQIgC7HZtYuc6dI92m6wCcoXYpdzSpVtPTIbO7jBKGpUrYMCIQCc0WNtFvN8Awqx9uuRVp5SUSe3rOt2D7M-rCKpgVv_0A%3D%3D&lsparams=mh%2Cmm%2Cmn%2Cms%2Cmv%2Cmvi%2Cpl%2Cinitcwndbps&mh=wB&mime=audio%2Fwebm&mm=31%2C29&mn=sn-h0jeener%2Csn-h0jeln7l&ms=au%2Crdu&mt=1683811031&mv=m&mvi=4&n=U8mCOo4eYD4n0A&ns=LToEdXWVFHcH53e3aTe1N7kN&pl=37&requiressl=yes&sig=AOq0QJ8wRQIga2iMQsToMxO7hTOx0gNAzhYoV1lL5PpE9lkAuBXt1nkCIQCuFuQXWNixIquEugtkT1C9khuKRP_C-wzSOiUmRp1DRg%3D%3D&source=youtube&sparams=expire%2Cei%2Cip%2Cid%2Citag%2Csource%2Crequiressl%2Cspc%2Cvprv%2Csvpuc%2Cxtags%2Cmime%2Cns%2Cgir%2Cclen%2Cdur%2Clmt&spc=qEK7B81AP536F3aOi5JzMyLCUDiktWigtEpf9nI2xg&svpuc=1&txp=4532434&vprv=1&xtags=acont%3Doriginal%3Alang%3Den-US"))]
    #[case::m4a_format(StreamFilter::default().audio_formats(&[AudioFormat::M4a]).clone(), Some("https://rr4---sn-h0jeener.googlevideo.com/videoplayback?c=WEB&clen=16390508&dur=1012.691&ei=6OtcZNqtBdOi7gP1upHYCQ&expire=1683832904&fexp=24007246&fvip=2&gir=yes&id=o-ABVtPh3j24hkJeXp8igjvreyODn-oV0CacOqb7pDjJoG&initcwndbps=1720000&ip=2003%3Ade%3Aaf31%3A5200%3A791a%3A897%3Ac15c%3Aae59&itag=140&keepalive=yes&lmt=1683782363698612&lsig=AG3C_xAwRQIgC7HZtYuc6dI92m6wCcoXYpdzSpVtPTIbO7jBKGpUrYMCIQCc0WNtFvN8Awqx9uuRVp5SUSe3rOt2D7M-rCKpgVv_0A%3D%3D&lsparams=mh%2Cmm%2Cmn%2Cms%2Cmv%2Cmvi%2Cpl%2Cinitcwndbps&mh=wB&mime=audio%2Fmp4&mm=31%2C29&mn=sn-h0jeener%2Csn-h0jeln7l&ms=au%2Crdu&mt=1683811031&mv=m&mvi=4&n=U8mCOo4eYD4n0A&ns=LToEdXWVFHcH53e3aTe1N7kN&pl=37&requiressl=yes&sig=AOq0QJ8wRgIhAMgM470I-QXq4lTRuPtXf5UInHB_tG0tTGXRhVZ6nwImAiEAn0JYRknq5dtTwcmzZheekxVOZKhZ2Rpxc_UyvX2CMRY%3D&source=youtube&sparams=expire%2Cei%2Cip%2Cid%2Citag%2Csource%2Crequiressl%2Cspc%2Cvprv%2Csvpuc%2Cxtags%2Cmime%2Cns%2Cgir%2Cclen%2Cdur%2Clmt&spc=qEK7B81AP536F3aOi5JzMyLCUDiktWigtEpf9nI2xg&svpuc=1&txp=4532434&vprv=1&xtags=acont%3Doriginal%3Alang%3Den-US"))]
    #[case::m4a_codec(StreamFilter::default().audio_codecs(&[AudioCodec::Mp4a]).clone(), Some("https://rr4---sn-h0jeener.googlevideo.com/videoplayback?c=WEB&clen=16390508&dur=1012.691&ei=6OtcZNqtBdOi7gP1upHYCQ&expire=1683832904&fexp=24007246&fvip=2&gir=yes&id=o-ABVtPh3j24hkJeXp8igjvreyODn-oV0CacOqb7pDjJoG&initcwndbps=1720000&ip=2003%3Ade%3Aaf31%3A5200%3A791a%3A897%3Ac15c%3Aae59&itag=140&keepalive=yes&lmt=1683782363698612&lsig=AG3C_xAwRQIgC7HZtYuc6dI92m6wCcoXYpdzSpVtPTIbO7jBKGpUrYMCIQCc0WNtFvN8Awqx9uuRVp5SUSe3rOt2D7M-rCKpgVv_0A%3D%3D&lsparams=mh%2Cmm%2Cmn%2Cms%2Cmv%2Cmvi%2Cpl%2Cinitcwndbps&mh=wB&mime=audio%2Fmp4&mm=31%2C29&mn=sn-h0jeener%2Csn-h0jeln7l&ms=au%2Crdu&mt=1683811031&mv=m&mvi=4&n=U8mCOo4eYD4n0A&ns=LToEdXWVFHcH53e3aTe1N7kN&pl=37&requiressl=yes&sig=AOq0QJ8wRgIhAMgM470I-QXq4lTRuPtXf5UInHB_tG0tTGXRhVZ6nwImAiEAn0JYRknq5dtTwcmzZheekxVOZKhZ2Rpxc_UyvX2CMRY%3D&source=youtube&sparams=expire%2Cei%2Cip%2Cid%2Citag%2Csource%2Crequiressl%2Cspc%2Cvprv%2Csvpuc%2Cxtags%2Cmime%2Cns%2Cgir%2Cclen%2Cdur%2Clmt&spc=qEK7B81AP536F3aOi5JzMyLCUDiktWigtEpf9nI2xg&svpuc=1&txp=4532434&vprv=1&xtags=acont%3Doriginal%3Alang%3Den-US"))]
    #[case::french(StreamFilter::default().audio_language("fr").clone(), Some("https://rr4---sn-h0jeener.googlevideo.com/videoplayback?c=WEB&clen=940286&dur=60.101&ei=6OtcZNqtBdOi7gP1upHYCQ&expire=1683832904&fexp=24007246&fvip=2&gir=yes&id=o-ABVtPh3j24hkJeXp8igjvreyODn-oV0CacOqb7pDjJoG&initcwndbps=1720000&ip=2003%3Ade%3Aaf31%3A5200%3A791a%3A897%3Ac15c%3Aae59&itag=251&keepalive=yes&lmt=1683774002236584&lsig=AG3C_xAwRQIgC7HZtYuc6dI92m6wCcoXYpdzSpVtPTIbO7jBKGpUrYMCIQCc0WNtFvN8Awqx9uuRVp5SUSe3rOt2D7M-rCKpgVv_0A%3D%3D&lsparams=mh%2Cmm%2Cmn%2Cms%2Cmv%2Cmvi%2Cpl%2Cinitcwndbps&mh=wB&mime=audio%2Fwebm&mm=31%2C29&mn=sn-h0jeener%2Csn-h0jeln7l&ms=au%2Crdu&mt=1683811031&mv=m&mvi=4&n=U8mCOo4eYD4n0A&ns=LToEdXWVFHcH53e3aTe1N7kN&pl=37&requiressl=yes&sig=AOq0QJ8wRQIhAIUUin7WZBnoVDb2p0wuTPc7HZwbF8I5sxzLrVN9WeBwAiBQTZwhxCQ1IdrUkkD1-cSGYBtMF1aKkjPZ-LWeie0aZA%3D%3D&source=youtube&sparams=expire%2Cei%2Cip%2Cid%2Citag%2Csource%2Crequiressl%2Cspc%2Cvprv%2Csvpuc%2Cxtags%2Cmime%2Cns%2Cgir%2Cclen%2Cdur%2Clmt&spc=qEK7B81AP536F3aOi5JzMyLCUDiktWigtEpf9nI2xg&svpuc=1&txp=4532434&vprv=1&xtags=acont%3Ddubbed%3Alang%3Dfr"))]
    #[case::br_fallback(StreamFilter::default().audio_max_bitrate(0).clone(), Some("https://rr4---sn-h0jeener.googlevideo.com/videoplayback?c=WEB&clen=6306327&dur=1012.661&ei=6OtcZNqtBdOi7gP1upHYCQ&expire=1683832904&fexp=24007246&fvip=2&gir=yes&id=o-ABVtPh3j24hkJeXp8igjvreyODn-oV0CacOqb7pDjJoG&initcwndbps=1720000&ip=2003%3Ade%3Aaf31%3A5200%3A791a%3A897%3Ac15c%3Aae59&itag=249&keepalive=yes&lmt=1683782187865292&lsig=AG3C_xAwRQIgC7HZtYuc6dI92m6wCcoXYpdzSpVtPTIbO7jBKGpUrYMCIQCc0WNtFvN8Awqx9uuRVp5SUSe3rOt2D7M-rCKpgVv_0A%3D%3D&lsparams=mh%2Cmm%2Cmn%2Cms%2Cmv%2Cmvi%2Cpl%2Cinitcwndbps&mh=wB&mime=audio%2Fwebm&mm=31%2C29&mn=sn-h0jeener%2Csn-h0jeln7l&ms=au%2Crdu&mt=1683811031&mv=m&mvi=4&n=U8mCOo4eYD4n0A&ns=LToEdXWVFHcH53e3aTe1N7kN&pl=37&requiressl=yes&sig=AOq0QJ8wRAIgW1DTCrLV_GyEM1rdjScgyceZE1llb73KJMFXmPm5Y04CIAYOLZuuzFX4ba5720kMOcQ1-Ld1DULs85nLxJglitCl&source=youtube&sparams=expire%2Cei%2Cip%2Cid%2Citag%2Csource%2Crequiressl%2Cspc%2Cvprv%2Csvpuc%2Cxtags%2Cmime%2Cns%2Cgir%2Cclen%2Cdur%2Clmt&spc=qEK7B81AP536F3aOi5JzMyLCUDiktWigtEpf9nI2xg&svpuc=1&txp=4532434&vprv=1&xtags=acont%3Doriginal%3Alang%3Den-US"))]
    #[case::lang_fallback(StreamFilter::default().audio_language("xx").clone(), Some("https://rr4---sn-h0jeener.googlevideo.com/videoplayback?c=WEB&clen=16104136&dur=1012.661&ei=6OtcZNqtBdOi7gP1upHYCQ&expire=1683832904&fexp=24007246&fvip=2&gir=yes&id=o-ABVtPh3j24hkJeXp8igjvreyODn-oV0CacOqb7pDjJoG&initcwndbps=1720000&ip=2003%3Ade%3Aaf31%3A5200%3A791a%3A897%3Ac15c%3Aae59&itag=251&keepalive=yes&lmt=1683782301237288&lsig=AG3C_xAwRQIgC7HZtYuc6dI92m6wCcoXYpdzSpVtPTIbO7jBKGpUrYMCIQCc0WNtFvN8Awqx9uuRVp5SUSe3rOt2D7M-rCKpgVv_0A%3D%3D&lsparams=mh%2Cmm%2Cmn%2Cms%2Cmv%2Cmvi%2Cpl%2Cinitcwndbps&mh=wB&mime=audio%2Fwebm&mm=31%2C29&mn=sn-h0jeener%2Csn-h0jeln7l&ms=au%2Crdu&mt=1683811031&mv=m&mvi=4&n=U8mCOo4eYD4n0A&ns=LToEdXWVFHcH53e3aTe1N7kN&pl=37&requiressl=yes&sig=AOq0QJ8wRQIhAPcUhhfkNVA_JcdU6KLTOFjRCnNl6n8gamJA-Q0PgCpIAiBTMV2k2JfHzbHBtsHxuNW7zHvSaYaUbz-dEIQC45o1eA%3D%3D&source=youtube&sparams=expire%2Cei%2Cip%2Cid%2Citag%2Csource%2Crequiressl%2Cspc%2Cvprv%2Csvpuc%2Cxtags%2Cmime%2Cns%2Cgir%2Cclen%2Cdur%2Clmt&spc=qEK7B81AP536F3aOi5JzMyLCUDiktWigtEpf9nI2xg&svpuc=1&txp=4532434&vprv=1&xtags=acont%3Doriginal%3Alang%3Den-US"))]
    #[case::noformat(StreamFilter::default().audio_formats(&[]).clone(), None)]
    #[case::nocodec(StreamFilter::default().audio_codecs(&[]).clone(), None)]
    fn t_select_audio_stream(#[case] filter: StreamFilter, #[case] expect_url: Option<&str>) {
        let selection = PLAYER_ML.select_audio_stream(&filter);

        match expect_url {
            Some(expect_url) => assert_eq!(selection.unwrap().url, expect_url),
            None => assert_eq!(selection, None),
        }
    }

    #[rstest]
    #[case::default(StreamFilter::default(), Some("https://rr5---sn-h0jelne7.googlevideo.com/videoplayback?aitags=133%2C134%2C135%2C136%2C160%2C242%2C243%2C244%2C247%2C278%2C298%2C299%2C302%2C303%2C308%2C315%2C330%2C331%2C332%2C333%2C334%2C335%2C336%2C337%2C394%2C395%2C396%2C397%2C398%2C399%2C400%2C401%2C694%2C695%2C696%2C697%2C698%2C699%2C700%2C701&c=WEB&clen=998696577&dur=313.780&ei=eckIY72IKcGZ8gOMt6CwDg&expire=1661541849&fexp=24001373%2C24007246&fvip=2&gir=yes&id=o-AOqXE9lVS424yszv6LN5V_gaevdHxenJl-tYNy3Drs6g&initcwndbps=1428750&ip=2003%3Ade%3Aaf05%3A2500%3A5dad%3A319b%3Aca30%3Ae212&itag=315&keepalive=yes&lmt=1647476955807851&lsig=AG3C_xAwRQIhAMioKyc-dqs-6uvAwLViCcCTXKHn9sIbo0cbSSBXGG4kAiBQNsRBAvQrbWdOjZIsQXYrfPEb1KDpE_AlSEGQZXB9uA%3D%3D&lsparams=mh%2Cmm%2Cmn%2Cms%2Cmv%2Cmvi%2Cpl%2Cinitcwndbps&mh=NH&mime=video%2Fwebm&mm=31%2C29&mn=sn-h0jelne7%2Csn-h0jeenl6&ms=au%2Crdu&mt=1661519833&mv=m&mvi=5&n=Zd7nrOM1B2C6PA&ns=426LxLap5MonJD_YWdS4lSYH&pl=37&rbqsm=fr&requiressl=yes&sig=AOq0QJ8wRAIfP4IVSo-00_kq_JIkuh032hcLoJzNEhYjvwgLiDpEzQIhALPVrvDBjRwiFddXiAyADmRtYygte4HvlJ3XOrkOf_TR&source=youtube&sparams=expire%2Cei%2Cip%2Cid%2Caitags%2Csource%2Crequiressl%2Cspc%2Cvprv%2Cmime%2Cns%2Cgir%2Cclen%2Cdur%2Clmt&spc=lT-KhuPtxVzL5-QbZ7S9zNeOHsWTdms&txp=4532434&vprv=1"))]
    #[case::hdr(StreamFilter::default().video_hdr().clone(), Some("https://rr5---sn-h0jelne7.googlevideo.com/videoplayback?aitags=133%2C134%2C135%2C136%2C160%2C242%2C243%2C244%2C247%2C278%2C298%2C299%2C302%2C303%2C308%2C315%2C330%2C331%2C332%2C333%2C334%2C335%2C336%2C337%2C394%2C395%2C396%2C397%2C398%2C399%2C400%2C401%2C694%2C695%2C696%2C697%2C698%2C699%2C700%2C701&c=WEB&clen=976824147&dur=313.780&ei=eckIY72IKcGZ8gOMt6CwDg&expire=1661541849&fexp=24001373%2C24007246&fvip=2&gir=yes&id=o-AOqXE9lVS424yszv6LN5V_gaevdHxenJl-tYNy3Drs6g&initcwndbps=1428750&ip=2003%3Ade%3Aaf05%3A2500%3A5dad%3A319b%3Aca30%3Ae212&itag=701&keepalive=yes&lmt=1647469891607029&lsig=AG3C_xAwRQIhAMioKyc-dqs-6uvAwLViCcCTXKHn9sIbo0cbSSBXGG4kAiBQNsRBAvQrbWdOjZIsQXYrfPEb1KDpE_AlSEGQZXB9uA%3D%3D&lsparams=mh%2Cmm%2Cmn%2Cms%2Cmv%2Cmvi%2Cpl%2Cinitcwndbps&mh=NH&mime=video%2Fmp4&mm=31%2C29&mn=sn-h0jelne7%2Csn-h0jeenl6&ms=au%2Crdu&mt=1661519833&mv=m&mvi=5&n=Zd7nrOM1B2C6PA&ns=426LxLap5MonJD_YWdS4lSYH&pl=37&rbqsm=fr&requiressl=yes&sig=AOq0QJ8wRgIhAOax_lAWCW5ENOYxe3gZfBHgHA5oZJPyMlYQFy73t7-pAiEA46J7dsT-1pv9smuoP3Kx5T7c_IJ6cEZN4U9UkSNuT7o%3D&source=youtube&sparams=expire%2Cei%2Cip%2Cid%2Caitags%2Csource%2Crequiressl%2Cspc%2Cvprv%2Cmime%2Cns%2Cgir%2Cclen%2Cdur%2Clmt&spc=lT-KhuPtxVzL5-QbZ7S9zNeOHsWTdms&txp=4532434&vprv=1"))]
    #[case::resolution(StreamFilter::default().video_max_res(720).clone(), Some("https://rr5---sn-h0jelne7.googlevideo.com/videoplayback?aitags=133%2C134%2C135%2C136%2C160%2C242%2C243%2C244%2C247%2C278%2C298%2C299%2C302%2C303%2C308%2C315%2C330%2C331%2C332%2C333%2C334%2C335%2C336%2C337%2C394%2C395%2C396%2C397%2C398%2C399%2C400%2C401%2C694%2C695%2C696%2C697%2C698%2C699%2C700%2C701&c=WEB&clen=76313586&dur=313.780&ei=eckIY72IKcGZ8gOMt6CwDg&expire=1661541849&fexp=24001373%2C24007246&fvip=2&gir=yes&id=o-AOqXE9lVS424yszv6LN5V_gaevdHxenJl-tYNy3Drs6g&initcwndbps=1428750&ip=2003%3Ade%3Aaf05%3A2500%3A5dad%3A319b%3Aca30%3Ae212&itag=302&keepalive=yes&lmt=1647455155369524&lsig=AG3C_xAwRQIhAMioKyc-dqs-6uvAwLViCcCTXKHn9sIbo0cbSSBXGG4kAiBQNsRBAvQrbWdOjZIsQXYrfPEb1KDpE_AlSEGQZXB9uA%3D%3D&lsparams=mh%2Cmm%2Cmn%2Cms%2Cmv%2Cmvi%2Cpl%2Cinitcwndbps&mh=NH&mime=video%2Fwebm&mm=31%2C29&mn=sn-h0jelne7%2Csn-h0jeenl6&ms=au%2Crdu&mt=1661519833&mv=m&mvi=5&n=Zd7nrOM1B2C6PA&ns=426LxLap5MonJD_YWdS4lSYH&pl=37&rbqsm=fr&requiressl=yes&sig=AOq0QJ8wRAIgW0H1434eh9Axw6zw95qezJB0D2aVd2bxEIs4T5bcfFACIDOjha9WLycp0L188FZyFGa1RBkLPoGrrJOppsaXqwDR&source=youtube&sparams=expire%2Cei%2Cip%2Cid%2Caitags%2Csource%2Crequiressl%2Cspc%2Cvprv%2Cmime%2Cns%2Cgir%2Cclen%2Cdur%2Clmt&spc=lT-KhuPtxVzL5-QbZ7S9zNeOHsWTdms&txp=4532434&vprv=1"))]
    #[case::resolution_fps(StreamFilter::default().video_max_res(720).video_max_fps(30).clone(), Some("https://rr5---sn-h0jelne7.googlevideo.com/videoplayback?aitags=133%2C134%2C135%2C136%2C160%2C242%2C243%2C244%2C247%2C278%2C298%2C299%2C302%2C303%2C308%2C315%2C330%2C331%2C332%2C333%2C334%2C335%2C336%2C337%2C394%2C395%2C396%2C397%2C398%2C399%2C400%2C401%2C694%2C695%2C696%2C697%2C698%2C699%2C700%2C701&c=WEB&clen=47531179&dur=313.780&ei=eckIY72IKcGZ8gOMt6CwDg&expire=1661541849&fexp=24001373%2C24007246&fvip=2&gir=yes&id=o-AOqXE9lVS424yszv6LN5V_gaevdHxenJl-tYNy3Drs6g&initcwndbps=1428750&ip=2003%3Ade%3Aaf05%3A2500%3A5dad%3A319b%3Aca30%3Ae212&itag=247&keepalive=yes&lmt=1647458657499381&lsig=AG3C_xAwRQIhAMioKyc-dqs-6uvAwLViCcCTXKHn9sIbo0cbSSBXGG4kAiBQNsRBAvQrbWdOjZIsQXYrfPEb1KDpE_AlSEGQZXB9uA%3D%3D&lsparams=mh%2Cmm%2Cmn%2Cms%2Cmv%2Cmvi%2Cpl%2Cinitcwndbps&mh=NH&mime=video%2Fwebm&mm=31%2C29&mn=sn-h0jelne7%2Csn-h0jeenl6&ms=au%2Crdu&mt=1661519833&mv=m&mvi=5&n=Zd7nrOM1B2C6PA&ns=426LxLap5MonJD_YWdS4lSYH&pl=37&rbqsm=fr&requiressl=yes&sig=AOq0QJ8wRgIhAMUsmcl1zgbr3YQranPWNV1kcxT5IdEoLL7FTFEDdHHPAiEAhQnrfYMU0A9xZ69MfBujWA4pXtCOQCg2Jn6ve9J_vBQ%3D&source=youtube&sparams=expire%2Cei%2Cip%2Cid%2Caitags%2Csource%2Crequiressl%2Cspc%2Cvprv%2Cmime%2Cns%2Cgir%2Cclen%2Cdur%2Clmt&spc=lT-KhuPtxVzL5-QbZ7S9zNeOHsWTdms&txp=4532434&vprv=1"))]
    #[case::res_fallback(StreamFilter::default().video_max_res(100).clone(), Some("https://rr5---sn-h0jelne7.googlevideo.com/videoplayback?aitags=133%2C134%2C135%2C136%2C160%2C242%2C243%2C244%2C247%2C278%2C298%2C299%2C302%2C303%2C308%2C315%2C330%2C331%2C332%2C333%2C334%2C335%2C336%2C337%2C394%2C395%2C396%2C397%2C398%2C399%2C400%2C401%2C694%2C695%2C696%2C697%2C698%2C699%2C700%2C701&c=WEB&clen=2763284&dur=313.780&ei=eckIY72IKcGZ8gOMt6CwDg&expire=1661541849&fexp=24001373%2C24007246&fvip=2&gir=yes&id=o-AOqXE9lVS424yszv6LN5V_gaevdHxenJl-tYNy3Drs6g&initcwndbps=1428750&ip=2003%3Ade%3Aaf05%3A2500%3A5dad%3A319b%3Aca30%3Ae212&itag=160&keepalive=yes&lmt=1647456833049253&lsig=AG3C_xAwRQIhAMioKyc-dqs-6uvAwLViCcCTXKHn9sIbo0cbSSBXGG4kAiBQNsRBAvQrbWdOjZIsQXYrfPEb1KDpE_AlSEGQZXB9uA%3D%3D&lsparams=mh%2Cmm%2Cmn%2Cms%2Cmv%2Cmvi%2Cpl%2Cinitcwndbps&mh=NH&mime=video%2Fmp4&mm=31%2C29&mn=sn-h0jelne7%2Csn-h0jeenl6&ms=au%2Crdu&mt=1661519833&mv=m&mvi=5&n=Zd7nrOM1B2C6PA&ns=426LxLap5MonJD_YWdS4lSYH&pl=37&rbqsm=fr&requiressl=yes&sig=AOq0QJ8wRQIgLPNxzLxppSSpnDEHxVblrQ38890NMbGnLXlmxljprfQCIQDn4Ir_sjYh7S3ms-Rynm-K0nJpHpQGYsz1nv4TiqeELQ%3D%3D&source=youtube&sparams=expire%2Cei%2Cip%2Cid%2Caitags%2Csource%2Crequiressl%2Cspc%2Cvprv%2Cmime%2Cns%2Cgir%2Cclen%2Cdur%2Clmt&spc=lT-KhuPtxVzL5-QbZ7S9zNeOHsWTdms&txp=4532434&vprv=1"))]
    #[case::webm_format(StreamFilter::default().video_formats(&[VideoFormat::Webm]).clone(), Some("https://rr5---sn-h0jelne7.googlevideo.com/videoplayback?aitags=133%2C134%2C135%2C136%2C160%2C242%2C243%2C244%2C247%2C278%2C298%2C299%2C302%2C303%2C308%2C315%2C330%2C331%2C332%2C333%2C334%2C335%2C336%2C337%2C394%2C395%2C396%2C397%2C398%2C399%2C400%2C401%2C694%2C695%2C696%2C697%2C698%2C699%2C700%2C701&c=WEB&clen=998696577&dur=313.780&ei=eckIY72IKcGZ8gOMt6CwDg&expire=1661541849&fexp=24001373%2C24007246&fvip=2&gir=yes&id=o-AOqXE9lVS424yszv6LN5V_gaevdHxenJl-tYNy3Drs6g&initcwndbps=1428750&ip=2003%3Ade%3Aaf05%3A2500%3A5dad%3A319b%3Aca30%3Ae212&itag=315&keepalive=yes&lmt=1647476955807851&lsig=AG3C_xAwRQIhAMioKyc-dqs-6uvAwLViCcCTXKHn9sIbo0cbSSBXGG4kAiBQNsRBAvQrbWdOjZIsQXYrfPEb1KDpE_AlSEGQZXB9uA%3D%3D&lsparams=mh%2Cmm%2Cmn%2Cms%2Cmv%2Cmvi%2Cpl%2Cinitcwndbps&mh=NH&mime=video%2Fwebm&mm=31%2C29&mn=sn-h0jelne7%2Csn-h0jeenl6&ms=au%2Crdu&mt=1661519833&mv=m&mvi=5&n=Zd7nrOM1B2C6PA&ns=426LxLap5MonJD_YWdS4lSYH&pl=37&rbqsm=fr&requiressl=yes&sig=AOq0QJ8wRAIfP4IVSo-00_kq_JIkuh032hcLoJzNEhYjvwgLiDpEzQIhALPVrvDBjRwiFddXiAyADmRtYygte4HvlJ3XOrkOf_TR&source=youtube&sparams=expire%2Cei%2Cip%2Cid%2Caitags%2Csource%2Crequiressl%2Cspc%2Cvprv%2Cmime%2Cns%2Cgir%2Cclen%2Cdur%2Clmt&spc=lT-KhuPtxVzL5-QbZ7S9zNeOHsWTdms&txp=4532434&vprv=1"))]
    #[case::vp9_codec(StreamFilter::default().video_codecs(&[VideoCodec::Vp9]).clone(), Some("https://rr5---sn-h0jelne7.googlevideo.com/videoplayback?aitags=133%2C134%2C135%2C136%2C160%2C242%2C243%2C244%2C247%2C278%2C298%2C299%2C302%2C303%2C308%2C315%2C330%2C331%2C332%2C333%2C334%2C335%2C336%2C337%2C394%2C395%2C396%2C397%2C398%2C399%2C400%2C401%2C694%2C695%2C696%2C697%2C698%2C699%2C700%2C701&c=WEB&clen=998696577&dur=313.780&ei=eckIY72IKcGZ8gOMt6CwDg&expire=1661541849&fexp=24001373%2C24007246&fvip=2&gir=yes&id=o-AOqXE9lVS424yszv6LN5V_gaevdHxenJl-tYNy3Drs6g&initcwndbps=1428750&ip=2003%3Ade%3Aaf05%3A2500%3A5dad%3A319b%3Aca30%3Ae212&itag=315&keepalive=yes&lmt=1647476955807851&lsig=AG3C_xAwRQIhAMioKyc-dqs-6uvAwLViCcCTXKHn9sIbo0cbSSBXGG4kAiBQNsRBAvQrbWdOjZIsQXYrfPEb1KDpE_AlSEGQZXB9uA%3D%3D&lsparams=mh%2Cmm%2Cmn%2Cms%2Cmv%2Cmvi%2Cpl%2Cinitcwndbps&mh=NH&mime=video%2Fwebm&mm=31%2C29&mn=sn-h0jelne7%2Csn-h0jeenl6&ms=au%2Crdu&mt=1661519833&mv=m&mvi=5&n=Zd7nrOM1B2C6PA&ns=426LxLap5MonJD_YWdS4lSYH&pl=37&rbqsm=fr&requiressl=yes&sig=AOq0QJ8wRAIfP4IVSo-00_kq_JIkuh032hcLoJzNEhYjvwgLiDpEzQIhALPVrvDBjRwiFddXiAyADmRtYygte4HvlJ3XOrkOf_TR&source=youtube&sparams=expire%2Cei%2Cip%2Cid%2Caitags%2Csource%2Crequiressl%2Cspc%2Cvprv%2Cmime%2Cns%2Cgir%2Cclen%2Cdur%2Clmt&spc=lT-KhuPtxVzL5-QbZ7S9zNeOHsWTdms&txp=4532434&vprv=1"))]
    #[case::noformat(StreamFilter::default().video_formats(&[]).clone(), None)]
    #[case::nocodec(StreamFilter::default().video_codecs(&[]).clone(), None)]
    fn t_select_video_only_stream(#[case] filter: StreamFilter, #[case] expect_url: Option<&str>) {
        let selection = PLAYER_HDR.select_video_only_stream(&filter);

        match expect_url {
            Some(expect_url) => assert_eq!(selection.unwrap().url, expect_url),
            None => assert_eq!(selection, None),
        }
    }

    #[rstest]
    #[case::default(
        StreamFilter::default(),
        Some("https://rr5---sn-h0jelne7.googlevideo.com/videoplayback?aitags=133%2C134%2C135%2C136%2C160%2C242%2C243%2C244%2C247%2C278%2C298%2C299%2C302%2C303%2C308%2C315%2C330%2C331%2C332%2C333%2C334%2C335%2C336%2C337%2C394%2C395%2C396%2C397%2C398%2C399%2C400%2C401%2C694%2C695%2C696%2C697%2C698%2C699%2C700%2C701&c=WEB&clen=998696577&dur=313.780&ei=eckIY72IKcGZ8gOMt6CwDg&expire=1661541849&fexp=24001373%2C24007246&fvip=2&gir=yes&id=o-AOqXE9lVS424yszv6LN5V_gaevdHxenJl-tYNy3Drs6g&initcwndbps=1428750&ip=2003%3Ade%3Aaf05%3A2500%3A5dad%3A319b%3Aca30%3Ae212&itag=315&keepalive=yes&lmt=1647476955807851&lsig=AG3C_xAwRQIhAMioKyc-dqs-6uvAwLViCcCTXKHn9sIbo0cbSSBXGG4kAiBQNsRBAvQrbWdOjZIsQXYrfPEb1KDpE_AlSEGQZXB9uA%3D%3D&lsparams=mh%2Cmm%2Cmn%2Cms%2Cmv%2Cmvi%2Cpl%2Cinitcwndbps&mh=NH&mime=video%2Fwebm&mm=31%2C29&mn=sn-h0jelne7%2Csn-h0jeenl6&ms=au%2Crdu&mt=1661519833&mv=m&mvi=5&n=Zd7nrOM1B2C6PA&ns=426LxLap5MonJD_YWdS4lSYH&pl=37&rbqsm=fr&requiressl=yes&sig=AOq0QJ8wRAIfP4IVSo-00_kq_JIkuh032hcLoJzNEhYjvwgLiDpEzQIhALPVrvDBjRwiFddXiAyADmRtYygte4HvlJ3XOrkOf_TR&source=youtube&sparams=expire%2Cei%2Cip%2Cid%2Caitags%2Csource%2Crequiressl%2Cspc%2Cvprv%2Cmime%2Cns%2Cgir%2Cclen%2Cdur%2Clmt&spc=lT-KhuPtxVzL5-QbZ7S9zNeOHsWTdms&txp=4532434&vprv=1"),
        Some("https://rr5---sn-h0jelne7.googlevideo.com/videoplayback?c=WEB&clen=5199784&dur=313.801&ei=eckIY72IKcGZ8gOMt6CwDg&expire=1661541849&fexp=24001373%2C24007246&fvip=2&gir=yes&id=o-AOqXE9lVS424yszv6LN5V_gaevdHxenJl-tYNy3Drs6g&initcwndbps=1428750&ip=2003%3Ade%3Aaf05%3A2500%3A5dad%3A319b%3Aca30%3Ae212&itag=251&keepalive=yes&lmt=1647453650291076&lsig=AG3C_xAwRQIhAMioKyc-dqs-6uvAwLViCcCTXKHn9sIbo0cbSSBXGG4kAiBQNsRBAvQrbWdOjZIsQXYrfPEb1KDpE_AlSEGQZXB9uA%3D%3D&lsparams=mh%2Cmm%2Cmn%2Cms%2Cmv%2Cmvi%2Cpl%2Cinitcwndbps&mh=NH&mime=audio%2Fwebm&mm=31%2C29&mn=sn-h0jelne7%2Csn-h0jeenl6&ms=au%2Crdu&mt=1661519833&mv=m&mvi=5&n=Zd7nrOM1B2C6PA&ns=426LxLap5MonJD_YWdS4lSYH&pl=37&rbqsm=fr&requiressl=yes&sig=AOq0QJ8wRQIhALtI3j8ZChpNb0LcyDZ3yosbWnSpqaO0-jKAe_UM_RQyAiAMwrpdeNbJEnQn3q1eveaAcRcNIwy5iJ4fIjeBW_MUfg%3D%3D&source=youtube&sparams=expire%2Cei%2Cip%2Cid%2Citag%2Csource%2Crequiressl%2Cspc%2Cvprv%2Cmime%2Cns%2Cgir%2Cclen%2Cdur%2Clmt&spc=lT-KhuPtxVzL5-QbZ7S9zNeOHsWTdms&txp=4532434&vprv=1")
    )]
    #[case::webm(
        StreamFilter::default().video_formats(&[VideoFormat::Webm]).clone(),
        Some("https://rr5---sn-h0jelne7.googlevideo.com/videoplayback?aitags=133%2C134%2C135%2C136%2C160%2C242%2C243%2C244%2C247%2C278%2C298%2C299%2C302%2C303%2C308%2C315%2C330%2C331%2C332%2C333%2C334%2C335%2C336%2C337%2C394%2C395%2C396%2C397%2C398%2C399%2C400%2C401%2C694%2C695%2C696%2C697%2C698%2C699%2C700%2C701&c=WEB&clen=998696577&dur=313.780&ei=eckIY72IKcGZ8gOMt6CwDg&expire=1661541849&fexp=24001373%2C24007246&fvip=2&gir=yes&id=o-AOqXE9lVS424yszv6LN5V_gaevdHxenJl-tYNy3Drs6g&initcwndbps=1428750&ip=2003%3Ade%3Aaf05%3A2500%3A5dad%3A319b%3Aca30%3Ae212&itag=315&keepalive=yes&lmt=1647476955807851&lsig=AG3C_xAwRQIhAMioKyc-dqs-6uvAwLViCcCTXKHn9sIbo0cbSSBXGG4kAiBQNsRBAvQrbWdOjZIsQXYrfPEb1KDpE_AlSEGQZXB9uA%3D%3D&lsparams=mh%2Cmm%2Cmn%2Cms%2Cmv%2Cmvi%2Cpl%2Cinitcwndbps&mh=NH&mime=video%2Fwebm&mm=31%2C29&mn=sn-h0jelne7%2Csn-h0jeenl6&ms=au%2Crdu&mt=1661519833&mv=m&mvi=5&n=Zd7nrOM1B2C6PA&ns=426LxLap5MonJD_YWdS4lSYH&pl=37&rbqsm=fr&requiressl=yes&sig=AOq0QJ8wRAIfP4IVSo-00_kq_JIkuh032hcLoJzNEhYjvwgLiDpEzQIhALPVrvDBjRwiFddXiAyADmRtYygte4HvlJ3XOrkOf_TR&source=youtube&sparams=expire%2Cei%2Cip%2Cid%2Caitags%2Csource%2Crequiressl%2Cspc%2Cvprv%2Cmime%2Cns%2Cgir%2Cclen%2Cdur%2Clmt&spc=lT-KhuPtxVzL5-QbZ7S9zNeOHsWTdms&txp=4532434&vprv=1"),
        Some("https://rr5---sn-h0jelne7.googlevideo.com/videoplayback?c=WEB&clen=5199784&dur=313.801&ei=eckIY72IKcGZ8gOMt6CwDg&expire=1661541849&fexp=24001373%2C24007246&fvip=2&gir=yes&id=o-AOqXE9lVS424yszv6LN5V_gaevdHxenJl-tYNy3Drs6g&initcwndbps=1428750&ip=2003%3Ade%3Aaf05%3A2500%3A5dad%3A319b%3Aca30%3Ae212&itag=251&keepalive=yes&lmt=1647453650291076&lsig=AG3C_xAwRQIhAMioKyc-dqs-6uvAwLViCcCTXKHn9sIbo0cbSSBXGG4kAiBQNsRBAvQrbWdOjZIsQXYrfPEb1KDpE_AlSEGQZXB9uA%3D%3D&lsparams=mh%2Cmm%2Cmn%2Cms%2Cmv%2Cmvi%2Cpl%2Cinitcwndbps&mh=NH&mime=audio%2Fwebm&mm=31%2C29&mn=sn-h0jelne7%2Csn-h0jeenl6&ms=au%2Crdu&mt=1661519833&mv=m&mvi=5&n=Zd7nrOM1B2C6PA&ns=426LxLap5MonJD_YWdS4lSYH&pl=37&rbqsm=fr&requiressl=yes&sig=AOq0QJ8wRQIhALtI3j8ZChpNb0LcyDZ3yosbWnSpqaO0-jKAe_UM_RQyAiAMwrpdeNbJEnQn3q1eveaAcRcNIwy5iJ4fIjeBW_MUfg%3D%3D&source=youtube&sparams=expire%2Cei%2Cip%2Cid%2Citag%2Csource%2Crequiressl%2Cspc%2Cvprv%2Cmime%2Cns%2Cgir%2Cclen%2Cdur%2Clmt&spc=lT-KhuPtxVzL5-QbZ7S9zNeOHsWTdms&txp=4532434&vprv=1")
    )]
    #[case::noaudio(
        StreamFilter::default().audio_formats(&[]).clone(),
        Some("https://rr5---sn-h0jelne7.googlevideo.com/videoplayback?c=WEB&clen=23544588&dur=313.834&ei=eckIY72IKcGZ8gOMt6CwDg&expire=1661541849&fexp=24001373%2C24007246&fvip=2&gir=yes&id=o-AOqXE9lVS424yszv6LN5V_gaevdHxenJl-tYNy3Drs6g&initcwndbps=1428750&ip=2003%3Ade%3Aaf05%3A2500%3A5dad%3A319b%3Aca30%3Ae212&itag=18&lmt=1647456546485912&lsig=AG3C_xAwRQIhAMioKyc-dqs-6uvAwLViCcCTXKHn9sIbo0cbSSBXGG4kAiBQNsRBAvQrbWdOjZIsQXYrfPEb1KDpE_AlSEGQZXB9uA%3D%3D&lsparams=mh%2Cmm%2Cmn%2Cms%2Cmv%2Cmvi%2Cpl%2Cinitcwndbps&mh=NH&mime=video%2Fmp4&mm=31%2C29&mn=sn-h0jelne7%2Csn-h0jeenl6&ms=au%2Crdu&mt=1661519833&mv=m&mvi=5&n=HWZNhARNT_nJgg&ns=pLFQxzhiCbZ9F2HJmDLveKoH&pl=37&ratebypass=yes&rbqsm=fr&requiressl=yes&sig=AOq0QJ8wRQIgeCEjusAq6p33rH0NHyTAbPIRaaEkjDE32AXBFzDvR-ICIQD0LI8hQVH8oCMWu6OuADzc1FSQhIqYs5RLkxBmObIdsw%3D%3D&source=youtube&sparams=expire%2Cei%2Cip%2Cid%2Citag%2Csource%2Crequiressl%2Cspc%2Cvprv%2Cmime%2Cns%2Cgir%2Cclen%2Cratebypass%2Cdur%2Clmt&spc=lT-KhuPtxVzL5-QbZ7S9zNeOHsWTdms&txp=4530434&vprv=1"),
        None
    )]
    #[case::novideo(
        StreamFilter::default().no_video().clone(),
        None,
        Some("https://rr5---sn-h0jelne7.googlevideo.com/videoplayback?c=WEB&clen=5199784&dur=313.801&ei=eckIY72IKcGZ8gOMt6CwDg&expire=1661541849&fexp=24001373%2C24007246&fvip=2&gir=yes&id=o-AOqXE9lVS424yszv6LN5V_gaevdHxenJl-tYNy3Drs6g&initcwndbps=1428750&ip=2003%3Ade%3Aaf05%3A2500%3A5dad%3A319b%3Aca30%3Ae212&itag=251&keepalive=yes&lmt=1647453650291076&lsig=AG3C_xAwRQIhAMioKyc-dqs-6uvAwLViCcCTXKHn9sIbo0cbSSBXGG4kAiBQNsRBAvQrbWdOjZIsQXYrfPEb1KDpE_AlSEGQZXB9uA%3D%3D&lsparams=mh%2Cmm%2Cmn%2Cms%2Cmv%2Cmvi%2Cpl%2Cinitcwndbps&mh=NH&mime=audio%2Fwebm&mm=31%2C29&mn=sn-h0jelne7%2Csn-h0jeenl6&ms=au%2Crdu&mt=1661519833&mv=m&mvi=5&n=Zd7nrOM1B2C6PA&ns=426LxLap5MonJD_YWdS4lSYH&pl=37&rbqsm=fr&requiressl=yes&sig=AOq0QJ8wRQIhALtI3j8ZChpNb0LcyDZ3yosbWnSpqaO0-jKAe_UM_RQyAiAMwrpdeNbJEnQn3q1eveaAcRcNIwy5iJ4fIjeBW_MUfg%3D%3D&source=youtube&sparams=expire%2Cei%2Cip%2Cid%2Citag%2Csource%2Crequiressl%2Cspc%2Cvprv%2Cmime%2Cns%2Cgir%2Cclen%2Cdur%2Clmt&spc=lT-KhuPtxVzL5-QbZ7S9zNeOHsWTdms&txp=4532434&vprv=1")
    )]
    #[case::noformat(StreamFilter::default().audio_formats(&[]).video_formats(&[]).clone(), None, None)]
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
