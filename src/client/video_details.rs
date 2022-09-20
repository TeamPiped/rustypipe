use std::convert::TryFrom;

use anyhow::{anyhow, bail, Result};
use reqwest::Method;
use serde::Serialize;

use crate::{
    model::{Channel, ChannelId, Comment, Language, Paginator, RecommendedVideo, VideoDetails},
    serializer::MapResult,
    timeago,
    util::{self, TryRemove},
};

use super::{
    response::{self, IconType, IsLive},
    ClientType, MapResponse, RustyPipeQuery, YTContext,
};

#[derive(Clone, Debug, Serialize)]
struct QVideo {
    context: YTContext,
    /// YouTube video ID
    video_id: String,
    /// Set to true to allow extraction of streams with sensitive content
    content_check_ok: bool,
    /// Probably refers to allowing sensitive content, too
    racy_check_ok: bool,
}

#[derive(Clone, Debug, Serialize)]
struct QVideoCont {
    context: YTContext,
    continuation: String,
}

impl RustyPipeQuery {
    pub async fn video_details(self, video_id: &str) -> Result<VideoDetails> {
        let context = self.get_context(ClientType::Desktop, true).await;
        let request_body = QVideo {
            context,
            video_id: video_id.to_owned(),
            content_check_ok: true,
            racy_check_ok: true,
        };

        self.execute_request::<response::VideoDetails, _, _>(
            ClientType::Desktop,
            "video_details",
            video_id,
            Method::POST,
            "next",
            &request_body,
        )
        .await
    }

    pub async fn video_recommendations(self, ctoken: &str) -> Result<Paginator<RecommendedVideo>> {
        let context = self.get_context(ClientType::Desktop, true).await;
        let request_body = QVideoCont {
            context,
            continuation: ctoken.to_owned(),
        };

        self.execute_request::<response::VideoRecommendations, _, _>(
            ClientType::Desktop,
            "video_recommendations",
            ctoken,
            Method::POST,
            "next",
            &request_body,
        )
        .await
    }

    pub async fn video_comments(self, ctoken: &str) -> Result<Paginator<Comment>> {
        let context = self.get_context(ClientType::Desktop, true).await;
        let request_body = QVideoCont {
            context,
            continuation: ctoken.to_owned(),
        };

        self.execute_request::<response::VideoComments, _, _>(
            ClientType::Desktop,
            "video_comments",
            ctoken,
            Method::POST,
            "next",
            &request_body,
        )
        .await
    }
}

impl MapResponse<VideoDetails> for response::VideoDetails {
    fn map_response(
        self,
        id: &str,
        lang: crate::model::Language,
        _deobf: Option<&crate::deobfuscate::Deobfuscator>,
    ) -> Result<MapResult<VideoDetails>> {
        let mut warnings = Vec::new();

        let video_id = self.current_video_endpoint.watch_endpoint.video_id;
        if id != video_id {
            bail!("got wrong playlist id {}, expected {}", video_id, id);
        }

        let mut primary_results = self
            .contents
            .two_column_watch_next_results
            .results
            .results
            .contents;
        warnings.append(&mut primary_results.warnings);

        let mut primary_info = None;
        let mut secondary_info = None;
        let mut comment_count_section = None;
        let mut comment_ctoken_section = None;

        primary_results.c.into_iter().for_each(|r| match r {
            response::video_details::VideoResultsItem::VideoPrimaryInfoRenderer { .. } => {
                primary_info = Some(r);
            }
            response::video_details::VideoResultsItem::VideoSecondaryInfoRenderer { .. } => {
                secondary_info = Some(r);
            }
            response::video_details::VideoResultsItem::ItemSectionRenderer(section) => {
                match section {
                    response::video_details::ItemSection::CommentsEntryPoint { mut contents } => {
                        comment_count_section = contents.try_swap_remove(0);
                    }
                    response::video_details::ItemSection::CommentItemSection { mut contents } => {
                        comment_ctoken_section = contents.try_swap_remove(0);
                    }
                    response::video_details::ItemSection::None => {},
                }
            }
            response::video_details::VideoResultsItem::None => {}
        });

        let (title, view_count, like_count, publish_date, publish_date_txt, is_live) =
            match primary_info {
                Some(response::video_details::VideoResultsItem::VideoPrimaryInfoRenderer {
                    title,
                    view_count,
                    video_actions,
                    date_text,
                }) => {
                    let like_btn = video_actions
                    .menu_renderer
                    .top_level_buttons
                    .into_iter()
                    .find_map(|button| {
                        let btn = match button {
                            response::video_details::TopLevelButton::ToggleButtonRenderer(btn) => btn,
                            response::video_details::TopLevelButton::SegmentedLikeDislikeButtonRenderer { like_button } => like_button.toggle_button_renderer,
                        };
                        match btn.default_icon.icon_type {
                            IconType::Like => Some(btn),
                            _ => None
                        }
                    });
                    (
                        title,
                        util::parse_numeric(&view_count.video_view_count_renderer.view_count)?,
                        // accessibility_data contains no digits if the like count is hidden,
                        // so we ignore parse errors here for now
                        like_btn.and_then(|btn| util::parse_numeric(&btn.accessibility_data).ok()),
                        timeago::parse_textual_date_or_warn(lang, &date_text, &mut warnings),
                        date_text,
                        view_count.video_view_count_renderer.is_live,
                    )
                }
                _ => bail!("could not find primary_info"),
            };

        /*
        TODO: use large number parser for this
        let comment_count = comment_count_section.and_then(|s| {
            util::parse_numeric_or_warn::<u32>(
                &s.comments_entry_point_header_renderer.comment_count,
                &mut warnings,
            )
        });*/

        let comment_ctoken = comment_ctoken_section.map(|s| {
            s.continuation_item_renderer
                .continuation_endpoint
                .continuation_command
                .token
        });

        let (owner, description, is_ccommons) = match secondary_info {
            Some(response::video_details::VideoResultsItem::VideoSecondaryInfoRenderer {
                owner,
                description,
                metadata_row_container,
            }) => {
                let is_ccommons = metadata_row_container
                    .map(|c| {
                        c.metadata_row_container_renderer.rows.iter().any(|cr| {
                            cr.metadata_row_renderer.contents.iter().any(|links| {
                                links.iter().any(|link| match link {
                                    crate::serializer::text::TextLink::Web { text: _, url } => {
                                        url == "https://www.youtube.com/t/creative_commons"
                                    }
                                    _ => false,
                                })
                            })
                        })
                    })
                    .unwrap_or_default();

                (owner.video_owner_renderer, description, is_ccommons)
            }
            _ => bail!("could not find secondary_info"),
        };

        let (channel_id, channel_name) = match owner.title {
            crate::serializer::text::TextLink::Browse {
                text,
                page_type,
                browse_id,
            } => match page_type {
                crate::serializer::text::PageType::Channel => (browse_id, text),
                _ => bail!("invalid channel link type"),
            },
            _ => bail!("invalid channel link"),
        };

        let recommended = self
            .contents
            .two_column_watch_next_results
            .secondary_results
            .map(|sr| {
                sr.secondary_results.results.map(|r| {
                    let mut res = map_recommendations(r, lang);
                    warnings.append(&mut res.warnings);
                    res.c
                })
            })
            .flatten()
            .unwrap_or_default();

        let mut engagement_panels = self.engagement_panels;
        warnings.append(&mut engagement_panels.warnings);

        let mut chapter_panel = None;
        let mut comment_panel = None;
        engagement_panels.c.into_iter().for_each(|panel| match panel.engagement_panel_section_list_renderer {
            response::video_details::EngagementPanelRenderer::EngagementPanelMacroMarkersDescriptionChapters { content } => {
                chapter_panel = Some(content);
            },
            response::video_details::EngagementPanelRenderer::EngagementPanelCommentsSection { header } => {
                comment_panel = Some(header);
            },
            response::video_details::EngagementPanelRenderer::None => {},
        });

        let latest_comments_ctoken = comment_panel.and_then(|comments| {
            let mut items = comments
                .engagement_panel_title_header_renderer
                .menu
                .sort_filter_sub_menu_renderer
                .sub_menu_items;
            items
                .try_swap_remove(1)
                .map(|c| c.service_endpoint.continuation_command.token)
        });

        Ok(MapResult {
            c: VideoDetails {
                id: video_id,
                title,
                description,
                channel: Channel {
                    id: channel_id,
                    name: channel_name,
                    avatar: owner.thumbnail.into(),
                    verification: owner.badges.into(),
                    subscriber_count: None,
                    subscriber_count_txt: owner.subscriber_count_text,
                },
                view_count,
                like_count,
                publish_date,
                publish_date_txt,
                is_live,
                is_ccommons,
                recommended,
                top_comments: Paginator {
                    count: None,
                    items: Vec::new(),
                    ctoken: comment_ctoken,
                },
                latest_comments: Paginator {
                    count: None,
                    items: Vec::new(),
                    ctoken: latest_comments_ctoken,
                },
            },
            warnings,
        })
    }
}

impl MapResponse<Paginator<RecommendedVideo>> for response::VideoRecommendations {
    fn map_response(
        self,
        _id: &str,
        lang: crate::model::Language,
        _deobf: Option<&crate::deobfuscate::Deobfuscator>,
    ) -> Result<MapResult<Paginator<RecommendedVideo>>> {
        let mut endpoints = self.on_response_received_endpoints;
        let cont = some_or_bail!(
            endpoints.try_swap_remove(0),
            Err(anyhow!("no continuation endpoint"))
        );

        Ok(map_recommendations(
            cont.append_continuation_items_action.continuation_items,
            lang,
        ))
    }
}

impl MapResponse<Paginator<Comment>> for response::VideoComments {
    fn map_response(
        self,
        _id: &str,
        lang: crate::model::Language,
        _deobf: Option<&crate::deobfuscate::Deobfuscator>,
    ) -> Result<MapResult<Paginator<Comment>>> {
        let mut warnings = self.on_response_received_endpoints.warnings;

        let mut comments = Vec::new();
        let mut comment_count = None;
        let mut ctoken = None;

        self.on_response_received_endpoints
            .c
            .into_iter()
            .for_each(|citem| {
                let mut items = citem.append_continuation_items_action.continuation_items;
                warnings.append(&mut items.warnings);
                items.c.into_iter().for_each(|item| match item {
                    response::video_details::CommentListItem::CommentThreadRenderer {
                        comment,
                        replies,
                        rendering_priority,
                    } => {
                        let mut res = map_comment(
                            comment.comment_renderer,
                            Some(replies),
                            rendering_priority,
                            lang,
                        );
                        comments.push(res.c);
                        warnings.append(&mut res.warnings)
                    }
                    response::video_details::CommentListItem::CommentRenderer(comment) => {
                        let mut res = map_comment(
                            comment,
                            None,
                            response::video_details::CommentPriority::RenderingPriorityUnknown,
                            lang,
                        );
                        comments.push(res.c);
                        warnings.append(&mut res.warnings)
                    }
                    response::video_details::CommentListItem::ContinuationItemRenderer {
                        continuation_endpoint,
                    } => {
                        ctoken = Some(continuation_endpoint.continuation_command.token);
                    }
                    response::video_details::CommentListItem::CommentsHeaderRenderer {
                        count_text,
                    } => {
                        comment_count = count_text.and_then(|txt| {
                            util::parse_numeric_or_warn::<u32>(&txt, &mut warnings)
                        });
                    }
                });
            });

        Ok(MapResult {
            c: Paginator {
                count: comment_count,
                items: comments,
                ctoken,
            },
            warnings,
        })
    }
}

fn map_recommendations(
    r: MapResult<Vec<response::VideoListItem<response::video_details::RecommendedVideo>>>,
    lang: Language,
) -> MapResult<Paginator<RecommendedVideo>> {
    let mut warnings = r.warnings;
    let mut ctoken = None;

    let items =
        r.c.into_iter()
            .filter_map(|item| match item {
                response::VideoListItem::GridVideoRenderer { video } => {
                    match ChannelId::try_from(video.channel) {
                        Ok(channel) => Some(RecommendedVideo {
                            id: video.video_id,
                            title: video.title,
                            length: video.length_text.and_then(|txt| {
                                util::parse_video_length_or_warn(&txt, &mut warnings)
                            }),
                            thumbnail: video.thumbnail.into(),
                            channel: Channel {
                                id: channel.id,
                                name: channel.name,
                                avatar: video.channel_thumbnail.into(),
                                verification: video.owner_badges.into(),
                                subscriber_count: None,
                                subscriber_count_txt: None,
                            },
                            publish_date: video.published_time_text.as_ref().and_then(|txt| {
                                timeago::parse_timeago_or_warn(lang, txt, &mut warnings)
                            }),
                            publish_date_txt: video.published_time_text,
                            view_count: util::parse_numeric_or_warn(
                                &video.view_count_text,
                                &mut warnings,
                            ),
                            is_live: video.badges.is_live(),
                        }),
                        Err(e) => {
                            warnings.push(e.to_string());
                            None
                        }
                    }
                }
                response::VideoListItem::ContinuationItemRenderer {
                    continuation_endpoint,
                } => {
                    ctoken = Some(continuation_endpoint.continuation_command.token);
                    None
                }
                response::VideoListItem::None => None,
            })
            .collect::<Vec<_>>();

    MapResult {
        c: Paginator {
            count: None,
            items,
            ctoken,
        },
        warnings,
    }
}

fn map_comment(
    c: response::video_details::CommentRenderer,
    replies: Option<response::video_details::Replies>,
    priority: response::video_details::CommentPriority,
    lang: Language,
) -> MapResult<Comment> {
    let mut warnings = Vec::new();

    let mut reply_ctoken = None;
    let replies = replies.map(|replies| {
        replies
            .comment_replies_renderer
            .contents
            .into_iter()
            .filter_map(|item| match item {
                response::video_details::CommentListItem::CommentRenderer(comment) => {
                    let mut res = map_comment(
                        comment,
                        None,
                        response::video_details::CommentPriority::default(),
                        lang,
                    );
                    warnings.append(&mut res.warnings);
                    Some(res.c)
                }
                response::video_details::CommentListItem::ContinuationItemRenderer {
                    continuation_endpoint,
                } => {
                    reply_ctoken = Some(continuation_endpoint.continuation_command.token);
                    None
                }
                _ => None,
            })
            .collect::<Vec<_>>()
    });

    MapResult {
        c: Comment {
            id: c.comment_id,
            text: c.content_text,
            author: match (c.author_endpoint, c.author_text) {
                (Some(aep), Some(name)) => Some(Channel {
                    id: aep.browse_endpoint.browse_id,
                    name,
                    avatar: c.author_thumbnail.into(),
                    verification: c
                        .author_comment_badge
                        .map(|b| b.author_comment_badge_renderer.icon.into())
                        .unwrap_or_default(),
                    subscriber_count: None,
                    subscriber_count_txt: None,
                }),
                _ => None,
            },
            publish_date: timeago::parse_timeago_or_warn(
                lang,
                &c.published_time_text,
                &mut warnings,
            ),
            publish_date_txt: c.published_time_text,
            like_count: util::parse_numeric_or_warn(
                &c.action_buttons
                    .comment_action_buttons_renderer
                    .like_button
                    .toggle_button_renderer
                    .accessibility_data,
                &mut warnings,
            ),
            reply_count: c.reply_count,
            replies: replies
                .map(|items| Paginator {
                    count: Some(c.reply_count),
                    items,
                    ctoken: reply_ctoken,
                })
                .unwrap_or_default(),
            by_owner: c.author_is_channel_owner,
            pinned: priority
                == response::video_details::CommentPriority::RenderingPriorityPinnedComment,
            hearted: c
                .action_buttons
                .comment_action_buttons_renderer
                .creator_heart
                .map(|h| h.creator_heart_renderer.is_hearted)
                .unwrap_or_default(),
        },
        warnings,
    }
}

#[cfg(test)]
mod tests {
    use crate::client::RustyPipe;

    #[test_log::test(tokio::test)]
    async fn get_video_details() {
        let rp = RustyPipe::builder().strict().build();
        let details = rp.query().video_details("HRKu0cvrr_o").await.unwrap();

        dbg!(&details);
    }

    #[test_log::test(tokio::test)]
    async fn get_video_recommendations() {
        let rp = RustyPipe::builder().strict().build();
        let details = rp.query().video_details("ZeerrnuLi5E").await.unwrap();
        let rec = rp
            .query()
            .video_recommendations(&details.recommended.ctoken.unwrap())
            .await
            .unwrap();

        dbg!(&rec);
    }

    #[test_log::test(tokio::test)]
    async fn get_video_comments() {
        let rp = RustyPipe::builder().strict().build();
        let details = rp.query().video_details("ZeerrnuLi5E").await.unwrap();
        let rec = rp
            .query()
            .video_comments(&details.top_comments.ctoken.unwrap())
            .await
            .unwrap();

        dbg!(&rec);
    }
}
