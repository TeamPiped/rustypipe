use std::collections::BTreeMap;

use anyhow::Result;

use crate::{model::ChannelRss, report::Report};

use super::{response, RustyPipeQuery};

impl RustyPipeQuery {
    pub async fn channel_rss(&self, channel_id: &str) -> Result<ChannelRss> {
        let url = format!(
            "https://www.youtube.com/feeds/videos.xml?channel_id={}",
            channel_id
        );
        let xml = self
            .client
            .http_request_txt(self.client.inner.http.get(&url).build()?)
            .await?;

        match quick_xml::de::from_str::<response::ChannelRss>(&xml) {
            Ok(feed) => Ok(feed.into()),
            Err(e) => {
                if let Some(reporter) = &self.client.inner.reporter {
                    let report = Report {
                        info: Default::default(),
                        level: crate::report::Level::ERR,
                        operation: "channel_rss".to_owned(),
                        error: Some(e.to_string()),
                        msgs: Vec::new(),
                        deobf_data: None,
                        http_request: crate::report::HTTPRequest {
                            url: url,
                            method: "GET".to_owned(),
                            req_header: BTreeMap::new(),
                            req_body: String::new(),
                            status: 200,
                            resp_body: xml,
                        },
                    };

                    reporter.report(&report);
                }

                Err(e.into())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{fs::File, io::BufReader, path::Path};

    use chrono::{Datelike, Timelike};

    use crate::{
        client::{response, RustyPipe},
        model::ChannelRss,
    };

    #[tokio::test]
    async fn get_channel_rss() {
        let rp = RustyPipe::builder().strict().build();
        let channel = rp
            .query()
            .channel_rss("UCHnyfMqiRRG1u-2MsSQLbXA")
            .await
            .unwrap();

        assert_eq!(channel.id, "UCHnyfMqiRRG1u-2MsSQLbXA");
        assert_eq!(channel.name, "Veritasium");
        assert_eq!(channel.create_date.year(), 2010);
        assert_eq!(channel.create_date.month(), 7);
        assert_eq!(channel.create_date.day(), 21);
        assert_eq!(channel.create_date.hour(), 7);
        assert_eq!(channel.create_date.minute(), 18);

        assert!(!channel.videos.is_empty());
    }

    #[test]
    fn map_channel_rss() {
        let xml_path = Path::new("testfiles/channel_rss/base.xml");
        let xml_file = File::open(xml_path).unwrap();

        let feed: response::ChannelRss =
            quick_xml::de::from_reader(BufReader::new(xml_file)).unwrap();
        let map_res: ChannelRss = feed.into();

        insta::assert_ron_snapshot!("map_channel_rss", map_res);
    }
}
