use std::convert::TryFrom;

use anyhow::anyhow;
use serde::{Deserialize, Deserializer};
use serde_with::{serde_as, DefaultOnError, DeserializeAs};

use crate::util;

/// # Text
///
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
/// Multiple "runs" aka components of text should be joined together
/// ```json
/// {
///   "runs": [
///     {"text": "Hello"},
///     {"text": " World"},
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
        #[serde_as(as = "Vec<Text>")]
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

impl<'de> DeserializeAs<'de, Vec<String>> for Text {
    fn deserialize_as<D>(deserializer: D) -> Result<Vec<String>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let text = Text::deserialize(deserializer)?;
        match text {
            Text::Simple { text } => Ok(vec![text]),
            Text::Multiple { runs } => Ok(runs),
        }
    }
}

/// # TextComponent
///
/// Some texts on the YouTube website include links. These can be links to
/// other YouTube entities (Channels, Videos) as well as websites.
///
/// Texts with links are mapped as a list of text components.
#[derive(Default, Debug, Clone)]
pub struct TextComponents(pub Vec<TextComponent>);

#[derive(Debug, Clone)]
pub enum TextComponent {
    Video {
        text: String,
        video_id: String,
        start_time: u32,
    },
    Browse {
        text: String,
        page_type: PageType,
        browse_id: String,
    },
    Web {
        text: String,
        url: String,
    },
    Text {
        text: String,
    },
}

/// YouTube's representation of a text with links. It consists of multiple
/// runs aka components, which can be simple strings or links.
#[derive(Deserialize)]
struct RichTextInternal {
    runs: Vec<RichTextRun>,
}

/// TextLinkRun is a single component from a YouTube text with links
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RichTextRun {
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
    url_endpoint: Option<UrlEndpoint>,
    #[serde(default)]
    #[serde_as(deserialize_as = "DefaultOnError")]
    command_metadata: Option<CommandMetadata>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct WatchEndpoint {
    video_id: String,
    #[serde(default)]
    start_time_seconds: u32,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct BrowseEndpoint {
    browse_id: String,
    browse_endpoint_context_supported_configs: Option<BrowseEndpointConfig>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct UrlEndpoint {
    url: String,
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

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
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

/// Map a single component of a rich text
fn map_richtext_run(lr: &RichTextRun) -> Option<TextComponent> {
    let text = lr.text.to_owned();
    let nav = &lr.navigation_endpoint;

    Some(match &nav.watch_endpoint {
        Some(w) => TextComponent::Video {
            text,
            video_id: w.video_id.to_owned(),
            start_time: w.start_time_seconds,
        },
        None => match &nav.browse_endpoint {
            Some(b) => TextComponent::Browse {
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
            None => match &nav.url_endpoint {
                Some(u) => TextComponent::Web {
                    text,
                    url: u.url.to_owned(),
                },
                None => TextComponent::Text { text },
            },
        },
    })
}

impl<'de> Deserialize<'de> for TextComponent {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let link = RichTextInternal::deserialize(deserializer)?;
        if link.runs.len() != 1 {
            return Err(serde::de::Error::invalid_length(
                link.runs.len(),
                &"1 run, use RichText for more",
            ));
        }

        Ok(some_or_bail!(
            map_richtext_run(&link.runs[0]),
            Err(serde::de::Error::custom("missing/invalid browse endpoint"))
        ))
    }
}

impl<'de> Deserialize<'de> for TextComponents {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let link = RichTextInternal::deserialize(deserializer)?;
        Ok(Self(
            link.runs.iter().filter_map(map_richtext_run).collect(),
        ))
    }
}

impl TryFrom<TextComponent> for crate::model::ChannelId {
    type Error = anyhow::Error;

    fn try_from(value: TextComponent) -> Result<Self, Self::Error> {
        match value {
            TextComponent::Browse {
                text,
                page_type,
                browse_id,
            } => match page_type {
                PageType::Channel => Ok(crate::model::ChannelId {
                    id: browse_id,
                    name: text,
                }),
                _ => Err(anyhow!("invalid channel link type")),
            },
            _ => Err(anyhow!("invalid channel link")),
        }
    }
}

impl From<TextComponent> for crate::model::richtext::TextComponent {
    fn from(component: TextComponent) -> Self {
        match component {
            TextComponent::Video {
                text,
                video_id,
                start_time,
            } => Self::Video {
                text,
                id: video_id,
                start_time,
            },
            TextComponent::Browse {
                text,
                page_type,
                browse_id,
            } => match page_type {
                PageType::Artist => Self::Artist {
                    text,
                    id: browse_id,
                },
                PageType::Album => Self::Album {
                    text,
                    id: browse_id,
                },
                PageType::Channel => Self::Channel {
                    text,
                    id: browse_id,
                },
                PageType::Playlist => Self::Playlist {
                    text,
                    id: browse_id,
                },
            },
            TextComponent::Web { text, url } => Self::Web {
                text,
                url: util::sanitize_yt_url(&url),
            },
            TextComponent::Text { text } => Self::Text(text),
        }
    }
}

impl From<TextComponents> for crate::model::richtext::RichText {
    fn from(components: TextComponents) -> Self {
        Self(components.0.into_iter().map(TextComponent::into).collect())
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccessibilityText {
    accessibility_data: AccessibilityData,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct AccessibilityData {
    label: String,
}

impl<'de> DeserializeAs<'de, String> for AccessibilityText {
    fn deserialize_as<D>(deserializer: D) -> Result<String, D::Error>
    where
        D: Deserializer<'de>,
    {
        let text = AccessibilityText::deserialize(deserializer)?;
        Ok(text.accessibility_data.label)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
        vec!["Hello World"]
    )]
    #[case(
        r#"{
            "txt": {
                "simpleText": "Hello World"
            }
        }"#,
        vec!["Hello World"]
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
        vec!["Abo für ", "MBCkpop", " beenden?"]
    )]
    fn t_deserialize_text(#[case] test_json: &str, #[case] exp: Vec<&str>) {
        #[serde_as]
        #[derive(Deserialize)]
        struct S {
            #[serde_as(as = "Text")]
            txt: String,
        }

        #[serde_as]
        #[derive(Deserialize)]
        struct SVec {
            #[serde_as(as = "Text")]
            txt: Vec<String>,
        }

        let res_str = serde_json::from_str::<S>(&test_json).unwrap();
        let res_vec = serde_json::from_str::<SVec>(&test_json).unwrap();

        assert_eq!(res_str.txt, exp.join(""));
        assert_eq!(res_vec.txt, exp);
    }

    #[derive(Debug, Deserialize)]
    struct SLink {
        ln: TextComponent,
    }

    #[derive(Debug, Deserialize)]
    struct SLinks {
        ln: TextComponents,
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
                text: "DEEP",
                video_id: "wZIoIgz5mbs",
                start_time: 0,
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
            ln: Text {
                text: "Hello World",
            },
        }
        "###);
    }

    #[test]
    fn t_link_web() {
        let test_json = r#"{
            "ln": {
                "runs": [
                    {
                        "text": "Creative Commons",
                        "navigationEndpoint": {
                            "clickTrackingParams": "CJsBEM2rARgBIhMImKz9y6Oc-QIVTJpVCh3VrAYM",
                            "commandMetadata": {
                              "webCommandMetadata": {
                                "url": "https://www.youtube.com/t/creative_commons",
                                "webPageType": "WEB_PAGE_TYPE_UNKNOWN",
                                "rootVe": 83769
                              }
                            },
                            "urlEndpoint": {
                              "url": "https://www.youtube.com/t/creative_commons"
                            }
                        }
                    }
                ]
            }
        }"#;

        let res = serde_json::from_str::<SLink>(&test_json).unwrap();
        insta::assert_debug_snapshot!(res, @r###"
        SLink {
            ln: Web {
                text: "Creative Commons",
                url: "https://www.youtube.com/t/creative_commons",
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
            ln: TextComponents(
                [
                    Browse {
                        text: "Roland Kaiser",
                        page_type: Artist,
                        browse_id: "UCtqi0viP-suK-okUQfaw8Ew",
                    },
                    Text {
                        text: " & ",
                    },
                    Browse {
                        text: "Maite Kelly",
                        page_type: Artist,
                        browse_id: "UCY06CayCwdaOd1CnDgjy6uw",
                    },
                ],
            ),
        }
        "###);
    }
}
