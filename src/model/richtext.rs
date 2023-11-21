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
            TextComponent::Text { text }
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
            TextComponent::Text { text } => text.clone(),
            _ => self.get_url(yt_host),
        }
    }
}

impl ToHtml for TextComponent {
    fn to_html_yt_host(&self, yt_host: &str) -> String {
        match self {
            TextComponent::Text { text } => util::escape_html(text),
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
            TextComponent::Text { text } => util::escape_markdown(text),
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

    use once_cell::sync::Lazy;

    use crate::client::response::url_endpoint::MusicVideoType;
    use crate::serializer::text;

    static TEXT_SOURCE: Lazy<text::TextComponents> = Lazy::new(|| {
        text::TextComponents(vec![
            text::TextComponent::Text { text: "🎧Listen and download aespa's debut single \"Black Mamba\": ".to_owned() },
            text::TextComponent::Web { text: "https://smarturl.it/aespa_BlackMamba".to_owned(), url: "https://www.youtube.com/redirect?event=video_description&redir_token=QUFFLUhqbFY1QmpQamJPSms0Z1FnVTlQUS00ZFhBZnBJZ3xBQ3Jtc0tuRGJBanludGoyRnphb2dZWVd3cUNnS3dEd0FnNHFOZEY1NHBJaHFmLXpaWUJwX3ZucDZxVnpGeHNGX1FpMzFkZW9jQkI2Mi1wNGJ1UVFNN3h1MnN3R3JLMzdxU01nZ01POHBGcmxHU2puSUk1WHRzQQ&q=https%3A%2F%2Fsmarturl.it%2Faespa_BlackMamba&v=ZeerrnuLi5E".to_owned() },
            text::TextComponent::Text { text: "\n🐍The Debut Stage ".to_owned() },
            text::TextComponent::Video { text: "https://youtu.be/Ky5RT5oGg0w".to_owned(), video_id: "Ky5RT5oGg0w".to_owned(), start_time: 0, vtype: MusicVideoType::Video },
            text::TextComponent::Text { text: "\n\n🎟️ aespa Showcase SYNK in LA! Tickets now on sale: ".to_owned() },
            text::TextComponent::Web { text: "https://www.ticketmaster.com/event/0A...".to_owned(), url: "https://www.youtube.com/redirect?event=video_description&redir_token=QUFFLUhqbFpUMEZiaXJWWkszaVZXaEM0emxWU1JQV3NoQXxBQ3Jtc0tuU2g4VWNPNE5UY3hoSWYtamFzX0h4bUVQLVJiRy1ubDZrTnh3MUpGdDNSaUo0ZlMyT3lUM28ycUVBdHJLMndGcDhla3BkOFpxSVFfOS1QdVJPVHBUTEV1LXpOV0J2QXdhV05lV210cEJtZUJMeHdaTQ&q=https%3A%2F%2Fwww.ticketmaster.com%2Fevent%2F0A005CCD9E871F6E&v=ZeerrnuLi5E".to_owned() },
            text::TextComponent::Text { text: "\n\nSubscribe to aespa Official YouTube Channel!\n".to_owned() },
            text::TextComponent::Web { text: "https://www.youtube.com/aespa?sub_con...".to_owned(), url: "https://www.youtube.com/aespa?sub_confirmation=1".to_owned() },
            text::TextComponent::Text { text: "\n\naespa official\n".to_owned() },
            text::TextComponent::Web { text: "https://www.youtube.com/c/aespa".to_owned(), url: "https://www.youtube.com/c/aespa".to_owned() },
            text::TextComponent::Text { text: "\n".to_owned() },
            text::TextComponent::Web { text: "https://www.instagram.com/aespa_official".to_owned(), url: "https://www.youtube.com/redirect?event=video_description&redir_token=QUFFLUhqbmE4UXZBdFM4allpdUkwaGQ1SGFBTklKYVVaQXxBQ3Jtc0tsOVg3WTM2Y0t1eE5YUm5vZjNTVjM4bncxTl9JeFdWeGJlbDZJa3BqTXZDQUdzVndPR3ZpV2ZEOGMzZ1FsT21HMEp5UllpWVZVb3djYTVzNGNFaWlmbzhmTEVmQ0RiVUxMNUM4MDV3ZGt3SHhJM3pGSQ&q=https%3A%2F%2Fwww.instagram.com%2Faespa_official&v=ZeerrnuLi5E".to_owned() },
            text::TextComponent::Text { text: "\n".to_owned() },
            text::TextComponent::Web { text: "https://www.tiktok.com/@aespa_official".to_owned(), url: "https://www.youtube.com/redirect?event=video_description&redir_token=QUFFLUhqa2hVUk9QQXZmMHk5ZkdEZnVKZXIyXzZvX09zZ3xBQ3Jtc0trZEhjd1lVc1NZMWs4TVY3UmpzdDhnX0lLYnZjekZqNUprWUpHV1ZOR2g0al84TlNLTEFjODktUWE3QUFFTlJ5RlpvOVNOWUdJXzF2ZHhzOHRTdGhlUG1OcmhZVkMtazBzYXJqNFVUYVBKUVI1ZzB4VQ&q=https%3A%2F%2Fwww.tiktok.com%2F%40aespa_official&v=ZeerrnuLi5E".to_owned() },
            text::TextComponent::Text { text: "\n".to_owned() },
            text::TextComponent::Web { text: "https://twitter.com/aespa_Official".to_owned(), url: "https://www.youtube.com/redirect?event=video_description&redir_token=QUFFLUhqbWFlRFFWWVpMeFRzU08ySWhJWVl0RUJpZzIxZ3xBQ3Jtc0tsekJiMUI4Zk1QdENObWpLZVppdk1nRVBkamJmX21VNGxaYjdUTEdoREx4Z3pWTm0wVHg4MWNTVmdxakNJT3VQQk5tSDVnZkNJZkhQSTF1d0ZEX3g0RUVDWjFjVzA1ZzVsTEVvMW5ISTdaZU1xYjhXSQ&q=https%3A%2F%2Ftwitter.com%2Faespa_Official&v=ZeerrnuLi5E".to_owned() },
            text::TextComponent::Text { text: "\n".to_owned() },
            text::TextComponent::Web { text: "https://www.facebook.com/aespa.official".to_owned(), url: "https://www.youtube.com/redirect?event=video_description&redir_token=QUFFLUhqbWJxUWVETWNwM0ltc0JYXzBjQ1h5dmQ2OXNzUXxBQ3Jtc0ttVy1JRHV2VVpUOUtDdUZTU0tROEtLX1k0bVFFNTdoZVpIUDhDbTkydmRmY2diR3RlQmlON1Y4NURsaU1YcTRKLXBzeGdkWWY1d0R3MzhMYXl6cE1OM0hMcEpkdXZvVXItQzRhMTVqVU1ySk93UG9Ydw&q=https%3A%2F%2Fwww.facebook.com%2Faespa.official&v=ZeerrnuLi5E".to_owned() },
            text::TextComponent::Text { text: "\n".to_owned() },
            text::TextComponent::Web { text: "https://weibo.com/aespa".to_owned(), url: "https://www.youtube.com/redirect?event=video_description&redir_token=QUFFLUhqbUZFOVFFSEtTRkU5LXluWk9uTVRHbU5tN2JGd3xBQ3Jtc0ttR003eUM4ZVBVM3JPdjdJMnZwRXpxZmJMMkhFbHYtbklJUG9LYXh5VHBXalgyWTZwc3RqcGlhT2JIR0RlNVpWUEpBajZ0X2d5ZkxEZUUyQmF4bE13NjhEdDZOak9saHdnb25qdnB3dnRiYmplbkY0MA&q=https%3A%2F%2Fweibo.com%2Faespa&v=ZeerrnuLi5E".to_owned() },
            text::TextComponent::Text { text: "\n\n".to_owned() },
            text::TextComponent::Text { text: "#aespa".to_owned() },
            text::TextComponent::Text { text: " ".to_owned() },
            text::TextComponent::Text { text: "#æspa".to_owned() },
            text::TextComponent::Text { text: " ".to_owned() },
            text::TextComponent::Text { text: "#BlackMamba".to_owned() },
            text::TextComponent::Text { text: " ".to_owned() },
            text::TextComponent::Text { text: "#블랙맘바".to_owned() },
            text::TextComponent::Text { text: " ".to_owned() },
            text::TextComponent::Text { text: "#에스파".to_owned() },
            text::TextComponent::Text { text: "\naespa 에스파 'Black Mamba' MV ℗ SM Entertainment".to_owned() },
        ])
    });

    #[test]
    fn to_plaintext() {
        let richtext = RichText::from(TEXT_SOURCE.clone());
        let plaintext = richtext.to_plaintext_yt_host("https://piped.kavin.rocks");
        assert_eq!(
            plaintext,
            r#"🎧Listen and download aespa's debut single "Black Mamba": https://smarturl.it/aespa_BlackMamba
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
aespa 에스파 'Black Mamba' MV ℗ SM Entertainment"#
        );
    }

    #[test]
    fn to_html() {
        let richtext = RichText::from(TEXT_SOURCE.clone());
        let html = richtext.to_html_yt_host("https://piped.kavin.rocks");
        assert_eq!(
            html,
            "🎧Listen and download aespa&#x27;s debut single &quot;Black Mamba&quot;: <a href=\"https://smarturl.it/aespa_BlackMamba\" target=\"_blank\" rel=\"noreferrer\">https://smarturl.it/aespa_BlackMamba</a><br>🐍The Debut Stage <a href=\"https://piped.kavin.rocks/watch?v=Ky5RT5oGg0w\">https://youtu.be/Ky5RT5oGg0w</a><br><br>🎟\u{fe0f} aespa Showcase SYNK in LA! Tickets now on sale: <a href=\"https://www.ticketmaster.com/event/0A005CCD9E871F6E\" target=\"_blank\" rel=\"noreferrer\">https://www.ticketmaster.com/event/0A...</a><br><br>Subscribe to aespa Official YouTube Channel!<br><a href=\"https://www.youtube.com/aespa?sub_confirmation=1\" target=\"_blank\" rel=\"noreferrer\">https://www.youtube.com/aespa?sub_con...</a><br><br>aespa official<br><a href=\"https://www.youtube.com/c/aespa\" target=\"_blank\" rel=\"noreferrer\">https://www.youtube.com/c/aespa</a><br><a href=\"https://www.instagram.com/aespa_official\" target=\"_blank\" rel=\"noreferrer\">https://www.instagram.com/aespa_official</a><br><a href=\"https://www.tiktok.com/@aespa_official\" target=\"_blank\" rel=\"noreferrer\">https://www.tiktok.com/@aespa_official</a><br><a href=\"https://twitter.com/aespa_Official\" target=\"_blank\" rel=\"noreferrer\">https://twitter.com/aespa_Official</a><br><a href=\"https://www.facebook.com/aespa.official\" target=\"_blank\" rel=\"noreferrer\">https://www.facebook.com/aespa.official</a><br><a href=\"https://weibo.com/aespa\" target=\"_blank\" rel=\"noreferrer\">https://weibo.com/aespa</a><br><br>#aespa #æspa #BlackMamba #블랙맘바 #에스파<br>aespa 에스파 &#x27;Black Mamba&#x27; MV ℗ SM Entertainment"
        );
    }

    #[test]
    fn to_markdown() {
        let richtext = RichText::from(TEXT_SOURCE.clone());
        let markdown = richtext.to_markdown_yt_host("https://piped.kavin.rocks");
        println!("{markdown}");
        assert_eq!(
            markdown,
            r#"🎧Listen and download aespa's debut single "Black Mamba"\: [https\://smarturl.it/aespa\_BlackMamba](https://smarturl.it/aespa_BlackMamba)<br>🐍The Debut Stage [https\://youtu.be/Ky5RT5oGg0w](https://piped.kavin.rocks/watch?v=Ky5RT5oGg0w)<br><br>🎟️ aespa Showcase SYNK in LA! Tickets now on sale\: [https\://www.ticketmaster.com/event/0A...](https://www.ticketmaster.com/event/0A005CCD9E871F6E)<br><br>Subscribe to aespa Official YouTube Channel!<br>[https\://www.youtube.com/aespa?sub\_con...](https://www.youtube.com/aespa?sub_confirmation=1)<br><br>aespa official<br>[https\://www.youtube.com/c/aespa](https://www.youtube.com/c/aespa)<br>[https\://www.instagram.com/aespa\_official](https://www.instagram.com/aespa_official)<br>[https\://www.tiktok.com/@aespa\_official](https://www.tiktok.com/@aespa_official)<br>[https\://twitter.com/aespa\_Official](https://twitter.com/aespa_Official)<br>[https\://www.facebook.com/aespa.official](https://www.facebook.com/aespa.official)<br>[https\://weibo.com/aespa](https://weibo.com/aespa)<br><br>\#aespa \#æspa \#BlackMamba \#블랙맘바 \#에스파<br>aespa 에스파 'Black Mamba' MV ℗ SM Entertainment"#
        );
    }
}
