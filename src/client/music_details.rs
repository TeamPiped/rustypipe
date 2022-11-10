use serde::Serialize;

use crate::{
    error::{Error, ExtractionError},
    model::MusicDetails,
    param::Language,
    serializer::MapResult,
};

use super::{response, ClientType, MapResponse, RustyPipeQuery, YTContext};

#[derive(Debug, Serialize)]
struct QMusicDetails<'a> {
    context: YTContext<'a>,
    // YouTube video ID
    video_id: &'a str,
    enable_persistent_playlist_panel: bool,
    is_audio_only: bool,
    tuner_setting_value: &'a str,
}

#[derive(Debug, Serialize)]
struct QRadio<'a> {
    context: YTContext<'a>,
    playlist_id: &'a str,
    params: &'a str,
    enable_persistent_playlist_panel: bool,
    is_audio_only: bool,
    tuner_setting_value: &'a str,
}

impl RustyPipeQuery {
    pub async fn music_radio(&self, radio_id: &str) -> Result<MusicDetails, Error> {
        let context = self.get_context(ClientType::DesktopMusic, true, None).await;
        let request_body = QRadio {
            context,
            playlist_id: radio_id,
            params: "wAEB8gECeAE%3D",
            enable_persistent_playlist_panel: true,
            is_audio_only: true,
            tuner_setting_value: "AUTOMIX_SETTING_NORMAL",
        };

        self.execute_request::<response::MusicDetails, _, _>(
            ClientType::DesktopMusic,
            "music_radio",
            radio_id,
            "next",
            &request_body,
        )
        .await
    }
}

impl MapResponse<MusicDetails> for response::MusicDetails {
    fn map_response(
        self,
        id: &str,
        lang: Language,
        _deobf: Option<&crate::deobfuscate::Deobfuscator>,
    ) -> Result<MapResult<MusicDetails>, ExtractionError> {
        dbg!(&self);

        Ok(MapResult {
            c: MusicDetails {},
            warnings: Vec::new(),
        })
    }
}
