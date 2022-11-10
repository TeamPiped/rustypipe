use serde::Deserialize;

/// Response model for YouTube Music track details
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MusicDetails {}
