use std::convert::TryFrom;

use fancy_regex::Regex;
use once_cell::sync::Lazy;
use serde::{Deserialize, Deserializer};
use serde_with::{serde_as, DeserializeAs};

use crate::{
    client::response::url_endpoint::{NavigationEndpoint, PageType},
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

/// This is a new rich text representation format that YouTube is A/B testing
/// at the moment. It consists of the full text and an array of ranges describing
/// the links.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AttributedText {
    content: String,
    #[serde(default)]
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
fn map_text_component(text: String, nav: NavigationEndpoint) -> TextComponent {
    match nav.watch_endpoint {
        Some(w) => TextComponent::Video {
            text,
            video_id: w.video_id,
            start_time: w.start_time_seconds,
        },
        None => match nav.browse_endpoint {
            Some(b) => TextComponent::Browse {
                page_type: match &b.browse_endpoint_context_supported_configs {
                    Some(bc) => bc.browse_endpoint_context_music_config.page_type,
                    None => match &nav.command_metadata {
                        Some(cm) => cm.web_command_metadata.web_page_type,
                        None => return TextComponent::Text { text },
                    },
                },
                text,
                browse_id: b.browse_id,
            },
            None => match nav.url_endpoint {
                Some(u) => TextComponent::Web { text, url: u.url },
                None => TextComponent::Text { text },
            },
        },
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
                cmd.on_tap.innertube_command,
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
                PageType::Channel => Ok(crate::model::ChannelId {
                    id: browse_id,
                    name: text,
                }),
                _ => Err(util::MappingError("invalid channel link type".into())),
            },
            _ => Err(util::MappingError("invalid channel link".into())),
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
            } => Self::YouTube {
                text,
                target: page_type.to_url_target(browse_id),
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

        let res_str = serde_json::from_str::<S>(&test_json).unwrap();
        let res_vec = serde_json::from_str::<SVec>(&test_json).unwrap();

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

    #[test]
    fn t_attributed_description() {
        let test_json = r#"{
            "ln": {
                "content": "🎧Listen and download aespa's debut single \"Black Mamba\": https://smarturl.it/aespa_BlackMamba\n🐍The Debut Stage    • aespa 에스파 'Black ...  \n\n🎟️ aespa Showcase SYNK in LA! Tickets now on sale: https://www.ticketmaster.com/event/0A...\n\nSubscribe to aespa Official YouTube Channel!\nhttps://www.youtube.com/aespa?sub_con...\n\naespa official\n   / aespa  \nhttps://www.instagram.com/aespa_official\nhttps://www.tiktok.com/@aespa_official\nhttps://twitter.com/aespa_Official\nhttps://www.facebook.com/aespa.official\nhttps://weibo.com/aespa\n\n#aespa #æspa #BlackMamba #블랙맘바 #에스파\naespa 에스파 'Black Mamba' MV ℗ SM Entertainment",
                "commandRuns": [
                  {
                    "startIndex": 58,
                    "length": 36,
                    "onTap": {
                      "innertubeCommand": {
                        "clickTrackingParams": "CJ0BEM2rARgBIhMIzvHr0sis-gIV0kZ6BR0GNA_4SJGXrtzn9erzZQ==",
                        "commandMetadata": {
                          "webCommandMetadata": {
                            "url": "https://www.youtube.com/redirect?event=video_description&redir_token=QUFFLUhqbm1qRVVfQUlObURLcnFFQXBTUkJSOEpqWGIzUXxBQ3Jtc0tsNUJIYm5xdERxZk9rZEw3YlJzV0ZIYTNaSjU2a21PaFhNUmxzdjI5VE1VRWUyczZwYmtmQXh3QXV0eXlkMDgxRUJoNVMzRFZ6RlZ6MGdXeXdWQXFTTGY2ZHhFcUFqdExRQ21PYzNfWmlBaHhqYXVUdw&q=https%3A%2F%2Fsmarturl.it%2Faespa_BlackMamba&v=ZeerrnuLi5E",
                            "webPageType": "WEB_PAGE_TYPE_UNKNOWN",
                            "rootVe": 83769
                          }
                        },
                        "urlEndpoint": {
                          "url": "https://www.youtube.com/redirect?event=video_description&redir_token=QUFFLUhqbm1qRVVfQUlObURLcnFFQXBTUkJSOEpqWGIzUXxBQ3Jtc0tsNUJIYm5xdERxZk9rZEw3YlJzV0ZIYTNaSjU2a21PaFhNUmxzdjI5VE1VRWUyczZwYmtmQXh3QXV0eXlkMDgxRUJoNVMzRFZ6RlZ6MGdXeXdWQXFTTGY2ZHhFcUFqdExRQ21PYzNfWmlBaHhqYXVUdw&q=https%3A%2F%2Fsmarturl.it%2Faespa_BlackMamba&v=ZeerrnuLi5E",
                          "target": "TARGET_NEW_WINDOW",
                          "nofollow": true
                        }
                      }
                    }
                  },
                  {
                    "startIndex": 113,
                    "length": 27,
                    "onTap": {
                      "innertubeCommand": {
                        "clickTrackingParams": "CJ0BEM2rARgBIhMIzvHr0sis-gIV0kZ6BR0GNA_4",
                        "commandMetadata": {
                          "webCommandMetadata": {
                            "url": "/watch?v=Ky5RT5oGg0w&t=0s",
                            "webPageType": "WEB_PAGE_TYPE_WATCH",
                            "rootVe": 3832
                          }
                        },
                        "watchEndpoint": {
                          "videoId": "Ky5RT5oGg0w",
                          "startTimeSeconds": 0,
                          "watchEndpointSupportedOnesieConfig": {
                            "html5PlaybackOnesieConfig": {
                              "commonConfig": {
                                "url": "https://rr5---sn-h0jeener.googlevideo.com/initplayback?source=youtube&orc=1&oeis=1&c=WEB&oad=3200&ovd=3200&oaad=11000&oavd=11000&ocs=700&oewis=1&oputc=1&ofpcc=1&msp=1&odeak=1&odepv=1&osfc=1&id=2b2e514f9a06834c&ip=2003%3Ade%3Aaf30%3A200%3Ad8ce%3A4044%3A2ba2%3A3881&initcwndbps=1556250&mt=1663992556&oweuc="
                              }
                            }
                          }
                        }
                      }
                    }
                  },
                  {
                    "startIndex": 194,
                    "length": 40,
                    "onTap": {
                      "innertubeCommand": {
                        "clickTrackingParams": "CJ0BEM2rARgBIhMIzvHr0sis-gIV0kZ6BR0GNA_4SJGXrtzn9erzZQ==",
                        "commandMetadata": {
                          "webCommandMetadata": {
                            "url": "https://www.youtube.com/redirect?event=video_description&redir_token=QUFFLUhqbU1ObGNaRDZaRmo1X1ZjejBoeTRnWkxuVUJxZ3xBQ3Jtc0ttWk1BVVhaRXRfN1VYWXBqMHdaYURTRFJNcUZJVlY3a21wRHE2ZGZaclE3WUM5bEZWbmFfT0sxWTZHOVotWVh6U3YtVk94SlA5NkRFTnBPcHVCWDJhMGJRQlI3ZHN0MnJleHp0c2lEVWNxeW1jSDZuVQ&q=https%3A%2F%2Fwww.ticketmaster.com%2Fevent%2F0A005CCD9E871F6E&v=ZeerrnuLi5E",
                            "webPageType": "WEB_PAGE_TYPE_UNKNOWN",
                            "rootVe": 83769
                          }
                        },
                        "urlEndpoint": {
                          "url": "https://www.youtube.com/redirect?event=video_description&redir_token=QUFFLUhqbU1ObGNaRDZaRmo1X1ZjejBoeTRnWkxuVUJxZ3xBQ3Jtc0ttWk1BVVhaRXRfN1VYWXBqMHdaYURTRFJNcUZJVlY3a21wRHE2ZGZaclE3WUM5bEZWbmFfT0sxWTZHOVotWVh6U3YtVk94SlA5NkRFTnBPcHVCWDJhMGJRQlI3ZHN0MnJleHp0c2lEVWNxeW1jSDZuVQ&q=https%3A%2F%2Fwww.ticketmaster.com%2Fevent%2F0A005CCD9E871F6E&v=ZeerrnuLi5E",
                          "target": "TARGET_NEW_WINDOW",
                          "nofollow": true
                        }
                      }
                    }
                  },
                  {
                    "startIndex": 281,
                    "length": 40,
                    "onTap": {
                      "innertubeCommand": {
                        "clickTrackingParams": "CJ0BEM2rARgBIhMIzvHr0sis-gIV0kZ6BR0GNA_4",
                        "commandMetadata": {
                          "webCommandMetadata": {
                            "url": "https://www.youtube.com/aespa?sub_confirmation=1",
                            "webPageType": "WEB_PAGE_TYPE_UNKNOWN",
                            "rootVe": 83769
                          }
                        },
                        "urlEndpoint": {
                          "url": "https://www.youtube.com/aespa?sub_confirmation=1",
                          "nofollow": true
                        }
                      }
                    }
                  },
                  {
                    "startIndex": 338,
                    "length": 12,
                    "onTap": {
                      "innertubeCommand": {
                        "clickTrackingParams": "CJ0BEM2rARgBIhMIzvHr0sis-gIV0kZ6BR0GNA_4",
                        "commandMetadata": {
                          "webCommandMetadata": {
                            "url": "https://www.youtube.com/c/aespa",
                            "webPageType": "WEB_PAGE_TYPE_UNKNOWN",
                            "rootVe": 83769
                          }
                        },
                        "urlEndpoint": {
                          "url": "https://www.youtube.com/c/aespa",
                          "nofollow": true
                        }
                      }
                    }
                  },
                  {
                    "startIndex": 351,
                    "length": 40,
                    "onTap": {
                      "innertubeCommand": {
                        "clickTrackingParams": "CJ0BEM2rARgBIhMIzvHr0sis-gIV0kZ6BR0GNA_4SJGXrtzn9erzZQ==",
                        "commandMetadata": {
                          "webCommandMetadata": {
                            "url": "https://www.youtube.com/redirect?event=video_description&redir_token=QUFFLUhqbE9FVEtZZkVLUExjdFBnZjZnZ19KNWRYOVZUd3xBQ3Jtc0tsbHpCa1hLTVJ6MEllczlzUEpoVi1IQ2F5NG1jMnlOT3p3bnlFeE80ZzlsaG5CUXlFQnFGTkMtN19DcVYzQkw3bVlVVmNwQlpYQWZnNGNsME45WE1WQ21sR3V1Z3k5RG9DUDE0VTZQTm53Mk9vTWhiOA&q=https%3A%2F%2Fwww.instagram.com%2Faespa_official&v=ZeerrnuLi5E",
                            "webPageType": "WEB_PAGE_TYPE_UNKNOWN",
                            "rootVe": 83769
                          }
                        },
                        "urlEndpoint": {
                          "url": "https://www.youtube.com/redirect?event=video_description&redir_token=QUFFLUhqbE9FVEtZZkVLUExjdFBnZjZnZ19KNWRYOVZUd3xBQ3Jtc0tsbHpCa1hLTVJ6MEllczlzUEpoVi1IQ2F5NG1jMnlOT3p3bnlFeE80ZzlsaG5CUXlFQnFGTkMtN19DcVYzQkw3bVlVVmNwQlpYQWZnNGNsME45WE1WQ21sR3V1Z3k5RG9DUDE0VTZQTm53Mk9vTWhiOA&q=https%3A%2F%2Fwww.instagram.com%2Faespa_official&v=ZeerrnuLi5E",
                          "target": "TARGET_NEW_WINDOW",
                          "nofollow": true
                        }
                      }
                    }
                  },
                  {
                    "startIndex": 392,
                    "length": 38,
                    "onTap": {
                      "innertubeCommand": {
                        "clickTrackingParams": "CJ0BEM2rARgBIhMIzvHr0sis-gIV0kZ6BR0GNA_4SJGXrtzn9erzZQ==",
                        "commandMetadata": {
                          "webCommandMetadata": {
                            "url": "https://www.youtube.com/redirect?event=video_description&redir_token=QUFFLUhqbVdlSGk3eDd5U0dUVG16VFJCQnhKVFBEUUxMQXxBQ3Jtc0tuX3ZJbENNY1ZSN0FFemdxTFdlcTVvc3AwZE05NEFvRW5nOHpZWDUtZG9ORHBnT1JGc2UySDh3WWl3MU53VjFvbHRSdjdxMUlGM2Z6SmdaLTVaWWxhamJEems0Uld3MGlTT0Z0bkh5Y0hpcnY1aXptSQ&q=https%3A%2F%2Fwww.tiktok.com%2F%40aespa_official&v=ZeerrnuLi5E",
                            "webPageType": "WEB_PAGE_TYPE_UNKNOWN",
                            "rootVe": 83769
                          }
                        },
                        "urlEndpoint": {
                          "url": "https://www.youtube.com/redirect?event=video_description&redir_token=QUFFLUhqbVdlSGk3eDd5U0dUVG16VFJCQnhKVFBEUUxMQXxBQ3Jtc0tuX3ZJbENNY1ZSN0FFemdxTFdlcTVvc3AwZE05NEFvRW5nOHpZWDUtZG9ORHBnT1JGc2UySDh3WWl3MU53VjFvbHRSdjdxMUlGM2Z6SmdaLTVaWWxhamJEems0Uld3MGlTT0Z0bkh5Y0hpcnY1aXptSQ&q=https%3A%2F%2Fwww.tiktok.com%2F%40aespa_official&v=ZeerrnuLi5E",
                          "target": "TARGET_NEW_WINDOW",
                          "nofollow": true
                        }
                      }
                    }
                  },
                  {
                    "startIndex": 431,
                    "length": 34,
                    "onTap": {
                      "innertubeCommand": {
                        "clickTrackingParams": "CJ0BEM2rARgBIhMIzvHr0sis-gIV0kZ6BR0GNA_4SJGXrtzn9erzZQ==",
                        "commandMetadata": {
                          "webCommandMetadata": {
                            "url": "https://www.youtube.com/redirect?event=video_description&redir_token=QUFFLUhqa3lNcG1lMHkwSzNLQVBrUXFNTXl0N1hNa04wUXxBQ3Jtc0tubm1sQkdaVjNYR04xOHpJV3NxZVBpb3I5V1FVOHVFNC1uWE5vb211ZmZKYzhTZXZfbjlkY09fanBRdHpjUkdRVGJJYS0xZ3NBNkVZQVhWSS0xVDYwRlRzQ0J3ODQxNDE0ODAxd1Q0cG5icVlNWndscw&q=https%3A%2F%2Ftwitter.com%2Faespa_Official&v=ZeerrnuLi5E",
                            "webPageType": "WEB_PAGE_TYPE_UNKNOWN",
                            "rootVe": 83769
                          }
                        },
                        "urlEndpoint": {
                          "url": "https://www.youtube.com/redirect?event=video_description&redir_token=QUFFLUhqa3lNcG1lMHkwSzNLQVBrUXFNTXl0N1hNa04wUXxBQ3Jtc0tubm1sQkdaVjNYR04xOHpJV3NxZVBpb3I5V1FVOHVFNC1uWE5vb211ZmZKYzhTZXZfbjlkY09fanBRdHpjUkdRVGJJYS0xZ3NBNkVZQVhWSS0xVDYwRlRzQ0J3ODQxNDE0ODAxd1Q0cG5icVlNWndscw&q=https%3A%2F%2Ftwitter.com%2Faespa_Official&v=ZeerrnuLi5E",
                          "target": "TARGET_NEW_WINDOW",
                          "nofollow": true
                        }
                      }
                    }
                  },
                  {
                    "startIndex": 466,
                    "length": 39,
                    "onTap": {
                      "innertubeCommand": {
                        "clickTrackingParams": "CJ0BEM2rARgBIhMIzvHr0sis-gIV0kZ6BR0GNA_4SJGXrtzn9erzZQ==",
                        "commandMetadata": {
                          "webCommandMetadata": {
                            "url": "https://www.youtube.com/redirect?event=video_description&redir_token=QUFFLUhqbjdBNG5yVEFwU0JMNGZaLUpQZ1ZoeGgwT0xOZ3xBQ3Jtc0tuRFdFNlJNV29PMThRNWo5MHZrREZ1ZU5oZlkxVmE4ZlU5STFCZW1mUFVSdXJ3VUQxUnNVVkUzLWJQMS1uRzVjdkRCV2ZxSWJ6cFNxRVVzejY0SDltZFZPc2xwS3ZPZGIxcFZ6cndIVkMtUjVtZ054cw&q=https%3A%2F%2Fwww.facebook.com%2Faespa.official&v=ZeerrnuLi5E",
                            "webPageType": "WEB_PAGE_TYPE_UNKNOWN",
                            "rootVe": 83769
                          }
                        },
                        "urlEndpoint": {
                          "url": "https://www.youtube.com/redirect?event=video_description&redir_token=QUFFLUhqbjdBNG5yVEFwU0JMNGZaLUpQZ1ZoeGgwT0xOZ3xBQ3Jtc0tuRFdFNlJNV29PMThRNWo5MHZrREZ1ZU5oZlkxVmE4ZlU5STFCZW1mUFVSdXJ3VUQxUnNVVkUzLWJQMS1uRzVjdkRCV2ZxSWJ6cFNxRVVzejY0SDltZFZPc2xwS3ZPZGIxcFZ6cndIVkMtUjVtZ054cw&q=https%3A%2F%2Fwww.facebook.com%2Faespa.official&v=ZeerrnuLi5E",
                          "target": "TARGET_NEW_WINDOW",
                          "nofollow": true
                        }
                      }
                    }
                  },
                  {
                    "startIndex": 506,
                    "length": 23,
                    "onTap": {
                      "innertubeCommand": {
                        "clickTrackingParams": "CJ0BEM2rARgBIhMIzvHr0sis-gIV0kZ6BR0GNA_4SJGXrtzn9erzZQ==",
                        "commandMetadata": {
                          "webCommandMetadata": {
                            "url": "https://www.youtube.com/redirect?event=video_description&redir_token=QUFFLUhqbEtGMHB6eXBESW92aEVLc1FybkRwQU95eTh6UXxBQ3Jtc0tuWXc5d2JsTHFYcHExdy1FTDFyUV9wdU1DSmxELUxGSGlPMzhBdFVkblRSZkNLQzRaMEJGUGhYLWp4RU40YUVwV3N3ZUpRTVVKVDRiY19zeE5RUkt2dW5aUVcxcHBRQldCOTE3YktXSXZlSFJhRWRjdw&q=https%3A%2F%2Fweibo.com%2Faespa&v=ZeerrnuLi5E",
                            "webPageType": "WEB_PAGE_TYPE_UNKNOWN",
                            "rootVe": 83769
                          }
                        },
                        "urlEndpoint": {
                          "url": "https://www.youtube.com/redirect?event=video_description&redir_token=QUFFLUhqbEtGMHB6eXBESW92aEVLc1FybkRwQU95eTh6UXxBQ3Jtc0tuWXc5d2JsTHFYcHExdy1FTDFyUV9wdU1DSmxELUxGSGlPMzhBdFVkblRSZkNLQzRaMEJGUGhYLWp4RU40YUVwV3N3ZUpRTVVKVDRiY19zeE5RUkt2dW5aUVcxcHBRQldCOTE3YktXSXZlSFJhRWRjdw&q=https%3A%2F%2Fweibo.com%2Faespa&v=ZeerrnuLi5E",
                          "target": "TARGET_NEW_WINDOW",
                          "nofollow": true
                        }
                      }
                    }
                  },
                  {
                    "startIndex": 531,
                    "length": 6,
                    "onTap": {
                      "innertubeCommand": {
                        "clickTrackingParams": "CKIBENzXBBgKIhMIzvHr0sis-gIV0kZ6BR0GNA_4",
                        "commandMetadata": {
                          "webCommandMetadata": {
                            "url": "/hashtag/aespa",
                            "webPageType": "WEB_PAGE_TYPE_BROWSE",
                            "rootVe": 6827,
                            "apiUrl": "/youtubei/v1/browse"
                          }
                        },
                        "browseEndpoint": {
                          "browseId": "FEhashtag",
                          "params": "6gUHCgVhZXNwYQ%3D%3D"
                        }
                      }
                    },
                    "loggingDirectives": {
                      "trackingParams": "CKIBENzXBBgKIhMIzvHr0sis-gIV0kZ6BR0GNA_4",
                      "enableDisplayloggerExperiment": true
                    }
                  },
                  {
                    "startIndex": 538,
                    "length": 5,
                    "onTap": {
                      "innertubeCommand": {
                        "clickTrackingParams": "CKEBENzXBBgLIhMIzvHr0sis-gIV0kZ6BR0GNA_4",
                        "commandMetadata": {
                          "webCommandMetadata": {
                            "url": "/hashtag/%C3%A6spa",
                            "webPageType": "WEB_PAGE_TYPE_BROWSE",
                            "rootVe": 6827,
                            "apiUrl": "/youtubei/v1/browse"
                          }
                        },
                        "browseEndpoint": {
                          "browseId": "FEhashtag",
                          "params": "6gUHCgXDpnNwYQ%3D%3D"
                        }
                      }
                    },
                    "loggingDirectives": {
                      "trackingParams": "CKEBENzXBBgLIhMIzvHr0sis-gIV0kZ6BR0GNA_4",
                      "enableDisplayloggerExperiment": true
                    }
                  },
                  {
                    "startIndex": 544,
                    "length": 11,
                    "onTap": {
                      "innertubeCommand": {
                        "clickTrackingParams": "CKABENzXBBgMIhMIzvHr0sis-gIV0kZ6BR0GNA_4",
                        "commandMetadata": {
                          "webCommandMetadata": {
                            "url": "/hashtag/blackmamba",
                            "webPageType": "WEB_PAGE_TYPE_BROWSE",
                            "rootVe": 6827,
                            "apiUrl": "/youtubei/v1/browse"
                          }
                        },
                        "browseEndpoint": {
                          "browseId": "FEhashtag",
                          "params": "6gUMCgpibGFja21hbWJh"
                        }
                      }
                    },
                    "loggingDirectives": {
                      "trackingParams": "CKABENzXBBgMIhMIzvHr0sis-gIV0kZ6BR0GNA_4",
                      "enableDisplayloggerExperiment": true
                    }
                  },
                  {
                    "startIndex": 556,
                    "length": 5,
                    "onTap": {
                      "innertubeCommand": {
                        "clickTrackingParams": "CJ8BENzXBBgNIhMIzvHr0sis-gIV0kZ6BR0GNA_4",
                        "commandMetadata": {
                          "webCommandMetadata": {
                            "url": "/hashtag/%EB%B8%94%EB%9E%99%EB%A7%98%EB%B0%94",
                            "webPageType": "WEB_PAGE_TYPE_BROWSE",
                            "rootVe": 6827,
                            "apiUrl": "/youtubei/v1/browse"
                          }
                        },
                        "browseEndpoint": {
                          "browseId": "FEhashtag",
                          "params": "6gUOCgzruJTrnpnrp5jrsJQ%3D"
                        }
                      }
                    },
                    "loggingDirectives": {
                      "trackingParams": "CJ8BENzXBBgNIhMIzvHr0sis-gIV0kZ6BR0GNA_4",
                      "enableDisplayloggerExperiment": true
                    }
                  },
                  {
                    "startIndex": 562,
                    "length": 4,
                    "onTap": {
                      "innertubeCommand": {
                        "clickTrackingParams": "CJ4BENzXBBgOIhMIzvHr0sis-gIV0kZ6BR0GNA_4",
                        "commandMetadata": {
                          "webCommandMetadata": {
                            "url": "/hashtag/%EC%97%90%EC%8A%A4%ED%8C%8C",
                            "webPageType": "WEB_PAGE_TYPE_BROWSE",
                            "rootVe": 6827,
                            "apiUrl": "/youtubei/v1/browse"
                          }
                        },
                        "browseEndpoint": {
                          "browseId": "FEhashtag",
                          "params": "6gULCgnsl5DsiqTtjIw%3D"
                        }
                      }
                    },
                    "loggingDirectives": {
                      "trackingParams": "CJ4BENzXBBgOIhMIzvHr0sis-gIV0kZ6BR0GNA_4",
                      "enableDisplayloggerExperiment": true
                    }
                  }
                ]
            }
        }"#;

        let res = serde_json::from_str::<SAttributed>(test_json).unwrap();
        insta::assert_debug_snapshot!(res, @r###"
        SAttributed {
            ln: TextComponents(
                [
                    Text {
                        text: "🎧Listen and download aespa's debut single \"Black Mamba\": ",
                    },
                    Web {
                        text: "https://smarturl.it/aespa_BlackMamba",
                        url: "https://www.youtube.com/redirect?event=video_description&redir_token=QUFFLUhqbm1qRVVfQUlObURLcnFFQXBTUkJSOEpqWGIzUXxBQ3Jtc0tsNUJIYm5xdERxZk9rZEw3YlJzV0ZIYTNaSjU2a21PaFhNUmxzdjI5VE1VRWUyczZwYmtmQXh3QXV0eXlkMDgxRUJoNVMzRFZ6RlZ6MGdXeXdWQXFTTGY2ZHhFcUFqdExRQ21PYzNfWmlBaHhqYXVUdw&q=https%3A%2F%2Fsmarturl.it%2Faespa_BlackMamba&v=ZeerrnuLi5E",
                    },
                    Text {
                        text: "\n🐍The Debut Stage ",
                    },
                    Video {
                        text: "aespa 에스파 'Black ...",
                        video_id: "Ky5RT5oGg0w",
                        start_time: 0,
                    },
                    Text {
                        text: "\n\n🎟\u{fe0f} aespa Showcase SYNK in LA! Tickets now on sale: ",
                    },
                    Web {
                        text: "https://www.ticketmaster.com/event/0A...",
                        url: "https://www.youtube.com/redirect?event=video_description&redir_token=QUFFLUhqbU1ObGNaRDZaRmo1X1ZjejBoeTRnWkxuVUJxZ3xBQ3Jtc0ttWk1BVVhaRXRfN1VYWXBqMHdaYURTRFJNcUZJVlY3a21wRHE2ZGZaclE3WUM5bEZWbmFfT0sxWTZHOVotWVh6U3YtVk94SlA5NkRFTnBPcHVCWDJhMGJRQlI3ZHN0MnJleHp0c2lEVWNxeW1jSDZuVQ&q=https%3A%2F%2Fwww.ticketmaster.com%2Fevent%2F0A005CCD9E871F6E&v=ZeerrnuLi5E",
                    },
                    Text {
                        text: "\n\nSubscribe to aespa Official YouTube Channel!\n",
                    },
                    Web {
                        text: "https://www.youtube.com/aespa?sub_con...",
                        url: "https://www.youtube.com/aespa?sub_confirmation=1",
                    },
                    Text {
                        text: "\n\naespa official\n",
                    },
                    Web {
                        text: "aespa",
                        url: "https://www.youtube.com/c/aespa",
                    },
                    Text {
                        text: "\n",
                    },
                    Web {
                        text: "https://www.instagram.com/aespa_official",
                        url: "https://www.youtube.com/redirect?event=video_description&redir_token=QUFFLUhqbE9FVEtZZkVLUExjdFBnZjZnZ19KNWRYOVZUd3xBQ3Jtc0tsbHpCa1hLTVJ6MEllczlzUEpoVi1IQ2F5NG1jMnlOT3p3bnlFeE80ZzlsaG5CUXlFQnFGTkMtN19DcVYzQkw3bVlVVmNwQlpYQWZnNGNsME45WE1WQ21sR3V1Z3k5RG9DUDE0VTZQTm53Mk9vTWhiOA&q=https%3A%2F%2Fwww.instagram.com%2Faespa_official&v=ZeerrnuLi5E",
                    },
                    Text {
                        text: "\n",
                    },
                    Web {
                        text: "https://www.tiktok.com/@aespa_official",
                        url: "https://www.youtube.com/redirect?event=video_description&redir_token=QUFFLUhqbVdlSGk3eDd5U0dUVG16VFJCQnhKVFBEUUxMQXxBQ3Jtc0tuX3ZJbENNY1ZSN0FFemdxTFdlcTVvc3AwZE05NEFvRW5nOHpZWDUtZG9ORHBnT1JGc2UySDh3WWl3MU53VjFvbHRSdjdxMUlGM2Z6SmdaLTVaWWxhamJEems0Uld3MGlTT0Z0bkh5Y0hpcnY1aXptSQ&q=https%3A%2F%2Fwww.tiktok.com%2F%40aespa_official&v=ZeerrnuLi5E",
                    },
                    Text {
                        text: "\n",
                    },
                    Web {
                        text: "https://twitter.com/aespa_Official",
                        url: "https://www.youtube.com/redirect?event=video_description&redir_token=QUFFLUhqa3lNcG1lMHkwSzNLQVBrUXFNTXl0N1hNa04wUXxBQ3Jtc0tubm1sQkdaVjNYR04xOHpJV3NxZVBpb3I5V1FVOHVFNC1uWE5vb211ZmZKYzhTZXZfbjlkY09fanBRdHpjUkdRVGJJYS0xZ3NBNkVZQVhWSS0xVDYwRlRzQ0J3ODQxNDE0ODAxd1Q0cG5icVlNWndscw&q=https%3A%2F%2Ftwitter.com%2Faespa_Official&v=ZeerrnuLi5E",
                    },
                    Text {
                        text: "\n",
                    },
                    Web {
                        text: "https://www.facebook.com/aespa.official",
                        url: "https://www.youtube.com/redirect?event=video_description&redir_token=QUFFLUhqbjdBNG5yVEFwU0JMNGZaLUpQZ1ZoeGgwT0xOZ3xBQ3Jtc0tuRFdFNlJNV29PMThRNWo5MHZrREZ1ZU5oZlkxVmE4ZlU5STFCZW1mUFVSdXJ3VUQxUnNVVkUzLWJQMS1uRzVjdkRCV2ZxSWJ6cFNxRVVzejY0SDltZFZPc2xwS3ZPZGIxcFZ6cndIVkMtUjVtZ054cw&q=https%3A%2F%2Fwww.facebook.com%2Faespa.official&v=ZeerrnuLi5E",
                    },
                    Text {
                        text: "\n",
                    },
                    Web {
                        text: "https://weibo.com/aespa",
                        url: "https://www.youtube.com/redirect?event=video_description&redir_token=QUFFLUhqbEtGMHB6eXBESW92aEVLc1FybkRwQU95eTh6UXxBQ3Jtc0tuWXc5d2JsTHFYcHExdy1FTDFyUV9wdU1DSmxELUxGSGlPMzhBdFVkblRSZkNLQzRaMEJGUGhYLWp4RU40YUVwV3N3ZUpRTVVKVDRiY19zeE5RUkt2dW5aUVcxcHBRQldCOTE3YktXSXZlSFJhRWRjdw&q=https%3A%2F%2Fweibo.com%2Faespa&v=ZeerrnuLi5E",
                    },
                    Text {
                        text: "\n\n",
                    },
                    Text {
                        text: "#aespa",
                    },
                    Text {
                        text: " ",
                    },
                    Text {
                        text: "#æspa",
                    },
                    Text {
                        text: " ",
                    },
                    Text {
                        text: "#BlackMamba",
                    },
                    Text {
                        text: " ",
                    },
                    Text {
                        text: "#블랙맘바",
                    },
                    Text {
                        text: " ",
                    },
                    Text {
                        text: "#에스파",
                    },
                    Text {
                        text: "\naespa 에스파 'Black Mamba' MV ℗ SM Entertainment",
                    },
                ],
            ),
        }
        "###);
    }
}
