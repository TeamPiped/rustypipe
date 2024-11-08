//! YouTube search filter

use std::collections::BTreeSet;

use crate::util::ProtoBuilder;

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
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Sort the search results
    #[must_use]
    pub fn sort(mut self, sort: Order) -> Self {
        self.sort = Some(sort);
        self
    }

    /// Sort the search results
    #[must_use]
    pub fn sort_opt(mut self, sort: Option<Order>) -> Self {
        self.sort = sort;
        self
    }

    /// Filter videos with specific features
    #[must_use]
    pub fn feature(mut self, feature: Feature) -> Self {
        self.features.insert(feature);
        self
    }

    /// Filter videos with specific features
    #[must_use]
    pub fn features(mut self, features: BTreeSet<Feature>) -> Self {
        self.features = features;
        self
    }

    /// Filter videos by upload date range
    #[must_use]
    pub fn date(mut self, date: UploadDate) -> Self {
        self.date = Some(date);
        self
    }

    /// Filter videos by upload date range
    #[must_use]
    pub fn date_opt(mut self, date: Option<UploadDate>) -> Self {
        self.date = date;
        self
    }

    /// Filter videos by item type
    #[must_use]
    pub fn item_type(mut self, item_type: ItemType) -> Self {
        self.item_type = Some(item_type);
        self
    }

    /// Filter videos by item type
    #[must_use]
    pub fn item_type_opt(mut self, item_type: Option<ItemType>) -> Self {
        self.item_type = item_type;
        self
    }

    /// Filter videos by length range
    #[must_use]
    pub fn length(mut self, length: Length) -> Self {
        self.length = Some(length);
        self
    }

    /// Filter videos by length range
    #[must_use]
    pub fn length_opt(mut self, length: Option<Length>) -> Self {
        self.length = length;
        self
    }

    /// Disable the automatic correction of mistyped search terms
    #[must_use]
    pub fn verbatim(mut self) -> Self {
        self.verbatim = true;
        self
    }

    /// Disable the automatic correction of mistyped search terms
    #[must_use]
    pub fn verbatim_set(mut self, verbatim: bool) -> Self {
        self.verbatim = verbatim;
        self
    }

    pub(crate) fn encode(&self) -> String {
        let mut filters = ProtoBuilder::new();

        if let Some(date) = self.date {
            filters.varint(1, date as u64);
        }
        if let Some(item_type) = self.item_type {
            filters.varint(2, item_type as u64);
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
        if !filters.is_empty() {
            pb.embedded(2, filters);
        }
        if self.verbatim {
            let mut extras = ProtoBuilder::new();
            extras.varint(1, 1);
            pb.embedded(8, extras);
        }
        // Disable filter for sensitive topics (e.g. suicide)
        pb.varint(30, 1);

        pb.to_base64()
    }
}

/// YouTube Music search filter
///
/// Allows you to filter YTM search results by item type.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum MusicSearchFilter {
    /// YouTube Music tracks
    Tracks,
    /// YouTube videos
    Videos,
    /// Albums
    Albums,
    /// Artists
    Artists,
    /// Playlists created by YouTube Music
    YtmPlaylists,
    /// Playlists created by YouTube users
    CommunityPlaylists,
    /// Users
    Users,
}

impl MusicSearchFilter {
    pub(crate) fn params(self) -> &'static str {
        match self {
            MusicSearchFilter::Tracks => "EgWKAQIIAWoQEAMQBBAJEAoQBRAREBAQFQ%3D%3D",
            MusicSearchFilter::Videos => "EgWKAQIQAWoQEAMQBBAJEAoQBRAREBAQFQ%3D%3D",
            MusicSearchFilter::Albums => "EgWKAQIYAWoQEAMQBBAJEAoQBRAREBAQFQ%3D%3D",
            MusicSearchFilter::Artists => "EgWKAQIgAWoQEAMQBBAJEAoQBRAREBAQFQ%3D%3D",
            MusicSearchFilter::YtmPlaylists => "EgeKAQQoADgBahIQAxAEEAkQDhAKEAUQERAQEBU%3D",
            MusicSearchFilter::CommunityPlaylists => "EgeKAQQoAEABahAQAxAEEAkQChAFEBEQEBAV",
            MusicSearchFilter::Users => "EgWKAQJYAWoQEAMQBBAJEAoQBRAREBAQFQ%3D%3D",
        }
    }
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::*;

    #[rstest]
    #[case(SearchFilter::new().item_type(ItemType::Video), "EgIQAfABAQ%253D%253D")]
    #[case(SearchFilter::new().item_type(ItemType::Channel), "EgIQAvABAQ%253D%253D")]
    #[case(SearchFilter::new().item_type(ItemType::Playlist), "EgIQA_ABAQ%253D%253D")]
    #[case(SearchFilter::new().date(UploadDate::Hour), "EgIIAfABAQ%253D%253D")]
    #[case(SearchFilter::new().date(UploadDate::Day), "EgIIAvABAQ%253D%253D")]
    #[case(SearchFilter::new().date(UploadDate::Week), "EgIIA_ABAQ%253D%253D")]
    #[case(SearchFilter::new().date(UploadDate::Month), "EgIIBPABAQ%253D%253D")]
    #[case(SearchFilter::new().date(UploadDate::Year), "EgIIBfABAQ%253D%253D")]
    #[case(SearchFilter::new().length(Length::Short), "EgIYAfABAQ%253D%253D")]
    #[case(SearchFilter::new().length(Length::Medium), "EgIYA_ABAQ%253D%253D")]
    #[case(SearchFilter::new().length(Length::Long), "EgIYAvABAQ%253D%253D")]
    #[case(SearchFilter::new().feature(Feature::IsLive), "EgJAAfABAQ%253D%253D")]
    #[case(SearchFilter::new().feature(Feature::Is4k), "EgJwAfABAQ%253D%253D")]
    #[case(SearchFilter::new().feature(Feature::IsHd), "EgIgAfABAQ%253D%253D")]
    #[case(SearchFilter::new().feature(Feature::Subtitles), "EgIoAfABAQ%253D%253D")]
    #[case(SearchFilter::new().feature(Feature::CCommons), "EgIwAfABAQ%253D%253D")]
    #[case(SearchFilter::new().feature(Feature::Is360), "EgJ4AfABAQ%253D%253D")]
    #[case(SearchFilter::new().feature(Feature::IsVr180), "EgPQAQHwAQE%253D")]
    #[case(SearchFilter::new().feature(Feature::Is3d), "EgI4AfABAQ%253D%253D")]
    #[case(SearchFilter::new().feature(Feature::IsHdr), "EgPIAQHwAQE%253D")]
    #[case(SearchFilter::new().sort(Order::Date), "CALwAQE%253D")]
    #[case(SearchFilter::new().sort(Order::Views), "CAPwAQE%253D")]
    #[case(SearchFilter::new().sort(Order::Rating), "CAHwAQE%253D")]
    fn t_filter(#[case] filter: SearchFilter, #[case] expect: &str) {
        assert_eq!(urlencoding::encode(&filter.encode()), expect);
    }
}
