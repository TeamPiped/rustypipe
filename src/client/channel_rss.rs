use std::fmt::Debug;

use crate::{
    error::{Error, ExtractionError},
    model::ChannelRss,
    report::Report,
    util,
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
    #[tracing::instrument(skip(self), level = "error")]
    pub async fn channel_rss<S: AsRef<str> + Debug>(
        &self,
        channel_id: S,
    ) -> Result<ChannelRss, Error> {
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

        match quick_xml::de::from_str::<response::ChannelRss>(&xml)
            .map_err(|e| ExtractionError::InvalidData(e.to_string().into()))
            .and_then(|feed| feed.map_response(channel_id))
        {
            Ok(res) => Ok(res),
            Err(e) => {
                if let Some(reporter) = &self.client.inner.reporter {
                    let report = Report {
                        info: self.rp_info(),
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
                Err(Error::Extraction(e))
            }
        }
    }
}

impl response::ChannelRss {
    fn map_response(self, id: &str) -> Result<ChannelRss, ExtractionError> {
        let channel_id = if self.channel_id.is_empty() {
            self.entry
                .iter()
                .find_map(|entry| {
                    Some(entry.channel_id.as_str())
                        .filter(|id| id.is_empty())
                        .map(str::to_owned)
                })
                .or_else(|| {
                    self.author
                        .uri
                        .strip_prefix("https://www.youtube.com/channel/")
                        .and_then(|id| {
                            if util::CHANNEL_ID_REGEX.is_match(id) {
                                Some(id.to_owned())
                            } else {
                                None
                            }
                        })
                })
                .ok_or(ExtractionError::InvalidData(
                    "could not get channel id".into(),
                ))?
        } else if self.channel_id.len() == 22 {
            // As of November 2023, YouTube seems to output channel IDs without the UC prefix
            format!("UC{}", self.channel_id)
        } else {
            self.channel_id
        };

        if channel_id != id {
            return Err(ExtractionError::WrongResult(format!(
                "got wrong channel id {channel_id}, expected {id}",
            )));
        }

        Ok(ChannelRss {
            id: channel_id,
            name: self.title,
            videos: self
                .entry
                .into_iter()
                .map(|item| crate::model::ChannelRssVideo {
                    id: item.video_id,
                    name: item.title,
                    description: item.media_group.description,
                    thumbnail: item.media_group.thumbnail.into(),
                    publish_date: item.published,
                    update_date: item.updated,
                    view_count: item.media_group.community.statistics.views,
                    like_count: item.media_group.community.rating.count,
                })
                .collect(),
            create_date: self.create_date,
        })
    }
}

#[cfg(test)]
mod tests {
    use std::{fs::File, io::BufReader};

    use crate::{client::response, util::tests::TESTFILES};

    use path_macro::path;
    use rstest::rstest;

    #[rstest]
    #[case::base("base", "UCHnyfMqiRRG1u-2MsSQLbXA")]
    #[case::no_likes("no_likes", "UCdfxp4cUWsWryZOy-o427dw")]
    #[case::no_channel_id("no_channel_id", "UCHnyfMqiRRG1u-2MsSQLbXA")]
    #[case::trimmed_channel_id("trimmed_channel_id", "UCHnyfMqiRRG1u-2MsSQLbXA")]
    fn map_channel_rss(#[case] name: &str, #[case] id: &str) {
        let xml_path = path!(*TESTFILES / "channel_rss" / format!("{}.xml", name));
        let xml_file = File::open(xml_path).unwrap();

        let feed: response::ChannelRss =
            quick_xml::de::from_reader(BufReader::new(xml_file)).unwrap();

        let map_res = feed.map_response(id).unwrap();
        insta::assert_ron_snapshot!(format!("map_channel_rss_{}", name), map_res);
    }
}
