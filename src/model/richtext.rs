//! Data model for texts with links

use serde::{Deserialize, Serialize};

use crate::util;

use super::UrlTarget;

/// Text content with links
#[derive(Default, Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[non_exhaustive]
pub struct RichText(pub Vec<TextComponent>);

/// Text component forming a [`RichText`] object
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[non_exhaustive]
pub enum TextComponent {
    /// Plain text
    Text {
        /// Plain text
        text: String,
        /// Text styling
        #[serde(default, skip_serializing_if = "Style::is_unstyled")]
        style: Style,
    },
    /// Web link
    Web {
        /// Link text
        text: String,
        /// Link URL
        url: String,
    },
    /// Link to a YouTube item
    YouTube {
        /// Link text
        text: String,
        /// YouTube URL target
        target: UrlTarget,
    },
}

/// Text styling
#[derive(Default, Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
#[non_exhaustive]
pub struct Style {
    /// **Bold**
    ///
    /// - HTML: `<b>Text</b>`
    /// - Markdown: `**Text**`
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub bold: bool,
    /// *Italic*
    ///
    /// - HTML: `<i>Text</i>`
    /// - Markdown: `*Text*`
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub italic: bool,
    /// ~~Strikethrough~~
    ///
    /// - HTML: `<s>Text</s>`
    /// - Markdown: `~~Text~~`
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub strikethrough: bool,
}

impl Style {
    /// Return true if the text is styled (bold/italic/strikethrough)
    pub fn is_styled(&self) -> bool {
        self.bold || self.italic || self.strikethrough
    }

    fn is_unstyled(&self) -> bool {
        !self.is_styled()
    }

    fn html_open(&self, s: &mut String) {
        if self.bold {
            s.push_str("<b>");
        }
        if self.italic {
            s.push_str("<i>");
        }
        if self.strikethrough {
            s.push_str("<s>");
        }
    }

    fn html_close(&self, s: &mut String) {
        if self.bold {
            s.push_str("</b>");
        }
        if self.italic {
            s.push_str("</i>");
        }
        if self.strikethrough {
            s.push_str("</s>");
        }
    }

    fn md_tag(&self, s: &mut String) {
        if self.bold {
            s.push_str("**");
        }
        if self.italic {
            s.push('*');
        }
        if self.strikethrough {
            s.push_str("~~");
        }
    }
}

/// Trait for converting rich text to plain text.
pub trait ToPlaintext {
    /// Convert rich text to plain text.
    fn to_plaintext(&self) -> String {
        self.to_plaintext_yt_host("https://www.youtube.com")
    }
    /// Convert rich text to plain text while changing YouTube links to a custom site.
    ///
    /// expected yt_host format (no trailing slash): `https://example.com`
    fn to_plaintext_yt_host(&self, yt_host: &str) -> String;
}

/// Trait for converting rich text to html.
pub trait ToHtml {
    /// Convert rich text to html.
    fn to_html(&self) -> String {
        self.to_html_yt_host("https://www.youtube.com")
    }
    /// Convert rich text to html while changing YouTube links to a custom site.
    ///
    /// expected yt_host format (no trailing slash): `https://example.com`
    fn to_html_yt_host(&self, yt_host: &str) -> String;
}

/// Trait for converting rich text to markdown.
pub trait ToMarkdown {
    /// Convert rich text to markdown.
    fn to_markdown(&self) -> String {
        self.to_markdown_yt_host("https://www.youtube.com")
    }
    /// Convert rich text to markdown while changing YouTube links to a custom site.
    ///
    /// expected yt_host format (no trailing slash): `https://example.com`
    fn to_markdown_yt_host(&self, yt_host: &str) -> String;
}

impl RichText {
    /// Returns `true` if the rich text contains no text components.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl TextComponent {
    /// Get the text from the component
    pub fn get_text(&self) -> &str {
        match self {
            TextComponent::Text { text, .. }
            | TextComponent::Web { text, .. }
            | TextComponent::YouTube { text, .. } => text,
        }
    }

    /// Get the link URL from the component
    ///
    /// Returns an empty string if the component is not a link.
    pub fn get_url(&self, yt_host: &str) -> String {
        match self {
            TextComponent::Text { .. } => String::new(),
            TextComponent::Web { url, .. } => url.clone(),
            TextComponent::YouTube { target, .. } => target.to_url_yt_host(yt_host),
        }
    }
}

impl ToPlaintext for TextComponent {
    fn to_plaintext_yt_host(&self, yt_host: &str) -> String {
        match self {
            TextComponent::Text { text, .. } => text.clone(),
            _ => self.get_url(yt_host),
        }
    }
}

impl ToHtml for TextComponent {
    fn to_html_yt_host(&self, yt_host: &str) -> String {
        match self {
            TextComponent::Text { text, style } => {
                let mut html = String::with_capacity(text.len());
                style.html_open(&mut html);
                util::escape_html_append(text, &mut html);
                style.html_close(&mut html);
                html
            }
            TextComponent::Web { text, .. } => {
                format!(
                    r#"<a href="{}" target="_blank" rel="noreferrer">{}</a>"#,
                    self.get_url(yt_host),
                    util::escape_html(text)
                )
            }
            _ => {
                format!(
                    r#"<a href="{}">{}</a>"#,
                    self.get_url(yt_host),
                    util::escape_html(self.get_text())
                )
            }
        }
    }
}

impl ToMarkdown for TextComponent {
    fn to_markdown_yt_host(&self, yt_host: &str) -> String {
        match self {
            TextComponent::Text { text, style } => {
                let mut md = String::with_capacity(text.len());
                style.md_tag(&mut md);
                util::escape_markdown_append(text, &mut md);
                style.md_tag(&mut md);
                md
            }
            TextComponent::Web { text, .. } | TextComponent::YouTube { text, .. } => {
                format!(
                    "[{}]({})",
                    util::escape_markdown(text),
                    self.get_url(yt_host)
                )
            }
        }
    }
}

impl ToPlaintext for RichText {
    fn to_plaintext_yt_host(&self, yt_host: &str) -> String {
        self.0
            .iter()
            .map(|c| c.to_plaintext_yt_host(yt_host))
            .collect()
    }
}

impl ToHtml for RichText {
    fn to_html_yt_host(&self, yt_host: &str) -> String {
        self.0.iter().map(|c| c.to_html_yt_host(yt_host)).collect()
    }
}

impl ToMarkdown for RichText {
    fn to_markdown_yt_host(&self, yt_host: &str) -> String {
        self.0
            .iter()
            .map(|c| c.to_markdown_yt_host(yt_host))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use insta::assert_snapshot;
    use once_cell::sync::Lazy;

    use crate::client::response::url_endpoint::MusicVideoType;
    use crate::serializer::text;

    static TEXT_SOURCE: Lazy<text::TextComponents> = Lazy::new(|| {
        text::TextComponents(vec![
            text::TextComponent::new("🎧Listen and download aespa's debut single \"Black Mamba\": "),
            text::TextComponent::Web { text: "https://smarturl.it/aespa_BlackMamba".to_owned(), url: "https://www.youtube.com/redirect?event=video_description&redir_token=QUFFLUhqbFY1QmpQamJPSms0Z1FnVTlQUS00ZFhBZnBJZ3xBQ3Jtc0tuRGJBanludGoyRnphb2dZWVd3cUNnS3dEd0FnNHFOZEY1NHBJaHFmLXpaWUJwX3ZucDZxVnpGeHNGX1FpMzFkZW9jQkI2Mi1wNGJ1UVFNN3h1MnN3R3JLMzdxU01nZ01POHBGcmxHU2puSUk1WHRzQQ&q=https%3A%2F%2Fsmarturl.it%2Faespa_BlackMamba&v=ZeerrnuLi5E".to_owned() },
            text::TextComponent::new("\n🐍The Debut Stage "),
            text::TextComponent::Video { text: "https://youtu.be/Ky5RT5oGg0w".to_owned(), video_id: "Ky5RT5oGg0w".to_owned(), start_time: 0, vtype: MusicVideoType::Video },
            text::TextComponent::new("\n\n🎟️ aespa Showcase SYNK in LA! Tickets now on sale: "),
            text::TextComponent::Web { text: "https://www.ticketmaster.com/event/0A...".to_owned(), url: "https://www.youtube.com/redirect?event=video_description&redir_token=QUFFLUhqbFpUMEZiaXJWWkszaVZXaEM0emxWU1JQV3NoQXxBQ3Jtc0tuU2g4VWNPNE5UY3hoSWYtamFzX0h4bUVQLVJiRy1ubDZrTnh3MUpGdDNSaUo0ZlMyT3lUM28ycUVBdHJLMndGcDhla3BkOFpxSVFfOS1QdVJPVHBUTEV1LXpOV0J2QXdhV05lV210cEJtZUJMeHdaTQ&q=https%3A%2F%2Fwww.ticketmaster.com%2Fevent%2F0A005CCD9E871F6E&v=ZeerrnuLi5E".to_owned() },
            text::TextComponent::new("\n\nSubscribe to aespa Official YouTube Channel!\n"),
            text::TextComponent::Web { text: "https://www.youtube.com/aespa?sub_con...".to_owned(), url: "https://www.youtube.com/aespa?sub_confirmation=1".to_owned() },
            text::TextComponent::new("\n\naespa official\n"),
            text::TextComponent::Web { text: "https://www.youtube.com/c/aespa".to_owned(), url: "https://www.youtube.com/c/aespa".to_owned() },
            text::TextComponent::new("\n"),
            text::TextComponent::Web { text: "https://www.instagram.com/aespa_official".to_owned(), url: "https://www.youtube.com/redirect?event=video_description&redir_token=QUFFLUhqbmE4UXZBdFM4allpdUkwaGQ1SGFBTklKYVVaQXxBQ3Jtc0tsOVg3WTM2Y0t1eE5YUm5vZjNTVjM4bncxTl9JeFdWeGJlbDZJa3BqTXZDQUdzVndPR3ZpV2ZEOGMzZ1FsT21HMEp5UllpWVZVb3djYTVzNGNFaWlmbzhmTEVmQ0RiVUxMNUM4MDV3ZGt3SHhJM3pGSQ&q=https%3A%2F%2Fwww.instagram.com%2Faespa_official&v=ZeerrnuLi5E".to_owned() },
            text::TextComponent::new("\n"),
            text::TextComponent::Web { text: "https://www.tiktok.com/@aespa_official".to_owned(), url: "https://www.youtube.com/redirect?event=video_description&redir_token=QUFFLUhqa2hVUk9QQXZmMHk5ZkdEZnVKZXIyXzZvX09zZ3xBQ3Jtc0trZEhjd1lVc1NZMWs4TVY3UmpzdDhnX0lLYnZjekZqNUprWUpHV1ZOR2g0al84TlNLTEFjODktUWE3QUFFTlJ5RlpvOVNOWUdJXzF2ZHhzOHRTdGhlUG1OcmhZVkMtazBzYXJqNFVUYVBKUVI1ZzB4VQ&q=https%3A%2F%2Fwww.tiktok.com%2F%40aespa_official&v=ZeerrnuLi5E".to_owned() },
            text::TextComponent::new("\n"),
            text::TextComponent::Web { text: "https://twitter.com/aespa_Official".to_owned(), url: "https://www.youtube.com/redirect?event=video_description&redir_token=QUFFLUhqbWFlRFFWWVpMeFRzU08ySWhJWVl0RUJpZzIxZ3xBQ3Jtc0tsekJiMUI4Zk1QdENObWpLZVppdk1nRVBkamJmX21VNGxaYjdUTEdoREx4Z3pWTm0wVHg4MWNTVmdxakNJT3VQQk5tSDVnZkNJZkhQSTF1d0ZEX3g0RUVDWjFjVzA1ZzVsTEVvMW5ISTdaZU1xYjhXSQ&q=https%3A%2F%2Ftwitter.com%2Faespa_Official&v=ZeerrnuLi5E".to_owned() },
            text::TextComponent::new("\n"),
            text::TextComponent::Web { text: "https://www.facebook.com/aespa.official".to_owned(), url: "https://www.youtube.com/redirect?event=video_description&redir_token=QUFFLUhqbWJxUWVETWNwM0ltc0JYXzBjQ1h5dmQ2OXNzUXxBQ3Jtc0ttVy1JRHV2VVpUOUtDdUZTU0tROEtLX1k0bVFFNTdoZVpIUDhDbTkydmRmY2diR3RlQmlON1Y4NURsaU1YcTRKLXBzeGdkWWY1d0R3MzhMYXl6cE1OM0hMcEpkdXZvVXItQzRhMTVqVU1ySk93UG9Ydw&q=https%3A%2F%2Fwww.facebook.com%2Faespa.official&v=ZeerrnuLi5E".to_owned() },
            text::TextComponent::new("\n"),
            text::TextComponent::Web { text: "https://weibo.com/aespa".to_owned(), url: "https://www.youtube.com/redirect?event=video_description&redir_token=QUFFLUhqbUZFOVFFSEtTRkU5LXluWk9uTVRHbU5tN2JGd3xBQ3Jtc0ttR003eUM4ZVBVM3JPdjdJMnZwRXpxZmJMMkhFbHYtbklJUG9LYXh5VHBXalgyWTZwc3RqcGlhT2JIR0RlNVpWUEpBajZ0X2d5ZkxEZUUyQmF4bE13NjhEdDZOak9saHdnb25qdnB3dnRiYmplbkY0MA&q=https%3A%2F%2Fweibo.com%2Faespa&v=ZeerrnuLi5E".to_owned() },
            text::TextComponent::new("\n\n"),
            text::TextComponent::new("#aespa"),
            text::TextComponent::new(" "),
            text::TextComponent::new("#æspa"),
            text::TextComponent::new(" "),
            text::TextComponent::new("#BlackMamba"),
            text::TextComponent::new(" "),
            text::TextComponent::new("#블랙맘바"),
            text::TextComponent::new(" "),
            text::TextComponent::new("#에스파"),
            text::TextComponent::new("\naespa 에스파 'Black Mamba' MV ℗ SM Entertainment"),
            text::TextComponent::new("\n\n"),

            text::TextComponent::new("Bold: "),
            text::TextComponent::Text { text: "Awesome".to_owned(), style: Style { bold: true, italic: false, strikethrough: false } },
            text::TextComponent::new("\nItalic: "),
            text::TextComponent::Text { text: "Great".to_owned(), style: Style { bold: false, italic: true, strikethrough: false } },
            text::TextComponent::new("\nStrikethrough: "),
            text::TextComponent::Text { text: "Gone".to_owned(), style: Style { bold: false, italic: false, strikethrough: true } },
            text::TextComponent::new("\nMixed: "),
            text::TextComponent::Text { text: "Everything".to_owned(), style: Style { bold: true, italic: true, strikethrough: true } },
        ])
    });

    #[test]
    fn to_plaintext() {
        let richtext = RichText::from(TEXT_SOURCE.clone());
        let plaintext = richtext.to_plaintext_yt_host("https://piped.kavin.rocks");

        assert_snapshot!(plaintext, @r###"
        🎧Listen and download aespa's debut single "Black Mamba": https://smarturl.it/aespa_BlackMamba
        🐍The Debut Stage https://piped.kavin.rocks/watch?v=Ky5RT5oGg0w

        🎟️ aespa Showcase SYNK in LA! Tickets now on sale: https://www.ticketmaster.com/event/0A005CCD9E871F6E

        Subscribe to aespa Official YouTube Channel!
        https://www.youtube.com/aespa?sub_confirmation=1

        aespa official
        https://www.youtube.com/c/aespa
        https://www.instagram.com/aespa_official
        https://www.tiktok.com/@aespa_official
        https://twitter.com/aespa_Official
        https://www.facebook.com/aespa.official
        https://weibo.com/aespa

        #aespa #æspa #BlackMamba #블랙맘바 #에스파
        aespa 에스파 'Black Mamba' MV ℗ SM Entertainment

        Bold: Awesome
        Italic: Great
        Strikethrough: Gone
        Mixed: Everything
        "###);
    }

    #[test]
    fn to_html() {
        let richtext = RichText::from(TEXT_SOURCE.clone());
        let html = richtext.to_html_yt_host("https://piped.kavin.rocks");
        assert_snapshot!(
            html,
            @r###"🎧Listen and download aespa&#x27;s debut single &quot;Black Mamba&quot;: <a href="https://smarturl.it/aespa_BlackMamba" target="_blank" rel="noreferrer">https://smarturl.it/aespa_BlackMamba</a><br>🐍The Debut Stage <a href="https://piped.kavin.rocks/watch?v=Ky5RT5oGg0w">https://youtu.be/Ky5RT5oGg0w</a><br><br>🎟️ aespa Showcase SYNK in LA! Tickets now on sale: <a href="https://www.ticketmaster.com/event/0A005CCD9E871F6E" target="_blank" rel="noreferrer">https://www.ticketmaster.com/event/0A...</a><br><br>Subscribe to aespa Official YouTube Channel!<br><a href="https://www.youtube.com/aespa?sub_confirmation=1" target="_blank" rel="noreferrer">https://www.youtube.com/aespa?sub_con...</a><br><br>aespa official<br><a href="https://www.youtube.com/c/aespa" target="_blank" rel="noreferrer">https://www.youtube.com/c/aespa</a><br><a href="https://www.instagram.com/aespa_official" target="_blank" rel="noreferrer">https://www.instagram.com/aespa_official</a><br><a href="https://www.tiktok.com/@aespa_official" target="_blank" rel="noreferrer">https://www.tiktok.com/@aespa_official</a><br><a href="https://twitter.com/aespa_Official" target="_blank" rel="noreferrer">https://twitter.com/aespa_Official</a><br><a href="https://www.facebook.com/aespa.official" target="_blank" rel="noreferrer">https://www.facebook.com/aespa.official</a><br><a href="https://weibo.com/aespa" target="_blank" rel="noreferrer">https://weibo.com/aespa</a><br><br>#aespa #æspa #BlackMamba #블랙맘바 #에스파<br>aespa 에스파 &#x27;Black Mamba&#x27; MV ℗ SM Entertainment<br><br>Bold: <b>Awesome</b><br>Italic: <i>Great</i><br>Strikethrough: <s>Gone</s><br>Mixed: <b><i><s>Everything</b></i></s>"###
        );
    }

    #[test]
    fn to_markdown() {
        let richtext = RichText::from(TEXT_SOURCE.clone());
        let markdown = richtext.to_markdown_yt_host("https://piped.kavin.rocks");
        println!("{markdown}");
        assert_snapshot!(
            markdown,
            @r###"🎧Listen and download aespa's debut single "Black Mamba"\: [https\://smarturl.it/aespa\_BlackMamba](https://smarturl.it/aespa_BlackMamba)<br>🐍The Debut Stage [https\://youtu.be/Ky5RT5oGg0w](https://piped.kavin.rocks/watch?v=Ky5RT5oGg0w)<br><br>🎟️ aespa Showcase SYNK in LA! Tickets now on sale\: [https\://www.ticketmaster.com/event/0A...](https://www.ticketmaster.com/event/0A005CCD9E871F6E)<br><br>Subscribe to aespa Official YouTube Channel!<br>[https\://www.youtube.com/aespa?sub\_con...](https://www.youtube.com/aespa?sub_confirmation=1)<br><br>aespa official<br>[https\://www.youtube.com/c/aespa](https://www.youtube.com/c/aespa)<br>[https\://www.instagram.com/aespa\_official](https://www.instagram.com/aespa_official)<br>[https\://www.tiktok.com/@aespa\_official](https://www.tiktok.com/@aespa_official)<br>[https\://twitter.com/aespa\_Official](https://twitter.com/aespa_Official)<br>[https\://www.facebook.com/aespa.official](https://www.facebook.com/aespa.official)<br>[https\://weibo.com/aespa](https://weibo.com/aespa)<br><br>\#aespa \#æspa \#BlackMamba \#블랙맘바 \#에스파<br>aespa 에스파 'Black Mamba' MV ℗ SM Entertainment<br><br>Bold\: **Awesome**<br>Italic\: *Great*<br>Strikethrough\: ~~Gone~~<br>Mixed\: ***~~Everything***~~"###
        );
    }
}
