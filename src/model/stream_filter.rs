//! Filters for selecting audio/video streams

use std::collections::HashSet;

use super::{
    AudioCodec, AudioFormat, AudioStream, VideoCodec, VideoFormat, VideoPlayer, VideoStream,
};

#[derive(Debug, Default, Clone)]
pub struct Filter {
    audio_max_bitrate: Option<u32>,
    audio_formats: Option<HashSet<AudioFormat>>,
    audio_codecs: Option<HashSet<AudioCodec>>,
    audio_language: Option<String>,
    video_max_res: Option<u32>,
    video_max_fps: Option<u8>,
    video_formats: Option<HashSet<VideoFormat>>,
    video_codecs: Option<HashSet<VideoCodec>>,
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
        match val {
            true => Self::Match,
            false => Self::Deny,
        }
    }

    fn soft(val: bool) -> Self {
        match val {
            true => Self::Match,
            false => Self::Allow,
        }
    }

    fn soft_lowest(val: bool) -> Self {
        match val {
            true => Self::Match,
            false => Self::AllowLowest,
        }
    }

    fn allow(val: bool) -> Self {
        match val {
            true => Self::Allow,
            false => Self::Deny,
        }
    }

    fn join(self, other: Self) -> Self {
        match self == Self::Deny {
            true => Self::Deny,
            false => self.min(other),
        }
    }
}

impl Filter {
    /// Set the maximum audio bitrate in bits per second.
    ///
    /// This is a soft filter, so if there is no stream with a bitrate
    /// <= the limit, the stream with the next higher bitrate is returned.
    pub fn audio_max_bitrate(&mut self, max_bitrate: u32) -> &mut Self {
        self.audio_max_bitrate = Some(max_bitrate);
        self
    }

    fn apply_audio_max_bitrate(&self, stream: &AudioStream) -> FilterResult {
        match self.audio_max_bitrate {
            Some(max_bitrate) => FilterResult::soft_lowest(stream.average_bitrate <= max_bitrate),
            None => FilterResult::Match,
        }
    }

    /// Set the supported audio container formats
    pub fn audio_formats(&mut self, formats: HashSet<AudioFormat>) -> &mut Self {
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
    pub fn audio_codecs(&mut self, codecs: HashSet<AudioCodec>) -> &mut Self {
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
    pub fn audio_language(&mut self, language: &str) -> &mut Self {
        self.audio_language = Some(language.to_owned());
        self
    }

    fn apply_audio_language(&self, stream: &AudioStream) -> FilterResult {
        match &self.audio_language {
            Some(language) => match &stream.track {
                Some(track) => match &track.lang {
                    Some(track_lang) => match track_lang == language {
                        true => FilterResult::Match,
                        false => FilterResult::allow(track.is_default),
                    },
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
    pub fn video_max_res(&mut self, max_res: u32) -> &mut Self {
        self.video_max_res = Some(max_res);
        self
    }

    fn apply_video_max_res(&self, stream: &VideoStream) -> FilterResult {
        match self.video_max_res {
            Some(max_res) => FilterResult::soft_lowest(stream.height.min(stream.width) <= max_res),
            None => FilterResult::Match,
        }
    }

    /// Set the maximum video framerate.
    ///
    /// This is a soft filter, so if there is no stream with a framerate
    /// <= the limit, the stream with the next higher framerate is returned.
    pub fn video_max_fps(&mut self, max_fps: u8) -> &mut Self {
        self.video_max_fps = Some(max_fps);
        self
    }

    fn apply_video_max_fps(&self, stream: &VideoStream) -> FilterResult {
        match self.video_max_fps {
            Some(max_fps) => FilterResult::soft_lowest(stream.fps <= max_fps),
            None => FilterResult::Match,
        }
    }

    /// Set the supported video container formats
    pub fn video_formats(&mut self, formats: HashSet<VideoFormat>) -> &mut Self {
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
    pub fn video_codecs(&mut self, codecs: HashSet<VideoCodec>) -> &mut Self {
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
    pub fn video_hdr(&mut self) -> &mut Self {
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
    pub fn no_video(&mut self) -> &mut Self {
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
    pub fn select_audio_stream(&self, filter: &Filter) -> Option<&AudioStream> {
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
        filter: &Filter,
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

    pub fn select_video_stream(&self, filter: &Filter) -> Option<&VideoStream> {
        Self::_select_video_stream(&self.video_streams, filter)
    }

    pub fn select_video_only_stream(&self, filter: &Filter) -> Option<&VideoStream> {
        Self::_select_video_stream(&self.video_only_streams, filter)
    }

    pub fn select_video_audio_stream(
        &self,
        filter: &Filter,
    ) -> (Option<&VideoStream>, Option<&AudioStream>) {
        let video_stream = self.select_video_stream(filter);
        let video_only_stream = self.select_video_only_stream(filter);

        match (video_stream, video_only_stream) {
            (None, None) => (None, self.select_audio_stream(filter)),
            (None, Some(video_only_stream)) => {
                (Some(video_only_stream), self.select_audio_stream(filter))
            }
            (Some(video_stream), None) => (Some(video_stream), None),
            (Some(video_stream), Some(video_only_stream)) => match video_only_stream > video_stream
            {
                true => (
                    Some(video_only_stream),
                    Some(some_or_bail!(
                        self.select_audio_stream(filter),
                        (Some(video_stream), None)
                    )),
                ),
                false => (Some(video_stream), None),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{fs::File, io::BufReader, path::Path};

    use super::*;
    use once_cell::sync::Lazy;
    use rstest::rstest;
    use velcro::hash_set;

    static PLAYER_ML: Lazy<VideoPlayer> = Lazy::new(|| {
        let json_path = Path::new("testfiles/player_model/multilanguage.json");
        let json_file = File::open(json_path).unwrap();

        serde_json::from_reader(BufReader::new(json_file)).unwrap()
    });

    static PLAYER_HDR: Lazy<VideoPlayer> = Lazy::new(|| {
        let json_path = Path::new("testfiles/player_model/hdr.json");
        let json_file = File::open(json_path).unwrap();

        serde_json::from_reader(BufReader::new(json_file)).unwrap()
    });

    #[rstest]
    #[case::default(Filter::default(), Some("https://rr4---sn-h0jeener.googlevideo.com/videoplayback?c=WEB&clen=16104134&dur=1012.661&ei=498HY6KvArqM6dsPiN6QgAE&expire=1661482051&fexp=24001373%2C24007246&fvip=2&gir=yes&id=o-AK8GbbQovVxldcBz4pkXu4EA9N2sU-4yPPa5hFT3bXta&initcwndbps=1392500&ip=2003%3Ade%3Aaf1e%3A8a00%3A84c6%3A28f3%3A9de2%3A464&itag=251&keepalive=yes&lmt=1659767097097120&lsig=AG3C_xAwRAIgLFPuLqOoHoNQax15AE9Q2YIZ7pM7-olbGWgYGv1MDccCIADKSc_HeOdmD7CDs4AkY5ZtWF4gdZd4rw99Cqlzakbk&lsparams=mh%2Cmm%2Cmn%2Cms%2Cmv%2Cmvi%2Cpl%2Cinitcwndbps&mh=wB&mime=audio%2Fwebm&mm=31%2C29&mn=sn-h0jeener%2Csn-h0jeln7l&ms=au%2Crdu&mt=1661460078&mv=m&mvi=4&n=LwUYrFgbIVPzmA&ns=bBqoZtLH6lsaX8ke0xgRMM8H&pl=37&rbqsm=fr&requiressl=yes&sig=AOq0QJ8wRQIgW4IxGJJFRAwZefvDdDkJfjhN7y3bPmh96BCFuyFn6pwCIQDW6pVnk_DwMC3FcZy5rXNUULMNWLdadScxwuhFTFR84g%3D%3D&source=youtube&sparams=expire%2Cei%2Cip%2Cid%2Citag%2Csource%2Crequiressl%2Cspc%2Cvprv%2Cxtags%2Cmime%2Cns%2Cgir%2Cclen%2Cdur%2Clmt&spc=lT-Khmjdu08WYG7DxAg_8xq0R2u5a6w&txp=4532434&vprv=1&xtags=lang%3Den"))]
    #[case::bitrate(Filter::default().audio_max_bitrate(100000).to_owned(), Some("https://rr4---sn-h0jeener.googlevideo.com/videoplayback?c=WEB&clen=8217507&dur=1012.661&ei=498HY6KvArqM6dsPiN6QgAE&expire=1661482051&fexp=24001373%2C24007246&fvip=2&gir=yes&id=o-AK8GbbQovVxldcBz4pkXu4EA9N2sU-4yPPa5hFT3bXta&initcwndbps=1392500&ip=2003%3Ade%3Aaf1e%3A8a00%3A84c6%3A28f3%3A9de2%3A464&itag=250&keepalive=yes&lmt=1659767073159859&lsig=AG3C_xAwRAIgLFPuLqOoHoNQax15AE9Q2YIZ7pM7-olbGWgYGv1MDccCIADKSc_HeOdmD7CDs4AkY5ZtWF4gdZd4rw99Cqlzakbk&lsparams=mh%2Cmm%2Cmn%2Cms%2Cmv%2Cmvi%2Cpl%2Cinitcwndbps&mh=wB&mime=audio%2Fwebm&mm=31%2C29&mn=sn-h0jeener%2Csn-h0jeln7l&ms=au%2Crdu&mt=1661460078&mv=m&mvi=4&n=LwUYrFgbIVPzmA&ns=bBqoZtLH6lsaX8ke0xgRMM8H&pl=37&rbqsm=fr&requiressl=yes&sig=AOq0QJ8wRAIgM9loLjlUtgwrALqSek4vO8KljcCltFjLw1TGX0d9lZ4CICRiTJ8a_KgdafXVo2vKwgLPuH2B7t0hF-ln2k_MI3ds&source=youtube&sparams=expire%2Cei%2Cip%2Cid%2Citag%2Csource%2Crequiressl%2Cspc%2Cvprv%2Cxtags%2Cmime%2Cns%2Cgir%2Cclen%2Cdur%2Clmt&spc=lT-Khmjdu08WYG7DxAg_8xq0R2u5a6w&txp=4532434&vprv=1&xtags=lang%3Den"))]
    #[case::m4a_format(Filter::default().audio_formats(hash_set!(AudioFormat::M4a)).to_owned(), Some("https://rr4---sn-h0jeener.googlevideo.com/videoplayback?c=WEB&clen=16390628&dur=1012.691&ei=498HY6KvArqM6dsPiN6QgAE&expire=1661482051&fexp=24001373%2C24007246&fvip=2&gir=yes&id=o-AK8GbbQovVxldcBz4pkXu4EA9N2sU-4yPPa5hFT3bXta&initcwndbps=1392500&ip=2003%3Ade%3Aaf1e%3A8a00%3A84c6%3A28f3%3A9de2%3A464&itag=140&keepalive=yes&lmt=1659766154827884&lsig=AG3C_xAwRAIgLFPuLqOoHoNQax15AE9Q2YIZ7pM7-olbGWgYGv1MDccCIADKSc_HeOdmD7CDs4AkY5ZtWF4gdZd4rw99Cqlzakbk&lsparams=mh%2Cmm%2Cmn%2Cms%2Cmv%2Cmvi%2Cpl%2Cinitcwndbps&mh=wB&mime=audio%2Fmp4&mm=31%2C29&mn=sn-h0jeener%2Csn-h0jeln7l&ms=au%2Crdu&mt=1661460078&mv=m&mvi=4&n=LwUYrFgbIVPzmA&ns=bBqoZtLH6lsaX8ke0xgRMM8H&pl=37&rbqsm=fr&requiressl=yes&sig=AOq0QJ8wRAIgc8oxPEHDO0cgO3ZcbPmv3nrkzfy52WchpV0HcBcUw24CIEFxLKBcM4vVqGeRkt581dFL2tetvHd93SHCTVEUnIn_&source=youtube&sparams=expire%2Cei%2Cip%2Cid%2Citag%2Csource%2Crequiressl%2Cspc%2Cvprv%2Cxtags%2Cmime%2Cns%2Cgir%2Cclen%2Cdur%2Clmt&spc=lT-Khmjdu08WYG7DxAg_8xq0R2u5a6w&txp=4532434&vprv=1&xtags=lang%3Den"))]
    #[case::m4a_codec(Filter::default().audio_codecs(hash_set!(AudioCodec::Mp4a)).to_owned(), Some("https://rr4---sn-h0jeener.googlevideo.com/videoplayback?c=WEB&clen=16390628&dur=1012.691&ei=498HY6KvArqM6dsPiN6QgAE&expire=1661482051&fexp=24001373%2C24007246&fvip=2&gir=yes&id=o-AK8GbbQovVxldcBz4pkXu4EA9N2sU-4yPPa5hFT3bXta&initcwndbps=1392500&ip=2003%3Ade%3Aaf1e%3A8a00%3A84c6%3A28f3%3A9de2%3A464&itag=140&keepalive=yes&lmt=1659766154827884&lsig=AG3C_xAwRAIgLFPuLqOoHoNQax15AE9Q2YIZ7pM7-olbGWgYGv1MDccCIADKSc_HeOdmD7CDs4AkY5ZtWF4gdZd4rw99Cqlzakbk&lsparams=mh%2Cmm%2Cmn%2Cms%2Cmv%2Cmvi%2Cpl%2Cinitcwndbps&mh=wB&mime=audio%2Fmp4&mm=31%2C29&mn=sn-h0jeener%2Csn-h0jeln7l&ms=au%2Crdu&mt=1661460078&mv=m&mvi=4&n=LwUYrFgbIVPzmA&ns=bBqoZtLH6lsaX8ke0xgRMM8H&pl=37&rbqsm=fr&requiressl=yes&sig=AOq0QJ8wRAIgc8oxPEHDO0cgO3ZcbPmv3nrkzfy52WchpV0HcBcUw24CIEFxLKBcM4vVqGeRkt581dFL2tetvHd93SHCTVEUnIn_&source=youtube&sparams=expire%2Cei%2Cip%2Cid%2Citag%2Csource%2Crequiressl%2Cspc%2Cvprv%2Cxtags%2Cmime%2Cns%2Cgir%2Cclen%2Cdur%2Clmt&spc=lT-Khmjdu08WYG7DxAg_8xq0R2u5a6w&txp=4532434&vprv=1&xtags=lang%3Den"))]
    #[case::french(Filter::default().audio_language("fr").to_owned(), Some("https://rr4---sn-h0jeener.googlevideo.com/videoplayback?c=WEB&clen=16357630&dur=1012.721&ei=498HY6KvArqM6dsPiN6QgAE&expire=1661482051&fexp=24001373%2C24007246&fvip=2&gir=yes&id=o-AK8GbbQovVxldcBz4pkXu4EA9N2sU-4yPPa5hFT3bXta&initcwndbps=1392500&ip=2003%3Ade%3Aaf1e%3A8a00%3A84c6%3A28f3%3A9de2%3A464&itag=251&keepalive=yes&lmt=1659767033119964&lsig=AG3C_xAwRAIgLFPuLqOoHoNQax15AE9Q2YIZ7pM7-olbGWgYGv1MDccCIADKSc_HeOdmD7CDs4AkY5ZtWF4gdZd4rw99Cqlzakbk&lsparams=mh%2Cmm%2Cmn%2Cms%2Cmv%2Cmvi%2Cpl%2Cinitcwndbps&mh=wB&mime=audio%2Fwebm&mm=31%2C29&mn=sn-h0jeener%2Csn-h0jeln7l&ms=au%2Crdu&mt=1661460078&mv=m&mvi=4&n=LwUYrFgbIVPzmA&ns=bBqoZtLH6lsaX8ke0xgRMM8H&pl=37&rbqsm=fr&requiressl=yes&sig=AOq0QJ8wRQIhAKtzVVyoS46hkuKX31EyUE6X6Q5wotcToOCnYKswX3x_AiB0G2SUdVoso39bYgewd3zT8Pf77DrVtahXh4kVb46T9g%3D%3D&source=youtube&sparams=expire%2Cei%2Cip%2Cid%2Citag%2Csource%2Crequiressl%2Cspc%2Cvprv%2Cxtags%2Cmime%2Cns%2Cgir%2Cclen%2Cdur%2Clmt&spc=lT-Khmjdu08WYG7DxAg_8xq0R2u5a6w&txp=4532434&vprv=1&xtags=lang%3Dfr"))]
    #[case::br_fallback(Filter::default().audio_max_bitrate(0).to_owned(), Some("https://rr4---sn-h0jeener.googlevideo.com/videoplayback?c=WEB&clen=6297404&dur=1012.661&ei=498HY6KvArqM6dsPiN6QgAE&expire=1661482051&fexp=24001373%2C24007246&fvip=2&gir=yes&id=o-AK8GbbQovVxldcBz4pkXu4EA9N2sU-4yPPa5hFT3bXta&initcwndbps=1392500&ip=2003%3Ade%3Aaf1e%3A8a00%3A84c6%3A28f3%3A9de2%3A464&itag=249&keepalive=yes&lmt=1659767062297621&lsig=AG3C_xAwRAIgLFPuLqOoHoNQax15AE9Q2YIZ7pM7-olbGWgYGv1MDccCIADKSc_HeOdmD7CDs4AkY5ZtWF4gdZd4rw99Cqlzakbk&lsparams=mh%2Cmm%2Cmn%2Cms%2Cmv%2Cmvi%2Cpl%2Cinitcwndbps&mh=wB&mime=audio%2Fwebm&mm=31%2C29&mn=sn-h0jeener%2Csn-h0jeln7l&ms=au%2Crdu&mt=1661460078&mv=m&mvi=4&n=LwUYrFgbIVPzmA&ns=bBqoZtLH6lsaX8ke0xgRMM8H&pl=37&rbqsm=fr&requiressl=yes&sig=AOq0QJ8wRgIhAPm10DeIvOt5Oc7e36cfhPC0ej2PslQqF3-CFVUl5TNfAiEAlgvwjlQK14e_-6j3W_hMvk9KHax8zd5shSVlYSR1P34%3D&source=youtube&sparams=expire%2Cei%2Cip%2Cid%2Citag%2Csource%2Crequiressl%2Cspc%2Cvprv%2Cxtags%2Cmime%2Cns%2Cgir%2Cclen%2Cdur%2Clmt&spc=lT-Khmjdu08WYG7DxAg_8xq0R2u5a6w&txp=4532434&vprv=1&xtags=lang%3Den"))]
    #[case::lang_fallback(Filter::default().audio_language("xx").to_owned(), Some("https://rr4---sn-h0jeener.googlevideo.com/videoplayback?c=WEB&clen=16104134&dur=1012.661&ei=498HY6KvArqM6dsPiN6QgAE&expire=1661482051&fexp=24001373%2C24007246&fvip=2&gir=yes&id=o-AK8GbbQovVxldcBz4pkXu4EA9N2sU-4yPPa5hFT3bXta&initcwndbps=1392500&ip=2003%3Ade%3Aaf1e%3A8a00%3A84c6%3A28f3%3A9de2%3A464&itag=251&keepalive=yes&lmt=1659767097097120&lsig=AG3C_xAwRAIgLFPuLqOoHoNQax15AE9Q2YIZ7pM7-olbGWgYGv1MDccCIADKSc_HeOdmD7CDs4AkY5ZtWF4gdZd4rw99Cqlzakbk&lsparams=mh%2Cmm%2Cmn%2Cms%2Cmv%2Cmvi%2Cpl%2Cinitcwndbps&mh=wB&mime=audio%2Fwebm&mm=31%2C29&mn=sn-h0jeener%2Csn-h0jeln7l&ms=au%2Crdu&mt=1661460078&mv=m&mvi=4&n=LwUYrFgbIVPzmA&ns=bBqoZtLH6lsaX8ke0xgRMM8H&pl=37&rbqsm=fr&requiressl=yes&sig=AOq0QJ8wRQIgW4IxGJJFRAwZefvDdDkJfjhN7y3bPmh96BCFuyFn6pwCIQDW6pVnk_DwMC3FcZy5rXNUULMNWLdadScxwuhFTFR84g%3D%3D&source=youtube&sparams=expire%2Cei%2Cip%2Cid%2Citag%2Csource%2Crequiressl%2Cspc%2Cvprv%2Cxtags%2Cmime%2Cns%2Cgir%2Cclen%2Cdur%2Clmt&spc=lT-Khmjdu08WYG7DxAg_8xq0R2u5a6w&txp=4532434&vprv=1&xtags=lang%3Den"))]
    #[case::noformat(Filter::default().audio_formats(hash_set!()).to_owned(), None)]
    #[case::nocodec(Filter::default().audio_codecs(hash_set!()).to_owned(), None)]
    fn t_select_audio_stream(#[case] filter: Filter, #[case] expect_url: Option<&str>) {
        let selection = PLAYER_ML.select_audio_stream(&filter);

        match expect_url {
            Some(expect_url) => assert_eq!(selection.unwrap().url, expect_url),
            None => assert_eq!(selection, None),
        }
    }

    #[rstest]
    #[case::default(Filter::default(), Some("https://rr5---sn-h0jelne7.googlevideo.com/videoplayback?aitags=133%2C134%2C135%2C136%2C160%2C242%2C243%2C244%2C247%2C278%2C298%2C299%2C302%2C303%2C308%2C315%2C330%2C331%2C332%2C333%2C334%2C335%2C336%2C337%2C394%2C395%2C396%2C397%2C398%2C399%2C400%2C401%2C694%2C695%2C696%2C697%2C698%2C699%2C700%2C701&c=WEB&clen=998696577&dur=313.780&ei=eckIY72IKcGZ8gOMt6CwDg&expire=1661541849&fexp=24001373%2C24007246&fvip=2&gir=yes&id=o-AOqXE9lVS424yszv6LN5V_gaevdHxenJl-tYNy3Drs6g&initcwndbps=1428750&ip=2003%3Ade%3Aaf05%3A2500%3A5dad%3A319b%3Aca30%3Ae212&itag=315&keepalive=yes&lmt=1647476955807851&lsig=AG3C_xAwRQIhAMioKyc-dqs-6uvAwLViCcCTXKHn9sIbo0cbSSBXGG4kAiBQNsRBAvQrbWdOjZIsQXYrfPEb1KDpE_AlSEGQZXB9uA%3D%3D&lsparams=mh%2Cmm%2Cmn%2Cms%2Cmv%2Cmvi%2Cpl%2Cinitcwndbps&mh=NH&mime=video%2Fwebm&mm=31%2C29&mn=sn-h0jelne7%2Csn-h0jeenl6&ms=au%2Crdu&mt=1661519833&mv=m&mvi=5&n=Zd7nrOM1B2C6PA&ns=426LxLap5MonJD_YWdS4lSYH&pl=37&rbqsm=fr&requiressl=yes&sig=AOq0QJ8wRAIfP4IVSo-00_kq_JIkuh032hcLoJzNEhYjvwgLiDpEzQIhALPVrvDBjRwiFddXiAyADmRtYygte4HvlJ3XOrkOf_TR&source=youtube&sparams=expire%2Cei%2Cip%2Cid%2Caitags%2Csource%2Crequiressl%2Cspc%2Cvprv%2Cmime%2Cns%2Cgir%2Cclen%2Cdur%2Clmt&spc=lT-KhuPtxVzL5-QbZ7S9zNeOHsWTdms&txp=4532434&vprv=1"))]
    #[case::hdr(Filter::default().video_hdr().to_owned(), Some("https://rr5---sn-h0jelne7.googlevideo.com/videoplayback?aitags=133%2C134%2C135%2C136%2C160%2C242%2C243%2C244%2C247%2C278%2C298%2C299%2C302%2C303%2C308%2C315%2C330%2C331%2C332%2C333%2C334%2C335%2C336%2C337%2C394%2C395%2C396%2C397%2C398%2C399%2C400%2C401%2C694%2C695%2C696%2C697%2C698%2C699%2C700%2C701&c=WEB&clen=976824147&dur=313.780&ei=eckIY72IKcGZ8gOMt6CwDg&expire=1661541849&fexp=24001373%2C24007246&fvip=2&gir=yes&id=o-AOqXE9lVS424yszv6LN5V_gaevdHxenJl-tYNy3Drs6g&initcwndbps=1428750&ip=2003%3Ade%3Aaf05%3A2500%3A5dad%3A319b%3Aca30%3Ae212&itag=701&keepalive=yes&lmt=1647469891607029&lsig=AG3C_xAwRQIhAMioKyc-dqs-6uvAwLViCcCTXKHn9sIbo0cbSSBXGG4kAiBQNsRBAvQrbWdOjZIsQXYrfPEb1KDpE_AlSEGQZXB9uA%3D%3D&lsparams=mh%2Cmm%2Cmn%2Cms%2Cmv%2Cmvi%2Cpl%2Cinitcwndbps&mh=NH&mime=video%2Fmp4&mm=31%2C29&mn=sn-h0jelne7%2Csn-h0jeenl6&ms=au%2Crdu&mt=1661519833&mv=m&mvi=5&n=Zd7nrOM1B2C6PA&ns=426LxLap5MonJD_YWdS4lSYH&pl=37&rbqsm=fr&requiressl=yes&sig=AOq0QJ8wRgIhAOax_lAWCW5ENOYxe3gZfBHgHA5oZJPyMlYQFy73t7-pAiEA46J7dsT-1pv9smuoP3Kx5T7c_IJ6cEZN4U9UkSNuT7o%3D&source=youtube&sparams=expire%2Cei%2Cip%2Cid%2Caitags%2Csource%2Crequiressl%2Cspc%2Cvprv%2Cmime%2Cns%2Cgir%2Cclen%2Cdur%2Clmt&spc=lT-KhuPtxVzL5-QbZ7S9zNeOHsWTdms&txp=4532434&vprv=1"))]
    #[case::resolution(Filter::default().video_max_res(720).to_owned(), Some("https://rr5---sn-h0jelne7.googlevideo.com/videoplayback?aitags=133%2C134%2C135%2C136%2C160%2C242%2C243%2C244%2C247%2C278%2C298%2C299%2C302%2C303%2C308%2C315%2C330%2C331%2C332%2C333%2C334%2C335%2C336%2C337%2C394%2C395%2C396%2C397%2C398%2C399%2C400%2C401%2C694%2C695%2C696%2C697%2C698%2C699%2C700%2C701&c=WEB&clen=76313586&dur=313.780&ei=eckIY72IKcGZ8gOMt6CwDg&expire=1661541849&fexp=24001373%2C24007246&fvip=2&gir=yes&id=o-AOqXE9lVS424yszv6LN5V_gaevdHxenJl-tYNy3Drs6g&initcwndbps=1428750&ip=2003%3Ade%3Aaf05%3A2500%3A5dad%3A319b%3Aca30%3Ae212&itag=302&keepalive=yes&lmt=1647455155369524&lsig=AG3C_xAwRQIhAMioKyc-dqs-6uvAwLViCcCTXKHn9sIbo0cbSSBXGG4kAiBQNsRBAvQrbWdOjZIsQXYrfPEb1KDpE_AlSEGQZXB9uA%3D%3D&lsparams=mh%2Cmm%2Cmn%2Cms%2Cmv%2Cmvi%2Cpl%2Cinitcwndbps&mh=NH&mime=video%2Fwebm&mm=31%2C29&mn=sn-h0jelne7%2Csn-h0jeenl6&ms=au%2Crdu&mt=1661519833&mv=m&mvi=5&n=Zd7nrOM1B2C6PA&ns=426LxLap5MonJD_YWdS4lSYH&pl=37&rbqsm=fr&requiressl=yes&sig=AOq0QJ8wRAIgW0H1434eh9Axw6zw95qezJB0D2aVd2bxEIs4T5bcfFACIDOjha9WLycp0L188FZyFGa1RBkLPoGrrJOppsaXqwDR&source=youtube&sparams=expire%2Cei%2Cip%2Cid%2Caitags%2Csource%2Crequiressl%2Cspc%2Cvprv%2Cmime%2Cns%2Cgir%2Cclen%2Cdur%2Clmt&spc=lT-KhuPtxVzL5-QbZ7S9zNeOHsWTdms&txp=4532434&vprv=1"))]
    #[case::resolution_fps(Filter::default().video_max_res(720).video_max_fps(30).to_owned(), Some("https://rr5---sn-h0jelne7.googlevideo.com/videoplayback?aitags=133%2C134%2C135%2C136%2C160%2C242%2C243%2C244%2C247%2C278%2C298%2C299%2C302%2C303%2C308%2C315%2C330%2C331%2C332%2C333%2C334%2C335%2C336%2C337%2C394%2C395%2C396%2C397%2C398%2C399%2C400%2C401%2C694%2C695%2C696%2C697%2C698%2C699%2C700%2C701&c=WEB&clen=47531179&dur=313.780&ei=eckIY72IKcGZ8gOMt6CwDg&expire=1661541849&fexp=24001373%2C24007246&fvip=2&gir=yes&id=o-AOqXE9lVS424yszv6LN5V_gaevdHxenJl-tYNy3Drs6g&initcwndbps=1428750&ip=2003%3Ade%3Aaf05%3A2500%3A5dad%3A319b%3Aca30%3Ae212&itag=247&keepalive=yes&lmt=1647458657499381&lsig=AG3C_xAwRQIhAMioKyc-dqs-6uvAwLViCcCTXKHn9sIbo0cbSSBXGG4kAiBQNsRBAvQrbWdOjZIsQXYrfPEb1KDpE_AlSEGQZXB9uA%3D%3D&lsparams=mh%2Cmm%2Cmn%2Cms%2Cmv%2Cmvi%2Cpl%2Cinitcwndbps&mh=NH&mime=video%2Fwebm&mm=31%2C29&mn=sn-h0jelne7%2Csn-h0jeenl6&ms=au%2Crdu&mt=1661519833&mv=m&mvi=5&n=Zd7nrOM1B2C6PA&ns=426LxLap5MonJD_YWdS4lSYH&pl=37&rbqsm=fr&requiressl=yes&sig=AOq0QJ8wRgIhAMUsmcl1zgbr3YQranPWNV1kcxT5IdEoLL7FTFEDdHHPAiEAhQnrfYMU0A9xZ69MfBujWA4pXtCOQCg2Jn6ve9J_vBQ%3D&source=youtube&sparams=expire%2Cei%2Cip%2Cid%2Caitags%2Csource%2Crequiressl%2Cspc%2Cvprv%2Cmime%2Cns%2Cgir%2Cclen%2Cdur%2Clmt&spc=lT-KhuPtxVzL5-QbZ7S9zNeOHsWTdms&txp=4532434&vprv=1"))]
    #[case::res_fallback(Filter::default().video_max_res(100).to_owned(), Some("https://rr5---sn-h0jelne7.googlevideo.com/videoplayback?aitags=133%2C134%2C135%2C136%2C160%2C242%2C243%2C244%2C247%2C278%2C298%2C299%2C302%2C303%2C308%2C315%2C330%2C331%2C332%2C333%2C334%2C335%2C336%2C337%2C394%2C395%2C396%2C397%2C398%2C399%2C400%2C401%2C694%2C695%2C696%2C697%2C698%2C699%2C700%2C701&c=WEB&clen=2763284&dur=313.780&ei=eckIY72IKcGZ8gOMt6CwDg&expire=1661541849&fexp=24001373%2C24007246&fvip=2&gir=yes&id=o-AOqXE9lVS424yszv6LN5V_gaevdHxenJl-tYNy3Drs6g&initcwndbps=1428750&ip=2003%3Ade%3Aaf05%3A2500%3A5dad%3A319b%3Aca30%3Ae212&itag=160&keepalive=yes&lmt=1647456833049253&lsig=AG3C_xAwRQIhAMioKyc-dqs-6uvAwLViCcCTXKHn9sIbo0cbSSBXGG4kAiBQNsRBAvQrbWdOjZIsQXYrfPEb1KDpE_AlSEGQZXB9uA%3D%3D&lsparams=mh%2Cmm%2Cmn%2Cms%2Cmv%2Cmvi%2Cpl%2Cinitcwndbps&mh=NH&mime=video%2Fmp4&mm=31%2C29&mn=sn-h0jelne7%2Csn-h0jeenl6&ms=au%2Crdu&mt=1661519833&mv=m&mvi=5&n=Zd7nrOM1B2C6PA&ns=426LxLap5MonJD_YWdS4lSYH&pl=37&rbqsm=fr&requiressl=yes&sig=AOq0QJ8wRQIgLPNxzLxppSSpnDEHxVblrQ38890NMbGnLXlmxljprfQCIQDn4Ir_sjYh7S3ms-Rynm-K0nJpHpQGYsz1nv4TiqeELQ%3D%3D&source=youtube&sparams=expire%2Cei%2Cip%2Cid%2Caitags%2Csource%2Crequiressl%2Cspc%2Cvprv%2Cmime%2Cns%2Cgir%2Cclen%2Cdur%2Clmt&spc=lT-KhuPtxVzL5-QbZ7S9zNeOHsWTdms&txp=4532434&vprv=1"))]
    #[case::webm_format(Filter::default().video_formats(hash_set!(VideoFormat::Webm)).to_owned(), Some("https://rr5---sn-h0jelne7.googlevideo.com/videoplayback?aitags=133%2C134%2C135%2C136%2C160%2C242%2C243%2C244%2C247%2C278%2C298%2C299%2C302%2C303%2C308%2C315%2C330%2C331%2C332%2C333%2C334%2C335%2C336%2C337%2C394%2C395%2C396%2C397%2C398%2C399%2C400%2C401%2C694%2C695%2C696%2C697%2C698%2C699%2C700%2C701&c=WEB&clen=998696577&dur=313.780&ei=eckIY72IKcGZ8gOMt6CwDg&expire=1661541849&fexp=24001373%2C24007246&fvip=2&gir=yes&id=o-AOqXE9lVS424yszv6LN5V_gaevdHxenJl-tYNy3Drs6g&initcwndbps=1428750&ip=2003%3Ade%3Aaf05%3A2500%3A5dad%3A319b%3Aca30%3Ae212&itag=315&keepalive=yes&lmt=1647476955807851&lsig=AG3C_xAwRQIhAMioKyc-dqs-6uvAwLViCcCTXKHn9sIbo0cbSSBXGG4kAiBQNsRBAvQrbWdOjZIsQXYrfPEb1KDpE_AlSEGQZXB9uA%3D%3D&lsparams=mh%2Cmm%2Cmn%2Cms%2Cmv%2Cmvi%2Cpl%2Cinitcwndbps&mh=NH&mime=video%2Fwebm&mm=31%2C29&mn=sn-h0jelne7%2Csn-h0jeenl6&ms=au%2Crdu&mt=1661519833&mv=m&mvi=5&n=Zd7nrOM1B2C6PA&ns=426LxLap5MonJD_YWdS4lSYH&pl=37&rbqsm=fr&requiressl=yes&sig=AOq0QJ8wRAIfP4IVSo-00_kq_JIkuh032hcLoJzNEhYjvwgLiDpEzQIhALPVrvDBjRwiFddXiAyADmRtYygte4HvlJ3XOrkOf_TR&source=youtube&sparams=expire%2Cei%2Cip%2Cid%2Caitags%2Csource%2Crequiressl%2Cspc%2Cvprv%2Cmime%2Cns%2Cgir%2Cclen%2Cdur%2Clmt&spc=lT-KhuPtxVzL5-QbZ7S9zNeOHsWTdms&txp=4532434&vprv=1"))]
    #[case::vp9_codec(Filter::default().video_codecs(hash_set!(VideoCodec::Vp9)).to_owned(), Some("https://rr5---sn-h0jelne7.googlevideo.com/videoplayback?aitags=133%2C134%2C135%2C136%2C160%2C242%2C243%2C244%2C247%2C278%2C298%2C299%2C302%2C303%2C308%2C315%2C330%2C331%2C332%2C333%2C334%2C335%2C336%2C337%2C394%2C395%2C396%2C397%2C398%2C399%2C400%2C401%2C694%2C695%2C696%2C697%2C698%2C699%2C700%2C701&c=WEB&clen=998696577&dur=313.780&ei=eckIY72IKcGZ8gOMt6CwDg&expire=1661541849&fexp=24001373%2C24007246&fvip=2&gir=yes&id=o-AOqXE9lVS424yszv6LN5V_gaevdHxenJl-tYNy3Drs6g&initcwndbps=1428750&ip=2003%3Ade%3Aaf05%3A2500%3A5dad%3A319b%3Aca30%3Ae212&itag=315&keepalive=yes&lmt=1647476955807851&lsig=AG3C_xAwRQIhAMioKyc-dqs-6uvAwLViCcCTXKHn9sIbo0cbSSBXGG4kAiBQNsRBAvQrbWdOjZIsQXYrfPEb1KDpE_AlSEGQZXB9uA%3D%3D&lsparams=mh%2Cmm%2Cmn%2Cms%2Cmv%2Cmvi%2Cpl%2Cinitcwndbps&mh=NH&mime=video%2Fwebm&mm=31%2C29&mn=sn-h0jelne7%2Csn-h0jeenl6&ms=au%2Crdu&mt=1661519833&mv=m&mvi=5&n=Zd7nrOM1B2C6PA&ns=426LxLap5MonJD_YWdS4lSYH&pl=37&rbqsm=fr&requiressl=yes&sig=AOq0QJ8wRAIfP4IVSo-00_kq_JIkuh032hcLoJzNEhYjvwgLiDpEzQIhALPVrvDBjRwiFddXiAyADmRtYygte4HvlJ3XOrkOf_TR&source=youtube&sparams=expire%2Cei%2Cip%2Cid%2Caitags%2Csource%2Crequiressl%2Cspc%2Cvprv%2Cmime%2Cns%2Cgir%2Cclen%2Cdur%2Clmt&spc=lT-KhuPtxVzL5-QbZ7S9zNeOHsWTdms&txp=4532434&vprv=1"))]
    #[case::noformat(Filter::default().video_formats(hash_set!()).to_owned(), None)]
    #[case::nocodec(Filter::default().video_codecs(hash_set!()).to_owned(), None)]
    fn t_select_video_only_stream(#[case] filter: Filter, #[case] expect_url: Option<&str>) {
        let selection = PLAYER_HDR.select_video_only_stream(&filter);

        match expect_url {
            Some(expect_url) => assert_eq!(selection.unwrap().url, expect_url),
            None => assert_eq!(selection, None),
        }
    }

    #[rstest]
    #[case::default(
        Filter::default(),
        Some("https://rr5---sn-h0jelne7.googlevideo.com/videoplayback?aitags=133%2C134%2C135%2C136%2C160%2C242%2C243%2C244%2C247%2C278%2C298%2C299%2C302%2C303%2C308%2C315%2C330%2C331%2C332%2C333%2C334%2C335%2C336%2C337%2C394%2C395%2C396%2C397%2C398%2C399%2C400%2C401%2C694%2C695%2C696%2C697%2C698%2C699%2C700%2C701&c=WEB&clen=998696577&dur=313.780&ei=eckIY72IKcGZ8gOMt6CwDg&expire=1661541849&fexp=24001373%2C24007246&fvip=2&gir=yes&id=o-AOqXE9lVS424yszv6LN5V_gaevdHxenJl-tYNy3Drs6g&initcwndbps=1428750&ip=2003%3Ade%3Aaf05%3A2500%3A5dad%3A319b%3Aca30%3Ae212&itag=315&keepalive=yes&lmt=1647476955807851&lsig=AG3C_xAwRQIhAMioKyc-dqs-6uvAwLViCcCTXKHn9sIbo0cbSSBXGG4kAiBQNsRBAvQrbWdOjZIsQXYrfPEb1KDpE_AlSEGQZXB9uA%3D%3D&lsparams=mh%2Cmm%2Cmn%2Cms%2Cmv%2Cmvi%2Cpl%2Cinitcwndbps&mh=NH&mime=video%2Fwebm&mm=31%2C29&mn=sn-h0jelne7%2Csn-h0jeenl6&ms=au%2Crdu&mt=1661519833&mv=m&mvi=5&n=Zd7nrOM1B2C6PA&ns=426LxLap5MonJD_YWdS4lSYH&pl=37&rbqsm=fr&requiressl=yes&sig=AOq0QJ8wRAIfP4IVSo-00_kq_JIkuh032hcLoJzNEhYjvwgLiDpEzQIhALPVrvDBjRwiFddXiAyADmRtYygte4HvlJ3XOrkOf_TR&source=youtube&sparams=expire%2Cei%2Cip%2Cid%2Caitags%2Csource%2Crequiressl%2Cspc%2Cvprv%2Cmime%2Cns%2Cgir%2Cclen%2Cdur%2Clmt&spc=lT-KhuPtxVzL5-QbZ7S9zNeOHsWTdms&txp=4532434&vprv=1"),
        Some("https://rr5---sn-h0jelne7.googlevideo.com/videoplayback?c=WEB&clen=5199784&dur=313.801&ei=eckIY72IKcGZ8gOMt6CwDg&expire=1661541849&fexp=24001373%2C24007246&fvip=2&gir=yes&id=o-AOqXE9lVS424yszv6LN5V_gaevdHxenJl-tYNy3Drs6g&initcwndbps=1428750&ip=2003%3Ade%3Aaf05%3A2500%3A5dad%3A319b%3Aca30%3Ae212&itag=251&keepalive=yes&lmt=1647453650291076&lsig=AG3C_xAwRQIhAMioKyc-dqs-6uvAwLViCcCTXKHn9sIbo0cbSSBXGG4kAiBQNsRBAvQrbWdOjZIsQXYrfPEb1KDpE_AlSEGQZXB9uA%3D%3D&lsparams=mh%2Cmm%2Cmn%2Cms%2Cmv%2Cmvi%2Cpl%2Cinitcwndbps&mh=NH&mime=audio%2Fwebm&mm=31%2C29&mn=sn-h0jelne7%2Csn-h0jeenl6&ms=au%2Crdu&mt=1661519833&mv=m&mvi=5&n=Zd7nrOM1B2C6PA&ns=426LxLap5MonJD_YWdS4lSYH&pl=37&rbqsm=fr&requiressl=yes&sig=AOq0QJ8wRQIhALtI3j8ZChpNb0LcyDZ3yosbWnSpqaO0-jKAe_UM_RQyAiAMwrpdeNbJEnQn3q1eveaAcRcNIwy5iJ4fIjeBW_MUfg%3D%3D&source=youtube&sparams=expire%2Cei%2Cip%2Cid%2Citag%2Csource%2Crequiressl%2Cspc%2Cvprv%2Cmime%2Cns%2Cgir%2Cclen%2Cdur%2Clmt&spc=lT-KhuPtxVzL5-QbZ7S9zNeOHsWTdms&txp=4532434&vprv=1")
    )]
    #[case::webm(
        Filter::default().video_formats(hash_set!(VideoFormat::Webm)).to_owned(),
        Some("https://rr5---sn-h0jelne7.googlevideo.com/videoplayback?aitags=133%2C134%2C135%2C136%2C160%2C242%2C243%2C244%2C247%2C278%2C298%2C299%2C302%2C303%2C308%2C315%2C330%2C331%2C332%2C333%2C334%2C335%2C336%2C337%2C394%2C395%2C396%2C397%2C398%2C399%2C400%2C401%2C694%2C695%2C696%2C697%2C698%2C699%2C700%2C701&c=WEB&clen=998696577&dur=313.780&ei=eckIY72IKcGZ8gOMt6CwDg&expire=1661541849&fexp=24001373%2C24007246&fvip=2&gir=yes&id=o-AOqXE9lVS424yszv6LN5V_gaevdHxenJl-tYNy3Drs6g&initcwndbps=1428750&ip=2003%3Ade%3Aaf05%3A2500%3A5dad%3A319b%3Aca30%3Ae212&itag=315&keepalive=yes&lmt=1647476955807851&lsig=AG3C_xAwRQIhAMioKyc-dqs-6uvAwLViCcCTXKHn9sIbo0cbSSBXGG4kAiBQNsRBAvQrbWdOjZIsQXYrfPEb1KDpE_AlSEGQZXB9uA%3D%3D&lsparams=mh%2Cmm%2Cmn%2Cms%2Cmv%2Cmvi%2Cpl%2Cinitcwndbps&mh=NH&mime=video%2Fwebm&mm=31%2C29&mn=sn-h0jelne7%2Csn-h0jeenl6&ms=au%2Crdu&mt=1661519833&mv=m&mvi=5&n=Zd7nrOM1B2C6PA&ns=426LxLap5MonJD_YWdS4lSYH&pl=37&rbqsm=fr&requiressl=yes&sig=AOq0QJ8wRAIfP4IVSo-00_kq_JIkuh032hcLoJzNEhYjvwgLiDpEzQIhALPVrvDBjRwiFddXiAyADmRtYygte4HvlJ3XOrkOf_TR&source=youtube&sparams=expire%2Cei%2Cip%2Cid%2Caitags%2Csource%2Crequiressl%2Cspc%2Cvprv%2Cmime%2Cns%2Cgir%2Cclen%2Cdur%2Clmt&spc=lT-KhuPtxVzL5-QbZ7S9zNeOHsWTdms&txp=4532434&vprv=1"),
        Some("https://rr5---sn-h0jelne7.googlevideo.com/videoplayback?c=WEB&clen=5199784&dur=313.801&ei=eckIY72IKcGZ8gOMt6CwDg&expire=1661541849&fexp=24001373%2C24007246&fvip=2&gir=yes&id=o-AOqXE9lVS424yszv6LN5V_gaevdHxenJl-tYNy3Drs6g&initcwndbps=1428750&ip=2003%3Ade%3Aaf05%3A2500%3A5dad%3A319b%3Aca30%3Ae212&itag=251&keepalive=yes&lmt=1647453650291076&lsig=AG3C_xAwRQIhAMioKyc-dqs-6uvAwLViCcCTXKHn9sIbo0cbSSBXGG4kAiBQNsRBAvQrbWdOjZIsQXYrfPEb1KDpE_AlSEGQZXB9uA%3D%3D&lsparams=mh%2Cmm%2Cmn%2Cms%2Cmv%2Cmvi%2Cpl%2Cinitcwndbps&mh=NH&mime=audio%2Fwebm&mm=31%2C29&mn=sn-h0jelne7%2Csn-h0jeenl6&ms=au%2Crdu&mt=1661519833&mv=m&mvi=5&n=Zd7nrOM1B2C6PA&ns=426LxLap5MonJD_YWdS4lSYH&pl=37&rbqsm=fr&requiressl=yes&sig=AOq0QJ8wRQIhALtI3j8ZChpNb0LcyDZ3yosbWnSpqaO0-jKAe_UM_RQyAiAMwrpdeNbJEnQn3q1eveaAcRcNIwy5iJ4fIjeBW_MUfg%3D%3D&source=youtube&sparams=expire%2Cei%2Cip%2Cid%2Citag%2Csource%2Crequiressl%2Cspc%2Cvprv%2Cmime%2Cns%2Cgir%2Cclen%2Cdur%2Clmt&spc=lT-KhuPtxVzL5-QbZ7S9zNeOHsWTdms&txp=4532434&vprv=1")
    )]
    #[case::noaudio(
        Filter::default().audio_formats(hash_set!()).to_owned(),
        Some("https://rr5---sn-h0jelne7.googlevideo.com/videoplayback?c=WEB&clen=23544588&dur=313.834&ei=eckIY72IKcGZ8gOMt6CwDg&expire=1661541849&fexp=24001373%2C24007246&fvip=2&gir=yes&id=o-AOqXE9lVS424yszv6LN5V_gaevdHxenJl-tYNy3Drs6g&initcwndbps=1428750&ip=2003%3Ade%3Aaf05%3A2500%3A5dad%3A319b%3Aca30%3Ae212&itag=18&lmt=1647456546485912&lsig=AG3C_xAwRQIhAMioKyc-dqs-6uvAwLViCcCTXKHn9sIbo0cbSSBXGG4kAiBQNsRBAvQrbWdOjZIsQXYrfPEb1KDpE_AlSEGQZXB9uA%3D%3D&lsparams=mh%2Cmm%2Cmn%2Cms%2Cmv%2Cmvi%2Cpl%2Cinitcwndbps&mh=NH&mime=video%2Fmp4&mm=31%2C29&mn=sn-h0jelne7%2Csn-h0jeenl6&ms=au%2Crdu&mt=1661519833&mv=m&mvi=5&n=HWZNhARNT_nJgg&ns=pLFQxzhiCbZ9F2HJmDLveKoH&pl=37&ratebypass=yes&rbqsm=fr&requiressl=yes&sig=AOq0QJ8wRQIgeCEjusAq6p33rH0NHyTAbPIRaaEkjDE32AXBFzDvR-ICIQD0LI8hQVH8oCMWu6OuADzc1FSQhIqYs5RLkxBmObIdsw%3D%3D&source=youtube&sparams=expire%2Cei%2Cip%2Cid%2Citag%2Csource%2Crequiressl%2Cspc%2Cvprv%2Cmime%2Cns%2Cgir%2Cclen%2Cratebypass%2Cdur%2Clmt&spc=lT-KhuPtxVzL5-QbZ7S9zNeOHsWTdms&txp=4530434&vprv=1"),
        None
    )]
    #[case::novideo(
        Filter::default().no_video().to_owned(),
        None,
        Some("https://rr5---sn-h0jelne7.googlevideo.com/videoplayback?c=WEB&clen=5199784&dur=313.801&ei=eckIY72IKcGZ8gOMt6CwDg&expire=1661541849&fexp=24001373%2C24007246&fvip=2&gir=yes&id=o-AOqXE9lVS424yszv6LN5V_gaevdHxenJl-tYNy3Drs6g&initcwndbps=1428750&ip=2003%3Ade%3Aaf05%3A2500%3A5dad%3A319b%3Aca30%3Ae212&itag=251&keepalive=yes&lmt=1647453650291076&lsig=AG3C_xAwRQIhAMioKyc-dqs-6uvAwLViCcCTXKHn9sIbo0cbSSBXGG4kAiBQNsRBAvQrbWdOjZIsQXYrfPEb1KDpE_AlSEGQZXB9uA%3D%3D&lsparams=mh%2Cmm%2Cmn%2Cms%2Cmv%2Cmvi%2Cpl%2Cinitcwndbps&mh=NH&mime=audio%2Fwebm&mm=31%2C29&mn=sn-h0jelne7%2Csn-h0jeenl6&ms=au%2Crdu&mt=1661519833&mv=m&mvi=5&n=Zd7nrOM1B2C6PA&ns=426LxLap5MonJD_YWdS4lSYH&pl=37&rbqsm=fr&requiressl=yes&sig=AOq0QJ8wRQIhALtI3j8ZChpNb0LcyDZ3yosbWnSpqaO0-jKAe_UM_RQyAiAMwrpdeNbJEnQn3q1eveaAcRcNIwy5iJ4fIjeBW_MUfg%3D%3D&source=youtube&sparams=expire%2Cei%2Cip%2Cid%2Citag%2Csource%2Crequiressl%2Cspc%2Cvprv%2Cmime%2Cns%2Cgir%2Cclen%2Cdur%2Clmt&spc=lT-KhuPtxVzL5-QbZ7S9zNeOHsWTdms&txp=4532434&vprv=1")
    )]
    #[case::noformat(Filter::default().audio_formats(hash_set!()).video_formats(hash_set!()).to_owned(), None, None)]
    fn t_select_video_audio_stream(
        #[case] filter: Filter,
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
