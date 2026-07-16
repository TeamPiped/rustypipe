use std::{collections::HashMap, fmt::Debug};

use crate::{
    error::{Error, ExtractionError},
    json::{
        value_from_json_value, yt_response_visitor_data, ytq, JsonDoc, JsonNode,
        JsonValue,
    },
    model::{
        paginator::{ContinuationEndpoint, Paginator},
        ChannelTag, Chapter, Comment, Verification, VideoDetails, VideoItem,
    },
    param::Language,
    request_body::ytbody,
    serializer::{
        text::{TextComponent, TextComponents},
        MapResult,
    },
    util::{self, timeago},
};

use super::{
    response::{self, url_endpoint, video_details::Payload},
    ClientType, MapEndpoint, MapRespCtx, RustyPipeQuery,
};

#[derive(Debug)]
struct VideoDetailsEndpoint;

#[derive(Debug)]
struct VideoCommentsEndpoint;

impl RustyPipeQuery {
    /// Get the metadata for a video
    #[tracing::instrument(skip(self), level = "error")]
    pub async fn video_details<S: AsRef<str> + Debug>(
        &self,
        video_id: S,
    ) -> Result<VideoDetails, Error> {
        let video_id = video_id.as_ref();
        let request_body = ytbody!({
            "videoId": video_id,
            "contentCheckOk": true,
            "racyCheckOk": true,
        });

        self.execute_request::<VideoDetailsEndpoint, _, _>(
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
        let request_body = ytbody!({
            "continuation": ctoken,
        });

        self.execute_request::<VideoCommentsEndpoint, _, _>(
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

struct VideoDetailsFields<'a> {
    video_results: Option<JsonNode<'a>>,
    secondary_results: Option<JsonNode<'a>>,
    current_video_endpoint: Option<response::video_details::CurrentVideoEndpoint>,
    engagement_panels: MapResult<Vec<response::video_details::EngagementPanel>>,
    visitor_data: Option<String>,
}

struct VideoSections {
    primary: VideoWatchMetadata,
    secondary: VideoOwnerInfo,
    comments: CommentEntryPoints,
}

#[derive(Clone, Debug)]
struct VideoWatchMetadata {
    title: String,
    view_count: u64,
    like_count: Option<u32>,
    publish_date: Option<time::OffsetDateTime>,
    publish_date_txt: Option<String>,
    is_live: bool,
}

#[derive(Clone, Debug)]
struct VideoOwnerInfo {
    channel: ChannelTag,
    collaborators: Vec<ChannelTag>,
    description: TextComponents,
    is_ccommons: bool,
}

#[derive(Clone, Debug, Default)]
struct CommentEntryPoints {
    comment_count: Option<u64>,
    top_comments_ctoken: Option<String>,
    latest_comments_ctoken: Option<String>,
}

#[derive(Clone, Debug, Default)]
struct EngagementPanels {
    chapters: Vec<Chapter>,
    latest_comments_ctoken: Option<String>,
}

fn comment_author_tag(
    id: String,
    name: String,
    avatar: Vec<crate::model::Thumbnail>,
    verification: Verification,
) -> ChannelTag {
    ChannelTag {
        id,
        name,
        avatar,
        verification,
        subscriber_count: None,
    }
}

#[derive(Debug)]
struct CommentParts {
    id: String,
    text: TextComponents,
    author: Option<ChannelTag>,
    publish_date_txt: String,
    like_count: Option<u32>,
    reply_count: u32,
    by_owner: bool,
    hearted: bool,
}

fn build_comment(
    parts: CommentParts,
    replies: Vec<Comment>,
    reply_ctoken: Option<String>,
    priority: response::video_details::CommentPriority,
    lang: Language,
    warnings: &mut Vec<String>,
) -> Comment {
    Comment {
        id: parts.id,
        text: parts.text.into(),
        author: parts.author,
        publish_date: timeago::parse_timeago_dt_or_warn(lang, &parts.publish_date_txt, warnings),
        publish_date_txt: parts.publish_date_txt,
        like_count: parts.like_count,
        reply_count: parts.reply_count,
        replies: Paginator::new(Some(parts.reply_count.into()), replies, reply_ctoken),
        by_owner: parts.by_owner,
        pinned: priority.into(),
        hearted: parts.hearted,
    }
}

fn deserialize_video_details_fields<'a>(
    root: &JsonNode<'a>,
) -> Result<VideoDetailsFields<'a>, ExtractionError> {
    Ok(VideoDetailsFields {
        video_results: root.query(ytq!(
            .contents.twoColumnWatchNextResults.results.results.contents
        )),
        secondary_results: root.query(ytq!(
            .contents.twoColumnWatchNextResults.secondaryResults.secondaryResults
        )),
        current_video_endpoint: root
            .query(ytq!(.currentVideoEndpoint))
            .map(|node| node.deserialize())
            .transpose()?,
        engagement_panels: root
            .query(ytq!(.engagementPanels))
            .map(|node| node.deserialize())
            .transpose()?
            .unwrap_or_default(),
        visitor_data: yt_response_visitor_data(root),
    })
}

fn parse_engagement_panels(
    engagement_panels: MapResult<Vec<response::video_details::EngagementPanel>>,
    warnings: &mut Vec<String>,
) -> EngagementPanels {
    let mut panels = engagement_panels;
    warnings.append(&mut panels.warnings);

    let mut chapters = Vec::new();
    let mut comment_panel: Option<JsonValue> = None;
    panels.c.into_iter().for_each(|panel| {
        if let Some(panel) = panel.engagement_panel_section_list_renderer {
            match panel.get("targetId").and_then(|v| v.as_str()) {
                Some("engagement-panel-macro-markers-description-chapters") => {
                    let doc = JsonDoc::new(crate::json::value_to_json_string(&panel));
                    if let Ok(mapped) = doc.with_root(map_chapter_panel) {
                        chapters = mapped;
                    }
                }
                Some("engagement-panel-comments-section") => {
                    comment_panel = panel.get("header").cloned();
                }
                _ => {}
            }
        }
    });

    let latest_comments_ctoken = comment_panel.and_then(|comments| {
        comments
            .get("engagementPanelTitleHeaderRenderer")
            .and_then(|v| v.get("menu"))
            .and_then(|v| v.get("sortFilterSubMenuRenderer"))
            .and_then(|v| v.get("subMenuItems"))
            .and_then(|v| v.as_array())
            .and_then(|items| items.get(1))
            .and_then(|item| item.get("serviceEndpoint"))
            .and_then(response::video_details::continuation_token)
    });

    EngagementPanels {
        chapters,
        latest_comments_ctoken,
    }
}

fn map_chapter_panel(root: JsonNode<'_>) -> Result<Vec<Chapter>, ExtractionError> {
    let Some(contents) = root.query(ytq!(.content.macroMarkersListRenderer.contents)) else {
        return Ok(Vec::new());
    };

    Ok(contents
        .items()
        .into_iter()
        .filter_map(|item| {
            let marker = item.query(ytq!(.macroMarkersListItemRenderer))?;
            Some(Chapter {
                name: marker.text_at(ytq!(.title))?,
                position: marker
                    .query(ytq!(.onTap.watchEndpoint.startTimeSeconds))
                    .and_then(|node| node.as_u64())
                    .and_then(|value| u32::try_from(value).ok())?,
                thumbnail: marker.query_thumbnails(ytq!(.thumbnail)),
            })
        })
        .collect())
}

fn parse_like_text(video_actions: response::video_details::VideoActions) -> Option<String> {
    video_actions
        .menu_renderer
        .top_level_buttons
        .into_iter()
        .find_map(|button| {
            let (icon, text) = like_button_text(button)?;
            match icon {
                response::IconType::Like => Some(text),
                _ => None,
            }
        })
}

fn like_button_text(button: JsonValue) -> Option<(response::IconType, String)> {
    if let Some(value) = button.get("toggleButtonRenderer") {
        let btn = value_from_json_value::<response::video_details::ToggleButton>(value)?;
        Some((btn.default_icon.icon_type, btn.accessibility_data))
    } else if let Some(value) = button
        .get("segmentedLikeDislikeButtonRenderer")
        .and_then(|renderer| renderer.get("likeButton"))
        .and_then(|button| button.get("toggleButtonRenderer"))
    {
        let btn = value_from_json_value::<response::video_details::ToggleButton>(value)?;
        Some((btn.default_icon.icon_type, btn.accessibility_data))
    } else {
        let text = button
            .get("segmentedLikeDislikeButtonViewModel")
            .and_then(|renderer| renderer.get("likeButtonViewModel"))
            .and_then(|button| button.get("likeButtonViewModel"))
            .and_then(|button| button.get("toggleButtonViewModel"))
            .and_then(|button| button.get("toggleButtonViewModel"))
            .and_then(|button| button.get("defaultButtonViewModel"))
            .and_then(|button| button.get("buttonViewModel"))
            .and_then(|button| button.get("accessibilityText"))
            .and_then(|text| text.as_str())?;
        Some((response::IconType::Like, text.to_owned()))
    }
}

fn parse_watch_metadata(
    node: &JsonNode<'_>,
    ctx: &MapRespCtx<'_>,
    warnings: &mut Vec<String>,
) -> Result<VideoWatchMetadata, ExtractionError> {
    let title = node
        .text_at(ytq!(.title))
        .ok_or_else(|| ExtractionError::InvalidData("missing primary_info title".into()))?;
    let view_count = node
        .query(ytq!(.viewCount.videoViewCountRenderer))
        .map(|node| node.deserialize::<response::video_details::ViewCountRenderer>())
        .transpose()?;
    let date_text = node.text_at(ytq!(.dateText));
    let video_actions = node
        .query(ytq!(.videoActions))
        .map(|node| node.deserialize::<response::video_details::VideoActions>())
        .transpose()?;
    let like_text = video_actions.and_then(parse_like_text);

    Ok(VideoWatchMetadata {
        title,
        view_count: view_count
            .as_ref()
            .and_then(|vc| util::parse_numeric(&vc.view_count).ok())
            .unwrap_or_default(),
        like_count: like_text.and_then(|txt| util::parse_numeric(&txt).ok()),
        publish_date: date_text.as_deref().and_then(|txt| {
            timeago::parse_textual_date_or_warn(ctx.lang, ctx.utc_offset, txt, warnings)
        }),
        publish_date_txt: date_text,
        is_live: view_count.map(|vc| vc.is_live).unwrap_or_default(),
    })
}

fn parse_creative_commons(node: &JsonNode<'_>) -> bool {
    let Some(rows) = node.query(ytq!(.metadataRowContainer.metadataRowContainerRenderer.rows))
    else {
        return false;
    };

    rows.items().into_iter().any(|row| {
        row.query(ytq!(.metadataRowRenderer.contents))
            .map(|contents| {
                contents.items().into_iter().any(|content| {
                    content
                        .deserialize::<TextComponents>()
                        .map(|links| {
                            links.0.iter().any(|link| match link {
                                TextComponent::Web { url, .. } => {
                                    url == "https://www.youtube.com/t/creative_commons"
                                }
                                _ => false,
                            })
                        })
                        .unwrap_or(false)
                })
            })
            .unwrap_or(false)
    })
}

fn parse_owner_info(
    node: &JsonNode<'_>,
    lang: Language,
    warnings: &mut Vec<String>,
) -> Result<VideoOwnerInfo, ExtractionError> {
    let owner = node
        .query(ytq!(.owner.videoOwnerRenderer))
        .ok_or_else(|| ExtractionError::InvalidData("could not find secondary_info".into()))?
        .deserialize::<response::video_details::VideoOwnerRenderer>()?;
    let description = node
        .query(ytq!(.attributedDescription))
        .and_then(|node| {
            response::video_details::AttributedDescription::deserialize_node(&node).ok()
        })
        .unwrap_or_default()
        .into();
    let is_ccommons = parse_creative_commons(node);

    let collaborator_channels = owner.collaborator_channels();
    let first_collaborator = collaborator_channels.first().cloned();
    let owner_avatar: Vec<crate::model::Thumbnail> = owner.thumbnail_or_avatar_stack().into();
    let owner_verification = owner.badges.into();
    let owner_subscriber_count = owner
        .subscriber_count_text
        .as_ref()
        .and_then(|txt| util::parse_large_numstr_or_warn(txt, lang, warnings));

    let mut collaborators = collaborator_channels
        .iter()
        .map(|(id, name)| ChannelTag {
            id: id.clone(),
            name: name.clone(),
            avatar: owner_avatar.clone(),
            verification: owner_verification,
            subscriber_count: owner_subscriber_count,
        })
        .collect::<Vec<_>>();

    let (channel_id, channel_name) = if let Some(title) = &owner.title {
        match title {
            TextComponent::Browse {
                text,
                page_type,
                browse_id,
                ..
            } => match page_type {
                response::url_endpoint::PageType::Channel => (browse_id.clone(), text.clone()),
                _ => {
                    return Err(ExtractionError::InvalidData(
                        "invalid channel link type".into(),
                    ))
                }
            },
            _ => return Err(ExtractionError::InvalidData("invalid channel link".into())),
        }
    } else if let Some((id, name)) = first_collaborator {
        (id, name)
    } else {
        return Err(ExtractionError::InvalidData("invalid channel link".into()));
    };

    if !collaborators.is_empty() {
        collaborators[0].id = channel_id.clone();
        collaborators[0].name = channel_name.clone();
    }

    Ok(VideoOwnerInfo {
        channel: ChannelTag {
            id: channel_id,
            name: channel_name,
            avatar: owner_avatar,
            verification: owner_verification,
            subscriber_count: owner_subscriber_count,
        },
        collaborators,
        description,
        is_ccommons,
    })
}

fn parse_comments_entry_points(
    node: &JsonNode<'_>,
    ctx: &MapRespCtx<'_>,
    warnings: &mut Vec<String>,
) -> CommentEntryPoints {
    let mut comments = CommentEntryPoints::default();
    let section_identifier = node
        .query(ytq!(.sectionIdentifier))
        .and_then(|node| node.as_str());

    match section_identifier.as_deref() {
        Some("comments-entry-point") => {
            comments.comment_count = node
                .text_at(ytq!(
                    .contents[0].commentsEntryPointHeaderRenderer.commentCount
                ))
                .and_then(|txt| util::parse_large_numstr_or_warn::<u64>(&txt, ctx.lang, warnings));
        }
        Some("comment-item-section") => {
            comments.top_comments_ctoken = node
                .query(ytq!(
                    .contents[0].continuationItemRenderer.continuationEndpoint
                ))
                .and_then(|node| node.deserialize::<JsonValue>().ok())
                .and_then(|endpoint| response::video_details::continuation_token(&endpoint));
        }
        _ => {}
    }

    comments
}

fn split_video_results(
    results: &JsonNode<'_>,
    ctx: &MapRespCtx<'_>,
) -> Result<(VideoSections, Vec<String>), ExtractionError> {
    let mut primary = None;
    let mut secondary = None;
    let mut comments = CommentEntryPoints::default();
    let mut warnings = Vec::new();

    for item in results.items() {
        if let Some(node) = item.query(ytq!(.videoPrimaryInfoRenderer)) {
            primary = Some(parse_watch_metadata(&node, ctx, &mut warnings)?);
        } else if let Some(node) = item.query(ytq!(.videoSecondaryInfoRenderer)) {
            secondary = Some(parse_owner_info(&node, ctx.lang, &mut warnings)?);
        } else if let Some(node) = item.query(ytq!(.itemSectionRenderer)) {
            let section_comments = parse_comments_entry_points(&node, ctx, &mut warnings);
            comments.comment_count = comments.comment_count.or(section_comments.comment_count);
            comments.top_comments_ctoken = comments
                .top_comments_ctoken
                .or(section_comments.top_comments_ctoken);
        }
    }

    let primary = primary
        .ok_or_else(|| ExtractionError::InvalidData("could not find primary_info".into()))?;
    let secondary = secondary
        .ok_or_else(|| ExtractionError::InvalidData("could not find secondary_info".into()))?;

    Ok((
        VideoSections {
            primary,
            secondary,
            comments,
        },
        warnings,
    ))
}

fn map_video_details_fields(
    fields: VideoDetailsFields<'_>,
    ctx: &MapRespCtx<'_>,
) -> Result<MapResult<VideoDetails>, ExtractionError> {
    let mut warnings = Vec::new();

    let video_results = fields
        .video_results
        .ok_or_else(|| ExtractionError::NotFound {
            id: ctx.id.to_owned(),
            msg: "no content".into(),
        })?;
    let current_video_endpoint =
        fields
            .current_video_endpoint
            .ok_or_else(|| ExtractionError::NotFound {
                id: ctx.id.to_owned(),
                msg: "no current_video_endpoint".into(),
            })?;

    let video_id = current_video_endpoint.watch_endpoint.video_id;
    if ctx.id != video_id {
        return Err(crate::client::check_id_matches(
            &video_id,
            ctx.id,
            "video",
        ));
    }

    let (sections, section_warnings) = split_video_results(&video_results, ctx)?;
    warnings.extend(section_warnings);
    let primary = sections.primary;
    let secondary = sections.secondary;

    let visitor_data = fields
        .visitor_data
        .or_else(|| ctx.visitor_data.map(str::to_owned));
    let recommended = fields
        .secondary_results
        .and_then(|sr| {
            let results = sr.query(ytq!(.results))?;
            let continuations = sr.query(ytq!(.continuations)).map(|node| {
                let (continuations, mut continuation_warnings) =
                    node.deserialize_items_lossy::<response::MusicContinuationData>();
                warnings.append(&mut continuation_warnings);
                continuations
            });
            Some({
                let mut res =
                    map_recommendations(&results, continuations, visitor_data.clone(), ctx);
                warnings.append(&mut res.warnings);
                res.c
            })
        })
        .unwrap_or_default();

    let panels = parse_engagement_panels(fields.engagement_panels, &mut warnings);

    let latest_comments_ctoken = panels
        .latest_comments_ctoken
        .clone()
        .or(sections.comments.latest_comments_ctoken.clone());

    Ok(MapResult {
        c: VideoDetails {
            id: video_id,
            name: primary.title,
            description: secondary.description.into(),
            channel: secondary.channel,
            collaborators: secondary.collaborators,
            view_count: primary.view_count,
            like_count: primary.like_count,
            publish_date: primary.publish_date,
            publish_date_txt: primary.publish_date_txt,
            is_live: primary.is_live,
            is_ccommons: secondary.is_ccommons,
            chapters: panels.chapters,
            recommended,
            top_comments: Paginator::new_ext(
                sections.comments.comment_count,
                Vec::new(),
                sections.comments.top_comments_ctoken,
                visitor_data.clone(),
                ContinuationEndpoint::Next,
                ctx.authenticated,
            ),
            latest_comments: Paginator::new_ext(
                sections.comments.comment_count,
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

impl MapEndpoint<VideoDetails> for VideoDetailsEndpoint {
    fn map(
        json: &JsonDoc,
        ctx: &MapRespCtx<'_>,
    ) -> Result<MapResult<VideoDetails>, ExtractionError> {
        json.with_root(|root| {
            let fields = deserialize_video_details_fields(&root)?;
            map_video_details_fields(fields, ctx)
        })
    }
}

struct VideoCommentsFields {
    on_response_received_endpoints: MapResult<Vec<response::video_details::CommentsContItem>>,
    framework_updates: Option<response::FrameworkUpdates<Payload>>,
}

fn deserialize_video_comments_fields(
    root: &JsonNode<'_>,
) -> Result<VideoCommentsFields, ExtractionError> {
    Ok(VideoCommentsFields {
        on_response_received_endpoints: root
            .require(
                ytq!(.onResponseReceivedEndpoints),
                "comment response endpoints",
            )?
            .deserialize()?,
        framework_updates: root
            .query(ytq!(.frameworkUpdates))
            .map(|node| node.deserialize())
            .transpose()?,
    })
}

fn map_video_comments_fields(
    fields: VideoCommentsFields,
    ctx: &MapRespCtx<'_>,
) -> Result<MapResult<Paginator<Comment>>, ExtractionError> {
    let received_endpoints = fields.on_response_received_endpoints;
    let mut warnings = Vec::new();

    let mut comments = Vec::new();
    let mut comment_count = None;
    let mut ctoken = None;

    let mut mutations = if let Some(upd) = fields.framework_updates {
        let mut m = upd.entity_batch_update.mutations;
        warnings.append(&mut m.warnings);
        m.items
    } else {
        HashMap::new()
    };

    received_endpoints.c.into_iter().for_each(|citem| {
        for item in citem.append_continuation_items_action.continuation_items {
            map_comment_item_node(
                item,
                &mut mutations,
                ctx.lang,
                &mut warnings,
                &mut comments,
                &mut comment_count,
                &mut ctoken,
            );
        }
    });

    Ok(MapResult {
        c: Paginator::new(comment_count, comments, ctoken),
        warnings,
    })
}

fn map_comment_item_node(
    item: JsonValue,
    mutations: &mut HashMap<String, response::video_details::Payload>,
    lang: Language,
    warnings: &mut Vec<String>,
    comments: &mut Vec<Comment>,
    comment_count: &mut Option<u64>,
    ctoken: &mut Option<String>,
) {
    if let Some(node) = item.get("commentThreadRenderer") {
        match value_from_json_value::<response::video_details::CommentThreadRenderer>(node) {
            Some(thread) => {
                if let Some(comment) = thread.comment {
                    match comment.get("commentRenderer").cloned().and_then(|value| {
                        value_from_json_value::<response::video_details::CommentRenderer>(&value)
                    }) {
                        Some(comment) => comments.push(
                            map_comment(
                                comment,
                                mutations,
                                Some(thread.replies),
                                thread.rendering_priority,
                                lang,
                                warnings,
                            )
                            .into(),
                        ),
                        None => warnings
                            .push("comment does not contain commentRenderer field".to_owned()),
                    }
                } else if let Some(vm) = thread.comment_view_model {
                    match vm.get("commentViewModel").cloned().and_then(|value| {
                        value_from_json_value::<response::video_details::CommentViewModel>(&value)
                    }) {
                        Some(vm) => {
                            if let Some(c) = map_comment_vm(
                                vm,
                                mutations,
                                Some(thread.replies),
                                thread.rendering_priority,
                                lang,
                                warnings,
                            ) {
                                comments.push(c.into());
                            }
                        }
                        None => warnings.push(
                            "commentViewModel does not contain commentViewModel field".to_owned(),
                        ),
                    }
                } else {
                    warnings.push(
                        "comment does not contain comment or commentViewModel field".to_owned(),
                    );
                }
            }
            None => warnings.push("could not deserialize commentThreadRenderer".to_owned()),
        }
    } else if let Some(node) = item.get("commentRenderer") {
        match value_from_json_value::<response::video_details::CommentRenderer>(node) {
            Some(comment) => comments.push(
                map_comment(
                    comment,
                    mutations,
                    None,
                    response::video_details::CommentPriority::RenderingPriorityUnknown,
                    lang,
                    warnings,
                )
                .into(),
            ),
            None => warnings.push("could not deserialize commentRenderer".to_owned()),
        }
    } else if let Some(node) = item.get("commentViewModel") {
        match value_from_json_value::<response::video_details::CommentViewModel>(node) {
            Some(vm) => {
                if let Some(c) = map_comment_vm(
                    vm,
                    mutations,
                    None,
                    response::video_details::CommentPriority::RenderingPriorityUnknown,
                    lang,
                    warnings,
                ) {
                    comments.push(c.into());
                }
            }
            None => warnings.push("could not deserialize commentViewModel".to_owned()),
        }
    } else if let Some(node) = item.get("continuationItemRenderer") {
        if ctoken.is_none() {
            *ctoken = comment_continuation_token(node);
        }
    } else if let Some(node) = item.get("commentsHeaderRenderer") {
        *comment_count = node
            .get("countText")
            .cloned()
            .and_then(|value| {
                JsonDoc::new(crate::json::value_to_json_string(&value))
                    .with_root(|root| Ok(root.text()))
                    .ok()
                    .flatten()
            })
            .and_then(|txt| util::parse_numeric_or_warn::<u64>(&txt, warnings));
    }
}

fn comment_continuation_token(node: &JsonValue) -> Option<String> {
    node.get("continuationEndpoint")
        .or_else(|| {
            node.get("button")
                .and_then(|button| button.get("buttonRenderer"))
                .and_then(|renderer| renderer.get("command"))
        })
        .and_then(response::video_details::continuation_token)
}

impl MapEndpoint<Paginator<Comment>> for VideoCommentsEndpoint {
    fn map(
        json: &JsonDoc,
        ctx: &MapRespCtx<'_>,
    ) -> Result<MapResult<Paginator<Comment>>, ExtractionError> {
        json.with_root(|root| {
            let fields = deserialize_video_comments_fields(&root)?;
            map_video_comments_fields(fields, ctx)
        })
    }
}

fn map_recommendations(
    results: &JsonNode<'_>,
    continuations: Option<Vec<response::MusicContinuationData>>,
    visitor_data: Option<String>,
    ctx: &MapRespCtx<'_>,
) -> MapResult<Paginator<VideoItem>> {
    let (mapped, ctoken, _) = response::video_item::map_video_items(results, ctx.lang);

    let ctoken = ctoken.or_else(|| {
        continuations
            .and_then(|c| c.into_iter().next())
            .map(|c| c.next_continuation_data.continuation)
    });

    MapResult {
        c: Paginator::new_ext(
            None,
            mapped.c,
            ctoken,
            visitor_data,
            ContinuationEndpoint::Next,
            ctx.authenticated,
        ),
        warnings: mapped.warnings,
    }
}

fn map_replies(
    mutations: &mut HashMap<String, response::video_details::Payload>,
    replies: Option<JsonValue>,
    priority: response::video_details::CommentPriority,
    lang: Language,
    warnings: &mut Vec<String>,
) -> (Vec<Comment>, Option<String>) {
    let mut reply_ctoken = None;
    let replies = replies
        .map(|replies| {
            replies
                .get("commentRepliesRenderer")
                .and_then(|renderer| renderer.get("contents"))
                .and_then(|contents| contents.as_array())
                .into_iter()
                .flat_map(|items| items.iter())
                .filter_map(|item| {
                    if let Some(node) = item.get("commentRenderer") {
                        value_from_json_value::<response::video_details::CommentRenderer>(node).map(
                            |comment| {
                                map_comment(comment, mutations, None, priority, lang, warnings)
                                    .into()
                            },
                        )
                    } else if let Some(node) = item.get("commentViewModel") {
                        value_from_json_value::<response::video_details::CommentViewModel>(node)
                            .and_then(|vm| {
                                map_comment_vm(vm, mutations, None, priority, lang, warnings)
                            })
                            .map(Into::into)
                    } else if let Some(node) = item.get("continuationItemRenderer") {
                        if reply_ctoken.is_none() {
                            reply_ctoken = comment_continuation_token(node);
                        }
                        None
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    (replies, reply_ctoken)
}

fn map_comment(
    c: response::video_details::CommentRenderer,
    mutations: &mut HashMap<String, response::video_details::Payload>,
    replies: Option<JsonValue>,
    priority: response::video_details::CommentPriority,
    lang: Language,
    warnings: &mut Vec<String>,
) -> Comment {
    let (replies, reply_ctoken) = map_replies(mutations, replies, priority, lang, warnings);

    build_comment(
        CommentParts {
            id: c.comment_id,
            text: c.content_text.into(),
            author: match (c.author_endpoint, c.author_text) {
                (Some(aep), Some(name)) => url_endpoint::browse_endpoint(&aep).map(|aep| {
                    comment_author_tag(
                        aep.browse_endpoint.browse_id,
                        name,
                        c.author_thumbnail.into(),
                        c.author_comment_badge
                            .map(|b| b.icon.into())
                            .unwrap_or_default(),
                    )
                }),
                _ => None,
            },
            publish_date_txt: c.published_time_text,
            like_count: match c.vote_count {
                Some(txt) => util::parse_numeric_or_warn(&txt, warnings),
                None => Some(0),
            },
            reply_count: c.reply_count as u32,
            by_owner: c.author_is_channel_owner,
            hearted: c
                .action_buttons
                .creator_heart
                .map(|h| h.is_hearted)
                .unwrap_or_default(),
        },
        replies,
        reply_ctoken,
        priority,
        lang,
        warnings,
    )
}

fn map_comment_vm(
    vm: response::video_details::CommentViewModel,
    mutations: &mut HashMap<String, response::video_details::Payload>,
    replies: Option<JsonValue>,
    priority: response::video_details::CommentPriority,
    lang: Language,
    warnings: &mut Vec<String>,
) -> Option<Comment> {
    let (replies, reply_ctoken) = map_replies(mutations, replies, priority, lang, warnings);

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

    Some(build_comment(
        CommentParts {
            id: vm.comment_id,
            text: voice_reply
                .filter(|_| ce.properties.content.is_empty())
                .unwrap_or(ce.properties.content)
                .into(),
            by_owner: ce.author.as_ref().map(|a| a.is_creator).unwrap_or_default(),
            author: ce.author.map(|a| {
                comment_author_tag(
                    a.channel_id,
                    a.display_name,
                    ce.avatar.image.into(),
                    if a.is_artist {
                        Verification::Artist
                    } else if a.is_verified {
                        Verification::Verified
                    } else {
                        Verification::None
                    },
                )
            }),
            like_count: parse_num(&ce.toolbar.like_count_notliked),
            reply_count,
            publish_date_txt: ce.properties.published_time,
            hearted,
        },
        replies,
        reply_ctoken,
        priority,
        lang,
        warnings,
    ))
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;
    use crate::{json::json_from_str, model::richtext::ToPlaintext, util::tests::TESTFILES};
    use path_macro::path;
    use rstest::rstest;

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
    #[case::collaborators("collaborators", "G78AnHpIw5w")]
    fn map_video_details(#[case] name: &str, #[case] id: &str) {
        let json_path = path!(*TESTFILES / "video_details" / format!("video_details_{name}.json"));
        let json = JsonDoc::new(std::fs::read_to_string(json_path).unwrap());
        let map_res = VideoDetailsEndpoint::map(&json, &MapRespCtx::test(id)).unwrap();

        assert!(
            map_res.warnings.is_empty(),
            "deserialization/mapping warnings: {:?}",
            map_res.warnings
        );
        if name == "collaborators" {
            insta::assert_ron_snapshot!(format!("map_video_details_{name}"), map_res.c, {
                ".publish_date" => "[date]",
                ".view_count" => "[view_count]",
                ".recommended" => "[recommended omitted]",
            });
        } else {
            insta::assert_ron_snapshot!(format!("map_video_details_{name}"), map_res.c, {
                ".publish_date" => "[date]",
                ".recommended.items[].publish_date" => "[date]",
            });
        }
    }

    #[test]
    fn map_video_details_not_found() {
        let json_path = path!(*TESTFILES / "video_details" / "video_details_not_found.json");
        let json = JsonDoc::new(std::fs::read_to_string(json_path).unwrap());
        let err = VideoDetailsEndpoint::map(&json, &MapRespCtx::test("")).unwrap_err();
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
        let json = JsonDoc::new(std::fs::read_to_string(json_path).unwrap());
        let map_res = VideoCommentsEndpoint::map(&json, &MapRespCtx::test("")).unwrap();

        assert!(
            map_res.warnings.is_empty(),
            "deserialization/mapping warnings: {:?}",
            map_res.warnings
        );
        insta::assert_ron_snapshot!(format!("map_comments_{name}"), map_res.c, {
            ".items[].publish_date" => "[date]",
        });
    }

    #[test]
    fn map_comment_view_model_replies() {
        let replies = json_from_str(
            r#"{
            "commentRepliesRenderer": {
                "contents": [
                    {
                        "commentViewModel": {
                            "commentId": "reply-id",
                            "commentKey": "reply-key",
                            "commentSurfaceKey": "reply-surface-key",
                            "toolbarStateKey": "reply-toolbar-key"
                        }
                    },
                    {
                        "continuationItemRenderer": {
                            "continuationEndpoint": {
                                "continuationCommand": {
                                    "token": "reply-token"
                                }
                            }
                        }
                    }
                ]
            }
        }"#,
        )
        .unwrap();
        let mut mutations = HashMap::from([
            (
                "reply-key".to_owned(),
                response::video_details::Payload::CommentEntityPayload(
                    value_from_json_value(
                        &json_from_str(
                            r#"{
                        "properties": {
                            "content": { "content": "Reply from vm" },
                            "publishedTime": "1 day ago"
                        },
                        "author": {
                            "channelId": "UCreply",
                            "displayName": "@reply",
                            "isVerified": false,
                            "isArtist": false,
                            "isCreator": true
                        },
                        "toolbar": {
                            "likeCountNotliked": "7",
                            "replyCount": "0"
                        },
                        "avatar": {
                            "image": {
                                "sources": [
                                    {
                                        "url": "https://example.com/avatar.jpg",
                                        "width": 88,
                                        "height": 88
                                    }
                                ]
                            }
                        }
                    }"#,
                        )
                        .unwrap(),
                    )
                    .unwrap(),
                ),
            ),
            (
                "reply-toolbar-key".to_owned(),
                response::video_details::Payload::EngagementToolbarStateEntityPayload {
                    heart_state: value_from_json_value(
                        &json_from_str(r#""TOOLBAR_HEART_STATE_HEARTED""#).unwrap(),
                    )
                    .unwrap(),
                },
            ),
        ]);

        let (replies, reply_ctoken) = map_replies(
            &mut mutations,
            Some(replies),
            response::video_details::CommentPriority::RenderingPriorityPinnedComment,
            crate::param::Language::En,
            &mut Vec::new(),
        );

        assert_eq!(reply_ctoken.as_deref(), Some("reply-token"));
        assert_eq!(replies.len(), 1);

        let reply = &replies[0];
        assert_eq!(reply.id, "reply-id");
        assert_eq!(reply.text.to_plaintext(), "Reply from vm");
        assert_eq!(
            reply.author.as_ref().map(|a| a.id.as_str()),
            Some("UCreply")
        );
        assert_eq!(reply.like_count, Some(7));
        assert_eq!(reply.reply_count, 0);
        assert!(reply.by_owner);
        assert!(reply.pinned);
        assert!(reply.hearted);
    }
}
