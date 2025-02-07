use std::{
    collections::HashMap,
    sync::{atomic::AtomicU32, Arc, RwLock},
};

use once_cell::sync::Lazy;
use rand::Rng;
use regex::Regex;
use reqwest::{header, Client};
use time::OffsetDateTime;

use crate::{
    client::{PoToken, CONSENT_COOKIE, YOUTUBE_MUSIC_HOME_URL},
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
/// The cache's maximum size is limited. If more IDs are added, the oldest ones are evicted.
#[derive(Clone)]
pub struct VisitorDataCache {
    inner: Arc<VisitorDataCacheRef>,
}

struct VisitorDataCacheRef {
    req_counter: AtomicU32,
    visitor_data: RwLock<Vec<String>>,
    session_potoken: RwLock<HashMap<String, PoToken>>,
    http: Client,
    /// Number of requests after which a new token is requested
    req_limit: u32,
    /// Maximum size of the cache
    max_size: usize,
}

static VISITOR_DATA_REGEX: Lazy<Regex> =
    Lazy::new(|| Regex::new(r#""visitorData":"([\w\d_\-%]+?)""#).unwrap());

impl VisitorDataCache {
    pub fn new(http: Client, req_limit: u32, max_size: usize) -> Self {
        Self {
            inner: VisitorDataCacheRef {
                req_counter: Default::default(),
                visitor_data: Default::default(),
                session_potoken: Default::default(),
                http,
                req_limit,
                max_size: max_size - 1,
            }
            .into(),
        }
    }

    /// Fetch a new visitor data ID from YouTube
    async fn fetch_visitor_data(&self) -> Result<String, Error> {
        tracing::debug!("getting YT visitor data");
        let resp = self
            .inner
            .http
            .get(YOUTUBE_MUSIC_HOME_URL)
            .header(header::ORIGIN, YOUTUBE_MUSIC_HOME_URL)
            .header(header::REFERER, YOUTUBE_MUSIC_HOME_URL)
            .header(header::COOKIE, CONSENT_COOKIE)
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

    /// Fetch a new visitor data ID and store it in the cache
    pub async fn new_visitor_data(&self) -> Result<String, Error> {
        let vd = self.fetch_visitor_data().await.unwrap();

        self.inner
            .req_counter
            .store(0, std::sync::atomic::Ordering::Relaxed);
        let mut vds = self.inner.visitor_data.write().unwrap();
        for _ in 0..(vds.len().saturating_sub(self.inner.max_size)) {
            let rem = vds.remove(0);
            {
                let mut pots = self.inner.session_potoken.write().unwrap();
                pots.remove(&rem);
            }
            tracing::debug!("visitor data {rem} removed from cache");
        }
        vds.push(vd.to_owned());
        tracing::debug!("visitor data {} added to cache ({} ids)", vd, vds.len());
        Ok(vd)
    }

    /// Get a visitor data ID from the cache
    pub async fn get(&self) -> Result<String, Error> {
        // Request a new visitor data ID in the background after a set number of requests
        if self
            .inner
            .req_counter
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed)
            >= self.inner.req_limit
        {
            self.inner
                .req_counter
                .store(0, std::sync::atomic::Ordering::Relaxed);
            let nc = self.clone();
            tokio::spawn(async move { nc.new_visitor_data().await });
        }

        {
            let vds = self.inner.visitor_data.read().unwrap();
            if !vds.is_empty() {
                let mut rng = rand::rng();
                let vd = vds[rng.random_range(0..vds.len())].to_owned();
                tracing::debug!("visitor data {vd} picked from cache");
                return Ok(vd);
            }
        }
        // Fetch new visitor data if the cache is empty
        self.new_visitor_data().await
    }

    /// Remove a visitor data ID from the cache.
    ///
    /// This also removes the PO token associated with that ID.
    pub fn remove(&self, visitor_data: &str) {
        let mut vds = self.inner.visitor_data.write().unwrap();
        if let Some(i) = vds.iter().position(|x| x == visitor_data) {
            vds.remove(i);
            let mut pots = self.inner.session_potoken.write().unwrap();
            pots.remove(visitor_data);
            tracing::debug!("visitor data {visitor_data} removed from cache");
        }
    }

    /// Store a session PO token in the cache
    pub fn store_pot(&self, visitor_data: &str, po_token: PoToken) {
        let mut pots = self.inner.session_potoken.write().unwrap();
        pots.insert(visitor_data.to_owned(), po_token);
    }

    /// Get a session PO token from the cache
    pub fn get_pot(&self, visitor_data: &str) -> Option<PoToken> {
        let pots = self.inner.session_potoken.read().unwrap();
        if let Some(entry) = pots.get(visitor_data) {
            if entry.valid_until > OffsetDateTime::now_utc() + time::Duration::minutes(10) {
                return Some(entry.clone());
            }
        }
        None
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
        let cache = VisitorDataCache::new(
            Client::builder().user_agent(DEFAULT_UA).build().unwrap(),
            2,
            2,
        );
        // Get initial visitor data
        let v1 = cache.get().await.unwrap();

        // Run as many request as necessary to fetch second visitor data
        for _ in 0..=cache.inner.req_limit {
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

    #[tokio::test]
    #[traced_test]
    async fn cache_potoken() {
        let cache = VisitorDataCache::new(
            Client::builder().user_agent(DEFAULT_UA).build().unwrap(),
            1,
            2,
        );
        let v1 = cache.get().await.unwrap();
        let pot1 = PoToken {
            po_token: "pot1".to_owned(),
            valid_until: OffsetDateTime::now_utc() + time::Duration::hours(1),
        };
        cache.store_pot(&v1, pot1.clone());
        assert_eq!(cache.get_pot(&v1).unwrap(), pot1);

        for _ in 0..4 {
            cache.get().await.unwrap();
        }

        for _ in 0..3 {
            tokio::time::sleep(Duration::from_millis(1000)).await;
            {
                let vd = cache.inner.visitor_data.read().unwrap();
                if !vd.contains(&v1) {
                    break;
                }
            }
        }
        {
            let vd = cache.inner.visitor_data.read().unwrap();
            assert!(!vd.contains(&v1), "first token still present");
        }

        assert_eq!(cache.get_pot(&v1), None);
    }
}
