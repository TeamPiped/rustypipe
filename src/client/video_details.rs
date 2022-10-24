use std::borrow::Cow;

use serde::Serialize;

use crate::{
    error::{Error, ExtractionError},
    model::{ChannelTag, Chapter, Comment, Paginator, VideoDetails, VideoItem},
    param::Language,
    serializer::MapResult,
    timeago,
    util::{self, TryRemove},
};

use super::{
    response::{self, IconType},
    ClientType, MapResponse, QContinuation, RustyPipeQuery, YTContext,
};

#[derive(Debug, Serialize)]
struct QVideo<'a> {
    context: YTContext<'a>,
    /// YouTube video ID
    video_id: &'a str,
    /// Set to true to allow extraction of streams with sensitive content
    content_check_ok: bool,
    /// Probably refers to allowing sensitive content, too
    racy_check_ok: bool,
}

impl RustyPipeQuery {
    pub async fn video_details(&self, video_id: &str) -> Result<VideoDetails, Error> {
        let context = self.get_context(ClientType::Desktop, true, None).await;
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

    pub async fn video_comments(
        &self,
        ctoken: &str,
        visitor_data: Option<&str>,
    ) -> Result<Paginator<Comment>, Error> {
        let context = self
            .get_context(ClientType::Desktop, true, visitor_data)
            .await;
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
        lang: Language,
        _deobf: Option<&crate::deobfuscate::Deobfuscator>,
    ) -> Result<MapResult<VideoDetails>, ExtractionError> {
        let mut warnings = Vec::new();

        let contents = self
            .contents
            .ok_or(ExtractionError::ContentUnavailable(Cow::Borrowed(
                "Video not found",
            )))?;
        let current_video_endpoint =
            self.current_video_endpoint
                .ok_or(ExtractionError::ContentUnavailable(Cow::Borrowed(
                    "Video not found",
                )))?;

        let video_id = current_video_endpoint.watch_endpoint.video_id;
        if id != video_id {
            return Err(ExtractionError::WrongResult(format!(
                "got wrong playlist id {}, expected {}",
                video_id, id
            )));
        }

        let mut primary_results = contents
            .two_column_watch_next_results
            .results
            .results
            .contents
            .ok_or(ExtractionError::ContentUnavailable(Cow::Borrowed(
                "Video not found",
            )))?;
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
                response::url_endpoint::PageType::Channel => (browse_id, text),
                _ => {
                    return Err(ExtractionError::InvalidData(
                        "invalid channel link type".into(),
                    ))
                }
            },
            _ => return Err(ExtractionError::InvalidData("invalid channel link".into())),
        };

        let recommended = contents
            .two_column_watch_next_results
            .secondary_results
            .and_then(|sr| {
                sr.secondary_results.results.map(|r| {
                    let mut res = map_recommendations(
                        r,
                        sr.secondary_results.continuations,
                        self.response_context.visitor_data,
                        lang,
                    );
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
                top_comments: Paginator::new_ext(
                    comment_count,
                    Vec::new(),
                    comment_ctoken,
                    None,
                    crate::param::ContinuationEndpoint::Next,
                ),
                latest_comments: Paginator::new_ext(
                    comment_count,
                    Vec::new(),
                    latest_comments_ctoken,
                    None,
                    crate::param::ContinuationEndpoint::Next,
                ),
            },
            warnings,
        })
    }
}

impl MapResponse<Paginator<Comment>> for response::VideoComments {
    fn map_response(
        self,
        _id: &str,
        lang: Language,
        _deobf: Option<&crate::deobfuscate::Deobfuscator>,
    ) -> Result<MapResult<Paginator<Comment>>, ExtractionError> {
        let received_endpoints = self.on_response_received_endpoints;
        let mut warnings = received_endpoints.warnings;

        let mut comments = Vec::new();
        let mut comment_count = None;
        let mut ctoken = None;

        received_endpoints.c.into_iter().for_each(|citem| {
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
                response::video_details::CommentListItem::CommentsHeaderRenderer { count_text } => {
                    comment_count = count_text
                        .and_then(|txt| util::parse_numeric_or_warn::<u64>(&txt, &mut warnings));
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
    r: MapResult<Vec<response::YouTubeListItem>>,
    continuations: Option<Vec<response::MusicContinuation>>,
    visitor_data: Option<String>,
    lang: Language,
) -> MapResult<Paginator<VideoItem>> {
    let mut mapper = response::YouTubeListMapper::<VideoItem>::new(lang);
    mapper.map_response(r);

    if let Some(continuations) = continuations {
        continuations.into_iter().for_each(|c| {
            mapper.ctoken = Some(c.next_continuation_data.continuation);
        })
    };

    MapResult {
        c: Paginator::new_ext(
            None,
            mapper.items,
            mapper.ctoken,
            visitor_data,
            crate::param::ContinuationEndpoint::Next,
        ),
        warnings: mapper.warnings,
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
            publish_date: timeago::parse_timeago_to_dt(lang, &c.published_time_text),
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

    use rstest::rstest;

    use crate::{
        client::{response, MapResponse},
        param::Language,
    };

    #[rstest]
    #[case::mv("mv", "ZeerrnuLi5E")]
    #[case::music("music", "XuM2onMGvTI")]
    #[case::ccommons("ccommons", "0rb9CfOvojk")]
    #[case::chapters("chapters", "nFDBxBUfE74")]
    #[case::live("live", "86YLFOog4GM")]
    #[case::agegate("agegate", "HRKu0cvrr_o")]
    #[case::newdesc("20220924_newdesc", "ZeerrnuLi5E")]
    #[case::new_cont("20221011_new_continuation", "ZeerrnuLi5E")]
    #[case::no_recommends("20221011_rec_isr", "nFDBxBUfE74")]
    fn map_video_details(#[case] name: &str, #[case] id: &str) {
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
    fn map_video_details_not_found() {
        let filename = "testfiles/video_details/video_details_not_found.json";
        let json_path = Path::new(&filename);
        let json_file = File::open(json_path).unwrap();

        let details: response::VideoDetails =
            serde_json::from_reader(BufReader::new(json_file)).unwrap();
        let err = details.map_response("", Language::En, None).unwrap_err();
        assert!(matches!(
            err,
            crate::error::ExtractionError::ContentUnavailable(_)
        ))
    }

    #[rstest]
    #[case::top("top")]
    #[case::latest("latest")]
    fn map_comments(#[case] name: &str) {
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
