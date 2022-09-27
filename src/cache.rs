//! Persistent cache storage

use std::{
    fs,
    path::{Path, PathBuf},
};

use log::error;

pub trait CacheStorage {
    fn write(&self, data: &str);
    fn read(&self) -> Option<String>;
}

pub struct FileStorage {
    path: PathBuf,
}

impl FileStorage {
    pub fn new<P: AsRef<Path>>(path: P) -> Self {
        Self {
            path: path.as_ref().to_path_buf(),
        }
    }
}

impl Default for FileStorage {
    fn default() -> Self {
        Self {
            path: Path::new("rustypipe_cache.json").into(),
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
