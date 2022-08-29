use serde::{Deserialize, Deserializer};
use serde_with::{serde_as, DefaultOnError, DeserializeAs};

/// The YouTube API has multiple ways of outputting text. This deserializer
/// is an attempt to unify them.
///
/// ```json
/// {
///   "text": "Hello World"
/// }
/// ```
///
/// ```json
/// {
///   "simpleText": "Hello World"
/// }
/// ```
///
/// Multiple "runs" of text should be joined with spaces
/// ```json
/// {
///   "runs": [
///     {"text": "Hello"},
///     {"text": "World"},
///   ]
/// }
/// ```
///

#[serde_as]
#[derive(Clone, Debug, Deserialize)]
#[serde(untagged)]
pub enum Text {
    Simple {
        #[serde(alias = "simpleText")]
        text: String,
    },
    Multiple {
        #[serde_as(as = "Vec<crate::serializer::text::Text>")]
        runs: Vec<String>,
    },
}

impl<'de> DeserializeAs<'de, String> for Text {
    fn deserialize_as<D>(deserializer: D) -> Result<String, D::Error>
    where
        D: Deserializer<'de>,
    {
        let text = Text::deserialize(deserializer)?;
        match text {
            Text::Simple { text } => Ok(text),
            Text::Multiple { runs } => Ok(runs.join("")),
        }
    }
}

#[derive(Debug, Clone)]
pub enum TextLink {
    Video {
        title: String,
        video_id: String,
    },
    Browse {
        text: String,
        page_type: PageType,
        browse_id: String,
    },
    None {
        text: String,
    },
}

pub struct TextLinks {}

#[derive(Deserialize)]
struct TextLinkInternal {
    runs: Vec<TextLinkRun>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct TextLinkRun {
    text: String,
    #[serde(default)]
    navigation_endpoint: NavigationEndpoint,
}

#[serde_as]
#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct NavigationEndpoint {
    #[serde(default)]
    #[serde_as(deserialize_as = "DefaultOnError")]
    watch_endpoint: Option<WatchEndpoint>,
    #[serde(default)]
    #[serde_as(deserialize_as = "DefaultOnError")]
    browse_endpoint: Option<BrowseEndpoint>,
    #[serde(default)]
    #[serde_as(deserialize_as = "DefaultOnError")]
    command_metadata: Option<CommandMetadata>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct WatchEndpoint {
    video_id: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct BrowseEndpoint {
    browse_id: String,
    browse_endpoint_context_supported_configs: Option<BrowseEndpointConfig>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct BrowseEndpointConfig {
    browse_endpoint_context_music_config: BrowseEndpointMusicConfig,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct BrowseEndpointMusicConfig {
    page_type: PageType,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CommandMetadata {
    web_command_metadata: WebCommandMetadata,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct WebCommandMetadata {
    web_page_type: PageType,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PageType {
    #[serde(rename = "MUSIC_PAGE_TYPE_ARTIST")]
    Artist,
    #[serde(rename = "MUSIC_PAGE_TYPE_ALBUM")]
    Album,
    #[serde(
        rename = "MUSIC_PAGE_TYPE_USER_CHANNEL",
        alias = "WEB_PAGE_TYPE_CHANNEL"
    )]
    Channel,
    #[serde(rename = "MUSIC_PAGE_TYPE_PLAYLIST", alias = "WEB_PAGE_TYPE_PLAYLIST")]
    Playlist,
}

fn map_text_linkrun(lr: &TextLinkRun) -> Option<TextLink> {
    let text = lr.text.to_owned();
    let nav = &lr.navigation_endpoint;

    Some(match &nav.watch_endpoint {
        Some(w) => TextLink::Video {
            title: text,
            video_id: w.video_id.to_owned(),
        },
        None => match &nav.browse_endpoint {
            Some(b) => TextLink::Browse {
                text,
                page_type: match &b.browse_endpoint_context_supported_configs {
                    Some(bc) => bc.browse_endpoint_context_music_config.page_type,
                    None => match &nav.command_metadata {
                        Some(cm) => cm.web_command_metadata.web_page_type,
                        None => return None,
                    },
                },
                browse_id: b.browse_id.to_owned(),
            },
            None => TextLink::None { text },
        },
    })
}

impl<'de> DeserializeAs<'de, TextLink> for TextLink {
    fn deserialize_as<D>(deserializer: D) -> Result<TextLink, D::Error>
    where
        D: Deserializer<'de>,
    {
        let link = TextLinkInternal::deserialize(deserializer)?;
        if link.runs.len() != 1 {
            return Err(serde::de::Error::invalid_length(
                link.runs.len(),
                &"1 run, use TextLinks for more",
            ));
        }

        Ok(some_or_bail!(
            map_text_linkrun(&link.runs[0]),
            Err(serde::de::Error::custom("missing/invalid browse endpoint"))
        ))
    }
}

impl<'de> DeserializeAs<'de, Vec<TextLink>> for TextLinks {
    fn deserialize_as<D>(deserializer: D) -> Result<Vec<TextLink>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let link = TextLinkInternal::deserialize(deserializer)?;
        Ok(link
            .runs
            .iter()
            .filter_map(|r| map_text_linkrun(r))
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::TextLink;
    use rstest::rstest;
    use serde::Deserialize;
    use serde_with::serde_as;

    #[rstest]
    #[case(
        r#"{
            "txt": {
                "text": "Hello World"
            }
        }"#,
        "Hello World"
    )]
    #[case(
        r#"{
            "txt": {
                "simpleText": "Hello World"
            }
        }"#,
        "Hello World"
    )]
    #[case(
        r#"{
            "txt": {
                "runs": [
                    {
                        "text": "Abo für "
                    },
                    {
                     "text": "MBCkpop"
                    },
                    {
                        "text": " beenden?"
                    }
                ]
            }
        }"#,
        "Abo für MBCkpop beenden?"
    )]
    fn t_deserialize_text(#[case] test_json: &str, #[case] exp: &str) {
        #[serde_as]
        #[derive(Deserialize)]
        struct S {
            #[serde_as(as = "crate::serializer::text::Text")]
            txt: String,
        }

        let res = serde_json::from_str::<S>(&test_json).unwrap();
        assert_eq!(res.txt, exp)
    }

    #[serde_as]
    #[derive(Debug, Deserialize)]
    struct SLink {
        #[serde_as(as = "crate::serializer::text::TextLink")]
        ln: TextLink,
    }

    #[serde_as]
    #[derive(Debug, Deserialize)]
    struct SLinks {
        #[serde_as(as = "crate::serializer::text::TextLinks")]
        ln: Vec<TextLink>,
    }

    #[test]
    fn t_link_video() {
        let test_json = r#"{
            "ln": {
                "runs": [
                    {
                        "text": "DEEP",
                        "navigationEndpoint": {
                            "watchEndpoint": {
                                "videoId": "wZIoIgz5mbs"
                            }
                        }
                    }
                ]
            }
        }"#;

        let res = serde_json::from_str::<SLink>(&test_json).unwrap();
        insta::assert_debug_snapshot!(res, @r###"
        SLink {
            ln: Video {
                title: "DEEP",
                video_id: "wZIoIgz5mbs",
            },
        }
        "###);
    }

    #[test]
    fn t_link_album() {
        let test_json = r#"{
            "ln": {
                "runs": [
                    {
                        "text": "DEEP - The 1st Mini Album",
                        "navigationEndpoint": {
                            "browseEndpoint": {
                                "browseId": "MPREb_TKV2ccxsj5i",
                                "browseEndpointContextSupportedConfigs": {
                                    "browseEndpointContextMusicConfig": {
                                        "pageType": "MUSIC_PAGE_TYPE_ALBUM"
                                    }
                                }
                            }
                        }
                    }
                ]
            }
        }"#;

        let res = serde_json::from_str::<SLink>(&test_json).unwrap();
        insta::assert_debug_snapshot!(res, @r###"
        SLink {
            ln: Browse {
                text: "DEEP - The 1st Mini Album",
                page_type: Album,
                browse_id: "MPREb_TKV2ccxsj5i",
            },
        }
        "###);
    }

    #[test]
    fn t_link_channel() {
        let test_json = r#"{
            "ln": {
                "runs": [
                    {
                        "text": "laserluca",
                        "navigationEndpoint": {
                            "commandMetadata": {
                                "webCommandMetadata": {
                                    "webPageType": "WEB_PAGE_TYPE_CHANNEL"
                                }
                            },
                            "browseEndpoint": {
                                "browseId": "UCmxc6kXbU1J-0pR2F3wIx9A"
                            }
                        }
                    }
                ]
            }
        }"#;

        let res = serde_json::from_str::<SLink>(&test_json).unwrap();
        insta::assert_debug_snapshot!(res, @r###"
        SLink {
            ln: Browse {
                text: "laserluca",
                page_type: Channel,
                browse_id: "UCmxc6kXbU1J-0pR2F3wIx9A",
            },
        }
        "###);
    }

    #[test]
    fn t_link_none() {
        let test_json = r#"{
            "ln": {
                "runs": [
                    {
                        "text": "Hello World"
                    }
                ]
            }
        }"#;

        let res = serde_json::from_str::<SLink>(&test_json).unwrap();
        insta::assert_debug_snapshot!(res, @r###"
        SLink {
            ln: None {
                text: "Hello World",
            },
        }
        "###);
    }

    #[test]
    fn t_links_artists() {
        let test_json = r#"{
            "ln": {
                "runs": [
                    {
                        "text": "Roland Kaiser",
                        "navigationEndpoint": {
                            "clickTrackingParams": "CNAMEMn0AhgFIhMI3aq914Tn-QIVi9ARCB3w6w_p",
                            "browseEndpoint": {
                                "browseId": "UCtqi0viP-suK-okUQfaw8Ew",
                                    "browseEndpointContextSupportedConfigs": {
                                    "browseEndpointContextMusicConfig": {
                                        "pageType": "MUSIC_PAGE_TYPE_ARTIST"
                                    }
                                }
                            }
                        }
                    },
                    { "text": " & " },
                    {
                        "text": "Maite Kelly",
                        "navigationEndpoint": {
                            "clickTrackingParams": "CNAMEMn0AhgFIhMI3aq914Tn-QIVi9ARCB3w6w_p",
                            "browseEndpoint": {
                                "browseId": "UCY06CayCwdaOd1CnDgjy6uw",
                                "browseEndpointContextSupportedConfigs": {
                                    "browseEndpointContextMusicConfig": {
                                        "pageType": "MUSIC_PAGE_TYPE_ARTIST"
                                    }
                                }
                            }
                        }
                    }
                ]
            }
        }"#;

        let res = serde_json::from_str::<SLinks>(&test_json).unwrap();
        insta::assert_debug_snapshot!(res, @r###"
        SLinks {
            ln: [
                Browse {
                    text: "Roland Kaiser",
                    page_type: Artist,
                    browse_id: "UCtqi0viP-suK-okUQfaw8Ew",
                },
                None {
                    text: " & ",
                },
                Browse {
                    text: "Maite Kelly",
                    page_type: Artist,
                    browse_id: "UCY06CayCwdaOd1CnDgjy6uw",
                },
            ],
        }
        "###);
    }
}
