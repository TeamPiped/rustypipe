//! # Persistent cache storage
//!
//! RustyPipe caches some information fetched from YouTube: specifically
//! the client versions and the JavaScript code used to deobfuscate the stream URLs.
//!
//! Without a persistent cache storage, this information would have to be re-fetched
//! with every new instantiation of the client. This would make operation a lot slower,
//! especially with CLI applications. For this reason, persisting the cache between
//! program executions is recommended.
//!
//! Since there are many diferent ways to store this data (Text file, SQL, Redis, etc),
//! RustyPipe allows you to plug in your own cache storage by implementing the
//! [`CacheStorage`] trait.
//!
//! RustyPipe already comes with the [`FileStorage`] implementation which stores
//! the cache as a JSON file.

use std::{
    fs,
    path::{Path, PathBuf},
};

use log::error;

pub(crate) const DEFAULT_CACHE_FILE: &str = "rustypipe_cache.json";

/// Cache storage trait
///
/// RustyPipe has to cache some information fetched from YouTube: specifically
/// the client versions and the JavaScript code used to deobfuscate the stream URLs.
///
/// This trait is used to abstract the cache storage behavior so you can store
/// cache data in your preferred way (File, SQL, Redis, etc).
///
/// The cache is read when building the [`RustyPipe`](crate::client::RustyPipe)
/// client and updated whenever additional data is fetched.
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
