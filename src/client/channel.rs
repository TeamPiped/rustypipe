use std::fmt::Debug;

use time::OffsetDateTime;
use url::Url;

use crate::{
    error::{Error, ExtractionError},
    json::{ytq, JsonDoc, JsonNode},
    model::{
        paginator::{ContinuationEndpoint, Paginator},
        traits::FromYtItem,
        Channel, ChannelInfo, PlaylistItem, Verification, VideoItem, YouTubeItem,
    },
    param::{ChannelContent, ChannelOrder, ChannelVideoTab, Language},
    request_body::ytbody,
    serializer::{text::TextComponent, MapResult},
    util::{self, timeago, ProtoBuilder},
};

use super::{response, ClientType, MapEndpoint, MapRespCtx, MapRespOptions, RustyPipeQuery};

/// Result of a [`RustyPipeQuery::channel_content`] call.
///
/// Different call shapes return different shapes: a regular `browse` call
/// returns the full channel header + paginator; an ordered continuation call
/// returns just a paginator (no channel metadata is fetched in that round-trip).
#[derive(Debug, Clone)]
pub enum ChannelContentResult<T> {
    /// Full channel header and paginator (regular `browse` call).
    Full(Channel<Paginator<T>>),
    /// Paginator only, e.g. for an ordered continuation call.
    Paginator(Paginator<VideoItem>),
}

impl<T> ChannelContentResult<T> {
    /// Returns the inner `Channel<Paginator<T>>` if this is the `Full` variant.
    pub fn full(self) -> Option<Channel<Paginator<T>>> {
        match self {
            Self::Full(c) => Some(c),
            Self::Paginator(_) => None,
        }
    }

    /// Returns the inner `Paginator<VideoItem>` if this is the `Paginator` variant.
    pub fn into_paginator(self) -> Option<Paginator<VideoItem>> {
        match self {
            Self::Full(_) => None,
            Self::Paginator(p) => Some(p),
        }
    }
}

#[derive(Debug)]
struct ChannelEndpoint;
#[derive(Debug)]
struct ChannelAboutEndpoint;

#[derive(Clone, Debug)]
struct ChannelHeader {
    id: String,
    name: String,
    handle: Option<String>,
    subscriber_count: Option<u64>,
    video_count: Option<u64>,
    avatar: Vec<crate::model::Thumbnail>,
    verification: Verification,
    description: String,
    tags: Vec<String>,
    banner: Vec<crate::model::Thumbnail>,
    has_shorts: bool,
    has_live: bool,
    visitor_data: Option<String>,
}

impl ChannelHeader {
    fn into_channel(self) -> Channel<()> {
        Channel {
            id: self.id,
            name: self.name,
            handle: self.handle,
            subscriber_count: self.subscriber_count,
            video_count: self.video_count,
            avatar: self.avatar,
            verification: self.verification,
            description: self.description,
            tags: self.tags,
            banner: self.banner,
            has_shorts: self.has_shorts,
            has_live: self.has_live,
            visitor_data: self.visitor_data,
            content: (),
        }
    }
}

impl RustyPipeQuery {
    /// Get a tab (videos, shorts, livestreams, playlists, or search) of a YouTube channel.
    ///
    /// Without an `order`, performs a regular `browse` call and returns the full
    /// channel header alongside the paginator. With `Some(order)`, the order is
    /// only meaningful for video-tab variants (`Videos`, `Shorts`, `Live`); in
    /// that case the call is satisfied via a continuation token and no channel
    /// header is fetched (the result is a paginator only).
    ///
    /// The generic item type `T` must implement
    /// [`FromYtItem`](crate::model::FromYtItem) so that both video and
    /// playlist responses can be returned through the same signature.
    #[tracing::instrument(skip(self), level = "error")]
    pub async fn channel_content<S, T>(
        &self,
        channel_id: S,
        content: ChannelContent<'_>,
        order: Option<ChannelOrder>,
    ) -> Result<ChannelContentResult<T>, Error>
    where
        S: AsRef<str> + Debug,
        T: FromYtItem,
    {
        let channel_id = channel_id.as_ref();

        if let Some(order) = order {
            let tab = content.as_video_tab().ok_or_else(|| {
                Error::Extraction(ExtractionError::InvalidData(
                    "order is only supported for Videos/Shorts/Live".into(),
                ))
            })?;
            let p: Paginator<VideoItem> = self
                .continuation(
                    order_ctoken(channel_id, tab, order, &random_target()),
                    ContinuationEndpoint::Browse,
                    None,
                )
                .await?;
            return Ok(ChannelContentResult::Paginator(p));
        }

        match content {
            ChannelContent::Videos | ChannelContent::Shorts | ChannelContent::Live => {
                let raw: Channel<Paginator<VideoItem>> = self
                    ._channel_videos(
                        channel_id,
                        content.browse_params(),
                        None,
                        "channel_videos",
                    )
                    .await?;
                Ok(ChannelContentResult::Full(Channel {
                    id: raw.id,
                    name: raw.name,
                    handle: raw.handle,
                    subscriber_count: raw.subscriber_count,
                    video_count: raw.video_count,
                    avatar: raw.avatar,
                    verification: raw.verification,
                    description: raw.description,
                    tags: raw.tags,
                    banner: raw.banner,
                    has_shorts: raw.has_shorts,
                    has_live: raw.has_live,
                    visitor_data: raw.visitor_data,
                    content: Paginator {
                        count: raw.content.count,
                        items: raw
                            .content
                            .items
                            .into_iter()
                            .map(YouTubeItem::Video)
                            .filter_map(T::from_yt_item)
                            .collect::<Vec<_>>(),
                        ctoken: raw.content.ctoken,
                        visitor_data: raw.content.visitor_data,
                        endpoint: raw.content.endpoint,
                        authenticated: raw.content.authenticated,
                    },
                }))
            }
            ChannelContent::Playlists => {
                let raw: Channel<Paginator<PlaylistItem>> =
                    self._channel_playlists(channel_id, "channel_playlists").await?;
                Ok(ChannelContentResult::Full(Channel {
                    id: raw.id,
                    name: raw.name,
                    handle: raw.handle,
                    subscriber_count: raw.subscriber_count,
                    video_count: raw.video_count,
                    avatar: raw.avatar,
                    verification: raw.verification,
                    description: raw.description,
                    tags: raw.tags,
                    banner: raw.banner,
                    has_shorts: raw.has_shorts,
                    has_live: raw.has_live,
                    visitor_data: raw.visitor_data,
                    content: Paginator {
                        count: raw.content.count,
                        items: raw
                            .content
                            .items
                            .into_iter()
                            .map(YouTubeItem::Playlist)
                            .filter_map(T::from_yt_item)
                            .collect::<Vec<_>>(),
                        ctoken: raw.content.ctoken,
                        visitor_data: raw.content.visitor_data,
                        endpoint: raw.content.endpoint,
                        authenticated: raw.content.authenticated,
                    },
                }))
            }
            ChannelContent::Search(query) => {
                let raw: Channel<Paginator<VideoItem>> = self
                    ._channel_videos(
                        channel_id,
                        ChannelContent::Search(query).browse_params(),
                        Some(query),
                        "channel_search",
                    )
                    .await?;
                Ok(ChannelContentResult::Full(Channel {
                    id: raw.id,
                    name: raw.name,
                    handle: raw.handle,
                    subscriber_count: raw.subscriber_count,
                    video_count: raw.video_count,
                    avatar: raw.avatar,
                    verification: raw.verification,
                    description: raw.description,
                    tags: raw.tags,
                    banner: raw.banner,
                    has_shorts: raw.has_shorts,
                    has_live: raw.has_live,
                    visitor_data: raw.visitor_data,
                    content: Paginator {
                        count: raw.content.count,
                        items: raw
                            .content
                            .items
                            .into_iter()
                            .map(YouTubeItem::Video)
                            .filter_map(T::from_yt_item)
                            .collect::<Vec<_>>(),
                        ctoken: raw.content.ctoken,
                        visitor_data: raw.content.visitor_data,
                        endpoint: raw.content.endpoint,
                        authenticated: raw.content.authenticated,
                    },
                }))
            }
        }
    }

    async fn _channel_videos(
        &self,
        channel_id: &str,
        params: &str,
        query: Option<&str>,
        operation: &str,
    ) -> Result<Channel<Paginator<VideoItem>>, Error> {
        let request_body = ytbody!({
            "browseId": channel_id,
            "params": params,
            ? "query": query,
        });

        self.execute_request::<ChannelEndpoint, _, _>(
            ClientType::Desktop,
            operation,
            channel_id,
            "browse",
            &request_body,
        )
        .await
    }

    async fn _channel_playlists(
        &self,
        channel_id: &str,
        operation: &str,
    ) -> Result<Channel<Paginator<PlaylistItem>>, Error> {
        let request_body = ytbody!({
            "browseId": channel_id,
            "params": ChannelContent::Playlists.browse_params(),
        });

        self.execute_request::<ChannelEndpoint, _, _>(
            ClientType::Desktop,
            operation,
            channel_id,
            "browse",
            &request_body,
        )
        .await
    }

    /// Get additional metadata from the *About* tab of a channel
    #[tracing::instrument(skip(self), level = "error")]
    pub async fn channel_info<S: AsRef<str> + Debug>(
        &self,
        channel_id: S,
    ) -> Result<ChannelInfo, Error> {
        let channel_id = channel_id.as_ref();
        let request_body = ytbody!({
            "continuation": channel_info_ctoken(channel_id, &random_target()),
        });

        self.execute_request_ctx::<ChannelAboutEndpoint, _, _>(
            ClientType::Desktop,
            "channel_info",
            channel_id,
            "browse",
            &request_body,
            MapRespOptions {
                unlocalized: true,
                ..Default::default()
            },
        )
        .await
    }
}

fn json_alerts_to_err(id: &str, root: &JsonNode<'_>) -> ExtractionError {
    let alerts = root.query(ytq!(.alerts)).map(|node| {
        let (alerts, _) = node.deserialize_items_lossy::<response::Alert>();
        alerts
    });
    response::alerts_to_err(id, alerts)
}

struct MapChannelData<'a> {
    header: JsonNode<'a>,
    metadata: response::channel::ChannelMetadataRenderer,
    microformat: response::channel::MicroformatDataRenderer,
    visitor_data: Option<String>,
    has_shorts: bool,
    has_live: bool,
}

fn metadata_part_text(part: &JsonNode<'_>) -> Option<String> {
    part.text_at(ytq!(($root || .avatarStack.avatarStackViewModel).text))
}

fn page_header_verification(header: &JsonNode<'_>) -> Verification {
    header
        .query(ytq!(.title.dynamicTextViewModel.text.attachmentRuns[0]))
        .and_then(|node| node.deserialize::<response::AttachmentRun>().ok())
        .map(Verification::from)
        .unwrap_or_default()
}

fn map_channel(
    d: MapChannelData<'_>,
    ctx: &MapRespCtx<'_>,
) -> Result<MapResult<ChannelHeader>, ExtractionError> {
    if d.metadata.external_id != ctx.id {
        return Err(crate::client::check_id_matches(
            d.metadata.external_id.clone(),
            ctx.id,
            "channel",
        ));
    }

    let handle = d
        .metadata
        .vanity_channel_url
        .as_ref()
        .and_then(|url| Url::parse(url).ok())
        .and_then(|url| {
            url.path()
                .strip_prefix('/')
                .filter(|handle| util::CHANNEL_HANDLE_REGEX.is_match(handle))
                .map(str::to_owned)
        });
    let mut warnings = Vec::new();

    Ok(MapResult {
        c: if let Some(header) = d.header.query(ytq!(.c4TabbedHeaderRenderer)) {
            let (badges, mut badge_warnings) = header
                .query(ytq!(.badges))
                .map(|node| node.deserialize_items_lossy::<response::ChannelBadge>())
                .unwrap_or_default();
            warnings.append(&mut badge_warnings);
            ChannelHeader {
                id: d.metadata.external_id,
                name: d.metadata.title,
                handle,
                subscriber_count: header.query(ytq!(.subscriberCountText)).and_then(|node| {
                    let txt = node.text()?;
                    util::parse_large_numstr_or_warn(&txt, ctx.lang, &mut warnings)
                }),
                video_count: None,
                avatar: header.query_thumbnails(ytq!(.avatar)),
                verification: badges.into(),
                description: d.metadata.description,
                tags: d.microformat.tags,
                banner: header.query_thumbnails(ytq!(.banner)),
                has_shorts: d.has_shorts,
                has_live: d.has_live,
                visitor_data: d.visitor_data,
            }
        } else if let Some(carousel) = d.header.query(ytq!(.carouselHeaderRenderer)) {
            let hdata = carousel.query(ytq!(.contents)).and_then(|contents| {
                contents.items().into_iter().find_map(|item| {
                    let item = item.query(ytq!(.topicChannelDetailsRenderer))?;
                    Some((
                        item.text_at(ytq!(.subscriberCountText || .subtitle)),
                        item.query_thumbnails(ytq!(.avatar)),
                    ))
                })
            });

            ChannelHeader {
                id: d.metadata.external_id,
                name: d.metadata.title,
                handle,
                subscriber_count: hdata.as_ref().and_then(|hdata| {
                    hdata.0.as_ref().and_then(|txt| {
                        util::parse_large_numstr_or_warn(txt, ctx.lang, &mut warnings)
                    })
                }),
                video_count: None,
                avatar: hdata.map(|hdata| hdata.1).unwrap_or_default(),
                verification: crate::model::Verification::Verified,
                description: d.metadata.description,
                tags: d.microformat.tags,
                banner: Vec::new(),
                has_shorts: d.has_shorts,
                has_live: d.has_live,
                visitor_data: d.visitor_data,
            }
        } else if let Some(header) = d
            .header
            .query(ytq!(.pageHeaderRenderer.content.pageHeaderViewModel))
        {
            let md_rows = header
                .query(ytq!(.metadata.contentMetadataViewModel.metadataRows))
                .map(|node| node.items())
                .unwrap_or_default();
            let (sub_part, vc_part) = if md_rows.len() > 1 {
                let parts = md_rows[1]
                    .query(ytq!(.metadataParts))
                    .map(|node| node.items())
                    .unwrap_or_default();
                (parts.first().cloned(), parts.get(1).cloned())
            } else {
                let parts = md_rows
                    .first()
                    .and_then(|row| row.query(ytq!(.metadataParts)))
                    .map(|node| node.items())
                    .unwrap_or_default();
                (parts.get(1).cloned(), None)
            };
            let subscriber_count = sub_part
                .as_ref()
                .and_then(|t| metadata_part_text(t))
                .and_then(|txt| {
                    util::parse_large_numstr_or_warn::<u64>(&txt, ctx.lang, &mut warnings)
                });
            let video_count = vc_part
                .as_ref()
                .and_then(|t| metadata_part_text(t))
                .and_then(|txt| util::parse_large_numstr_or_warn(&txt, ctx.lang, &mut warnings));

            ChannelHeader {
                id: d.metadata.external_id,
                name: d.metadata.title,
                handle: handle.or_else(|| {
                    md_rows
                        .first()
                        .and_then(|row| row.query(ytq!(.metadataParts)))
                        .and_then(|parts| parts.items().get(1).cloned())
                        .and_then(|part| metadata_part_text(&part))
                        .filter(|txt| util::CHANNEL_HANDLE_REGEX.is_match(txt))
                }),
                subscriber_count,
                video_count,
                avatar: header.query_thumbnails(ytq!(
                    .image.decoratedAvatarViewModel.avatar.avatarViewModel.image
                )),
                verification: page_header_verification(&header),
                description: d.metadata.description,
                tags: d.microformat.tags,
                banner: header.query_thumbnails(ytq!(.banner.imageBannerViewModel.image)),
                has_shorts: d.has_shorts,
                has_live: d.has_live,
                visitor_data: d.visitor_data,
            }
        } else {
            return Err(ExtractionError::InvalidData("no channel header".into()));
        },
        warnings,
    })
}

fn map_channel_shell<'a>(
    root: &JsonNode<'a>,
    ctx: &MapRespCtx<'_>,
    content: response::channel::MappedChannelContent<'a>,
) -> Result<(MapResult<ChannelHeader>, Option<JsonNode<'a>>), ExtractionError> {
    let header = root
        .query(ytq!(.header))
        .ok_or_else(|| ExtractionError::NotFound {
            id: ctx.id.to_owned(),
            msg: "no header".into(),
        })?;
    let metadata = root
        .query(ytq!(.metadata.channelMetadataRenderer))
        .ok_or_else(|| ExtractionError::NotFound {
            id: ctx.id.to_owned(),
            msg: "no metadata".into(),
        })?
        .deserialize::<response::channel::ChannelMetadataRenderer>()?;
    let microformat = root
        .query(ytq!(.microformat.microformatDataRenderer))
        .ok_or_else(|| ExtractionError::NotFound {
            id: ctx.id.to_owned(),
            msg: "no microformat".into(),
        })?
        .deserialize::<response::channel::MicroformatDataRenderer>()?;
    let visitor_data = ctx.visitor_data(root);

    let channel_data = map_channel(
        MapChannelData {
            header,
            metadata,
            microformat,
            visitor_data: visitor_data.clone(),
            has_shorts: content.has_shorts,
            has_live: content.has_live,
        },
        ctx,
    )?;
    Ok((channel_data, content.list_node))
}

fn combine_channel_data<T>(channel_data: Channel<()>, content: T) -> Channel<T> {
    Channel {
        id: channel_data.id,
        name: channel_data.name,
        handle: channel_data.handle,
        subscriber_count: channel_data.subscriber_count,
        video_count: channel_data.video_count,
        avatar: channel_data.avatar,
        verification: channel_data.verification,
        description: channel_data.description,
        tags: channel_data.tags,
        banner: channel_data.banner,
        has_shorts: channel_data.has_shorts,
        has_live: channel_data.has_live,
        visitor_data: channel_data.visitor_data,
        content,
    }
}

impl MapEndpoint<Channel<Paginator<VideoItem>>> for ChannelEndpoint {
    fn map(
        json: &JsonDoc,
        ctx: &MapRespCtx<'_>,
    ) -> Result<MapResult<Channel<Paginator<VideoItem>>>, ExtractionError> {
        json.with_root(|root| {
            let content = response::channel::map_channel_content(ctx.id, &root, || {
                json_alerts_to_err(ctx.id, &root)
            })?;
            let (channel_data, list_node) = map_channel_shell(&root, ctx, content)?;
            let visitor_data = channel_data.c.visitor_data.clone();

            let (mapped, ctoken) = response::video_item::map_channel_video_items(
                list_node.as_ref(),
                ctx.lang,
                &channel_data.c.clone().into_channel(),
                channel_data.warnings,
            );
            let p = Paginator::new_ext(
                None,
                mapped.c,
                ctoken,
                visitor_data,
                ContinuationEndpoint::Browse,
                false,
            );

            Ok(MapResult {
                c: combine_channel_data(channel_data.c.into_channel(), p),
                warnings: mapped.warnings,
            })
        })
    }
}

impl MapEndpoint<Channel<Paginator<PlaylistItem>>> for ChannelEndpoint {
    fn map(
        json: &JsonDoc,
        ctx: &MapRespCtx<'_>,
    ) -> Result<MapResult<Channel<Paginator<PlaylistItem>>>, ExtractionError> {
        json.with_root(|root| {
            let content = response::channel::map_channel_content(ctx.id, &root, || {
                json_alerts_to_err(ctx.id, &root)
            })?;
            let (channel_data, list_node) = map_channel_shell(&root, ctx, content)?;
            let (mapped, ctoken) = response::video_item::map_channel_playlist_items(
                list_node.as_ref(),
                ctx.lang,
                &channel_data.c.clone().into_channel(),
                channel_data.warnings,
            );
            let p = Paginator::new(None, mapped.c, ctoken);

            Ok(MapResult {
                c: combine_channel_data(channel_data.c.into_channel(), p),
                warnings: mapped.warnings,
            })
        })
    }
}

impl MapEndpoint<ChannelInfo> for ChannelAboutEndpoint {
    fn map(
        json: &JsonDoc,
        ctx: &MapRespCtx<'_>,
    ) -> Result<MapResult<ChannelInfo>, ExtractionError> {
        json.with_root(|root| {
            let lang = Language::En;

            if let Some(endpoints) = root.query(ytq!(.onResponseReceivedEndpoints)) {
                let ep = endpoints
                    .items()
                    .into_iter()
                    .next()
                    .ok_or(ExtractionError::InvalidData("no received endpoint".into()))?;
                let continuations = ep
                    .query(ytq!(
                        .(.appendContinuationItemsAction || .reloadContinuationItemsCommand).continuationItems
                    ))
                    .ok_or(ExtractionError::InvalidData("no aboutChannel data".into()))?;
                let about = continuations
                    .items()
                    .into_iter()
                    .find_map(|node| node.query(ytq!(.aboutChannelRenderer)))
                    .and_then(|node| {
                        node.deserialize::<response::channel::AboutChannelRenderer>()
                            .ok()
                    })
                    .ok_or(ExtractionError::InvalidData("no aboutChannel data".into()))?
                    .metadata
                    .about_channel_view_model;
                let mut warnings = Vec::new();

                let links = about
                    .links
                    .into_iter()
                    .filter_map(|l| {
                        let lv = l.channel_external_link_view_model;
                        if let TextComponent::Web { url, .. } = lv.link {
                            Some((String::from(lv.title), util::sanitize_yt_url(&url)))
                        } else {
                            None
                        }
                    })
                    .collect::<Vec<_>>();

                return Ok(MapResult {
                    c: ChannelInfo {
                        id: about.channel_id,
                        url: about.canonical_channel_url,
                        description: about.description,
                        subscriber_count: about.subscriber_count_text.and_then(|txt| {
                            util::parse_large_numstr_or_warn(&txt, lang, &mut warnings)
                        }),
                        video_count: about
                            .video_count_text
                            .and_then(|txt| util::parse_numeric_or_warn(&txt, &mut warnings)),
                        create_date: about.joined_date_text.and_then(|txt| {
                            timeago::parse_textual_date_or_warn(
                                lang,
                                ctx.utc_offset,
                                &txt,
                                &mut warnings,
                            )
                            .map(OffsetDateTime::date)
                        }),
                        view_count: about
                            .view_count_text
                            .and_then(|txt| util::parse_numeric_or_warn(&txt, &mut warnings)),
                        country: about.country.and_then(|c| util::country_from_name(&c)),
                        links,
                    },
                    warnings,
                });
            }

            if root.query(ytq!(.contents)).is_some() {
                response::channel::map_channel_content(ctx.id, &root, || {
                    json_alerts_to_err(ctx.id, &root)
                })?;
                return Err(ExtractionError::InvalidData(
                    "could not extract aboutData".into(),
                ));
            }

            Err(ExtractionError::InvalidData(
                "could not extract aboutData".into(),
            ))
        })
    }
}

/// Get the continuation token to fetch channel videos in the given order
fn order_ctoken(
    channel_id: &str,
    tab: ChannelVideoTab,
    order: ChannelOrder,
    target_id: &str,
) -> String {
    let mut pb_tab = ProtoBuilder::new();
    pb_tab.string(2, target_id);

    match tab {
        ChannelVideoTab::Videos => match order {
            ChannelOrder::Latest => {
                pb_tab.varint(3, 1);
                pb_tab.varint(4, 4);
            }
            ChannelOrder::Popular => {
                pb_tab.varint(3, 2);
                pb_tab.varint(4, 2);
            }
            ChannelOrder::Oldest => {
                pb_tab.varint(3, 4);
                pb_tab.varint(4, 5);
            }
        },
        ChannelVideoTab::Shorts => match order {
            ChannelOrder::Latest => pb_tab.varint(4, 4),
            ChannelOrder::Popular => pb_tab.varint(4, 2),
            ChannelOrder::Oldest => pb_tab.varint(4, 5),
        },
        ChannelVideoTab::Live => match order {
            ChannelOrder::Latest => pb_tab.varint(5, 12),
            ChannelOrder::Popular => pb_tab.varint(5, 14),
            ChannelOrder::Oldest => pb_tab.varint(5, 13),
        },
    }

    let mut pb_3 = ProtoBuilder::new();
    pb_3.embedded(tab.order_ctoken_id(), pb_tab);

    let mut pb_110 = ProtoBuilder::new();
    pb_110.embedded(3, pb_3);

    let mut pbi = ProtoBuilder::new();
    pbi.embedded(110, pb_110);

    let mut pb_80226972 = ProtoBuilder::new();
    pb_80226972.string(2, channel_id);
    pb_80226972.string(3, &pbi.to_base64());

    let mut pb = ProtoBuilder::new();
    pb.embedded(80_226_972, pb_80226972);

    pb.to_base64()
}

/// Get the continuation token to fetch channel
fn channel_info_ctoken(channel_id: &str, target_id: &str) -> String {
    let mut pb_3 = ProtoBuilder::new();
    pb_3.string(19, target_id);

    let mut pb_110 = ProtoBuilder::new();
    pb_110.embedded(3, pb_3);

    let mut pbi = ProtoBuilder::new();
    pbi.embedded(110, pb_110);

    let mut pb_80226972 = ProtoBuilder::new();
    pb_80226972.string(2, channel_id);
    pb_80226972.string(3, &pbi.to_base64());

    let mut pb = ProtoBuilder::new();
    pb.embedded(80_226_972, pb_80226972);

    pb.to_base64()
}

/// Create a random UUId to build continuation tokens
fn random_target() -> String {
    format!("\n${}", util::random_uuid())
}

#[cfg(test)]
mod tests {
    use std::fs;

    use path_macro::path;
    use rstest::rstest;

    use crate::{
        error::{ExtractionError, UnavailabilityReason},
        model::{paginator::Paginator, Channel, ChannelInfo, PlaylistItem, VideoItem},
        serializer::MapResult,
        util::tests::TESTFILES,
    };

    use super::{
        channel_info_ctoken, order_ctoken, ChannelAboutEndpoint, ChannelEndpoint, MapEndpoint,
        MapRespCtx,
    };
    use crate::{
        json::JsonDoc,
        param::{ChannelOrder, ChannelVideoTab},
    };

    #[rstest]
    #[case::base("videos_base", "UC2DjFE7Xf11URZqWBigcVOQ")]
    #[case::music("videos_music", "UC_vmjW5e1xEHhYjY2a0kK1A")]
    #[case::withshorts("videos_shorts", "UCh8gHdtzO2tXd593_bjErWg")]
    #[case::live("videos_live", "UChs0pSaEoNLV4mevBFGaoKA")]
    #[case::empty("videos_empty", "UCxBa895m48H5idw5li7h-0g")]
    #[case::upcoming("videos_upcoming", "UCcvfHa-GHSOHFAjU0-Ie57A")]
    #[case::richgrid("videos_20221011_richgrid", "UCh8gHdtzO2tXd593_bjErWg")]
    #[case::richgrid2("videos_20221011_richgrid2", "UC2DjFE7Xf11URZqWBigcVOQ")]
    #[case::coachella("videos_20230415_coachella", "UCHF66aWLOxBW4l6VkSrS3cQ")]
    #[case::shorts("shorts", "UCh8gHdtzO2tXd593_bjErWg")]
    #[case::livestreams("livestreams", "UC2DjFE7Xf11URZqWBigcVOQ")]
    #[case::pageheader("shorts_20240129_pageheader", "UCh8gHdtzO2tXd593_bjErWg")]
    #[case::pageheader2("videos_20240324_pageheader2", "UC2DjFE7Xf11URZqWBigcVOQ")]
    #[case::lockup("shorts_20240910_lockup", "UCh8gHdtzO2tXd593_bjErWg")]
    fn map_channel_videos(#[case] name: &str, #[case] id: &str) {
        let json_path = path!(*TESTFILES / "channel" / format!("channel_{name}.json"));
        let json = JsonDoc::new(fs::read_to_string(json_path).unwrap());
        let map_res: MapResult<Channel<Paginator<VideoItem>>> =
            ChannelEndpoint::map(&json, &MapRespCtx::test(id)).unwrap();

        assert!(
            map_res.warnings.is_empty(),
            "deserialization/mapping warnings: {:?}",
            map_res.warnings
        );

        if name == "videos_upcoming" {
            insta::assert_ron_snapshot!(format!("map_channel_{name}"), map_res.c, {
                ".content.items[1:].publish_date" => "[date]",
            });
        } else {
            insta::assert_ron_snapshot!(format!("map_channel_{name}"), map_res.c, {
                ".content.items[].publish_date" => "[date]",
            });
        }
    }

    #[test]
    fn channel_agegate() {
        let json_path = path!(*TESTFILES / "channel" / format!("channel_agegate.json"));
        let json = JsonDoc::new(fs::read_to_string(json_path).unwrap());
        let res: Result<MapResult<Channel<Paginator<VideoItem>>>, ExtractionError> =
            ChannelEndpoint::map(&json, &MapRespCtx::test("UCbfnHqxXs_K3kvaH-WlNlig"));
        if let Err(ExtractionError::Unavailable { reason, msg }) = res {
            assert_eq!(reason, UnavailabilityReason::AgeRestricted);
            assert!(msg.starts_with("Laphroaig Whisky: "));
        } else {
            panic!("invalid res: {res:?}")
        }
    }

    #[rstest]
    #[case::base("base")]
    #[case::lockup("20241109_lockup")]
    fn map_channel_playlists(#[case] name: &str) {
        let json_path = path!(*TESTFILES / "channel" / format!("channel_playlists_{name}.json"));
        let json = JsonDoc::new(fs::read_to_string(json_path).unwrap());
        let map_res: MapResult<Channel<Paginator<PlaylistItem>>> =
            ChannelEndpoint::map(&json, &MapRespCtx::test("UC2DjFE7Xf11URZqWBigcVOQ"))
                .unwrap();

        assert!(
            map_res.warnings.is_empty(),
            "deserialization/mapping warnings: {:?}",
            map_res.warnings
        );
        insta::assert_ron_snapshot!(format!("map_channel_playlists_{name}"), map_res.c);
    }

    #[rstest]
    fn map_channel_info() {
        let json_path = path!(*TESTFILES / "channel" / "channel_info.json");
        let json = JsonDoc::new(fs::read_to_string(json_path).unwrap());
        let map_res: MapResult<ChannelInfo> = ChannelAboutEndpoint::map(
            &json,
            &MapRespCtx::test("UC2DjFE7Xf11U-RZqWBigcVOQ"),
        )
        .unwrap();

        assert!(
            map_res.warnings.is_empty(),
            "deserialization/mapping warnings: {:?}",
            map_res.warnings
        );
        insta::assert_ron_snapshot!("map_channel_info", map_res.c);
    }

    #[test]
    fn t_order_ctoken() {
        let channel_id = "UCXuqSBlHAE6Xw-yeJA0Tunw";

        let videos_popular_token = order_ctoken(
            channel_id,
            ChannelVideoTab::Videos,
            ChannelOrder::Popular,
            "\n$6461d7c8-0000-2040-87aa-089e0827e420",
        );
        assert_eq!(videos_popular_token, "4qmFsgJgEhhVQ1h1cVNCbEhBRTZYdy15ZUpBMFR1bncaRDhnWXdHaTU2TEJJbUNpUTJORFl4WkRkak9DMHdNREF3TFRJd05EQXRPRGRoWVMwd09EbGxNRGd5TjJVME1qQVlBaUFD");

        let shorts_popular_token = order_ctoken(
            channel_id,
            ChannelVideoTab::Shorts,
            ChannelOrder::Popular,
            "\n$64679ffb-0000-26b3-a1bd-582429d2c794",
        );
        assert_eq!(shorts_popular_token, "4qmFsgJkEhhVQ1h1cVNCbEhBRTZYdy15ZUpBMFR1bncaSDhnWXVHaXhTS2hJbUNpUTJORFkzT1dabVlpMHdNREF3TFRJMllqTXRZVEZpWkMwMU9ESTBNamxrTW1NM09UUWdBZyUzRCUzRA%3D%3D");

        let live_popular_token = order_ctoken(
            channel_id,
            ChannelVideoTab::Live,
            ChannelOrder::Popular,
            "\n$64693069-0000-2a1e-8c7d-582429bd5ba8",
        );
        assert_eq!(live_popular_token, "4qmFsgJkEhhVQ1h1cVNCbEhBRTZYdy15ZUpBMFR1bncaSDhnWXVHaXh5S2hJbUNpUTJORFk1TXpBMk9TMHdNREF3TFRKaE1XVXRPR00zWkMwMU9ESTBNamxpWkRWaVlUZ29EZyUzRCUzRA%3D%3D");
    }

    #[test]
    fn t_channel_info_ctoken() {
        let channel_id = "UCh8gHdtzO2tXd593_bjErWg";

        let token = channel_info_ctoken(channel_id, "\n$655b339a-0000-20b9-92dc-582429d254b4");
        assert_eq!(token, "4qmFsgJgEhhVQ2g4Z0hkdHpPMnRYZDU5M19iakVyV2caRDhnWXJHaW1hQVNZS0pEWTFOV0l6TXpsaExUQXdNREF0TWpCaU9TMDVNbVJqTFRVNE1qUXlPV1F5TlRSaU5BJTNEJTNE");
    }
}
