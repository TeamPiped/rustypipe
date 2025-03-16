use std::{collections::HashMap, fmt::Debug};

use serde::Serialize;

use crate::{
    error::{Error, ExtractionError},
    model::{
        paginator::{ContinuationEndpoint, Paginator},
        ChannelTag, Chapter, Comment, Verification, VideoDetails, VideoItem,
    },
    param::Language,
    serializer::MapResult,
    util::{self, timeago, TryRemove},
};

use super::{
    response::{self, video_details::Payload, IconType},
    ClientType, MapRespCtx, MapResponse, QContinuation, RustyPipeQuery,
};

#[derive(Debug, Serialize)]
struct QVideo<'a> {
    /// YouTube video ID
    video_id: &'a str,
    /// Set to true to allow extraction of streams with sensitive content
    content_check_ok: bool,
    /// Probably refers to allowing sensitive content, too
    racy_check_ok: bool,
}

impl RustyPipeQuery {
    /// Get the metadata for a video
    #[tracing::instrument(skip(self), level = "error")]
    pub async fn video_details<S: AsRef<str> + Debug>(
        &self,
        video_id: S,
    ) -> Result<VideoDetails, Error> {
        let video_id = video_id.as_ref();
        let request_body = QVideo {
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

    /// Get the comments for a video using the continuation token obtained from `rusty_pipe_query.video_details()`
    #[tracing::instrument(skip(self), level = "error")]
    pub async fn video_comments<S: AsRef<str> + Debug>(
        &self,
        ctoken: S,
        visitor_data: Option<&str>,
    ) -> Result<Paginator<Comment>, Error> {
        let ctoken = ctoken.as_ref();
        let request_body = QContinuation {
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
        .map(|p| Paginator {
            visitor_data: visitor_data.map(str::to_owned),
            ..p
        })
    }
}

impl MapResponse<VideoDetails> for response::VideoDetails {
    fn map_response(
        self,
        ctx: &MapRespCtx<'_>,
    ) -> Result<MapResult<VideoDetails>, ExtractionError> {
        let mut warnings = Vec::new();

        let contents = self.contents.ok_or_else(|| ExtractionError::NotFound {
            id: ctx.id.to_owned(),
            msg: "no content".into(),
        })?;
        let current_video_endpoint =
            self.current_video_endpoint
                .ok_or_else(|| ExtractionError::NotFound {
                    id: ctx.id.to_owned(),
                    msg: "no current_video_endpoint".into(),
                })?;

        let video_id = current_video_endpoint.watch_endpoint.video_id;
        if ctx.id != video_id {
            return Err(ExtractionError::WrongResult(format!(
                "got wrong video id {}, expected {}",
                video_id, ctx.id
            )));
        }

        let mut primary_results = contents
            .two_column_watch_next_results
            .results
            .results
            .contents
            .ok_or_else(|| ExtractionError::NotFound {
                id: ctx.id.into(),
                msg: "no primary_results".into(),
            })?;
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
                    response::video_details::ItemSection::CommentsEntryPoint { contents } => {
                        comment_count_section = contents.into_iter().next();
                    }
                    response::video_details::ItemSection::CommentItemSection { contents } => {
                        comment_ctoken_section = contents.into_iter().next();
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
                    let like_text = video_actions
                    .menu_renderer
                    .top_level_buttons
                    .into_iter()
                    .find_map(|button| {
                        let (icon, text) = match button {
                            response::video_details::TopLevelButton::ToggleButtonRenderer(btn) => (btn.default_icon.icon_type, btn.accessibility_data),
                            response::video_details::TopLevelButton::SegmentedLikeDislikeButtonRenderer { like_button } => (like_button.toggle_button_renderer.default_icon.icon_type, like_button.toggle_button_renderer.accessibility_data),
                            response::video_details::TopLevelButton::SegmentedLikeDislikeButtonViewModel { like_button_view_model } => {
                                (IconType::Like, like_button_view_model.like_button_view_model.toggle_button_view_model.toggle_button_view_model.default_button_view_model.button_view_model.accessibility_text)
                            },
                        };
                        match icon {
                            IconType::Like => Some(text),
                            _ => None
                        }
                    });
                    (
                        title,
                        // view count field contains `No views` if the view count is zero
                        view_count
                            .as_ref()
                            .and_then(|vc| {
                                util::parse_numeric(&vc.video_view_count_renderer.view_count).ok()
                            })
                            .unwrap_or_default(),
                        // accessibility_data contains no digits if the like count is hidden,
                        // so we ignore parse errors here for now
                        like_text.and_then(|txt| util::parse_numeric(&txt).ok()),
                        date_text.as_deref().and_then(|txt| {
                            timeago::parse_textual_date_or_warn(
                                ctx.lang,
                                ctx.utc_offset,
                                txt,
                                &mut warnings,
                            )
                        }),
                        date_text,
                        view_count
                            .map(|vc| vc.video_view_count_renderer.is_live)
                            .unwrap_or_default(),
                    )
                }
                _ => {
                    return Err(ExtractionError::InvalidData(
                        "could not find primary_info".into(),
                    ))
                }
            };

        let comment_count = comment_count_section.and_then(|s| {
            util::parse_large_numstr_or_warn::<u64>(
                &s.comments_entry_point_header_renderer.comment_count,
                ctx.lang,
                &mut warnings,
            )
        });

        let comment_ctoken = comment_ctoken_section.and_then(|s| {
            s.continuation_item_renderer
                .continuation_endpoint
                .into_token()
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
                ..
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

        let visitor_data = self
            .response_context
            .visitor_data
            .or_else(|| ctx.visitor_data.map(str::to_owned));
        let recommended = contents
            .two_column_watch_next_results
            .secondary_results
            .and_then(|sr| {
                sr.secondary_results.results.map(|r| {
                    let mut res = map_recommendations(
                        r,
                        sr.secondary_results.continuations,
                        visitor_data.clone(),
                        ctx,
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
                        name: item.macro_markers_list_item_renderer.title,
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
                .and_then(|c| c.service_endpoint.into_token())
        });

        Ok(MapResult {
            c: VideoDetails {
                id: video_id,
                name: title,
                description,
                channel: ChannelTag {
                    id: channel_id,
                    name: channel_name,
                    avatar: owner.thumbnail.into(),
                    verification: owner.badges.into(),
                    subscriber_count: owner.subscriber_count_text.and_then(|txt| {
                        util::parse_large_numstr_or_warn(&txt, ctx.lang, &mut warnings)
                    }),
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
                    visitor_data.clone(),
                    ContinuationEndpoint::Next,
                    ctx.authenticated,
                ),
                latest_comments: Paginator::new_ext(
                    comment_count,
                    Vec::new(),
                    latest_comments_ctoken,
                    visitor_data.clone(),
                    ContinuationEndpoint::Next,
                    ctx.authenticated,
                ),
                visitor_data,
            },
            warnings,
        })
    }
}

impl MapResponse<Paginator<Comment>> for response::VideoComments {
    fn map_response(
        self,
        ctx: &MapRespCtx<'_>,
    ) -> Result<MapResult<Paginator<Comment>>, ExtractionError> {
        let received_endpoints = self.on_response_received_endpoints;
        let mut warnings = Vec::new();

        let mut comments = Vec::new();
        let mut comment_count = None;
        let mut ctoken = None;

        let mut mutations = if let Some(upd) = self.framework_updates {
            let mut m = upd.entity_batch_update.mutations;
            warnings.append(&mut m.warnings);
            m.items
        } else {
            HashMap::new()
        };

        received_endpoints.c.into_iter().for_each(|citem| {
            let mut items = citem.append_continuation_items_action.continuation_items;
            warnings.append(&mut items.warnings);
            items.c.into_iter().for_each(|item| match item {
                response::video_details::CommentListItem::CommentThreadRenderer(thread) => {
                    if let Some(comment) = thread.comment {
                        comments.push(map_comment(
                            comment.comment_renderer,
                            Some(thread.replies),
                            thread.rendering_priority,
                            ctx.lang,
                            &mut warnings,
                        ));
                    } else if let Some(vm) = thread.comment_view_model {
                        if let Some(c) = map_comment_vm(
                            vm.comment_view_model,
                            &mut mutations,
                            Some(thread.replies),
                            thread.rendering_priority,
                            ctx.lang,
                            &mut warnings,
                        ) {
                            comments.push(c);
                        }
                    } else {
                        warnings.push(
                            "comment does not contain comment or commentViewModel field".to_owned(),
                        );
                    }
                }
                response::video_details::CommentListItem::CommentRenderer(comment) => {
                    comments.push(map_comment(
                        comment,
                        None,
                        response::video_details::CommentPriority::RenderingPriorityUnknown,
                        ctx.lang,
                        &mut warnings,
                    ));
                }
                response::video_details::CommentListItem::CommentViewModel(vm) => {
                    if let Some(c) = map_comment_vm(
                        vm,
                        &mut mutations,
                        None,
                        response::video_details::CommentPriority::RenderingPriorityUnknown,
                        ctx.lang,
                        &mut warnings,
                    ) {
                        comments.push(c);
                    }
                }
                response::video_details::CommentListItem::ContinuationItemRenderer(cont) => {
                    if ctoken.is_none() {
                        ctoken = cont.into_token();
                    }
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
    continuations: Option<Vec<response::MusicContinuationData>>,
    visitor_data: Option<String>,
    ctx: &MapRespCtx<'_>,
) -> MapResult<Paginator<VideoItem>> {
    let mut mapper = response::YouTubeListMapper::<VideoItem>::new(ctx.lang);
    mapper.map_response(r);

    mapper.ctoken = mapper.ctoken.or_else(|| {
        continuations
            .and_then(|c| c.into_iter().next())
            .map(|c| c.next_continuation_data.continuation)
    });

    MapResult {
        c: Paginator::new_ext(
            None,
            mapper.items,
            mapper.ctoken,
            visitor_data,
            ContinuationEndpoint::Next,
            ctx.authenticated,
        ),
        warnings: mapper.warnings,
    }
}

fn map_replies(
    replies: Option<response::video_details::Replies>,
    lang: Language,
    warnings: &mut Vec<String>,
) -> (Vec<Comment>, Option<String>) {
    let mut reply_ctoken = None;
    let replies = replies
        .map(|replies| {
            replies
                .comment_replies_renderer
                .contents
                .into_iter()
                .filter_map(|item| match item {
                    response::video_details::CommentListItem::CommentRenderer(comment) => {
                        Some(map_comment(
                            comment,
                            None,
                            response::video_details::CommentPriority::default(),
                            lang,
                            warnings,
                        ))
                    }
                    response::video_details::CommentListItem::ContinuationItemRenderer(cont) => {
                        if reply_ctoken.is_none() {
                            reply_ctoken = cont.into_token();
                        }
                        None
                    }
                    _ => None,
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    (replies, reply_ctoken)
}

fn map_comment(
    c: response::video_details::CommentRenderer,
    replies: Option<response::video_details::Replies>,
    priority: response::video_details::CommentPriority,
    lang: Language,
    warnings: &mut Vec<String>,
) -> Comment {
    let (replies, reply_ctoken) = map_replies(replies, lang, warnings);

    Comment {
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
        publish_date: timeago::parse_timeago_dt_or_warn(lang, &c.published_time_text, warnings),
        publish_date_txt: c.published_time_text,
        like_count: match c.vote_count {
            Some(txt) => util::parse_numeric_or_warn(&txt, warnings),
            None => Some(0),
        },
        reply_count: c.reply_count as u32,
        replies: Paginator::new(Some(c.reply_count), replies, reply_ctoken),
        by_owner: c.author_is_channel_owner,
        pinned: priority.into(),
        hearted: c
            .action_buttons
            .comment_action_buttons_renderer
            .creator_heart
            .map(|h| h.creator_heart_renderer.is_hearted)
            .unwrap_or_default(),
    }
}

fn map_comment_vm(
    vm: response::video_details::CommentViewModel,
    mutations: &mut HashMap<String, response::video_details::Payload>,
    replies: Option<response::video_details::Replies>,
    priority: response::video_details::CommentPriority,
    lang: Language,
    warnings: &mut Vec<String>,
) -> Option<Comment> {
    let (replies, reply_ctoken) = map_replies(replies, lang, warnings);

    let ce = if let Some(Payload::CommentEntityPayload(ce)) = mutations.remove(&vm.comment_key) {
        ce
    } else {
        warnings.push(format!(
            "comment `{}` does not have entity payload (key: `{}`)",
            vm.comment_id, vm.comment_key
        ));
        return None;
    };
    let hearted = if let Some(Payload::EngagementToolbarStateEntityPayload { heart_state }) =
        mutations.get(&vm.toolbar_state_key)
    {
        (*heart_state).into()
    } else {
        false
    };
    let voice_reply = if let Some(Payload::CommentSurfaceEntityPayload(sf)) =
        mutations.remove(&vm.comment_surface_key)
    {
        sf.voice_reply_container_view_model
            .map(|vr| vr.voice_reply_container_view_model.transcript_text)
    } else {
        None
    };

    let mut parse_num = |s: &str| -> Option<u32> {
        if s.is_empty() || s == " " {
            Some(0)
        } else {
            util::parse_large_numstr_or_warn(s, lang, warnings)
        }
    };

    let reply_count = parse_num(&ce.toolbar.reply_count).unwrap_or_default();

    Some(Comment {
        id: vm.comment_id,
        text: voice_reply
            .filter(|_| ce.properties.content.is_empty())
            .unwrap_or(ce.properties.content)
            .into(),
        by_owner: ce.author.as_ref().map(|a| a.is_creator).unwrap_or_default(),
        author: ce.author.map(|a| ChannelTag {
            id: a.channel_id,
            name: a.display_name,
            avatar: ce.avatar.image.into(),
            verification: if a.is_artist {
                Verification::Artist
            } else if a.is_verified {
                Verification::Verified
            } else {
                Verification::None
            },
            subscriber_count: None,
        }),
        like_count: parse_num(&ce.toolbar.like_count_notliked),
        reply_count,
        replies: Paginator::new(Some(reply_count.into()), replies, reply_ctoken),
        publish_date: timeago::parse_timeago_dt_or_warn(
            lang,
            &ce.properties.published_time,
            warnings,
        ),
        publish_date_txt: ce.properties.published_time,
        pinned: priority.into(),
        hearted,
    })
}

#[cfg(test)]
mod tests {
    use std::{fs::File, io::BufReader};

    use path_macro::path;
    use rstest::rstest;

    use crate::{
        client::{response, MapRespCtx, MapResponse},
        util::tests::TESTFILES,
    };

    #[rstest]
    #[case::mv("mv", "ZeerrnuLi5E")]
    #[case::music("music", "XuM2onMGvTI")]
    #[case::ccommons("ccommons", "0rb9CfOvojk")]
    #[case::chapters("chapters", "nFDBxBUfE74")]
    #[case::live("live", "86YLFOog4GM")]
    #[case::agegate("agegate", "HRKu0cvrr_o")]
    #[case::ab_newdesc("20220924_newdesc", "ZeerrnuLi5E")]
    #[case::ab_new_cont("20221011_new_continuation", "ZeerrnuLi5E")]
    #[case::ab_no_recommends("20221011_rec_isr", "nFDBxBUfE74")]
    #[case::ab_new_likes("20231103_likes", "ZeerrnuLi5E")]
    #[case::mix("20241109_mix", "XuM2onMGvTI")]
    fn map_video_details(#[case] name: &str, #[case] id: &str) {
        let json_path = path!(*TESTFILES / "video_details" / format!("video_details_{name}.json"));
        let json_file = File::open(json_path).unwrap();

        let details: response::VideoDetails =
            serde_json::from_reader(BufReader::new(json_file)).unwrap();
        let map_res = details.map_response(&MapRespCtx::test(id)).unwrap();

        assert!(
            map_res.warnings.is_empty(),
            "deserialization/mapping warnings: {:?}",
            map_res.warnings
        );
        insta::assert_ron_snapshot!(format!("map_video_details_{name}"), map_res.c, {
            ".publish_date" => "[date]",
            ".recommended.items[].publish_date" => "[date]",
        });
    }

    #[test]
    fn map_video_details_not_found() {
        let json_path = path!(*TESTFILES / "video_details" / "video_details_not_found.json");
        let json_file = File::open(json_path).unwrap();

        let details: response::VideoDetails =
            serde_json::from_reader(BufReader::new(json_file)).unwrap();
        let err = details.map_response(&MapRespCtx::test("")).unwrap_err();
        assert!(matches!(
            err,
            crate::error::ExtractionError::NotFound { .. }
        ));
    }

    #[rstest]
    #[case::top("top")]
    #[case::latest("latest")]
    #[case::frameworkupd("20240401_frameworkupd")]
    #[case::frameworkupd_reply("20240401_frameworkupd_reply")]
    #[case::voice_reply("20241218_voice_reply")]
    fn map_comments(#[case] name: &str) {
        let json_path = path!(*TESTFILES / "video_details" / format!("comments_{name}.json"));
        let json_file = File::open(json_path).unwrap();

        let comments: response::VideoComments =
            serde_json::from_reader(BufReader::new(json_file)).unwrap();
        let map_res = comments.map_response(&MapRespCtx::test("")).unwrap();

        assert!(
            map_res.warnings.is_empty(),
            "deserialization/mapping warnings: {:?}",
            map_res.warnings
        );
        insta::assert_ron_snapshot!(format!("map_comments_{name}"), map_res.c, {
            ".items[].publish_date" => "[date]",
        });
    }
}
