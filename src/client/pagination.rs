use crate::error::Result;

use crate::model::{
    ChannelPlaylist, ChannelVideo, Comment, Paginator, PlaylistVideo, RecommendedVideo, SearchItem,
};

use super::RustyPipeQuery;

impl Paginator<PlaylistVideo> {
    pub async fn next(&self, query: RustyPipeQuery) -> Result<Option<Self>> {
        Ok(match &self.ctoken {
            Some(ctoken) => Some(query.playlist_continuation(ctoken).await?),
            None => None,
        })
    }

    pub async fn extend(&mut self, query: RustyPipeQuery) -> Result<bool> {
        match self.next(query).await {
            Ok(Some(paginator)) => {
                let mut items = paginator.items;
                self.items.append(&mut items);
                self.ctoken = paginator.ctoken;
                Ok(true)
            }
            Ok(None) => Ok(false),
            Err(e) => Err(e),
        }
    }

    pub async fn extend_pages(&mut self, query: RustyPipeQuery, n_pages: usize) -> Result<()> {
        for _ in 0..n_pages {
            match self.extend(query.clone()).await {
                Ok(false) => break,
                Err(e) => return Err(e),
                _ => {}
            }
        }
        Ok(())
    }

    pub async fn extend_limit(&mut self, query: RustyPipeQuery, n_items: usize) -> Result<()> {
        while self.items.len() < n_items {
            match self.extend(query.clone()).await {
                Ok(false) => break,
                Err(e) => return Err(e),
                _ => {}
            }
        }
        Ok(())
    }
}

impl Paginator<RecommendedVideo> {
    pub async fn next(&self, query: RustyPipeQuery) -> Result<Option<Self>> {
        Ok(match &self.ctoken {
            Some(ctoken) => Some(query.video_recommendations(ctoken).await?),
            None => None,
        })
    }

    pub async fn extend(&mut self, query: RustyPipeQuery) -> Result<bool> {
        match self.next(query).await {
            Ok(Some(paginator)) => {
                let mut items = paginator.items;
                self.items.append(&mut items);
                self.ctoken = paginator.ctoken;
                Ok(true)
            }
            Ok(None) => Ok(false),
            Err(e) => Err(e),
        }
    }

    pub async fn extend_pages(&mut self, query: RustyPipeQuery, n_pages: usize) -> Result<()> {
        for _ in 0..n_pages {
            match self.extend(query.clone()).await {
                Ok(false) => break,
                Err(e) => return Err(e),
                _ => {}
            }
        }
        Ok(())
    }

    pub async fn extend_limit(&mut self, query: RustyPipeQuery, n_items: usize) -> Result<()> {
        while self.items.len() < n_items {
            match self.extend(query.clone()).await {
                Ok(false) => break,
                Err(e) => return Err(e),
                _ => {}
            }
        }
        Ok(())
    }
}

impl Paginator<ChannelVideo> {
    pub async fn next(&self, query: RustyPipeQuery) -> Result<Option<Self>> {
        Ok(match &self.ctoken {
            Some(ctoken) => Some(query.channel_videos_continuation(ctoken).await?),
            None => None,
        })
    }

    pub async fn extend(&mut self, query: RustyPipeQuery) -> Result<bool> {
        match self.next(query).await {
            Ok(Some(paginator)) => {
                let mut items = paginator.items;
                self.items.append(&mut items);
                self.ctoken = paginator.ctoken;
                Ok(true)
            }
            Ok(None) => Ok(false),
            Err(e) => Err(e),
        }
    }

    pub async fn extend_pages(&mut self, query: RustyPipeQuery, n_pages: usize) -> Result<()> {
        for _ in 0..n_pages {
            match self.extend(query.clone()).await {
                Ok(false) => break,
                Err(e) => return Err(e),
                _ => {}
            }
        }
        Ok(())
    }

    pub async fn extend_limit(&mut self, query: RustyPipeQuery, n_items: usize) -> Result<()> {
        while self.items.len() < n_items {
            match self.extend(query.clone()).await {
                Ok(false) => break,
                Err(e) => return Err(e),
                _ => {}
            }
        }
        Ok(())
    }
}

impl Paginator<ChannelPlaylist> {
    pub async fn next(&self, query: RustyPipeQuery) -> Result<Option<Self>> {
        Ok(match &self.ctoken {
            Some(ctoken) => Some(query.channel_playlists_continuation(ctoken).await?),
            None => None,
        })
    }

    pub async fn extend(&mut self, query: RustyPipeQuery) -> Result<bool> {
        match self.next(query).await {
            Ok(Some(paginator)) => {
                let mut items = paginator.items;
                self.items.append(&mut items);
                self.ctoken = paginator.ctoken;
                Ok(true)
            }
            Ok(None) => Ok(false),
            Err(e) => Err(e),
        }
    }

    pub async fn extend_pages(&mut self, query: RustyPipeQuery, n_pages: usize) -> Result<()> {
        for _ in 0..n_pages {
            match self.extend(query.clone()).await {
                Ok(false) => break,
                Err(e) => return Err(e),
                _ => {}
            }
        }
        Ok(())
    }

    pub async fn extend_limit(&mut self, query: RustyPipeQuery, n_items: usize) -> Result<()> {
        while self.items.len() < n_items {
            match self.extend(query.clone()).await {
                Ok(false) => break,
                Err(e) => return Err(e),
                _ => {}
            }
        }
        Ok(())
    }
}

impl Paginator<Comment> {
    pub async fn next(&self, query: RustyPipeQuery) -> Result<Option<Self>> {
        Ok(match &self.ctoken {
            Some(ctoken) => Some(query.video_comments(ctoken).await?),
            None => None,
        })
    }

    pub async fn extend(&mut self, query: RustyPipeQuery) -> Result<bool> {
        match self.next(query).await {
            Ok(Some(paginator)) => {
                let mut items = paginator.items;
                self.items.append(&mut items);
                self.ctoken = paginator.ctoken;
                Ok(true)
            }
            Ok(None) => Ok(false),
            Err(e) => Err(e),
        }
    }

    pub async fn extend_pages(&mut self, query: RustyPipeQuery, n_pages: usize) -> Result<()> {
        for _ in 0..n_pages {
            match self.extend(query.clone()).await {
                Ok(false) => break,
                Err(e) => return Err(e),
                _ => {}
            }
        }
        Ok(())
    }

    pub async fn extend_limit(&mut self, query: RustyPipeQuery, n_items: usize) -> Result<()> {
        while self.items.len() < n_items {
            match self.extend(query.clone()).await {
                Ok(false) => break,
                Err(e) => return Err(e),
                _ => {}
            }
        }
        Ok(())
    }
}

impl Paginator<SearchItem> {
    pub async fn next(&self, query: RustyPipeQuery) -> Result<Option<Self>> {
        Ok(match &self.ctoken {
            Some(ctoken) => Some(query.search_continuation(ctoken).await?),
            None => None,
        })
    }

    pub async fn extend(&mut self, query: RustyPipeQuery) -> Result<bool> {
        match self.next(query).await {
            Ok(Some(paginator)) => {
                let mut items = paginator.items;
                self.items.append(&mut items);
                self.ctoken = paginator.ctoken;
                Ok(true)
            }
            Ok(None) => Ok(false),
            Err(e) => Err(e),
        }
    }

    pub async fn extend_pages(&mut self, query: RustyPipeQuery, n_pages: usize) -> Result<()> {
        for _ in 0..n_pages {
            match self.extend(query.clone()).await {
                Ok(false) => break,
                Err(e) => return Err(e),
                _ => {}
            }
        }
        Ok(())
    }

    pub async fn extend_limit(&mut self, query: RustyPipeQuery, n_items: usize) -> Result<()> {
        while self.items.len() < n_items {
            match self.extend(query.clone()).await {
                Ok(false) => break,
                Err(e) => return Err(e),
                _ => {}
            }
        }
        Ok(())
    }
}
