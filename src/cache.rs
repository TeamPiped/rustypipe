use std::{future::Future, sync::Arc};

use anyhow::Result;
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;

#[derive(Default, Debug)]
pub struct Cache {
    data: Arc<Mutex<CacheData>>,
}

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub struct CacheData {
    desktop_client: Option<CacheEntry<DesktopClientData>>,
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
pub struct DesktopClientData {
    pub client_version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DeobfData {
    js_url: String,
    sig_fn: String,
    nsig_fn: String,
    sts: String,
}

impl Cache {
    pub async fn get_desktop_client_data<F>(&self, updater: F) -> Result<DesktopClientData>
    where
        F: Future<Output = Result<DesktopClientData>> + Send + 'static,
    {
        let mut cache = self.data.lock().await;

        if cache.desktop_client.is_none()
            || cache.desktop_client.as_ref().unwrap().last_update < Utc::now() - Duration::days(1)
        {
            let cdata = updater.await?;
            cache.desktop_client = Some(CacheEntry::from(cdata.clone()));
            Ok(cdata)
        } else {
            Ok(cache.desktop_client.as_ref().unwrap().data.clone())
        }
    }

    pub async fn get_deobf_data<F>(&self, updater: F) -> Result<DeobfData>
    where
        F: Future<Output = Result<DeobfData>> + Send + 'static,
    {
        let mut cache = self.data.lock().await;
        if cache.deobf.is_none()
            || cache.deobf.as_ref().unwrap().last_update < Utc::now() - Duration::days(1)
        {
            let deobf_data = updater.await?;
            cache.deobf = Some(CacheEntry::from(deobf_data.clone()));
            Ok(deobf_data)
        } else {
            Ok(cache.deobf.as_ref().unwrap().data.clone())
        }
    }

    pub async fn to_json(&self) -> Result<String, serde_json::Error> {
        let cache = self.data.lock().await;
        serde_json::to_string(&cache.clone())
    }

    pub async fn from_json(&self, json: &str) -> Result<(), serde_json::Error> {
        let cd = serde_json::from_str::<CacheData>(json)?;

        let mut cache = self.data.lock().await;
        *cache = cd;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test() {
        let cache = Cache::default();

        let cdata = cache
            .get_desktop_client_data(async {
                Ok(DesktopClientData {
                    client_version: "1.2.3".to_owned(),
                })
            })
            .await
            .unwrap();

        assert_eq!(
            cdata,
            DesktopClientData {
                client_version: "1.2.3".to_owned()
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

        let json = cache.to_json().await.unwrap();

        let new_cache = Cache::default();
        new_cache.from_json(&json).await.unwrap();

        assert_eq!(
            new_cache
                .get_desktop_client_data(async {
                    Ok(DesktopClientData {
                        client_version: "".to_owned(),
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
