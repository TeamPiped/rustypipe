use std::convert::TryFrom;

use once_cell::sync::Lazy;
use regex::Regex;
use serde::{Deserialize, Deserializer};
use serde_with::{serde_as, DeserializeAs, VecSkipError};

use crate::{
    client::response::url_endpoint::{MusicVideoType, NavigationEndpoint, PageType},
    model::UrlTarget,
    util,
};

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
pub(crate) enum Text {
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
pub(crate) struct TextComponents(pub Vec<TextComponent>);

#[derive(Debug, Clone)]
pub(crate) enum TextComponent {
    Video {
        text: String,
        video_id: String,
        start_time: u32,
        /// True if the item is a video, false if it is a YTM track
        is_video: bool,
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
    #[serde(default)]
    runs: Vec<RichTextRun>,
}

/// TextLinkRun is a single component from a YouTube text with links
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RichTextRun {
    text: String,
    navigation_endpoint: Option<NavigationEndpoint>,
}

/// This is a new rich text representation format that YouTube is A/B testing
/// at the moment. It consists of the full text and an array of ranges describing
/// the links.
#[serde_as]
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AttributedText {
    content: String,
    #[serde(default)]
    #[serde_as(as = "VecSkipError<_>")]
    command_runs: Vec<AttributedTextRun>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct AttributedTextRun {
    start_index: usize,
    length: usize,
    on_tap: AttributedTextOnTap,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct AttributedTextOnTap {
    innertube_command: NavigationEndpoint,
}

impl From<RichTextRun> for TextComponent {
    fn from(run: RichTextRun) -> Self {
        map_text_component(run.text, run.navigation_endpoint)
    }
}

/// Map a single component of a rich text
fn map_text_component(text: String, nav: Option<NavigationEndpoint>) -> TextComponent {
    match nav {
        Some(NavigationEndpoint::Watch { watch_endpoint }) => TextComponent::Video {
            text,
            video_id: watch_endpoint.video_id,
            start_time: watch_endpoint.start_time_seconds,
            is_video: watch_endpoint
                .watch_endpoint_music_supported_configs
                .watch_endpoint_music_config
                .music_video_type
                == MusicVideoType::Video,
        },
        Some(NavigationEndpoint::Browse {
            browse_endpoint,
            command_metadata,
        }) => TextComponent::Browse {
            page_type: match &browse_endpoint.browse_endpoint_context_supported_configs {
                Some(bc) => bc.browse_endpoint_context_music_config.page_type,
                None => match &command_metadata {
                    Some(cm) => cm.web_command_metadata.web_page_type,
                    None => return TextComponent::Text { text },
                },
            },
            text,
            browse_id: browse_endpoint.browse_id,
        },
        Some(NavigationEndpoint::Url { url_endpoint }) => TextComponent::Web {
            text,
            url: url_endpoint.url,
        },
        None => TextComponent::Text { text },
    }
}

impl<'de> Deserialize<'de> for TextComponent {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let mut text = RichTextInternal::deserialize(deserializer)?;
        if text.runs.len() != 1 {
            return Err(serde::de::Error::invalid_length(
                text.runs.len(),
                &"1 run, use TextComponents for more",
            ));
        }

        Ok(text.runs.swap_remove(0).into())
    }
}

impl<'de> Deserialize<'de> for TextComponents {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let text = RichTextInternal::deserialize(deserializer)?;
        Ok(Self(
            text.runs.into_iter().map(TextComponent::from).collect(),
        ))
    }
}

impl<'de> DeserializeAs<'de, TextComponents> for AttributedText {
    fn deserialize_as<D>(deserializer: D) -> Result<TextComponents, D::Error>
    where
        D: Deserializer<'de>,
    {
        let text = AttributedText::deserialize(deserializer)?;

        let mut i_utf16 = 0;
        let mut chars = text.content.chars();

        // Take a string from the char iterator until the given
        // UTF-16 index. This mimics the Javascript substring behavior.
        let mut take_chars = |until: usize| {
            if until <= i_utf16 {
                return String::new();
            }

            let mut buf = String::with_capacity(until - i_utf16);
            for c in chars.by_ref() {
                buf.push(c);

                // is character on Basic Multilingual Plane -> 16bit in UTF-16,
                // counts as 1 JS character, otherwise 32bit, counts as 2 JS characters
                if (c as u32) > 0xffff {
                    i_utf16 += 1;
                };
                i_utf16 += 1;

                if i_utf16 >= until {
                    break;
                }
            }
            buf
        };

        let mut components = Vec::with_capacity(text.command_runs.len() + 1);
        text.command_runs.into_iter().for_each(|cmd| {
            let txt_before = take_chars(cmd.start_index);
            let txt_link = take_chars(cmd.start_index + cmd.length);

            // Trim link text:
            // 3xnbsp, (/ •), nbsp, Name, 2xnbsp
            // Channel: `\u{a0}\u{a0}\u{a0}/\u{a0}aespa\u{a0}\u{a0}`
            // Video: `\u{a0}\u{a0}\u{a0}•\u{a0}aespa\u{a0}에스파\u{a0}'Black\u{a0}...\u{a0}\u{a0}`

            // Replace no-break spaces, trim off whitespace and prefix character
            let txt_link = txt_link.trim();
            let txt_link = txt_link.replace('\u{a0}', " ");

            static LINK_PREFIX: Lazy<Regex> = Lazy::new(|| Regex::new("^[/•] *").unwrap());
            let txt_link = LINK_PREFIX.replace(&txt_link, "");

            if !txt_before.is_empty() {
                components.push(TextComponent::Text { text: txt_before });
            }
            components.push(map_text_component(
                txt_link.to_string(),
                Some(cmd.on_tap.innertube_command),
            ));
        });

        let end = chars.as_str();
        if !end.is_empty() {
            components.push(TextComponent::Text {
                text: end.to_owned(),
            });
        }

        Ok(TextComponents(components))
    }
}

impl TryFrom<TextComponent> for crate::model::ChannelId {
    type Error = util::MappingError;

    fn try_from(value: TextComponent) -> Result<Self, Self::Error> {
        match value {
            TextComponent::Browse {
                text,
                page_type,
                browse_id,
            } => match page_type {
                PageType::Channel | PageType::Artist => Ok(crate::model::ChannelId {
                    id: browse_id,
                    name: text,
                }),
                _ => Err(util::MappingError("invalid channel link type".into())),
            },
            _ => Err(util::MappingError("invalid channel link".into())),
        }
    }
}

impl TryFrom<TextComponent> for crate::model::AlbumId {
    type Error = ();

    fn try_from(value: TextComponent) -> Result<Self, Self::Error> {
        match value {
            TextComponent::Browse {
                text,
                page_type: PageType::Album,
                browse_id,
            } => Ok(Self {
                id: browse_id,
                name: text,
            }),
            _ => Err(()),
        }
    }
}

impl From<TextComponent> for crate::model::ArtistId {
    fn from(component: TextComponent) -> Self {
        match component {
            TextComponent::Browse {
                text,
                page_type,
                browse_id,
            } => match page_type {
                PageType::Channel | PageType::Artist => Self {
                    id: Some(browse_id),
                    name: text,
                },
                _ => Self {
                    id: None,
                    name: text,
                },
            },
            TextComponent::Video { text, .. }
            | TextComponent::Web { text, .. }
            | TextComponent::Text { text } => Self {
                id: None,
                name: text,
            },
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
                ..
            } => Self::YouTube {
                text,
                target: UrlTarget::Video {
                    id: video_id,
                    start_time,
                },
            },
            TextComponent::Browse {
                text,
                page_type,
                browse_id,
            } => match page_type.to_url_target(browse_id) {
                Some(target) => Self::YouTube { text, target },
                None => Self::Text { text },
            },
            TextComponent::Web { text, url } => Self::Web {
                text,
                url: util::sanitize_yt_url(&url),
            },
            TextComponent::Text { text } => Self::Text { text },
        }
    }
}

impl From<TextComponents> for crate::model::richtext::RichText {
    fn from(components: TextComponents) -> Self {
        Self(components.0.into_iter().map(TextComponent::into).collect())
    }
}

impl TextComponent {
    pub fn as_str(&self) -> &str {
        match self {
            TextComponent::Video { text, .. }
            | TextComponent::Browse { text, .. }
            | TextComponent::Web { text, .. }
            | TextComponent::Text { text } => text,
        }
    }
}

impl TextComponents {
    /// Return the string representation of the first text component
    pub fn first_str(&self) -> &str {
        self.0
            .first()
            .map(TextComponent::as_str)
            .unwrap_or_default()
    }

    /// Split the text components using the given separation string.
    ///
    /// Example: `["Abc", "-", "Hello", "World", "-", "Xyz"]` ->
    /// `["Abc"], ["Hello", "World"], ["Xyz"]`
    pub fn split(self, separator: &str) -> Vec<TextComponents> {
        let mut buf = Vec::new();
        let mut inner = Vec::new();

        for c in self.0 {
            if c.as_str() == separator {
                if !inner.is_empty() {
                    buf.push(TextComponents(inner));
                    inner = Vec::new();
                }
            } else {
                inner.push(c);
            }
        }

        if !inner.is_empty() {
            buf.push(TextComponents(inner));
        }

        buf
    }
}

impl ToString for TextComponents {
    fn to_string(&self) -> String {
        self.0.iter().map(TextComponent::as_str).collect::<String>()
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AccessibilityText {
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
    use std::{fs::File, io::BufReader};

    use path_macro::path;
    use rstest::rstest;
    use serde::Deserialize;
    use serde_with::serde_as;

    use super::*;
    use crate::util::tests::TESTFILES;

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
        #[allow(dead_code)]
        struct S {
            #[serde_as(as = "Text")]
            txt: String,
        }

        #[serde_as]
        #[derive(Deserialize)]
        #[allow(dead_code)]
        struct SVec {
            #[serde_as(as = "Text")]
            txt: Vec<String>,
        }

        let res_str = serde_json::from_str::<S>(test_json).unwrap();
        let res_vec = serde_json::from_str::<SVec>(test_json).unwrap();

        assert_eq!(res_str.txt, exp.join(""));
        assert_eq!(res_vec.txt, exp);
    }

    #[derive(Debug, Deserialize)]
    #[allow(dead_code)]
    struct SLink {
        ln: TextComponent,
    }

    #[derive(Debug, Deserialize)]
    #[allow(dead_code)]
    struct SLinks {
        ln: TextComponents,
    }

    #[serde_as]
    #[derive(Debug, Deserialize)]
    #[allow(dead_code)]
    struct SAttributed {
        #[serde_as(as = "AttributedText")]
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

        let res = serde_json::from_str::<SLink>(test_json).unwrap();
        insta::assert_debug_snapshot!(res, @r###"
        SLink {
            ln: Video {
                text: "DEEP",
                video_id: "wZIoIgz5mbs",
                start_time: 0,
                is_video: true,
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

        let res = serde_json::from_str::<SLink>(test_json).unwrap();
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

        let res = serde_json::from_str::<SLink>(test_json).unwrap();
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

        let res = serde_json::from_str::<SLink>(test_json).unwrap();
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

        let res = serde_json::from_str::<SLink>(test_json).unwrap();
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

        let res = serde_json::from_str::<SLinks>(test_json).unwrap();
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

    #[test]
    fn t_links_empty() {
        let test_json = r#"{"ln": {}}"#;

        let res = serde_json::from_str::<SLinks>(test_json).unwrap();
        assert!(res.ln.0.is_empty())
    }

    #[test]
    fn t_attributed_description() {
        let json_path = path!(*TESTFILES / "text" / "attributed_description.json");
        let json_file = File::open(json_path).unwrap();
        let res: SAttributed = serde_json::from_reader(BufReader::new(json_file)).unwrap();
        insta::assert_debug_snapshot!(res);
    }

    #[test]
    fn split_text_cmp() {
        let text = TextComponents(vec![
            TextComponent::Text {
                text: "Hello".to_owned(),
            },
            TextComponent::Text {
                text: " World".to_owned(),
            },
            TextComponent::Text {
                text: util::DOT_SEPARATOR.to_owned(),
            },
            TextComponent::Text {
                text: "T2".to_owned(),
            },
            TextComponent::Text {
                text: util::DOT_SEPARATOR.to_owned(),
            },
            TextComponent::Text {
                text: "T3".to_owned(),
            },
        ]);

        let split = text.split(util::DOT_SEPARATOR);
        insta::assert_debug_snapshot!(split, @r###"
        [
            TextComponents(
                [
                    Text {
                        text: "Hello",
                    },
                    Text {
                        text: " World",
                    },
                ],
            ),
            TextComponents(
                [
                    Text {
                        text: "T2",
                    },
                ],
            ),
            TextComponents(
                [
                    Text {
                        text: "T3",
                    },
                ],
            ),
        ]
        "###);
    }
}
