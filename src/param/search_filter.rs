//! YouTube search filter

use std::collections::BTreeSet;

use crate::util::{self, ProtoBuilder};

/// YouTube search filter
///
/// Allows you to filter YouTube's search results by
/// item type, features (e.g. HD, 3D, Creative commons), upload date
/// and length.
///
/// Additionally you can sort the search results by rating, upload date
/// or view count.
#[derive(Default, Debug)]
pub struct SearchFilter {
    sort: Option<Order>,
    features: BTreeSet<Feature>,
    date: Option<UploadDate>,
    item_type: Option<ItemType>,
    length: Option<Length>,
    verbatim: bool,
}

/// Video feature to filter by
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Feature {
    /// HD resolution
    IsHd = 4,
    /// Video with subtitles
    Subtitles = 5,
    /// Video published under the Creative Commons BY 3.0 license
    CCommons = 6,
    /// 3D Video
    Is3d = 7,
    /// Active livestream
    IsLive = 8,
    /// 4K resolution
    Is4k = 14,
    /// 360° Video
    Is360 = 15,
    /// 180° VR-Video
    IsVr180 = 26,
    /// HDR Video
    IsHdr = 25,
}

/// Sort order of search results
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Order {
    /// Sort by Like/Dislike ratio
    Rating = 1,
    /// Sort by upload date
    Date = 2,
    /// Sort by view count
    Views = 3,
}

/// Upload date range to filter by
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UploadDate {
    /// 1 hour old or newer
    Hour = 1,
    /// 1 day old or newer
    Day = 2,
    /// 1 week old or newer
    Week = 3,
    /// 1 month old or newer
    Month = 4,
    /// 1 year old or newer
    Year = 5,
}

/// YouTube item type to filter by
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(missing_docs)]
pub enum ItemType {
    Video = 1,
    Channel = 2,
    Playlist = 3,
}

/// Video length range to filter by
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Length {
    /// < 4min
    Short = 1,
    /// 4-20min
    Medium = 3,
    /// > 20min
    Long = 2,
}

impl SearchFilter {
    /// Get a new [`SearchFilter`]
    pub fn new() -> Self {
        Self::default()
    }

    /// Sort the search results
    pub fn sort(mut self, sort: Order) -> Self {
        self.sort = Some(sort);
        self
    }

    /// Sort the search results
    pub fn sort_opt(mut self, sort: Option<Order>) -> Self {
        self.sort = sort;
        self
    }

    /// Filter videos with specific features
    pub fn feature(mut self, feature: Feature) -> Self {
        self.features.insert(feature);
        self
    }

    /// Filter videos with specific features
    pub fn features(mut self, features: BTreeSet<Feature>) -> Self {
        self.features = features;
        self
    }

    /// Filter videos by upload date range
    pub fn date(mut self, date: UploadDate) -> Self {
        self.date = Some(date);
        self
    }

    /// Filter videos by upload date range
    pub fn date_opt(mut self, date: Option<UploadDate>) -> Self {
        self.date = date;
        self
    }

    /// Filter videos by entity type
    pub fn item_type(mut self, entity: ItemType) -> Self {
        self.item_type = Some(entity);
        self
    }

    /// Filter videos by entity type
    pub fn item_type_opt(mut self, entity: Option<ItemType>) -> Self {
        self.item_type = entity;
        self
    }

    /// Filter videos by length range
    pub fn length(mut self, length: Length) -> Self {
        self.length = Some(length);
        self
    }

    /// Filter videos by length range
    pub fn length_opt(mut self, length: Option<Length>) -> Self {
        self.length = length;
        self
    }

    /// Disable the automatic correction of mistyped search terms
    pub fn verbatim(mut self) -> Self {
        self.verbatim = true;
        self
    }

    /// Disable the automatic correction of mistyped search terms
    pub fn verbatim_set(mut self, verbatim: bool) -> Self {
        self.verbatim = verbatim;
        self
    }

    pub(crate) fn encode(&self) -> String {
        let mut filters = ProtoBuilder::new();

        if let Some(date) = self.date {
            filters.varint(1, date as u64);
        }
        if let Some(entity) = self.item_type {
            filters.varint(2, entity as u64);
        }
        if let Some(length) = self.length {
            filters.varint(3, length as u64);
        }

        self.features.iter().for_each(|feat| {
            filters.varint(*feat as u32, 1);
        });

        let mut pb = ProtoBuilder::new();

        if let Some(sort) = self.sort {
            pb.varint(1, sort as u64);
        }
        if !filters.bytes.is_empty() {
            pb.embedded(2, filters);
        }
        if self.verbatim {
            let mut extras = ProtoBuilder::new();
            extras.varint(1, 1);
            pb.embedded(8, extras)
        }

        let b64 = base64::encode(pb.bytes);
        util::urlencode(&b64)
    }
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::*;

    #[rstest]
    #[case(SearchFilter::new().item_type(ItemType::Video), "EgIQAQ%253D%253D")]
    #[case(SearchFilter::new().item_type(ItemType::Channel), "EgIQAg%253D%253D")]
    #[case(SearchFilter::new().item_type(ItemType::Playlist), "EgIQAw%253D%253D")]
    #[case(SearchFilter::new().date(UploadDate::Hour), "EgIIAQ%253D%253D")]
    #[case(SearchFilter::new().date(UploadDate::Day), "EgIIAg%253D%253D")]
    #[case(SearchFilter::new().date(UploadDate::Week), "EgIIAw%253D%253D")]
    #[case(SearchFilter::new().date(UploadDate::Month), "EgIIBA%253D%253D")]
    #[case(SearchFilter::new().date(UploadDate::Year), "EgIIBQ%253D%253D")]
    #[case(SearchFilter::new().length(Length::Short), "EgIYAQ%253D%253D")]
    #[case(SearchFilter::new().length(Length::Medium), "EgIYAw%253D%253D")]
    #[case(SearchFilter::new().length(Length::Long), "EgIYAg%253D%253D")]
    #[case(SearchFilter::new().feature(Feature::IsLive), "EgJAAQ%253D%253D")]
    #[case(SearchFilter::new().feature(Feature::Is4k), "EgJwAQ%253D%253D")]
    #[case(SearchFilter::new().feature(Feature::IsHd), "EgIgAQ%253D%253D")]
    #[case(SearchFilter::new().feature(Feature::Subtitles), "EgIoAQ%253D%253D")]
    #[case(SearchFilter::new().feature(Feature::CCommons), "EgIwAQ%253D%253D")]
    #[case(SearchFilter::new().feature(Feature::Is360), "EgJ4AQ%253D%253D")]
    #[case(SearchFilter::new().feature(Feature::IsVr180), "EgPQAQE%253D")]
    #[case(SearchFilter::new().feature(Feature::Is3d), "EgI4AQ%253D%253D")]
    #[case(SearchFilter::new().feature(Feature::IsHdr), "EgPIAQE%253D")]
    #[case(SearchFilter::new().sort(Order::Date), "CAI%253D")]
    #[case(SearchFilter::new().sort(Order::Views), "CAM%253D")]
    #[case(SearchFilter::new().sort(Order::Rating), "CAE%253D")]
    fn t_filter(#[case] filter: SearchFilter, #[case] expect: &str) {
        assert_eq!(util::urlencode(&filter.encode()), expect);
    }
}
