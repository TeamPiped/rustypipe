use std::collections::BTreeMap;

use crate::{
    error::{Error, ExtractionError},
    model::ChannelRss,
    report::Report,
};

use super::{response, RustyPipeQuery};

impl RustyPipeQuery {
    pub async fn channel_rss(&self, channel_id: &str) -> Result<ChannelRss, Error> {
        let url = format!(
            "https://www.youtube.com/feeds/videos.xml?channel_id={}",
            channel_id
        );
        let xml = self
            .client
            .http_request_txt(self.client.inner.http.get(&url).build()?)
            .await
            .map_err(|e| match e {
                Error::HttpStatus(404) => Error::Extraction(ExtractionError::ContentUnavailable(
                    "Channel not found".into(),
                )),
                _ => e,
            })?;

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
                            url,
                            method: "GET".to_owned(),
                            req_header: BTreeMap::new(),
                            req_body: String::new(),
                            status: 200,
                            resp_body: xml,
                        },
                    };

                    reporter.report(&report);
                }

                Err(ExtractionError::InvalidData(
                    format!("could not deserialize xml: {}", e).into(),
                )
                .into())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{fs::File, io::BufReader, path::Path};

    use crate::{client::response, model::ChannelRss};

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
