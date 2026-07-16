use std::collections::BTreeMap;

use ordered_hash_map::OrderedHashMap;
use rustypipe::{model::AlbumType, param::Language};
use serde::{Deserialize, Serialize};
use serde_with::{serde_as, DefaultOnError, VecSkipError};

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct DictEntry {
    /// List of languages that should be treated equally (e.g. EnUs/EnGb/EnIn)
    pub equivalent: Vec<Language>,
    /// Should the language be parsed by character instead of by word?
    /// (e.g. Chinese/Japanese)
    pub by_char: bool,
    /// True if the month has to be parsed before the day
    ///
    /// Examples:
    ///
    /// - 03.01.2020 => DMY => false
    /// - 01/03/2020 => MDY => true
    pub month_before_day: bool,
    /// Tokens for parsing timeago strings.
    ///
    /// Format: Parsed token -> \[Quantity\] Identifier
    ///
    /// Identifiers: `Y`(ear), `M`(month), `W`(eek), `D`(ay),
    /// `h`(our), `m`(inute), `s`(econd)
    pub timeago_tokens: OrderedHashMap<String, String>,
    /// Order in which to parse numeric date components. Formatted as
    /// a string of date identifiers (Y, M, D).
    ///
    /// Examples:
    ///
    /// - 03.01.2020 => `"DMY"`
    /// - Jan 3, 2020 => `"DY"`
    pub date_order: String,
    /// Tokens for parsing month names.
    ///
    /// Format: Parsed token -> Month number (starting from 1)
    pub months: BTreeMap<String, u8>,
    /// Tokens for parsing date strings with no digits (e.g. Today, Tomorrow)
    ///
    /// Format: Parsed token -> \[Quantity\] Identifier
    pub timeago_nd_tokens: OrderedHashMap<String, String>,
    /// Are commas (instead of points) used as decimal separators?
    pub comma_decimal: bool,
    /// Tokens for parsing decimal prefixes (K, M, B, ...)
    ///
    /// Format: Parsed token -> decimal power
    pub number_tokens: BTreeMap<String, u8>,
    /// Tokens for parsing number strings with no digits (e.g. "No videos")
    ///
    /// Format: Parsed token -> value
    pub number_nd_tokens: BTreeMap<String, u8>,
    /// Names of album types (Album, Single, ...)
    ///
    /// Format: Parsed text -> Album type
    pub album_types: BTreeMap<String, AlbumType>,
    /// Channel name prefix on playlist pages (e.g. `by`)
    pub chan_prefix: String,
    /// Channel name suffix on playlist pages
    pub chan_suffix: String,
    /// "Other versions" title on album pages
    pub album_versions_title: String,
}

/// Parsed TimeAgo string, contains amount and time unit.
///
/// Example: "14 hours ago" => `TimeAgo {n: 14, unit: TimeUnit::Hour}`
#[derive(Debug, Copy, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TimeAgo {
    /// Number of time units
    pub n: u8,
    /// Time unit
    pub unit: TimeUnit,
}

impl std::fmt::Display for TimeAgo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.n > 1 {
            write!(f, "{}{}", self.n, self.unit.as_str())
        } else {
            f.write_str(self.unit.as_str())
        }
    }
}

/// Parsed time unit
#[derive(Debug, Copy, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[serde(rename_all = "lowercase")]
pub enum TimeUnit {
    Second,
    Minute,
    Hour,
    Day,
    Week,
    Month,
    Year,
    LastWeek,
    LastWeekday,
}

impl TimeUnit {
    pub fn as_str(&self) -> &str {
        match self {
            TimeUnit::Second => "s",
            TimeUnit::Minute => "m",
            TimeUnit::Hour => "h",
            TimeUnit::Day => "D",
            TimeUnit::Week => "W",
            TimeUnit::Month => "M",
            TimeUnit::Year => "Y",
            TimeUnit::LastWeek => "Wl",
            TimeUnit::LastWeekday => "Wd",
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QBrowse<'a> {
    pub browse_id: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<&'a str>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QCont<'a> {
    pub continuation: &'a str,
}

#[derive(Clone, Debug, Deserialize)]
pub struct TextRuns {
    pub runs: Vec<Text>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Text {
    #[serde(alias = "simpleText")]
    pub text: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Channel {
    pub contents: TwoColumnBrowseResults,
    pub header: ChannelHeader,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChannelHeader {
    pub c4_tabbed_header_renderer: HeaderRenderer,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HeaderRenderer {
    pub subscriber_count_text: Text,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TwoColumnBrowseResults {
    pub two_column_browse_results_renderer: TabsRenderer,
}

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TabsRenderer {
    #[serde_as(as = "VecSkipError<_>")]
    pub tabs: Vec<ChannelTab>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChannelTab {
    pub tab_renderer: ChannelTabRenderer,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChannelTabRenderer {
    pub content: RichGrid,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RichGrid {
    pub rich_grid_renderer: RichGridRenderer,
}

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RichGridRenderer {
    #[serde_as(as = "VecSkipError<_>")]
    pub contents: Vec<RichItemRendererWrap>,
    #[serde(default)]
    #[serde_as(as = "DefaultOnError")]
    pub header: Option<RichGridHeader>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RichItemRendererWrap {
    pub rich_item_renderer: RichItemRenderer,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RichItemRenderer {
    pub content: VideoRendererWrap,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VideoRendererWrap {
    pub video_renderer: VideoRenderer,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VideoRenderer {
    /// `24,194 views`
    pub view_count_text: Text,
    /// `19K views`
    pub short_view_count_text: Text,
    pub length_text: LengthText,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LengthText {
    /// `18 minutes, 26 seconds`
    pub accessibility: Accessibility,
    /// `18:26`
    pub simple_text: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Accessibility {
    pub accessibility_data: AccessibilityData,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccessibilityData {
    pub label: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RichGridHeader {
    pub feed_filter_chip_bar_renderer: ChipBar,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChipBar {
    pub contents: Vec<Chip>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Chip {
    pub chip_cloud_chip_renderer: ChipRenderer,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChipRenderer {
    pub navigation_endpoint: NavigationEndpoint,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NavigationEndpoint {
    pub continuation_command: ContinuationCommand,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContinuationCommand {
    pub token: String,
}

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContinuationResponse {
    pub on_response_received_actions: Vec<ContinuationAction>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContinuationAction {
    pub reload_continuation_items_command: ContinuationItemsWrap,
}

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContinuationItemsWrap {
    #[serde_as(as = "VecSkipError<_>")]
    pub continuation_items: Vec<RichItemRendererWrap>,
}
