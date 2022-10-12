use crate::error::Error;
use crate::model::{
    ChannelPlaylist, ChannelVideo, Comment, Paginator, PlaylistVideo, RecommendedVideo, SearchItem,
};

use super::RustyPipeQuery;

macro_rules! paginator {
    ($entity_type:ty, $cont_function:path) => {
        impl Paginator<$entity_type> {
            pub async fn next(&self, query: RustyPipeQuery) -> Result<Option<Self>, Error> {
                Ok(match &self.ctoken {
                    Some(ctoken) => Some($cont_function(query, ctoken).await?),
                    None => None,
                })
            }

            pub async fn extend(&mut self, query: RustyPipeQuery) -> Result<bool, Error> {
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

            pub async fn extend_pages(
                &mut self,
                query: RustyPipeQuery,
                n_pages: usize,
            ) -> Result<(), Error> {
                for _ in 0..n_pages {
                    match self.extend(query.clone()).await {
                        Ok(false) => break,
                        Err(e) => return Err(e),
                        _ => {}
                    }
                }
                Ok(())
            }

            pub async fn extend_limit(
                &mut self,
                query: RustyPipeQuery,
                n_items: usize,
            ) -> Result<(), Error> {
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
    };
}

paginator!(PlaylistVideo, RustyPipeQuery::playlist_continuation);
paginator!(RecommendedVideo, RustyPipeQuery::video_recommendations);
paginator!(Comment, RustyPipeQuery::video_comments);
paginator!(ChannelVideo, RustyPipeQuery::channel_videos_continuation);
paginator!(
    ChannelPlaylist,
    RustyPipeQuery::channel_playlists_continuation
);
paginator!(SearchItem, RustyPipeQuery::search_continuation);
