use std::{
    fs::File,
    future::Future,
    io::BufReader,
    path::{Path, PathBuf},
    sync::Arc,
};

use anyhow::Result;
use chrono::{DateTime, Duration, Utc};
use log::{error, info};
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;

#[derive(Default, Debug, Clone)]
pub struct Cache {
    file: Option<PathBuf>,
    data: Arc<Mutex<CacheData>>,
}

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
struct CacheData {
    desktop_client: Option<CacheEntry<ClientData>>,
    music_client: Option<CacheEntry<ClientData>>,
    deobf: Option<CacheEntry<DeobfData>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CacheEntry<T> {
    last_update: DateTime<Utc>,
    data: T,
}

impl<T> From<T> for CacheEntry<T> {
    fn from(f: T) -> Self {
        Self {
            last_update: Utc::now(),
            data: f,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ClientData {
    pub version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DeobfData {
    pub js_url: String,
    pub sig_fn: String,
    pub nsig_fn: String,
    pub sts: String,
}

impl Cache {
    pub async fn get_desktop_client_data<F>(&self, updater: F) -> Result<ClientData>
    where
        F: Future<Output = Result<ClientData>> + Send + 'static,
    {
        let mut cache = self.data.lock().await;

        if cache.desktop_client.is_none()
            || cache.desktop_client.as_ref().unwrap().last_update < Utc::now() - Duration::hours(24)
        {
            let cdata = updater.await?;
            cache.desktop_client = Some(CacheEntry::from(cdata.clone()));
            self.save(&cache);
            Ok(cdata)
        } else {
            Ok(cache.desktop_client.as_ref().unwrap().data.clone())
        }
    }

    pub async fn get_music_client_data<F>(&self, updater: F) -> Result<ClientData>
    where
        F: Future<Output = Result<ClientData>> + Send + 'static,
    {
        let mut cache = self.data.lock().await;

        if cache.music_client.is_none()
            || cache.music_client.as_ref().unwrap().last_update < Utc::now() - Duration::hours(24)
        {
            let cdata = updater.await?;
            cache.music_client = Some(CacheEntry::from(cdata.clone()));
            self.save(&cache);
            Ok(cdata)
        } else {
            Ok(cache.music_client.as_ref().unwrap().data.clone())
        }
    }

    pub async fn get_deobf_data<F>(&self, updater: F) -> Result<DeobfData>
    where
        F: Future<Output = Result<DeobfData>> + Send + 'static,
    {
        let mut cache = self.data.lock().await;
        if cache.deobf.is_none()
            || cache.deobf.as_ref().unwrap().last_update < Utc::now() - Duration::hours(24)
        {
            let deobf_data = updater.await?;
            cache.deobf = Some(CacheEntry::from(deobf_data.clone()));
            self.save(&cache);
            Ok(deobf_data)
        } else {
            Ok(cache.deobf.as_ref().unwrap().data.clone())
        }
    }

    pub async fn to_json(&self) -> Result<String> {
        let cache = self.data.lock().await;
        Ok(serde_json::to_string(&cache.clone())?)
    }

    pub async fn to_json_file<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let cache = self.data.lock().await;
        Ok(serde_json::to_writer(&File::create(path)?, &cache.clone())?)
    }

    pub fn from_json(json: &str) -> Self {
        let data: CacheData = match serde_json::from_str(json) {
            Ok(cd) => cd,
            Err(e) => {
                error!(
                    "Could not load cache from json, falling back to default. Error: {}",
                    e
                );
                CacheData::default()
            }
        };
        Cache {
            data: Arc::new(Mutex::new(data)),
            file: None,
        }
    }

    pub fn from_json_file<P: AsRef<Path>>(path: P) -> Self {
        let file = match File::open(path.as_ref()) {
            Ok(file) => file,
            Err(e) => {
                if e.kind() == std::io::ErrorKind::NotFound {
                    info!(
                        "Cache json file at {} not found, will be created",
                        path.as_ref().to_string_lossy()
                    )
                } else {
                    error!(
                        "Could not open cache json file, falling back to default. Error: {}",
                        e
                    );
                }
                return Cache {
                    file: Some(path.as_ref().to_path_buf()),
                    ..Default::default()
                };
            }
        };
        let data: CacheData = match serde_json::from_reader(BufReader::new(file)) {
            Ok(data) => data,
            Err(e) => {
                error!(
                    "Could not load cache from json, falling back to default. Error: {}",
                    e
                );
                return Cache {
                    file: Some(path.as_ref().to_path_buf()),
                    ..Default::default()
                };
            }
        };
        Cache {
            data: Arc::new(Mutex::new(data)),
            file: Some(path.as_ref().to_path_buf()),
        }
    }

    fn save(&self, cache: &CacheData) {
        match self.file.as_ref() {
            Some(file) => match File::create(file) {
                Ok(file) => match serde_json::to_writer(file, cache) {
                    Ok(_) => {}
                    Err(e) => error!("Could not write cache to json. Error: {}", e),
                },
                Err(e) => error!("Could not open cache json file. Error: {}", e),
            },
            None => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use temp_testdir::TempDir;

    use super::*;

    #[tokio::test]
    async fn test() {
        let cache = Cache::default();

        let desktop_c = cache
            .get_desktop_client_data(async {
                Ok(ClientData {
                    version: "1.2.3".to_owned(),
                })
            })
            .await
            .unwrap();

        assert_eq!(
            desktop_c,
            ClientData {
                version: "1.2.3".to_owned()
            }
        );

        let music_c = cache
            .get_music_client_data(async {
                Ok(ClientData {
                    version: "4.5.6".to_owned(),
                })
            })
            .await
            .unwrap();

        assert_eq!(
            music_c,
            ClientData {
                version: "4.5.6".to_owned()
            }
        );

        let deobf_data = cache
            .get_deobf_data(async {
                Ok(DeobfData {
                    js_url:
                        "https://www.youtube.com/s/player/011af516/player_ias.vflset/en_US/base.js"
                            .to_owned(),
                    sig_fn: "t_sig_fn".to_owned(),
                    nsig_fn: "t_nsig_fn".to_owned(),
                    sts: "t_sts".to_owned(),
                })
            })
            .await
            .unwrap();

        assert_eq!(
            deobf_data,
            DeobfData {
                js_url: "https://www.youtube.com/s/player/011af516/player_ias.vflset/en_US/base.js"
                    .to_owned(),
                sig_fn: "t_sig_fn".to_owned(),
                nsig_fn: "t_nsig_fn".to_owned(),
                sts: "t_sts".to_owned(),
            }
        );

        // Create a new cache from the first one's json
        // and check if it returns the same cached data
        let json = cache.to_json().await.unwrap();
        let new_cache = Cache::from_json(&json);

        assert_eq!(
            new_cache
                .get_desktop_client_data(async {
                    Ok(ClientData {
                        version: "".to_owned(),
                    })
                })
                .await
                .unwrap(),
            desktop_c
        );

        assert_eq!(
            new_cache
                .get_music_client_data(async {
                    Ok(ClientData {
                        version: "".to_owned(),
                    })
                })
                .await
                .unwrap(),
            music_c
        );

        assert_eq!(
            new_cache
                .get_deobf_data(async {
                    Ok(DeobfData {
                        js_url: "".to_owned(),
                        nsig_fn: "".to_owned(),
                        sig_fn: "".to_owned(),
                        sts: "".to_owned(),
                    })
                })
                .await
                .unwrap(),
            deobf_data
        );
    }

    #[tokio::test]
    async fn test_file() {
        let temp = TempDir::default();
        let mut file_path = PathBuf::from(temp.as_ref());
        file_path.push("cache.json");

        let cache = Cache::from_json_file(file_path.clone());

        let cdata = cache
            .get_desktop_client_data(async {
                Ok(ClientData {
                    version: "1.2.3".to_owned(),
                })
            })
            .await
            .unwrap();

        let deobf_data = cache
            .get_deobf_data(async {
                Ok(DeobfData {
                    js_url:
                        "https://www.youtube.com/s/player/011af516/player_ias.vflset/en_US/base.js"
                            .to_owned(),
                    sig_fn: "t_sig_fn".to_owned(),
                    nsig_fn: "t_nsig_fn".to_owned(),
                    sts: "t_sts".to_owned(),
                })
            })
            .await
            .unwrap();

        assert!(file_path.exists());
        let new_cache = Cache::from_json_file(file_path.clone());

        assert_eq!(
            new_cache
                .get_desktop_client_data(async {
                    Ok(ClientData {
                        version: "".to_owned(),
                    })
                })
                .await
                .unwrap(),
            cdata
        );

        assert_eq!(
            new_cache
                .get_deobf_data(async {
                    Ok(DeobfData {
                        js_url: "".to_owned(),
                        nsig_fn: "".to_owned(),
                        sig_fn: "".to_owned(),
                        sts: "".to_owned(),
                    })
                })
                .await
                .unwrap(),
            deobf_data
        );
    }
}
