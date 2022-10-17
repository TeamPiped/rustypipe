use crate::error::{Error, ExtractionError};
use crate::model::{Comment, ContinuationEndpoint, Paginator, PlaylistVideo, YouTubeItem};
use crate::serializer::MapResult;
use crate::util::TryRemove;

use super::{response, ClientType, MapResponse, QContinuation, RustyPipeQuery};

impl RustyPipeQuery {
    pub async fn continuation<T: TryFrom<YouTubeItem>>(
        self,
        ctoken: &str,
        endpoint: ContinuationEndpoint,
        visitor_data: Option<&str>,
    ) -> Result<Paginator<T>, Error> {
        let mut context = self.get_context(ClientType::Desktop, true).await;
        context.client.visitor_data = visitor_data.map(str::to_owned);
        let request_body = QContinuation {
            context,
            continuation: ctoken,
        };

        let p = self
            .execute_request::<response::Continuation, Paginator<YouTubeItem>, _>(
                ClientType::Desktop,
                "continuation",
                ctoken,
                endpoint.as_str(),
                &request_body,
            )
            .await?;

        Ok(Paginator {
            count: p.count,
            items: p
                .items
                .into_iter()
                .filter_map(|item| T::try_from(item).ok())
                .collect(),
            ctoken: p.ctoken,
            visitor_data: p.visitor_data,
            endpoint,
        })
    }
}

impl<T: TryFrom<YouTubeItem>> MapResponse<Paginator<T>> for response::Continuation {
    fn map_response(
        self,
        _id: &str,
        lang: crate::param::Language,
        _deobf: Option<&crate::deobfuscate::Deobfuscator>,
    ) -> Result<MapResult<Paginator<T>>, ExtractionError> {
        let mut actions = self
            .on_response_received_actions
            .ok_or(ExtractionError::Retry)?;
        let items = some_or_bail!(
            actions.try_swap_remove(0),
            Err(ExtractionError::InvalidData(
                "no item section renderer".into()
            ))
        )
        .append_continuation_items_action
        .continuation_items;

        let mut mapper = response::YouTubeListMapper::<YouTubeItem>::new(lang);
        mapper.map_response(items);

        Ok(MapResult {
            c: Paginator::new(
                self.estimated_results,
                mapper
                    .items
                    .into_iter()
                    .filter_map(|item| T::try_from(item).ok())
                    .collect(),
                mapper.ctoken,
            ),
            warnings: mapper.warnings,
        })
    }
}

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

paginator!(Comment, RustyPipeQuery::video_comments);
paginator!(PlaylistVideo, RustyPipeQuery::playlist_continuation);

impl<T: TryFrom<YouTubeItem>> Paginator<T> {
    pub async fn next(&self, query: RustyPipeQuery) -> Result<Option<Self>, Error> {
        Ok(match &self.ctoken {
            Some(ctoken) => Some(
                query
                    .continuation(ctoken, self.endpoint, self.visitor_data.as_deref())
                    .await?,
            ),
            _ => None,
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

#[cfg(test)]
mod tests {
    use std::{fs::File, io::BufReader, path::Path};

    use rstest::rstest;

    use crate::{
        client::{response, MapResponse},
        error::ExtractionError,
        model::{Paginator, PlaylistItem, YouTubeItem},
        param::Language,
        serializer::MapResult,
    };

    #[rstest]
    #[case("search", "search/cont")]
    #[case("startpage", "trends/startpage_cont")]
    #[case("recommendations", "video_details/recommendations")]
    fn map_continuation_items(#[case] name: &str, #[case] path: &str) {
        let filename = format!("testfiles/{}.json", path);
        let json_path = Path::new(&filename);
        let json_file = File::open(json_path).unwrap();

        let items: response::Continuation =
            serde_json::from_reader(BufReader::new(json_file)).unwrap();
        let map_res: MapResult<Paginator<YouTubeItem>> =
            items.map_response("", Language::En, None).unwrap();

        assert!(
            map_res.warnings.is_empty(),
            "deserialization/mapping warnings: {:?}",
            map_res.warnings
        );
        insta::assert_ron_snapshot!(format!("map_{}", name), map_res.c, {
            ".items.*.publish_date" => "[date]",
        });
    }

    #[rstest]
    #[case("channel_playlists", "channel/channel_playlists_cont")]
    fn map_continuation_playlists(#[case] name: &str, #[case] path: &str) {
        let filename = format!("testfiles/{}.json", path);
        let json_path = Path::new(&filename);
        let json_file = File::open(json_path).unwrap();

        let items: response::Continuation =
            serde_json::from_reader(BufReader::new(json_file)).unwrap();
        let map_res: MapResult<Paginator<PlaylistItem>> =
            items.map_response("", Language::En, None).unwrap();

        assert!(
            map_res.warnings.is_empty(),
            "deserialization/mapping warnings: {:?}",
            map_res.warnings
        );
        insta::assert_ron_snapshot!(format!("map_{}", name), map_res.c);
    }

    #[test]
    fn map_recommendations_empty() {
        let filename = format!("testfiles/video_details/recommendations_empty.json");
        let json_path = Path::new(&filename);
        let json_file = File::open(json_path).unwrap();

        let recommendations: response::Continuation =
            serde_json::from_reader(BufReader::new(json_file)).unwrap();
        let map_res: Result<MapResult<Paginator<YouTubeItem>>, ExtractionError> =
            recommendations.map_response("", Language::En, None);
        let err = map_res.unwrap_err();

        assert!(matches!(err, crate::error::ExtractionError::Retry));
    }
}
