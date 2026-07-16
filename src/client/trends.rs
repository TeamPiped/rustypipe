use crate::{
    error::{Error, ExtractionError},
    json::{yt_two_column_list_items, ytq, JsonDoc, JsonNode},
    model::{
        paginator::{ContinuationEndpoint, Paginator},
        VideoItem,
    },
    param::TrendingTab,
    request_body::ytbody,
    serializer::{ItemsAccumulator, MapResult},
};

use super::{response, ClientType, MapEndpoint, MapRespCtx, RustyPipeQuery};

#[derive(Debug)]
struct TrendingEndpoint;

fn collect_trending_item(
    node: &JsonNode<'_>,
    acc: &mut ItemsAccumulator<VideoItem>,
    lang: crate::param::Language,
) {
    if let Some(content) = node.query(ytq!(.richItemRenderer.content)) {
        collect_trending_item(&content, acc, lang);
        return;
    }

    if let Some(contents) = node.query(ytq!(.sectionListRenderer.contents)) {
        contents
            .items()
            .into_iter()
            .for_each(|item| collect_trending_item(&item, acc, lang));
        return;
    }

    if node
        .query(ytq!(
            .videoRenderer
                || .gridVideoRenderer
                || .compactVideoRenderer
                || .reelItemRenderer
                || .shortsLockupViewModel
                || .playlistVideoRenderer
                || .lockupViewModel
                || .continuationItemRenderer
        ))
        .is_some()
    {
        let (mapped, item_ctoken, _) = response::video_item::map_video_item(node, lang);
        acc.add_mapped_vec(mapped, item_ctoken);
        return;
    }

    if let Some(contents) =
        node.query(ytq!(.richSectionRenderer.content.richShelfRenderer.contents))
    {
        contents
            .items()
            .into_iter()
            .for_each(|item| collect_trending_item(&item, acc, lang));
        return;
    }

    if let Some(contents) = node.query(ytq!(.itemSectionRenderer.contents)) {
        contents
            .items()
            .into_iter()
            .for_each(|item| collect_trending_item(&item, acc, lang));
        return;
    }

    if let Some(items_node) = node.query(ytq!(
        .shelfRenderer.content.(.horizontalListRenderer || .expandedShelfContentsRenderer).items
    )) {
        let (mapped, list_ctoken, _) = response::video_item::map_video_items(&items_node, lang);
        acc.add_mapped_vec(mapped, list_ctoken);
        return;
    }

    if let Some(items_node) = node.query(ytq!(.expandedShelfContentsRenderer.items)) {
        let (mapped, list_ctoken, _) = response::video_item::map_video_items(&items_node, lang);
        acc.add_mapped_vec(mapped, list_ctoken);
    }
}

fn collect_trending_items(
    root: &JsonNode<'_>,
    lang: crate::param::Language,
) -> Result<(MapResult<Vec<VideoItem>>, Option<String>), ExtractionError> {
    let contents = yt_two_column_list_items(root)?;
    let mut acc = ItemsAccumulator::new();

    for item in contents.items() {
        collect_trending_item(&item, &mut acc, lang);
    }

    Ok(acc.finish())
}

impl RustyPipeQuery {
    /// Get the videos from the default YouTube explore tab.
    ///
    /// This currently maps to [`TrendingTab::News`].
    #[tracing::instrument(skip(self), level = "error")]
    pub async fn trending(&self) -> Result<Paginator<VideoItem>, Error> {
        self.trending_tab(TrendingTab::default()).await
    }

    /// Get the videos from a YouTube explore tab.
    #[tracing::instrument(skip(self), level = "error")]
    pub async fn trending_tab(&self, tab: TrendingTab) -> Result<Paginator<VideoItem>, Error> {
        let request_body = ytbody!({
            "browseId": tab.browse_id(),
        });

        self.execute_request::<TrendingEndpoint, _, _>(
            ClientType::Desktop,
            "trends",
            tab.browse_id(),
            "browse",
            &request_body,
        )
        .await
    }
}

impl MapEndpoint<Paginator<VideoItem>> for TrendingEndpoint {
    fn map(
        json: &JsonDoc,
        ctx: &MapRespCtx<'_>,
    ) -> Result<MapResult<Paginator<VideoItem>>, ExtractionError> {
        json.with_root(|root| {
            let (mapped, ctoken) = collect_trending_items(&root, ctx.lang)?;
            Ok(MapResult {
                c: Paginator::new_ext(
                    None,
                    mapped.c,
                    ctoken,
                    ctx.visitor_data(&root),
                    ContinuationEndpoint::Browse,
                    ctx.authenticated,
                ),
                warnings: mapped.warnings,
            })
        })
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use path_macro::path;
    use rstest::rstest;

    use super::*;
    use crate::{model::VideoItem, util::tests::TESTFILES};

    fn map_video_paginator(p: Paginator<crate::model::YouTubeItem>) -> Paginator<VideoItem> {
        Paginator {
            count: p.count,
            items: p
                .items
                .into_iter()
                .filter_map(crate::model::traits::FromYtItem::from_yt_item)
                .collect(),
            ctoken: p.ctoken,
            visitor_data: p.visitor_data,
            endpoint: p.endpoint,
            authenticated: p.authenticated,
        }
    }

    #[rstest]
    #[case::base("videos")]
    #[case::page_header_renderer("20230501_page_header_renderer")]
    fn map_trending(#[case] name: &str) {
        let filename = match name {
            "videos" => "startpage.json".to_owned(),
            _ => format!("trending_{name}.json"),
        };
        let json_path = path!(*TESTFILES / "trends" / filename);
        let json = JsonDoc::new(fs::read_to_string(json_path).unwrap());
        let map_res: MapResult<Paginator<VideoItem>> =
            TrendingEndpoint::map(&json, &MapRespCtx::test("")).unwrap();

        assert!(
            map_res.warnings.is_empty(),
            "deserialization/mapping warnings: {:?}",
            map_res.warnings
        );

        insta::assert_ron_snapshot!(format!("map_trending_{name}"), map_res.c, {
            ".items[].publish_date" => "[date]",
        });
    }

    #[test]
    fn map_trending_continuation() {
        let json_path = path!(*TESTFILES / "trends" / "startpage_cont.json");
        let json = JsonDoc::new(fs::read_to_string(json_path).unwrap());
        let map_res = crate::client::pagination::ContinuationMarker::map(
            &json,
            &MapRespCtx::test(""),
        )
        .unwrap();
        let paginator: Paginator<VideoItem> = map_video_paginator(map_res.c);

        assert!(
            map_res.warnings.is_empty(),
            "deserialization/mapping warnings: {:?}",
            map_res.warnings
        );

        insta::assert_ron_snapshot!("map_trending_continuation", paginator, {
            ".items[].publish_date" => "[date]",
        });
    }
}
