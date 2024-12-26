use serde::Deserialize;

use super::music_playlist::Contents;

#[derive(Debug, Deserialize)]
pub(crate) struct MusicHistory {
    pub contents: Contents,
}
