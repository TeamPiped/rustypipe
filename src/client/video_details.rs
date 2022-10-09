use std::convert::TryFrom;

use serde::Serialize;

use crate::{
    error::{Error, ExtractionError},
    model::{
        ChannelId, ChannelTag, Chapter, Comment, Language, Paginator, RecommendedVideo,
        VideoDetails,
    },
    serializer::MapResult,
    timeago,
    util::{self, TryRemove},
};

use super::{
    response::{self, IconType, IsLive, IsShort},
    ClientType, MapResponse, QContinuation, RustyPipeQuery, YTContext,
};

#[derive(Debug, Serialize)]
struct QVideo<'a> {
    context: YTContext,
    /// YouTube video ID
    video_id: &'a str,
    /// Set to true to allow extraction of streams with sensitive content
    content_check_ok: bool,
    /// Probably refers to allowing sensitive content, too
    racy_check_ok: bool,
}

impl RustyPipeQuery {
    pub async fn video_details(self, video_id: &str) -> Result<VideoDetails, Error> {
        let context = self.get_context(ClientType::Desktop, true).await;
        let request_body = QVideo {
            context,
            video_id,
            content_check_ok: true,
            racy_check_ok: true,
        };

        self.execute_request::<response::VideoDetails, _, _>(
            ClientType::Desktop,
            "video_details",
            video_id,
            "next",
            &request_body,
        )
        .await
    }

    pub async fn video_recommendations(
        self,
        ctoken: &str,
    ) -> Result<Paginator<RecommendedVideo>, Error> {
        let context = self.get_context(ClientType::Desktop, true).await;
        let request_body = QContinuation {
            context,
            continuation: ctoken,
        };

        self.execute_request::<response::VideoRecommendations, _, _>(
            ClientType::Desktop,
            "video_recommendations",
            ctoken,
            "next",
            &request_body,
        )
        .await
    }

    pub async fn video_comments(self, ctoken: &str) -> Result<Paginator<Comment>, Error> {
        let context = self.get_context(ClientType::Desktop, true).await;
        let request_body = QContinuation {
            context,
            continuation: ctoken,
        };

        self.execute_request::<response::VideoComments, _, _>(
            ClientType::Desktop,
            "video_comments",
            ctoken,
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
    ) -> Result<MapResult<VideoDetails>, ExtractionError> {
        let mut warnings = Vec::new();

        let video_id = self.current_video_endpoint.watch_endpoint.video_id;
        if id != video_id {
            return Err(ExtractionError::WrongResult(format!(
                "got wrong playlist id {}, expected {}",
                video_id, id
            )));
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
                    response::video_details::ItemSection::None => {}
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
                        // view count field contains `No views` if the view count is zero
                        util::parse_numeric(&view_count.video_view_count_renderer.view_count)
                            .unwrap_or_default(),
                        // accessibility_data contains no digits if the like count is hidden,
                        // so we ignore parse errors here for now
                        like_btn.and_then(|btn| util::parse_numeric(&btn.accessibility_data).ok()),
                        timeago::parse_textual_date_or_warn(lang, &date_text, &mut warnings),
                        date_text,
                        view_count.video_view_count_renderer.is_live,
                    )
                }
                _ => {
                    return Err(ExtractionError::InvalidData(
                        "could not find primary_info".into(),
                    ))
                }
            };

        let comment_count = comment_count_section.and_then(|s| {
            util::parse_large_numstr::<u64>(
                &s.comments_entry_point_header_renderer.comment_count,
                lang,
            )
        });

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
                attributed_description,
                metadata_row_container,
            }) => {
                let is_ccommons = metadata_row_container
                    .map(|c| {
                        c.metadata_row_container_renderer.rows.iter().any(|cr| {
                            cr.metadata_row_renderer.contents.iter().any(|links| {
                                links.0.iter().any(|link| match link {
                                    crate::serializer::text::TextComponent::Web {
                                        text: _,
                                        url,
                                    } => url == "https://www.youtube.com/t/creative_commons",
                                    _ => false,
                                })
                            })
                        })
                    })
                    .unwrap_or_default();

                let desc = description
                    .or(attributed_description)
                    .unwrap_or_default()
                    .into();

                (owner.video_owner_renderer, desc, is_ccommons)
            }
            _ => {
                return Err(ExtractionError::InvalidData(
                    "could not find secondary_info".into(),
                ))
            }
        };

        let (channel_id, channel_name) = match owner.title {
            crate::serializer::text::TextComponent::Browse {
                text,
                page_type,
                browse_id,
            } => match page_type {
                crate::serializer::text::PageType::Channel => (browse_id, text),
                _ => {
                    return Err(ExtractionError::InvalidData(
                        "invalid channel link type".into(),
                    ))
                }
            },
            _ => return Err(ExtractionError::InvalidData("invalid channel link".into())),
        };

        let recommended = self
            .contents
            .two_column_watch_next_results
            .secondary_results
            .and_then(|sr| {
                sr.secondary_results.results.map(|r| {
                    let mut res = map_recommendations(r, lang);
                    warnings.append(&mut res.warnings);
                    res.c
                })
            })
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

        let chapters = chapter_panel
            .map(|chapters| {
                let mut content = chapters.macro_markers_list_renderer.contents;
                warnings.append(&mut content.warnings);
                content
                    .c
                    .into_iter()
                    .map(|item| Chapter {
                        title: item.macro_markers_list_item_renderer.title,
                        position: item
                            .macro_markers_list_item_renderer
                            .on_tap
                            .watch_endpoint
                            .start_time_seconds,
                        thumbnail: item.macro_markers_list_item_renderer.thumbnail.into(),
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

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
                channel: ChannelTag {
                    id: channel_id,
                    name: channel_name,
                    avatar: owner.thumbnail.into(),
                    verification: owner.badges.into(),
                    subscriber_count: owner
                        .subscriber_count_text
                        .and_then(|txt| util::parse_large_numstr(&txt, lang)),
                },
                view_count,
                like_count,
                publish_date,
                publish_date_txt,
                is_live,
                is_ccommons,
                chapters,
                recommended,
                top_comments: Paginator::new(comment_count, Vec::new(), comment_ctoken),
                latest_comments: Paginator::new(comment_count, Vec::new(), latest_comments_ctoken),
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
    ) -> Result<MapResult<Paginator<RecommendedVideo>>, ExtractionError> {
        let mut endpoints = self.on_response_received_endpoints;
        let cont = some_or_bail!(
            endpoints.try_swap_remove(0),
            Err(ExtractionError::InvalidData(
                "no continuation endpoint".into()
            ))
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
    ) -> Result<MapResult<Paginator<Comment>>, ExtractionError> {
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
                            util::parse_numeric_or_warn::<u64>(&txt, &mut warnings)
                        });
                    }
                });
            });

        Ok(MapResult {
            c: Paginator::new(comment_count, comments, ctoken),
            warnings,
        })
    }
}

fn map_recommendations(
    r: MapResult<Vec<response::VideoListItem>>,
    lang: Language,
) -> MapResult<Paginator<RecommendedVideo>> {
    let mut warnings = r.warnings;
    let mut ctoken = None;

    let items =
        r.c.into_iter()
            .filter_map(|item| match item {
                response::VideoListItem::CompactVideoRenderer(video) => {
                    match ChannelId::try_from(video.channel) {
                        Ok(channel) => Some(RecommendedVideo {
                            id: video.video_id,
                            title: video.title,
                            length: video.length_text.and_then(|txt| {
                                util::parse_video_length_or_warn(&txt, &mut warnings)
                            }),
                            thumbnail: video.thumbnail.into(),
                            channel: ChannelTag {
                                id: channel.id,
                                name: channel.name,
                                avatar: video.channel_thumbnail.into(),
                                verification: video.owner_badges.into(),
                                subscriber_count: None,
                            },
                            publish_date: video.published_time_text.as_ref().and_then(|txt| {
                                timeago::parse_timeago_or_warn(lang, txt, &mut warnings)
                            }),
                            publish_date_txt: video.published_time_text,
                            view_count: video
                                .view_count_text
                                .map(|txt| util::parse_numeric(&txt).unwrap_or_default()),
                            is_live: video.badges.is_live(),
                            is_short: video.thumbnail_overlays.is_short(),
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
                _ => None,
            })
            .collect::<Vec<_>>();

    MapResult {
        c: Paginator::new(None, items, ctoken),
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
            text: c.content_text.into(),
            author: match (c.author_endpoint, c.author_text) {
                (Some(aep), Some(name)) => Some(ChannelTag {
                    id: aep.browse_endpoint.browse_id,
                    name,
                    avatar: c.author_thumbnail.into(),
                    verification: c
                        .author_comment_badge
                        .map(|b| b.author_comment_badge_renderer.icon.into())
                        .unwrap_or_default(),
                    subscriber_count: None,
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
                .map(|items| Paginator::new(Some(c.reply_count), items, reply_ctoken))
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
    use std::{fs::File, io::BufReader, path::Path};

    use chrono::Datelike;
    use rstest::rstest;

    use crate::{
        client::{response, MapResponse, RustyPipe},
        model::{richtext::ToPlaintext, Language, Verification},
    };

    #[tokio::test]
    async fn get_video_details() {
        let rp = RustyPipe::builder().strict().build();
        let details = rp.query().video_details("ZeerrnuLi5E").await.unwrap();

        // dbg!(&details);

        assert_eq!(details.id, "ZeerrnuLi5E");
        assert_eq!(details.title, "aespa 에스파 'Black Mamba' MV");
        let desc = details.description.to_plaintext();
        assert!(
            desc.contains("Listen and download aespa's debut single \"Black Mamba\""),
            "bad description: {}",
            desc
        );

        assert_eq!(details.channel.id, "UCEf_Bc-KVd7onSeifS3py9g");
        assert_eq!(details.channel.name, "SMTOWN");
        assert!(!details.channel.avatar.is_empty(), "no channel avatars");
        assert_eq!(details.channel.verification, Verification::Verified);
        assert!(
            details.channel.subscriber_count.unwrap() > 30000000,
            "expected >30M subs, got {}",
            details.channel.subscriber_count.unwrap()
        );

        assert!(
            details.view_count > 232000000,
            "expected > 232M views, got {}",
            details.view_count
        );
        assert!(
            details.like_count.unwrap() > 4000000,
            "expected > 4M likes, got {}",
            details.like_count.unwrap()
        );

        let date = details.publish_date.unwrap();
        assert_eq!(date.year(), 2020);
        assert_eq!(date.month(), 11);
        assert_eq!(date.day(), 17);

        assert!(!details.is_live);
        assert!(!details.is_ccommons);

        assert!(!details.recommended.items.is_empty());
        assert!(!details.recommended.is_exhausted());

        assert!(
            details.top_comments.count.unwrap() > 700000,
            "expected > 700K comments, got {}",
            details.top_comments.count.unwrap()
        );
        assert!(!details.top_comments.is_exhausted());
        assert!(!details.latest_comments.is_exhausted());
    }

    #[tokio::test]
    async fn get_video_details_music() {
        let rp = RustyPipe::builder().strict().build();
        let details = rp.query().video_details("XuM2onMGvTI").await.unwrap();

        // dbg!(&details);

        assert_eq!(details.id, "XuM2onMGvTI");
        assert_eq!(details.title, "Gäa");
        let desc = details.description.to_plaintext();
        assert!(desc.contains("Gäa · Oonagh"), "bad description: {}", desc);

        assert_eq!(details.channel.id, "UCVGvnqB-5znqPSbMGlhF4Pw");
        assert_eq!(details.channel.name, "Sentamusic");
        assert!(!details.channel.avatar.is_empty(), "no channel avatars");
        assert_eq!(details.channel.verification, Verification::Artist);
        assert!(
            details.channel.subscriber_count.unwrap() > 33000,
            "expected >33K subs, got {}",
            details.channel.subscriber_count.unwrap()
        );

        assert!(
            details.view_count > 20309,
            "expected > 20309 views, got {}",
            details.view_count
        );
        assert!(
            details.like_count.unwrap() > 145,
            "expected > 145 likes, got {}",
            details.like_count.unwrap()
        );

        let date = details.publish_date.unwrap();
        assert_eq!(date.year(), 2020);
        assert_eq!(date.month(), 8);
        assert_eq!(date.day(), 6);

        assert!(!details.is_live);
        assert!(!details.is_ccommons);

        assert!(!details.recommended.items.is_empty());
        assert!(!details.recommended.is_exhausted());

        // Comments are disabled for this video
        assert_eq!(details.top_comments.count, Some(0));
        assert_eq!(details.latest_comments.count, Some(0));
        assert!(details.top_comments.is_empty());
        assert!(details.latest_comments.is_empty());
    }

    #[tokio::test]
    async fn get_video_details_ccommons() {
        let rp = RustyPipe::builder().strict().build();
        let details = rp.query().video_details("0rb9CfOvojk").await.unwrap();

        // dbg!(&details);

        assert_eq!(details.id, "0rb9CfOvojk");
        assert_eq!(
            details.title,
            "BahnMining - Pünktlichkeit ist eine Zier (David Kriesel)"
        );
        let desc = details.description.to_plaintext();
        assert!(
            desc.contains("Seit Anfang 2019 hat David jeden einzelnen Halt jeder einzelnen Zugfahrt auf jedem einzelnen Fernbahnhof in ganz Deutschland"),
            "bad description: {}",
            desc
        );

        assert_eq!(details.channel.id, "UC2TXq_t06Hjdr2g_KdKpHQg");
        assert_eq!(details.channel.name, "media.ccc.de");
        assert!(!details.channel.avatar.is_empty(), "no channel avatars");
        assert_eq!(details.channel.verification, Verification::None);
        assert!(
            details.channel.subscriber_count.unwrap() > 170000,
            "expected >170K subs, got {}",
            details.channel.subscriber_count.unwrap()
        );

        assert!(
            details.view_count > 2517358,
            "expected > 2517358 views, got {}",
            details.view_count
        );
        assert!(
            details.like_count.unwrap() > 52330,
            "expected > 52330 likes, got {}",
            details.like_count.unwrap()
        );

        let date = details.publish_date.unwrap();
        assert_eq!(date.year(), 2019);
        assert_eq!(date.month(), 12);
        assert_eq!(date.day(), 29);

        assert!(!details.is_live);
        assert!(details.is_ccommons);

        assert!(!details.recommended.items.is_empty());
        assert!(!details.recommended.is_exhausted());

        assert!(
            details.top_comments.count.unwrap() > 2199,
            "expected > 2199 comments, got {}",
            details.top_comments.count.unwrap()
        );
        assert!(!details.top_comments.is_exhausted());
        assert!(!details.latest_comments.is_exhausted());
    }

    #[tokio::test]
    async fn get_video_details_chapters() {
        let rp = RustyPipe::builder().strict().build();
        let details = rp.query().video_details("nFDBxBUfE74").await.unwrap();

        // dbg!(&details);

        assert_eq!(details.id, "nFDBxBUfE74");
        assert_eq!(details.title, "The Prepper PC");
        let desc = details.description.to_plaintext();
        assert!(
            desc.contains("These days, you can game almost anywhere on the planet, anytime. But what if that planet was in the middle of an apocalypse"),
            "bad description: {}",
            desc
        );

        assert_eq!(details.channel.id, "UCXuqSBlHAE6Xw-yeJA0Tunw");
        assert_eq!(details.channel.name, "Linus Tech Tips");
        assert!(!details.channel.avatar.is_empty(), "no channel avatars");
        assert_eq!(details.channel.verification, Verification::Verified);
        assert!(
            details.channel.subscriber_count.unwrap() > 14700000,
            "expected >14.7M subs, got {}",
            details.channel.subscriber_count.unwrap()
        );

        assert!(
            details.view_count > 1157262,
            "expected > 1157262 views, got {}",
            details.view_count
        );
        assert!(
            details.like_count.unwrap() > 54670,
            "expected > 54670 likes, got {}",
            details.like_count.unwrap()
        );

        let date = details.publish_date.unwrap();
        assert_eq!(date.year(), 2022);
        assert_eq!(date.month(), 9);
        assert_eq!(date.day(), 15);

        assert!(!details.is_live);
        assert!(!details.is_ccommons);

        insta::assert_ron_snapshot!(details.chapters, {
            "[].thumbnail" => insta::dynamic_redaction(move |value, _path| {
                assert!(!value.as_slice().unwrap().is_empty());
                "[ok]"
            }),
        }, @r###"
        [
          Chapter(
            title: "Intro",
            position: 0,
            thumbnail: "[ok]",
          ),
          Chapter(
            title: "The PC Built for Super Efficiency",
            position: 42,
            thumbnail: "[ok]",
          ),
          Chapter(
            title: "Our BURIAL ENCLOSURE?!",
            position: 161,
            thumbnail: "[ok]",
          ),
          Chapter(
            title: "Our Power Solution (Thanks Jackery!)",
            position: 211,
            thumbnail: "[ok]",
          ),
          Chapter(
            title: "Diggin\' Holes",
            position: 287,
            thumbnail: "[ok]",
          ),
          Chapter(
            title: "Colonoscopy?",
            position: 330,
            thumbnail: "[ok]",
          ),
          Chapter(
            title: "Diggin\' like a man",
            position: 424,
            thumbnail: "[ok]",
          ),
          Chapter(
            title: "The world\'s worst woodsman",
            position: 509,
            thumbnail: "[ok]",
          ),
          Chapter(
            title: "Backyard cable management",
            position: 543,
            thumbnail: "[ok]",
          ),
          Chapter(
            title: "Time to bury this boy",
            position: 602,
            thumbnail: "[ok]",
          ),
          Chapter(
            title: "Solar Power Generation",
            position: 646,
            thumbnail: "[ok]",
          ),
          Chapter(
            title: "Issues",
            position: 697,
            thumbnail: "[ok]",
          ),
          Chapter(
            title: "First Play Test",
            position: 728,
            thumbnail: "[ok]",
          ),
          Chapter(
            title: "Conclusion",
            position: 800,
            thumbnail: "[ok]",
          ),
        ]
        "###);

        assert!(!details.recommended.items.is_empty());
        assert!(!details.recommended.is_exhausted());

        assert!(
            details.top_comments.count.unwrap() > 3199,
            "expected > 3199 comments, got {}",
            details.top_comments.count.unwrap()
        );
        assert!(!details.top_comments.is_exhausted());
        assert!(!details.latest_comments.is_exhausted());
    }

    #[tokio::test]
    async fn get_video_details_live() {
        let rp = RustyPipe::builder().strict().build();
        let details = rp.query().video_details("86YLFOog4GM").await.unwrap();

        // dbg!(&details);

        assert_eq!(details.id, "86YLFOog4GM");
        assert_eq!(
            details.title,
            "🌎 Nasa Live Stream  - Earth From Space :  Live Views from the ISS"
        );
        let desc = details.description.to_plaintext();
        assert!(
            desc.contains("Live NASA - Views Of Earth from Space"),
            "bad description: {}",
            desc
        );

        assert_eq!(details.channel.id, "UCakgsb0w7QB0VHdnCc-OVEA");
        assert_eq!(details.channel.name, "Space Videos");
        assert!(!details.channel.avatar.is_empty(), "no channel avatars");
        assert_eq!(details.channel.verification, Verification::Verified);
        assert!(
            details.channel.subscriber_count.unwrap() > 5500000,
            "expected >5.5M subs, got {}",
            details.channel.subscriber_count.unwrap()
        );

        assert!(
            details.view_count > 10,
            "expected > 10 views, got {}",
            details.view_count
        );
        assert!(
            details.like_count.unwrap() > 872290,
            "expected > 872290 likes, got {}",
            details.like_count.unwrap()
        );

        let date = details.publish_date.unwrap();
        assert_eq!(date.year(), 2021);
        assert_eq!(date.month(), 9);
        assert_eq!(date.day(), 23);

        assert!(details.is_live);
        assert!(!details.is_ccommons);

        assert!(!details.recommended.items.is_empty());
        assert!(!details.recommended.is_exhausted());

        // No comments because livestream
        assert_eq!(details.top_comments.count, Some(0));
        assert_eq!(details.latest_comments.count, Some(0));
        assert!(details.top_comments.is_empty());
        assert!(details.latest_comments.is_empty());
    }

    #[tokio::test]
    async fn get_video_details_agegate() {
        let rp = RustyPipe::builder().strict().build();
        let details = rp.query().video_details("HRKu0cvrr_o").await.unwrap();

        // dbg!(&details);

        assert_eq!(details.id, "HRKu0cvrr_o");
        assert_eq!(
            details.title,
            "AlphaOmegaSin Fanboy Logic: Likes/Dislikes Disabled = Point Invalid Lol wtf?"
        );
        insta::assert_ron_snapshot!(details.description, @"RichText([])");

        assert_eq!(details.channel.id, "UCQT2yul0lr6Ie9qNQNmw-sg");
        assert_eq!(details.channel.name, "PrinceOfFALLEN");
        assert!(!details.channel.avatar.is_empty(), "no channel avatars");
        assert_eq!(details.channel.verification, Verification::None);
        assert!(
            details.channel.subscriber_count.unwrap() > 1400,
            "expected >1400 subs, got {}",
            details.channel.subscriber_count.unwrap()
        );

        assert!(
            details.view_count > 200,
            "expected > 200 views, got {}",
            details.view_count
        );
        assert!(details.like_count.is_none(), "like count not hidden");

        let date = details.publish_date.unwrap();
        assert_eq!(date.year(), 2019);
        assert_eq!(date.month(), 1);
        assert_eq!(date.day(), 2);

        assert!(!details.is_live);
        assert!(!details.is_ccommons);

        // No recommendations because agegate
        assert_eq!(details.recommended.count, Some(0));
        assert!(details.recommended.items.is_empty());
    }

    #[tokio::test]
    async fn get_video_recommendations() {
        let rp = RustyPipe::builder().strict().build();
        let details = rp.query().video_details("ZeerrnuLi5E").await.unwrap();
        let next_recommendations = details.recommended.next(rp.query()).await.unwrap().unwrap();
        dbg!(&next_recommendations);

        assert!(
            next_recommendations.items.len() > 10,
            "expected > 10 next recommendations, got {}",
            next_recommendations.items.len()
        );
        assert!(!next_recommendations.is_exhausted());
    }

    #[tokio::test]
    async fn get_video_comments() {
        let rp = RustyPipe::builder().strict().build();
        let details = rp.query().video_details("ZeerrnuLi5E").await.unwrap();

        let top_comments = details
            .top_comments
            .next(rp.query())
            .await
            .unwrap()
            .unwrap();
        assert!(
            top_comments.items.len() > 10,
            "expected > 10 next comments, got {}",
            top_comments.items.len()
        );
        assert!(!top_comments.is_exhausted());

        let n_comments = top_comments.count.unwrap();
        assert!(
            n_comments > 700000,
            "expected > 700k comments, got {}",
            n_comments
        );
        // Comment count should be exact after fetching first page
        assert!(n_comments % 1000 != 0);

        let latest_comments = details
            .latest_comments
            .next(rp.query())
            .await
            .unwrap()
            .unwrap();
        assert!(
            latest_comments.items.len() > 10,
            "expected > 10 next comments, got {}",
            latest_comments.items.len()
        );
        assert!(!latest_comments.is_exhausted());
    }

    #[rstest]
    #[case::mv("mv", "ZeerrnuLi5E")]
    #[case::music("music", "XuM2onMGvTI")]
    #[case::ccommons("ccommons", "0rb9CfOvojk")]
    #[case::chapters("chapters", "nFDBxBUfE74")]
    #[case::live("live", "86YLFOog4GM")]
    #[case::agegate("agegate", "HRKu0cvrr_o")]
    #[case::newdesc("newdesc", "ZeerrnuLi5E")]
    fn t_map_video_details(#[case] name: &str, #[case] id: &str) {
        let filename = format!("testfiles/video_details/video_details_{}.json", name);
        let json_path = Path::new(&filename);
        let json_file = File::open(json_path).unwrap();

        let details: response::VideoDetails =
            serde_json::from_reader(BufReader::new(json_file)).unwrap();
        let map_res = details.map_response(id, Language::En, None).unwrap();

        assert!(
            map_res.warnings.is_empty(),
            "deserialization/mapping warnings: {:?}",
            map_res.warnings
        );
        insta::assert_ron_snapshot!(format!("map_video_details_{}", name), map_res.c, {
            ".publish_date" => "[date]",
            ".recommended.items[].publish_date" => "[date]",
        });
    }

    #[test]
    fn t_map_recommendations() {
        let json_path = Path::new("testfiles/video_details/recommendations.json");
        let json_file = File::open(json_path).unwrap();

        let recommendations: response::VideoRecommendations =
            serde_json::from_reader(BufReader::new(json_file)).unwrap();
        let map_res = recommendations
            .map_response("", Language::En, None)
            .unwrap();

        assert!(
            map_res.warnings.is_empty(),
            "deserialization/mapping warnings: {:?}",
            map_res.warnings
        );
        insta::assert_ron_snapshot!("map_recommendations", map_res.c, {
            ".items[].publish_date" => "[date]",
        });
    }

    #[rstest]
    #[case::top("top")]
    #[case::latest("latest")]
    fn t_map_comments(#[case] name: &str) {
        let filename = format!("testfiles/video_details/comments_{}.json", name);
        let json_path = Path::new(&filename);
        let json_file = File::open(json_path).unwrap();

        let comments: response::VideoComments =
            serde_json::from_reader(BufReader::new(json_file)).unwrap();
        let map_res = comments.map_response("", Language::En, None).unwrap();

        assert!(
            map_res.warnings.is_empty(),
            "deserialization/mapping warnings: {:?}",
            map_res.warnings
        );
        insta::assert_ron_snapshot!(format!("map_comments_{}", name), map_res.c, {
            ".items[].publish_date" => "[date]",
        });
    }
}
