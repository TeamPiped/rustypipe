use crate::{
    error::{Error, ExtractionError},
    model::ChannelRss,
    report::{Report, RustyPipeInfo},
};

use super::{response, RustyPipeQuery};

impl RustyPipeQuery {
    /// Get the 15 latest videos from the channel's RSS feed
    ///
    /// Example: <https://www.youtube.com/feeds/videos.xml?channel_id=UC2DjFE7Xf11URZqWBigcVOQ>
    ///
    /// Fetching RSS feeds is a lot faster than querying the InnerTube API, so this method is great
    /// for checking a lot of channels or implementing a subscription feed.
    ///
    /// The downside of using the RSS feed is that it does not provide video durations.
    pub async fn channel_rss<S: AsRef<str>>(&self, channel_id: S) -> Result<ChannelRss, Error> {
        let channel_id = channel_id.as_ref();
        let url = format!("https://www.youtube.com/feeds/videos.xml?channel_id={channel_id}");
        let xml = self
            .client
            .http_request_txt(&self.client.inner.http.get(&url).build()?)
            .await
            .map_err(|e| match e {
                Error::HttpStatus(404, _) => Error::Extraction(ExtractionError::NotFound {
                    id: channel_id.to_owned(),
                    msg: "404".into(),
                }),
                _ => e,
            })?;

        match quick_xml::de::from_str::<response::ChannelRss>(&xml) {
            Ok(feed) => Ok(feed.into()),
            Err(e) => {
                if let Some(reporter) = &self.client.inner.reporter {
                    let report = Report {
                        info: RustyPipeInfo::default(),
                        level: crate::report::Level::ERR,
                        operation: "channel_rss",
                        error: Some(e.to_string()),
                        msgs: Vec::new(),
                        deobf_data: None,
                        http_request: crate::report::HTTPRequest {
                            url: &url,
                            method: "GET",
                            status: 200,
                            req_header: None,
                            req_body: None,
                            resp_body: xml,
                        },
                    };

                    reporter.report(&report);
                }

                Err(
                    ExtractionError::InvalidData(format!("could not deserialize xml: {e}").into())
                        .into(),
                )
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{fs::File, io::BufReader};

    use crate::{client::response, model::ChannelRss, util::tests::TESTFILES};

    use path_macro::path;
    use rstest::rstest;

    #[rstest]
    #[case::base("base")]
    #[case::no_likes("no_likes")]
    #[case::no_channel_id("no_channel_id")]
    fn map_channel_rss(#[case] name: &str) {
        let xml_path = path!(*TESTFILES / "channel_rss" / format!("{}.xml", name));
        let xml_file = File::open(xml_path).unwrap();

        let feed: response::ChannelRss =
            quick_xml::de::from_reader(BufReader::new(xml_file)).unwrap();

        let map_res: ChannelRss = feed.into();

        insta::assert_ron_snapshot!(format!("map_channel_rss_{}", name), map_res);
    }
}
