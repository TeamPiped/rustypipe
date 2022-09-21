use std::convert::TryFrom;

use anyhow::{anyhow, bail, Result};
use reqwest::Method;
use serde::Serialize;

use crate::{
    model::{
        Channel, ChannelId, Chapter, Comment, Language, Paginator, RecommendedVideo, VideoDetails,
    },
    serializer::MapResult,
    timeago,
    util::{self, TryRemove},
};

use super::{
    response::{self, IconType, IsLive},
    ClientType, MapResponse, RustyPipeQuery, YTContext,
};

#[derive(Debug, Serialize)]
struct QVideo {
    context: YTContext,
    /// YouTube video ID
    video_id: String,
    /// Set to true to allow extraction of streams with sensitive content
    content_check_ok: bool,
    /// Probably refers to allowing sensitive content, too
    racy_check_ok: bool,
}

#[derive(Debug, Serialize)]
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

                (owner.video_owner_renderer, description.into(), is_ccommons)
            }
            _ => bail!("could not find secondary_info"),
        };

        let (channel_id, channel_name) = match owner.title {
            crate::serializer::text::TextComponent::Browse {
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
                chapters,
                recommended,
                top_comments: Paginator::new(None, Vec::new(), comment_ctoken),
                latest_comments: Paginator::new(None, Vec::new(), latest_comments_ctoken),
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
                            view_count: video
                                .view_count_text
                                .and_then(|txt| util::parse_numeric_or_warn(&txt, &mut warnings)),
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
    use chrono::Datelike;

    use crate::{client::RustyPipe, model::Verification};

    #[tokio::test]
    async fn get_video_details() {
        let rp = RustyPipe::builder().strict().build();
        let details = rp.query().video_details("ZeerrnuLi5E").await.unwrap();

        // dbg!(&details);

        assert_eq!(details.id, "ZeerrnuLi5E");
        assert_eq!(details.title, "aespa 에스파 'Black Mamba' MV");
        insta::assert_yaml_snapshot!(details.description, @r###"
        ---
        - Text: "🎧Listen and download aespa's debut single \"Black Mamba\": "
        - Web:
            text: "https://smarturl.it/aespa_BlackMamba"
            url: "https://smarturl.it/aespa_BlackMamba"
        - Text: "\n🐍The Debut Stage "
        - Video:
            text: "https://youtu.be/Ky5RT5oGg0w"
            id: Ky5RT5oGg0w
            start_time: 0
        - Text: "\n\n🎟️ aespa Showcase SYNK in LA! Tickets now on sale: "
        - Web:
            text: "https://www.ticketmaster.com/event/0A..."
            url: "https://www.ticketmaster.com/event/0A005CCD9E871F6E"
        - Text: "\n\nSubscribe to aespa Official YouTube Channel!\n"
        - Web:
            text: "https://www.youtube.com/aespa?sub_con..."
            url: "https://www.youtube.com/aespa?sub_confirmation=1"
        - Text: "\n\naespa official\n"
        - Web:
            text: "https://www.youtube.com/c/aespa"
            url: "https://www.youtube.com/c/aespa"
        - Text: "\n"
        - Web:
            text: "https://www.instagram.com/aespa_official"
            url: "https://www.instagram.com/aespa_official"
        - Text: "\n"
        - Web:
            text: "https://www.tiktok.com/@aespa_official"
            url: "https://www.tiktok.com/@aespa_official"
        - Text: "\n"
        - Web:
            text: "https://twitter.com/aespa_Official"
            url: "https://twitter.com/aespa_Official"
        - Text: "\n"
        - Web:
            text: "https://www.facebook.com/aespa.official"
            url: "https://www.facebook.com/aespa.official"
        - Text: "\n"
        - Web:
            text: "https://weibo.com/aespa"
            url: "https://weibo.com/aespa"
        - Text: "\n\n"
        - Text: " "
        - Text: " "
        - Text: " "
        - Text: " "
        - Text: "\naespa 에스파 'Black Mamba' MV ℗ SM Entertainment"
        "###);

        assert_eq!(details.channel.id, "UCEf_Bc-KVd7onSeifS3py9g");
        assert_eq!(details.channel.name, "SMTOWN");
        assert!(!details.channel.avatar.is_empty(), "no channel avatars");
        assert_eq!(details.channel.verification, Verification::Verified);
        // TODO: assert!(details.channel.subscriber_count.unwrap() > 30000000, "expected >30M subs, got {}", details.channel.subscriber_count);

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

        // assert!(
        //     details.top_comments.count.unwrap() > 700000,
        //     "expected > 700K comments, got {}",
        //     details.top_comments.count.unwrap()
        // );
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
        insta::assert_yaml_snapshot!(details.description, @r###"
        ---
        - Text: "Provided to YouTube by Universal Music Group\n\nGäa · Oonagh\n\nBest Of\n\n℗ An Airforce1 Records / We Love Music recording; ℗ 2014 Universal Music GmbH\n\nReleased on: 2020-08-07\n\nProducer, Associated  Performer, Background  Vocalist: Hardy Krech\nProducer: Mark Nissen\nAssociated  Performer, Background  Vocalist: Andreas Fahnert\nAssociated  Performer, Background  Vocalist: Velile Mchunu\nAssociated  Performer, Background  Vocalist: Billy King\nAssociated  Performer, Background  Vocalist: Alex Prince\nAssociated  Performer, Flute: Sandro Friedrich\nProgrammer: Hartmut Krech\nEditor: Severin Zahler\nComposer  Lyricist: Hartmut Krech\nComposer  Lyricist: Mark Nissen\nAuthor: Lukas Hainer\nAuthor: Michael Boden\n\nAuto-generated by YouTube."
        "###);

        assert_eq!(details.channel.id, "UCVGvnqB-5znqPSbMGlhF4Pw");
        assert_eq!(details.channel.name, "Sentamusic");
        assert!(!details.channel.avatar.is_empty(), "no channel avatars");
        assert_eq!(details.channel.verification, Verification::Artist);
        // TODO: assert!(details.channel.subscriber_count.unwrap() > 33000, "expected >33K subs, got {}", details.channel.subscriber_count);

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
        insta::assert_yaml_snapshot!(details.description, @r###"
        ---
        - Web:
            text: "https://media.ccc.de/v/36c3-10652-bah..."
            url: "https://media.ccc.de/v/36c3-10652-bahnmining_-_punktlichkeit_ist_eine_zier"
        - Text: "\n\n\n\nSeit Anfang 2019 hat David jeden einzelnen Halt jeder einzelnen Zugfahrt auf jedem einzelnen Fernbahnhof in ganz Deutschland systematisch gespeichert. Inklusive Verspätungen und allem drum und dran. Und die werden wir in einem bunten Vortrag erforschen und endlich mal wieder ein bisschen Spaß mit Daten haben.\n\nRechtlicher Hinweis: Es liegt eine schriftliche Genehmigung der Bahn vor, von ihr abgerufene Rohdaten aggregieren und für Vorträge nutzen zu dürfen. Inhaltliche Absprachen oder gar Auflagen existieren nicht.\n\nDie Bahn gibt ihre Verspätungen in \"Prozent pünktlicher Züge pro Monat\" an. Das ist so radikal zusammengefasst, dass man daraus natürlich nichts interessantes lesen kann. Jetzt stellt euch mal vor, man könnte da mal ein bisschen genauer reingucken.\n\nStellt sich raus: Das geht! Davids Datensatz umfasst knapp 25 Millionen Halte - mehr als 50.000 pro Tag. Wir haben die Rohdaten und sind in unserer Betrachtung völlig frei. \n\nDer Vortrag hat wieder mehrere rote Fäden.\n\n 1) Wir vermessen ein fast komplettes Fernverkehrsjahr der deutschen Bahn.   Hier etwas Erwartungsmanagement: Sinn ist keinesfalls Bahn-Bashing oder Sensationsheischerei - wer einen Hassvortrag gegen die Bahn erwartet, ist in dieser Veranstaltung falsch. Wir werden die Daten aber nutzen, um die Bahn einmal ein bisschen kennenzulernen. Die Bahn ist eine riesige Maschine mit Millionen beweglicher Teile. Wie viele Zugfahrten gibt es überhaupt? Was sind die größten Bahnhöfe? Wir werden natürlich auch die unerfreulichen Themen ansprechen, für die sich im Moment viele interessieren: Ist das Problem mit den Zugverspätungen wirklich so schlimm, wie alle sagen? Gibt es Orte und Zeiten, an denen es besonders hapert? Und wo fallen Züge einfach aus?\n\n 2) Es gibt wieder mehrere Blicke über den Tellerrand, wie bei Davids vorherigen Vorträgen auch. Ihr werdet wieder ganz automatisch und nebenher einen allgemeinverständlichen Einblick in die heutige Datenauswerterei bekommen. (Eine verbreitete Verschwörungstheorie sagt, euch zur Auswertung öffentlicher Daten zu inspirieren, wäre sogar der Hauptzweck von Davids Vorträgen. :-) )Die Welt braucht Leute mit Ratio, die Analyse wichtiger als Kreischerei finden. Und darum beschreibt David auch, wie man so ein durchaus aufwändiges Hobbyprojekt technisch angeht, Anfängerfehler vermeidet, und verantwortungsvoll handelt.\n\nDavid Kriesel\n\n"
        - Web:
            text: "https://fahrplan.events.ccc.de/congre..."
            url: "https://fahrplan.events.ccc.de/congress/2019/Fahrplan/events/10652.html"
        - Text: "\n\n"
        "###);

        assert_eq!(details.channel.id, "UC2TXq_t06Hjdr2g_KdKpHQg");
        assert_eq!(details.channel.name, "media.ccc.de");
        assert!(!details.channel.avatar.is_empty(), "no channel avatars");
        assert_eq!(details.channel.verification, Verification::None);
        // TODO: assert!(details.channel.subscriber_count.unwrap() > 170000, "expected >170K subs, got {}", details.channel.subscriber_count);

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

        // assert!(
        //     details.top_comments.count.unwrap() > 700000,
        //     "expected > 700K comments, got {}",
        //     details.top_comments.count.unwrap()
        // );
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
        insta::assert_yaml_snapshot!(details.description, @r###"
        ---
        - Text: "Thanks to Jackery for sponsoring today's video! Check out Jackery's Solar Generator 2000 Pro and get 10% off with code LinusTechTips at "
        - Web:
            text: "https://lmg.gg/SG2000PROLTT"
            url: "https://lmg.gg/SG2000PROLTT"
        - Text: "\n\nThese days, you can game almost anywhere on the planet, anytime. But what if that planet was in the middle of an apocalypse? After you’ve stashed years of food, water, and toilet paper away, how will you pass the time? With this PC, you can be prepared to game until the whole mess to sort itself out. \n\nDiscuss on the forum: "
        - Web:
            text: "https://linustechtips.com/topic/14554..."
            url: "https://linustechtips.com/topic/1455447-the-prepper-pc-sponsored/"
        - Text: "\n\nBuy a Jackery Solar Generator 2000 Pro: "
        - Web:
            text: "https://geni.us/034L"
            url: "https://geni.us/034L"
        - Text: "\n\nBuy a Jackery Explorer 2000 Pro: "
        - Web:
            text: "https://lmg.gg/1dyF4"
            url: "https://lmg.gg/1dyF4"
        - Text: "\n\nBuy a Seasonic Fanless TX: "
        - Web:
            text: "https://geni.us/S0Wt76G"
            url: "https://geni.us/S0Wt76G"
        - Text: "\n\nBuy an Intel Core i3 (12th Gen) i3-12100: "
        - Web:
            text: "https://geni.us/hLZvxa"
            url: "https://geni.us/hLZvxa"
        - Text: "\n\nBuy an RTX 3050: "
        - Web:
            text: "https://geni.us/6A6hl"
            url: "https://geni.us/6A6hl"
        - Text: "\n\nBuy an RX 6500XT: "
        - Web:
            text: "https://geni.us/fUF1p"
            url: "https://geni.us/fUF1p"
        - Text: "\n\nPurchases made through some store links may provide some compensation to Linus Media Group.\n\n► GET MERCH: "
        - Web:
            text: "https://lttstore.com"
            url: "https://lttstore.com/"
        - Text: "\n► SUPPORT US ON FLOATPLANE: "
        - Web:
            text: "https://www.floatplane.com/ltt"
            url: "https://www.floatplane.com/ltt"
        - Text: "\n► AFFILIATES, SPONSORS & REFERRALS: "
        - Web:
            text: "https://lmg.gg/sponsors"
            url: "https://lmg.gg/sponsors"
        - Text: "\n► PODCAST GEAR: "
        - Web:
            text: "https://lmg.gg/podcastgear"
            url: "https://lmg.gg/podcastgear"
        - Text: "\n\n\nFOLLOW US \n---------------------------------------------------  \nTwitter: "
        - Web:
            text: "https://twitter.com/linustech"
            url: "https://twitter.com/linustech"
        - Text: "\nFacebook: "
        - Web:
            text: "http://www.facebook.com/LinusTech"
            url: "http://www.facebook.com/LinusTech"
        - Text: "\nInstagram: "
        - Web:
            text: "https://www.instagram.com/linustech"
            url: "https://www.instagram.com/linustech"
        - Text: "\nTikTok: "
        - Web:
            text: "https://www.tiktok.com/@linustech"
            url: "https://www.tiktok.com/@linustech"
        - Text: "\nTwitch: "
        - Web:
            text: "https://www.twitch.tv/linustech"
            url: "https://www.twitch.tv/linustech"
        - Text: "\n\nMUSIC CREDIT\n---------------------------------------------------\nIntro: Laszlo - Supernova\nVideo Link: "
        - Video:
            text: "https://www.youtube.com/watch?v=PKfxm..."
            id: PKfxmFU3lWY
            start_time: 0
        - Text: "\niTunes Download Link: "
        - Web:
            text: "https://itunes.apple.com/us/album/sup..."
            url: "https://itunes.apple.com/us/album/supernova/id936805712"
        - Text: "\nArtist Link: "
        - Web:
            text: "https://soundcloud.com/laszlomusic"
            url: "https://soundcloud.com/laszlomusic"
        - Text: "\n\nOutro: Approaching Nirvana - Sugar High\nVideo Link: "
        - Video:
            text: "https://www.youtube.com/watch?v=ngsGB..."
            id: ngsGBSCDwcI
            start_time: 0
        - Text: "\nListen on Spotify: "
        - Web:
            text: "http://spoti.fi/UxWkUw"
            url: "http://spoti.fi/UxWkUw"
        - Text: "\nArtist Link: "
        - Web:
            text: "http://www.youtube.com/approachingnir..."
            url: "http://www.youtube.com/approachingnirvana"
        - Text: "\n\nIntro animation by MBarek Abdelwassaa "
        - Web:
            text: "https://www.instagram.com/mbarek_abdel/"
            url: "https://www.instagram.com/mbarek_abdel/"
        - Text: "\nMonitor And Keyboard by vadimmihalkevich / CC BY 4.0  "
        - Web:
            text: "https://geni.us/PgGWp"
            url: "https://geni.us/PgGWp"
        - Text: "\nMechanical RGB Keyboard by BigBrotherECE / CC BY 4.0 "
        - Web:
            text: "https://geni.us/mj6pHk4"
            url: "https://geni.us/mj6pHk4"
        - Text: "\nMouse Gamer free Model By Oscar Creativo / CC BY 4.0 "
        - Web:
            text: "https://geni.us/Ps3XfE"
            url: "https://geni.us/Ps3XfE"
        - Text: "\n\nCHAPTERS\n---------------------------------------------------\n"
        - Video:
            text: "0:00"
            id: nFDBxBUfE74
            start_time: 0
        - Text: " Intro\n"
        - Video:
            text: "0:42"
            id: nFDBxBUfE74
            start_time: 42
        - Text: " The PC Built for Super Efficiency\n"
        - Video:
            text: "2:41"
            id: nFDBxBUfE74
            start_time: 161
        - Text: " Our BURIAL ENCLOSURE?!\n"
        - Video:
            text: "3:31"
            id: nFDBxBUfE74
            start_time: 211
        - Text: " Our Power Solution (Thanks Jackery!)\n"
        - Video:
            text: "4:47"
            id: nFDBxBUfE74
            start_time: 287
        - Text: " Diggin' Holes\n"
        - Video:
            text: "5:30"
            id: nFDBxBUfE74
            start_time: 330
        - Text: " Colonoscopy?\n"
        - Video:
            text: "7:04"
            id: nFDBxBUfE74
            start_time: 424
        - Text: " Diggin' like a man\n"
        - Video:
            text: "8:29"
            id: nFDBxBUfE74
            start_time: 509
        - Text: " The world's worst woodsman\n"
        - Video:
            text: "9:03"
            id: nFDBxBUfE74
            start_time: 543
        - Text: " Backyard cable management\n"
        - Video:
            text: "10:02"
            id: nFDBxBUfE74
            start_time: 602
        - Text: " Time to bury this boy\n"
        - Video:
            text: "10:46"
            id: nFDBxBUfE74
            start_time: 646
        - Text: " Solar Power Generation\n"
        - Video:
            text: "11:37"
            id: nFDBxBUfE74
            start_time: 697
        - Text: " Issues\n"
        - Video:
            text: "12:08"
            id: nFDBxBUfE74
            start_time: 728
        - Text: " First Play Test\n"
        - Video:
            text: "13:20"
            id: nFDBxBUfE74
            start_time: 800
        - Text: " Conclusion"
        "###);

        assert_eq!(details.channel.id, "UCXuqSBlHAE6Xw-yeJA0Tunw");
        assert_eq!(details.channel.name, "Linus Tech Tips");
        assert!(!details.channel.avatar.is_empty(), "no channel avatars");
        assert_eq!(details.channel.verification, Verification::Verified);
        // TODO: assert!(details.channel.subscriber_count.unwrap() > 14700000, "expected >14.7M subs, got {}", details.channel.subscriber_count);

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

        insta::assert_yaml_snapshot!(details.chapters, {
            "[].thumbnail" => insta::dynamic_redaction(move |value, _path| {
                assert!(!value.as_slice().unwrap().is_empty());
                "[ok]"
            }),
        }, @r###"
        ---
        - title: Intro
          position: 0
          thumbnail: "[ok]"
        - title: The PC Built for Super Efficiency
          position: 42
          thumbnail: "[ok]"
        - title: Our BURIAL ENCLOSURE?!
          position: 161
          thumbnail: "[ok]"
        - title: Our Power Solution (Thanks Jackery!)
          position: 211
          thumbnail: "[ok]"
        - title: "Diggin' Holes"
          position: 287
          thumbnail: "[ok]"
        - title: Colonoscopy?
          position: 330
          thumbnail: "[ok]"
        - title: "Diggin' like a man"
          position: 424
          thumbnail: "[ok]"
        - title: "The world's worst woodsman"
          position: 509
          thumbnail: "[ok]"
        - title: Backyard cable management
          position: 543
          thumbnail: "[ok]"
        - title: Time to bury this boy
          position: 602
          thumbnail: "[ok]"
        - title: Solar Power Generation
          position: 646
          thumbnail: "[ok]"
        - title: Issues
          position: 697
          thumbnail: "[ok]"
        - title: First Play Test
          position: 728
          thumbnail: "[ok]"
        - title: Conclusion
          position: 800
          thumbnail: "[ok]"
        "###);

        assert!(!details.recommended.items.is_empty());
        assert!(!details.recommended.is_exhausted());

        // assert!(
        //     details.top_comments.count.unwrap() > 700000,
        //     "expected > 700K comments, got {}",
        //     details.top_comments.count.unwrap()
        // );
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
        // TODO: not full description
        insta::assert_yaml_snapshot!(details.description, @r###"
        ---
        - Text: "Live NASA - Views Of Earth from Space\nLive video feed of Earth from the International Space Station (ISS) Cameras\n-----------------------------------------------------------------------------------------------------\nWatch our latest video - The Sun - 4K Video / Solar Flares\n"
        - Video:
            text: "https://www.youtube.com/watch?v=SEzK4..."
            id: SEzK4ZfMvUQ
            start_time: 0
        - Text: "\n-----------------------------------------------------------------------------------------------------\nNasa ISS live stream from aboard the International Space Station  as it circles the earth at 240 miles above the planet, on the edge of space in low earth orbit. \n\nThe station is crewed by NASA astronauts as well as Russian Cosmonauts and a mixture of Japanese, Canadian and European astronauts as well.\n\n"
        - Text: " "
        - Text: " "
        - Text: " "
        - Text: " "
        - Text: "\n\nThe  Expedition 67 Crew are: \n Sergey Korsakov\nOleg Artemyev\nDenis Matveev\nKjell Lindgren\nRobert Hines\nJessica Watkins\nSamantha Cristoforetti\n\nYulia Peresild\nKlim Shipenko - onboard as part of a film.\n\nTHIS WILL SHOW LIVE and  PRE-RECORDED FOOTAGE - depending on signal from the station or if the ISS is on the night side of Earth.\n\nWhen the feed is live the words LIVE NOW will appear in the top left hand corner of the screen.\nAs the Space Station passes into a period of night every 45 mins video is unavailable - during this time, and other breaks in transmission,  recorded footage is shown .\nWhen back in daylight the live stream of earth will recommence\n\nIf you are here to talk about a flat earth then please don't bother. You can stay and watch our beautiful globe earth as it spins in space , but please don't share your nonsense beliefs in our chat.\n\nGot a question about this feed? Read our FAQ's\n"
        - Web:
            text: "https://spacevideosfaq.tumblr.com/"
            url: "https://spacevideosfaq.tumblr.com/"
        - Text: "\n\nWatch the earth roll by courtesy of the NASA Live cameras\nInternational Space Station Live Feed: Thanks to NASA for this\n"
        - Web:
            text: "http://www.nasa.gov"
            url: "http://www.nasa.gov/"
        - Text: " The ISS passes into the dark side of the earth for roughly half of each of its 90 minute orbits. During this time no video is available.\n\nMusic by Kevin Macleod \n"
        - Web:
            text: "http://incompetech.com/music/royalty-..."
            url: "http://incompetech.com/music/royalty-free/"
        "###);

        assert_eq!(details.channel.id, "UCakgsb0w7QB0VHdnCc-OVEA");
        assert_eq!(details.channel.name, "Space Videos");
        assert!(!details.channel.avatar.is_empty(), "no channel avatars");
        assert_eq!(details.channel.verification, Verification::Verified);
        // TODO: assert!(details.channel.subscriber_count.unwrap() > 5500000, "expected >5.5M subs, got {}", details.channel.subscriber_count);

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
        insta::assert_yaml_snapshot!(details.description, @r###"
        ---
        []
        "###);

        assert_eq!(details.channel.id, "UCQT2yul0lr6Ie9qNQNmw-sg");
        assert_eq!(details.channel.name, "PrinceOfFALLEN");
        assert!(!details.channel.avatar.is_empty(), "no channel avatars");
        assert_eq!(details.channel.verification, Verification::None);
        // TODO: assert!(details.channel.subscriber_count.unwrap() > 1400, "expected >1400 subs, got {}", details.channel.subscriber_count);

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
}
