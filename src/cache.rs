//! Persistent cache storage

use std::{
    fs,
    path::{Path, PathBuf},
};

use log::error;

pub(crate) const DEFAULT_CACHE_FILE: &str = "rustypipe_cache.json";

/// RustyPipe has to cache some information fetched from YouTube: specifically
/// the client versions and the JavaScript code used to deobfuscate the stream URLs.
///
/// This trait is used to abstract the cache storage behavior so you can store
/// cache data in your preferred way (File, SQL, Redis, etc).
///
/// The cache is read when building the [`crate::client::RustyPipe`] client and updated
/// whenever additional data is fetched.
pub trait CacheStorage: Sync + Send {
    /// Write the given string to the cache
    fn write(&self, data: &str);
    /// Read the string from the cache
    ///
    /// Returns [`None`] when the cache is empty or the reading failed.
    fn read(&self) -> Option<String>;
}

/// [`CacheStorage`] implementation that writes the cache to a JSON file
/// at the given location.
pub struct FileStorage {
    path: PathBuf,
}

impl FileStorage {
    /// Create a new JSON-file based cache storage
    pub fn new<P: AsRef<Path>>(path: P) -> Self {
        Self {
            path: path.as_ref().to_path_buf(),
        }
    }
}

impl Default for FileStorage {
    fn default() -> Self {
        Self {
            path: Path::new(DEFAULT_CACHE_FILE).into(),
        }
    }
}

impl CacheStorage for FileStorage {
    fn write(&self, data: &str) {
        fs::write(&self.path, data).unwrap_or_else(|e| {
            error!(
                "Could not write cache to file `{}`. Error: {}",
                self.path.to_string_lossy(),
                e
            );
        });
    }

    fn read(&self) -> Option<String> {
        if !self.path.exists() {
            return None;
        }

        match fs::read_to_string(&self.path) {
            Ok(data) => Some(data),
            Err(e) => {
                error!(
                    "Could not load cache from file `{}`. Error: {}",
                    self.path.to_string_lossy(),
                    e
                );
                None
            }
        }
    }
}
