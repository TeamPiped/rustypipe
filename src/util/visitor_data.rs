use std::sync::{atomic::AtomicU32, Arc, RwLock};

use once_cell::sync::Lazy;
use rand::Rng;
use regex::Regex;
use reqwest::{header, Client};

use crate::{
    client::YOUTUBE_MUSIC_HOME_URL,
    error::{Error, ExtractionError},
    util,
};

/// To increase privacy and possibly circumvent rate limits, RustyPipe uses multiple
/// visitor data IDs. These are held in this cache object.
///
/// On instantiation, the cache is empty, so for the first requests new visitor data IDs
/// have to be requested. For subsequent requests a random ID from the cache is picked.
/// After req_limit requests, a new token is requested asynchronously and added to the cache
/// to prevent the IDs from being overused.
///
/// The cache holds a maximum of 100 visitor data IDs. If more are added, the oldest ones
/// are evicted.
#[derive(Clone)]
pub struct VisitorDataCache {
    inner: Arc<VisitorDataCacheRef>,
}

struct VisitorDataCacheRef {
    req_counter: AtomicU32,
    visitor_data: RwLock<Vec<String>>,
    http: Client,
}

static VISITOR_DATA_REGEX: Lazy<Regex> =
    Lazy::new(|| Regex::new(r#""visitorData":"([\w\d_\-%]+?)""#).unwrap());
/// Number of requests after which a new token is requested
const REQ_LIMIT: u32 = 10;
/// Maximum size of the cache (-1)
const MAX_SIZE: usize = 99;

impl VisitorDataCache {
    pub fn new(http: Client) -> Self {
        Self {
            inner: VisitorDataCacheRef {
                req_counter: Default::default(),
                visitor_data: Default::default(),
                http,
            }
            .into(),
        }
    }

    async fn get_visitor_data(&self) -> Result<String, Error> {
        tracing::debug!("getting YT visitor data");
        let resp = self
            .inner
            .http
            .get(YOUTUBE_MUSIC_HOME_URL)
            .header(header::ORIGIN, YOUTUBE_MUSIC_HOME_URL)
            .header(header::REFERER, YOUTUBE_MUSIC_HOME_URL)
            .send()
            .await?;

        let vdata = resp
            .headers()
            .get_all(header::SET_COOKIE)
            .iter()
            .find_map(|c| {
                if let Ok(cookie) = c.to_str() {
                    if let Some(after) = cookie.strip_prefix("__Secure-YEC=") {
                        return after
                            .split_once(';')
                            .map(|s| s.0.to_owned())
                            .filter(|s| !s.is_empty());
                    }
                }
                None
            });

        match vdata {
            Some(vdata) => Ok(vdata),
            None => {
                if resp.status().is_success() {
                    // Extract visitor data from html
                    let html = resp.text().await?;

                    util::get_cg_from_regex(&VISITOR_DATA_REGEX, &html, 1).ok_or(Error::Extraction(
                        ExtractionError::InvalidData(
                            "Could not find visitor data on html page".into(),
                        ),
                    ))
                } else {
                    Err(Error::Extraction(ExtractionError::InvalidData(
                        format!("Could not get visitor data, status: {}", resp.status()).into(),
                    )))
                }
            }
        }
    }

    pub async fn new_visitor_data(&self) -> Result<String, Error> {
        self.inner
            .req_counter
            .store(0, std::sync::atomic::Ordering::SeqCst);
        let vd = self.get_visitor_data().await.unwrap();
        let mut vds = self.inner.visitor_data.write().unwrap();
        for _ in 0..(vds.len().saturating_sub(MAX_SIZE)) {
            let rem = vds.remove(0);
            tracing::debug!("visitor data {rem} removed from cache");
        }
        vds.push(vd.to_owned());
        tracing::debug!("visitor data {} added to cache ({} ids)", vd, vds.len());
        Ok(vd)
    }

    pub async fn get(&self) -> Result<String, Error> {
        // Request new visitor data in the background every 10 requests
        if self
            .inner
            .req_counter
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst)
            >= REQ_LIMIT
        {
            let nc = self.clone();
            tokio::spawn(async move { nc.new_visitor_data().await });
        }

        {
            let vds = self.inner.visitor_data.read().unwrap();
            if !vds.is_empty() {
                let mut rng = rand::thread_rng();
                let vd = vds[rng.gen_range(0..vds.len())].to_owned();
                tracing::debug!("visitor data {vd} picked from cache");
                return Ok(vd);
            }
        }
        // Fetch new visitor data if the cache is empty
        self.new_visitor_data().await
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use crate::client::DEFAULT_UA;

    use super::*;

    use tracing_test::traced_test;

    #[tokio::test]
    #[traced_test]
    async fn get_visitor_data() {
        let cache =
            VisitorDataCache::new(Client::builder().user_agent(DEFAULT_UA).build().unwrap());
        // Get initial visitor data
        let v1 = cache.get().await.unwrap();

        // Run as many request as necessary to fetch second visitor data
        for _ in 0..=REQ_LIMIT {
            let got = cache.get().await.unwrap();
            assert_eq!(got, v1);
        }

        // Second visitor data does not arrive instantly, request immediately after returns the first data
        let vds_len = cache.inner.visitor_data.read().unwrap().len();
        assert_eq!(vds_len, 1);

        // Wait for the second visitor data to arrive
        tokio::time::sleep(Duration::from_millis(1000)).await;
        let vds_len = cache.inner.visitor_data.read().unwrap().len();
        assert_eq!(vds_len, 2);
    }
}
