use std::{borrow::Cow, convert::TryFrom};

use serde::{Deserialize, Deserializer};
use serde_with::{serde_as, DefaultOnError, DeserializeAs, VecSkipError};

use crate::{
    client::response::{
        url_endpoint::{
            MusicPage, MusicPageType, MusicVideoType, NavigationEndpoint, OnTap, PageType,
        },
        AttachmentRun,
    },
    model::{richtext::Style, UrlTarget, Verification},
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
        #[serde(alias = "simpleText", alias = "content")]
        text: String,
    },
    Multiple {
        #[serde_as(as = "Vec<Text>")]
        runs: Vec<String>,
    },
    Str(String),
}

impl<'de> DeserializeAs<'de, String> for Text {
    fn deserialize_as<D>(deserializer: D) -> Result<String, D::Error>
    where
        D: Deserializer<'de>,
    {
        let text = Text::deserialize(deserializer)?;
        match text {
            Text::Simple { text } | Text::Str(text) => Ok(text),
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
            Text::Simple { text } | Text::Str(text) => Ok(vec![text]),
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
        vtype: MusicVideoType,
    },
    Browse {
        text: String,
        page_type: PageType,
        browse_id: String,
        verification: Verification,
    },
    Web {
        text: String,
        url: String,
    },
    Text {
        text: String,
        style: Style,
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
#[serde_as]
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RichTextRun {
    text: String,
    #[serde(default)]
    #[serde_as(as = "DefaultOnError")]
    navigation_endpoint: Option<NavigationEndpoint>,
    #[serde(default)]
    bold: bool,
    #[serde(default)]
    italic: bool,
    #[serde(default)]
    strikethrough: bool,
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
    command_runs: Vec<CommandRun>,
    #[serde(default)]
    #[serde_as(as = "VecSkipError<_>")]
    style_runs: Vec<StyleRun>,
    #[serde(default)]
    #[serde_as(as = "VecSkipError<_>")]
    attachment_runs: Vec<AttachmentRun>,
}

#[serde_as]
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CommandRun {
    start_index: usize,
    length: usize,
    on_tap: OnTap,
    #[serde(default)]
    #[serde_as(as = "DefaultOnError<_>")]
    on_tap_options: Option<AttributedTextOnTapOptions>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct StyleRun {
    start_index: usize,
    length: usize,
    #[serde(default)]
    weight_label: WeightLabel,
    #[serde(default)]
    italic: bool,
    #[serde(default)]
    strikethrough: Strikethrough,
}

#[derive(Default, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
enum WeightLabel {
    FontWeightMedium,
    #[default]
    #[serde(other)]
    FontWeightNormal,
}

#[derive(Default, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
enum Strikethrough {
    LineStyleSingle,
    #[default]
    #[serde(other)]
    None,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct AttributedTextOnTapOptions {
    accessibility_info: AccessibilityInfo,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct AccessibilityInfo {
    accessibility_label: String,
}

struct AttributedTextRun {
    start_index: usize,
    length: usize,
    content: AttributedTextRunContent,
}

enum AttributedTextRunContent {
    Link(NavigationEndpoint, Option<String>),
    Style(Style),
}

impl From<RichTextRun> for TextComponent {
    fn from(run: RichTextRun) -> Self {
        map_text_component(
            run.text,
            Style {
                bold: run.bold,
                italic: run.italic,
                strikethrough: run.strikethrough,
            },
            run.navigation_endpoint,
            Verification::None,
        )
    }
}

impl From<CommandRun> for AttributedTextRun {
    fn from(value: CommandRun) -> Self {
        Self {
            start_index: value.start_index,
            length: value.length,
            content: AttributedTextRunContent::Link(
                value.on_tap.innertube_command,
                value
                    .on_tap_options
                    .map(|o| o.accessibility_info.accessibility_label),
            ),
        }
    }
}

impl StyleRun {
    fn into_attributed_text_run(self) -> Option<AttributedTextRun> {
        let style = Style {
            bold: matches!(self.weight_label, WeightLabel::FontWeightMedium),
            italic: self.italic,
            strikethrough: matches!(self.strikethrough, Strikethrough::LineStyleSingle),
        };
        if style.is_styled() {
            Some(AttributedTextRun {
                start_index: self.start_index,
                length: self.length,
                content: AttributedTextRunContent::Style(style),
            })
        } else {
            None
        }
    }
}

/// Map a single component of a rich text
fn map_text_component(
    text: String,
    style: Style,
    nav: Option<NavigationEndpoint>,
    verification: Verification,
) -> TextComponent {
    match nav {
        Some(NavigationEndpoint::Watch { watch_endpoint }) => TextComponent::Video {
            text,
            video_id: watch_endpoint.video_id,
            start_time: watch_endpoint.start_time_seconds,
            vtype: watch_endpoint
                .watch_endpoint_music_supported_configs
                .watch_endpoint_music_config
                .music_video_type,
        },
        Some(NavigationEndpoint::Browse {
            browse_endpoint,
            command_metadata,
        }) => TextComponent::Browse {
            page_type: match &browse_endpoint.browse_endpoint_context_supported_configs {
                Some(bc) => bc.browse_endpoint_context_music_config.page_type,
                None => match &command_metadata {
                    Some(cm) => cm.web_command_metadata.web_page_type,
                    None => return TextComponent::Text { text, style },
                },
            },
            text,
            browse_id: browse_endpoint.browse_id,
            verification,
        },
        Some(NavigationEndpoint::Url { url_endpoint }) => TextComponent::Web {
            text,
            url: url_endpoint.url,
        },
        Some(NavigationEndpoint::WatchPlaylist {
            watch_playlist_endpoint,
        }) => TextComponent::Browse {
            text,
            page_type: PageType::Playlist,
            browse_id: watch_playlist_endpoint.playlist_id,
            verification,
        },
        None | Some(NavigationEndpoint::CreatePlaylist { .. }) => {
            TextComponent::Text { text, style }
        }
    }
}

impl<'de> Deserialize<'de> for TextComponent {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let text = RichTextInternal::deserialize(deserializer)?;
        text.runs
            .into_iter()
            .next()
            .map(TextComponent::from)
            .ok_or(serde::de::Error::invalid_length(0, &"at least 1"))
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

        let mut runs = text
            .command_runs
            .into_iter()
            .map(AttributedTextRun::from)
            .collect::<Vec<_>>();
        runs.extend(
            text.style_runs
                .into_iter()
                .filter_map(StyleRun::into_attributed_text_run),
        );
        runs.sort_by_key(|run| run.start_index);

        let verification = text
            .attachment_runs
            .into_iter()
            .next()
            .map(Verification::from)
            .unwrap_or_default();
        let mut components: Vec<TextComponent> = Vec::with_capacity(runs.len() + 1);

        fn process_txt_before(components: &[TextComponent], txt_before: Cow<'_, str>) -> String {
            // YouTube sometimes inserts zero-width spaces at the start of comments
            let txt_before = match txt_before.strip_prefix('\u{200b}') {
                Some(t) => Cow::Borrowed(t),
                None => txt_before,
            };
            // Ensure that text after link components always begins with a space
            if !txt_before
                .chars()
                .next()
                .map(|c| c.is_whitespace())
                .unwrap_or_default()
                && components
                    .last()
                    .map(|c| {
                        !matches!(c, TextComponent::Text { .. })
                            && !c
                                .as_str()
                                .chars()
                                .last()
                                .map(|c| c.is_whitespace())
                                .unwrap_or_default()
                    })
                    .unwrap_or_default()
            {
                format!(" {txt_before}")
            } else {
                txt_before.into_owned()
            }
        }

        for run in runs {
            let txt_before = process_txt_before(&components, take_chars(run.start_index).into());
            let txt_run = take_chars(run.start_index + run.length);

            if !txt_before.is_empty() {
                components.push(TextComponent::new(txt_before));
            }
            components.push(match run.content {
                AttributedTextRunContent::Link(link, label) => {
                    // Trim link text:
                    // 3xnbsp, (/ •), nbsp, Name, 2xnbsp
                    // Channel: `\u{a0}\u{a0}\u{a0}/\u{a0}aespa\u{a0}\u{a0}`
                    // Video: `\u{a0}\u{a0}\u{a0}•\u{a0}aespa\u{a0}에스파\u{a0}'Black\u{a0}...\u{a0}\u{a0}`

                    // Replace no-break spaces, trim off whitespace and prefix character
                    let txt_link = txt_run.trim();
                    let txt_link = txt_link.replace('\u{a0}', " ");

                    if let Some(txt_link) = txt_link.strip_prefix(['/', '•']) {
                        let txt_link = txt_link.trim_start();
                        match (&link, label) {
                            (NavigationEndpoint::Url { .. }, Some(label)) => {
                                // Prefix chip-style web links with the service name from accessibility label
                                // Example: `Twitter: aespa_official`
                                if let Some(first_word) = label.split_whitespace().next() {
                                    map_text_component(
                                        format!("{first_word}: {txt_link}"),
                                        Style::default(),
                                        Some(link),
                                        verification,
                                    )
                                } else {
                                    map_text_component(
                                        txt_link.to_owned(),
                                        Style::default(),
                                        Some(link),
                                        verification,
                                    )
                                }
                            }
                            _ => map_text_component(
                                txt_link.to_owned(),
                                Style::default(),
                                Some(link),
                                verification,
                            ),
                        }
                    } else {
                        map_text_component(txt_link, Style::default(), Some(link), verification)
                    }
                }
                AttributedTextRunContent::Style(style) => {
                    map_text_component(txt_run.to_string(), style, None, verification)
                }
            })
        }

        let end = process_txt_before(&components, chars.as_str().into());
        if !end.is_empty() {
            components.push(TextComponent::new(end));
        }

        Ok(TextComponents(components))
    }
}

impl<'de> DeserializeAs<'de, TextComponent> for AttributedText {
    fn deserialize_as<D>(deserializer: D) -> Result<TextComponent, D::Error>
    where
        D: Deserializer<'de>,
    {
        let components: TextComponents = AttributedText::deserialize_as(deserializer)?;
        components
            .0
            .into_iter()
            .next()
            .ok_or(serde::de::Error::invalid_length(0, &"at least 1"))
    }
}

impl<'de> DeserializeAs<'de, String> for AttributedText {
    fn deserialize_as<D>(deserializer: D) -> Result<String, D::Error>
    where
        D: Deserializer<'de>,
    {
        let components: TextComponents = AttributedText::deserialize_as(deserializer)?;
        Ok(components
            .0
            .into_iter()
            .fold(String::new(), |acc, c| acc + c.as_str()))
    }
}

impl TryFrom<TextComponent> for crate::model::ChannelId {
    type Error = ();

    fn try_from(value: TextComponent) -> Result<Self, Self::Error> {
        match value {
            TextComponent::Browse {
                text,
                page_type: PageType::Channel | PageType::Artist,
                browse_id,
                ..
            } => Ok(crate::model::ChannelId {
                id: browse_id,
                name: text,
            }),
            _ => Err(()),
        }
    }
}

impl TryFrom<TextComponent> for crate::model::ChannelTag {
    type Error = ();

    fn try_from(value: TextComponent) -> Result<Self, Self::Error> {
        match value {
            TextComponent::Browse {
                text,
                page_type: PageType::Channel | PageType::Artist,
                browse_id,
                verification,
            } => Ok(crate::model::ChannelTag {
                id: browse_id,
                name: text,
                avatar: Vec::new(),
                verification,
                subscriber_count: None,
            }),
            _ => Err(()),
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
                ..
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
                ..
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
            | TextComponent::Text { text, .. } => Self {
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
                ..
            } => match page_type.to_url_target(browse_id) {
                Some(target) => Self::YouTube { text, target },
                None => Self::Text {
                    text,
                    style: Default::default(),
                },
            },
            TextComponent::Web { text, url } => Self::Web {
                text,
                url: util::sanitize_yt_url(&url),
            },
            TextComponent::Text { text, style } => Self::Text { text, style },
        }
    }
}

impl From<TextComponents> for crate::model::richtext::RichText {
    fn from(components: TextComponents) -> Self {
        Self(components.0.into_iter().map(TextComponent::into).collect())
    }
}

impl TextComponent {
    pub fn new<S: Into<String>>(s: S) -> Self {
        Self::Text {
            text: s.into(),
            style: Style::default(),
        }
    }

    pub fn as_str(&self) -> &str {
        match self {
            TextComponent::Video { text, .. }
            | TextComponent::Browse { text, .. }
            | TextComponent::Web { text, .. }
            | TextComponent::Text { text, .. } => text,
        }
    }

    pub fn into_string(self) -> String {
        match self {
            TextComponent::Video { text, .. }
            | TextComponent::Browse { text, .. }
            | TextComponent::Web { text, .. }
            | TextComponent::Text { text, .. } => text,
        }
    }

    pub fn music_page(self) -> Option<MusicPage> {
        match self {
            TextComponent::Video {
                video_id, vtype, ..
            } => Some(MusicPage {
                id: video_id,
                typ: MusicPageType::Track { vtype },
            }),
            TextComponent::Browse {
                page_type,
                browse_id,
                ..
            } => Some(MusicPage::from_browse(browse_id, page_type)),
            _ => None,
        }
    }
}

impl From<TextComponent> for String {
    fn from(value: TextComponent) -> Self {
        match value {
            TextComponent::Video { text, .. }
            | TextComponent::Browse { text, .. }
            | TextComponent::Web { text, .. }
            | TextComponent::Text { text, .. } => text,
        }
    }
}

impl TextComponents {
    pub fn is_empty(&self) -> bool {
        self.0.iter().all(|c| c.as_str().is_empty())
    }

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

impl std::fmt::Display for TextComponents {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for t in &self.0 {
            f.write_str(t.as_str())?;
        }
        Ok(())
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
    #[case(r#"{"txt":"Hello World"}"#, vec!["Hello World"])]
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
                vtype: Video,
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
                verification: None,
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
                verification: None,
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
                style: Style {
                    bold: false,
                    italic: false,
                    strikethrough: false,
                },
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
                        verification: None,
                    },
                    Text {
                        text: " & ",
                        style: Style {
                            bold: false,
                            italic: false,
                            strikethrough: false,
                        },
                    },
                    Browse {
                        text: "Maite Kelly",
                        page_type: Artist,
                        browse_id: "UCY06CayCwdaOd1CnDgjy6uw",
                        verification: None,
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
        assert!(res.ln.0.is_empty());
    }

    #[test]
    fn t_attributed_description() {
        let json_path = path!(*TESTFILES / "text" / "attributed_description.json");
        let json_file = File::open(json_path).unwrap();
        let res: SAttributed = serde_json::from_reader(BufReader::new(json_file)).unwrap();
        insta::assert_debug_snapshot!(res);
    }

    #[test]
    fn styled_comment() {
        let json_path = path!(*TESTFILES / "text" / "styled_comment.json");
        let json_file = File::open(json_path).unwrap();
        let res: SAttributed = serde_json::from_reader(BufReader::new(json_file)).unwrap();
        insta::assert_debug_snapshot!(res);
    }

    #[test]
    fn split_text_cmp() {
        let text = TextComponents(vec![
            TextComponent::new("Hello"),
            TextComponent::new(" World"),
            TextComponent::new(util::DOT_SEPARATOR),
            TextComponent::new("T2"),
            TextComponent::new(util::DOT_SEPARATOR),
            TextComponent::new("T3"),
        ]);

        let split = text.split(util::DOT_SEPARATOR);
        insta::assert_debug_snapshot!(split);
    }
}
