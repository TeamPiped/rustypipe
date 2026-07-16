use std::fmt;

use flexon::{
    pointer::JsonPointer, value::builder::ObjectBuilder, LazyValue, OwnedValue, Value as FlexValue,
};
use serde::de::DeserializeOwned;
use serde::Serialize;

use crate::{error::ExtractionError, model::Thumbnail};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PathSegment {
    Key(&'static str),
    Index(usize),
}

impl JsonPointer for PathSegment {
    fn as_key(&self) -> Option<&str> {
        match self {
            Self::Key(key) => Some(key),
            Self::Index(_) => None,
        }
    }

    fn as_index(&self) -> Option<usize> {
        match self {
            Self::Key(_) => None,
            Self::Index(idx) => Some(*idx),
        }
    }
}

#[derive(Clone, Copy)]
pub(crate) struct Query {
    branches: &'static [&'static [PathSegment]],
}

impl Query {
    pub(crate) const fn first_of(branches: &'static [&'static [PathSegment]]) -> Self {
        Self { branches }
    }
}

impl fmt::Debug for Query {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Query").finish_non_exhaustive()
    }
}

#[derive(Clone, Debug)]
pub(crate) struct JsonDoc {
    body: String,
}

impl JsonDoc {
    pub(crate) fn new(body: String) -> Self {
        Self { body }
    }

    pub(crate) fn with_root<T>(
        &self,
        f: impl for<'a> FnOnce(JsonNode<'a>) -> Result<T, ExtractionError>,
    ) -> Result<T, ExtractionError> {
        f(JsonNode::root(self))
    }

    fn resolve_value<'a>(&'a self, path: &[PathSegment]) -> Result<FlexValue<'a>, ExtractionError> {
        if path.is_empty() {
            flexon::parse::<_, FlexValue<'a>>(self.body.as_str())
                .map_err(|e| ExtractionError::InvalidData(format!("{e:?}").into()))
        } else {
            flexon::parse_at::<_, FlexValue<'a>, _>(self.body.as_str(), path.iter().copied())
                .map_err(|e| ExtractionError::InvalidData(format!("{e:?}").into()))
        }
    }

    fn resolve_raw(&self, path: &[PathSegment]) -> Result<String, ExtractionError> {
        let value = if path.is_empty() {
            flexon::parse::<_, LazyValue<'_>>(self.body.as_str())
        } else {
            flexon::parse_at::<_, LazyValue<'_>, _>(self.body.as_str(), path.iter().copied())
        }
        .map_err(|e| ExtractionError::InvalidData(format!("{e:?}").into()))?;

        if let Some(raw) = value.as_raw() {
            return Ok(raw.trim_to_value().to_owned());
        }

        let value = self.resolve_value(path)?;
        let json = if let Some(v) = value.as_str() {
            flexon::to_string(v).map_err(|e| ExtractionError::InvalidData(e.to_string().into()))?
        } else if let Some(v) = value.as_bool() {
            if v {
                "true".to_owned()
            } else {
                "false".to_owned()
            }
        } else if value.is_null() {
            "null".to_owned()
        } else if let Some(v) = value.as_u64() {
            v.to_string()
        } else if let Some(v) = value.as_i64() {
            v.to_string()
        } else if let Some(v) = value.as_f64() {
            v.to_string()
        } else {
            return Err(ExtractionError::InvalidData(
                "could not materialize JSON subtree".into(),
            ));
        };

        Ok(json)
    }
}

#[derive(Clone, Debug)]
pub(crate) struct JsonNode<'a> {
    doc: &'a JsonDoc,
    path: Vec<PathSegment>,
}

impl<'a> JsonNode<'a> {
    fn root(doc: &'a JsonDoc) -> Self {
        Self {
            doc,
            path: Vec::new(),
        }
    }

    pub(crate) fn query(&self, query: Query) -> Option<Self> {
        query.branches.iter().find_map(|branch| {
            let mut path = self.path.clone();
            path.extend_from_slice(branch);
            let value = self.doc.resolve_value(&path).ok()?;
            if value.is_null() {
                return None;
            }
            Some(Self {
                doc: self.doc,
                path,
            })
        })
    }

    /// Query the node and parse the result as a list of [`Thumbnail`]s.
    /// Returns an empty `Vec` if the path doesn't match.
    pub(crate) fn query_thumbnails(&self, query: Query) -> Vec<Thumbnail> {
        self.query(query)
            .map(|node| yt_thumbnails(&node))
            .unwrap_or_default()
    }

    pub(crate) fn require(
        &self,
        query: Query,
        what: &'static str,
    ) -> Result<Self, ExtractionError> {
        self.query(query).ok_or_else(|| {
            ExtractionError::InvalidData(format!("missing {what} in JSON response").into())
        })
    }

    pub(crate) fn first_of(&self, queries: &[Query]) -> Option<Self> {
        queries.iter().find_map(|query| self.query(*query))
    }

    pub(crate) fn as_str(&self) -> Option<String> {
        self.doc
            .resolve_value(&self.path)
            .ok()
            .and_then(|value| value.as_str().map(str::to_owned))
    }

    pub(crate) fn as_bool(&self) -> Option<bool> {
        self.doc
            .resolve_value(&self.path)
            .ok()
            .and_then(|value| value.as_bool())
    }

    pub(crate) fn as_u64(&self) -> Option<u64> {
        self.doc
            .resolve_value(&self.path)
            .ok()
            .and_then(|value| value.as_u64())
    }

    /// Query a sub-path and resolve it as a string. Equivalent to
    /// `self.query(q).and_then(|n| n.as_str())`.
    pub(crate) fn query_str(&self, q: Query) -> Option<String> {
        self.query(q).and_then(|n| n.as_str())
    }

    /// Query a sub-path and resolve it as a `u64`.
    pub(crate) fn query_u64(&self, q: Query) -> Option<u64> {
        self.query(q).and_then(|n| n.as_u64())
    }

    /// Query a sub-path and resolve it as a `u32`. Returns `None` if the
    /// value is not a `u64` that fits in `u32`.
    pub(crate) fn query_u32(&self, q: Query) -> Option<u32> {
        self.query_u64(q).and_then(|n| u32::try_from(n).ok())
    }

    /// Query a sub-path and deserialize it as `T` via `flexon::from_str`.
    /// Returns `Err(ExtractionError::InvalidData)` if the path doesn't match
    /// or the value can't be deserialized.
    #[allow(dead_code)]
    pub(crate) fn deserialize_at<T: DeserializeOwned>(
        &self,
        q: Query,
    ) -> Result<T, ExtractionError> {
        self.query(q)
            .ok_or_else(|| ExtractionError::InvalidData("ytq! returned None".into()))?
            .deserialize()
    }

    /// Query a sub-path and resolve it as a text string (via `yt_text`).
    /// Equivalent to `self.query(q).and_then(|n| n.text())`.
    pub(crate) fn text_at(&self, q: Query) -> Option<String> {
        self.query(q).and_then(|n| n.text())
    }

    /// Try to deserialize a sub-path as `T`. Pushes a warning on
    /// *deserialization* failure and returns `None`. A missing path is
    /// treated silently as `None` (no warning).
    pub(crate) fn try_deserialize<T: DeserializeOwned>(
        &self,
        q: Query,
        warnings: &mut Vec<String>,
    ) -> Option<T> {
        let node = self.query(q)?;
        match node.deserialize::<T>() {
            Ok(v) => Some(v),
            Err(e) => {
                warnings.push(e.to_string());
                None
            }
        }
    }

    /// Try to deserialize the items of a sub-array as `T` (lossy, accumulating
    /// warnings). Returns an empty `Vec` if the path doesn't match.
    #[allow(dead_code)]
    pub(crate) fn try_deserialize_items<T: DeserializeOwned>(
        &self,
        q: Query,
        warnings: &mut Vec<String>,
    ) -> Vec<T> {
        match self.query(q) {
            Some(node) => {
                let (items, mut new_warnings) = node.deserialize_items_lossy::<T>();
                warnings.append(&mut new_warnings);
                items
            }
            None => Vec::new(),
        }
    }

    pub(crate) fn items(&self) -> Vec<Self> {
        self.doc
            .resolve_value(&self.path)
            .ok()
            .and_then(|value| value.as_array().map(|arr| arr.len()))
            .map(|len| {
                (0..len)
                    .map(|idx| {
                        let mut path = self.path.clone();
                        path.push(PathSegment::Index(idx));
                        Self {
                            doc: self.doc,
                            path,
                        }
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default()
    }

    pub(crate) fn deserialize<T: DeserializeOwned>(&self) -> Result<T, ExtractionError> {
        let raw = self.doc.resolve_raw(&self.path)?;
        flexon::from_str(&raw).map_err(|e| ExtractionError::InvalidData(e.to_string().into()))
    }

    pub(crate) fn deserialize_items_lossy<T: DeserializeOwned>(&self) -> (Vec<T>, Vec<String>) {
        let mut out = Vec::new();
        let mut warnings = Vec::new();

        for item in self.items() {
            match item.deserialize::<T>() {
                Ok(value) => out.push(value),
                Err(err) => warnings.push(err.to_string()),
            }
        }

        (out, warnings)
    }

    pub(crate) fn text(&self) -> Option<String> {
        yt_text(self)
    }

    /// Resolve this node to an owned [`JsonValue`].
    pub(crate) fn to_json_value(&self) -> Option<JsonValue> {
        let raw = self.doc.resolve_raw(&self.path).ok()?;
        flexon::from_str::<JsonValue>(&raw).ok()
    }
}

/// Types that can be parsed from a [`JsonNode`] using the `ytq!` macro paths
/// and (optionally) a custom `from_node` body generated by the
/// `#[derive(FromYtNode)]` proc-macro in `rustypipe-derive`.
///
/// No blanket `Deserialize` impl is provided, so the macro is required to
/// implement the body. This avoids the stack-overflow problem that arises
/// from a blanket `T: Deserialize` impl on `FromYtNode`.
pub(crate) trait FromYtNode<'a>: Sized {
    fn from_node(node: &JsonNode<'a>) -> Option<Self>;
}

pub(crate) type JsonValue = OwnedValue;

pub(crate) fn json_from_str<T: DeserializeOwned>(
    body: &str,
) -> Result<T, flexon::serde::de::Error> {
    flexon::from_str(body)
}

pub(crate) fn json_to_string<T: Serialize + ?Sized>(
    value: &T,
) -> Result<String, flexon::serde::ser::Error> {
    flexon::to_string(value)
}

pub(crate) fn value_from_json_value<T: DeserializeOwned>(value: &JsonValue) -> Option<T> {
    json_to_string(value)
        .ok()
        .and_then(|raw| json_from_str(&raw).ok())
}

pub(crate) fn value_from_json_value_owned<T: DeserializeOwned>(value: JsonValue) -> Option<T> {
    value_from_json_value(&value)
}

pub(crate) fn value_to_json_string(value: &JsonValue) -> String {
    json_to_string(value).unwrap_or_default()
}

pub(crate) fn json_null() -> JsonValue {
    JsonValue::Null
}

pub(crate) fn object_value(items: impl IntoIterator<Item = (String, JsonValue)>) -> JsonValue {
    let mut object = flexon::value::Object::new();
    for (key, value) in items {
        object.on_value(key.into(), value);
    }
    JsonValue::Object(object)
}

pub(crate) fn yt_text(node: &JsonNode<'_>) -> Option<String> {
    if let Some(text) = node.as_str() {
        return Some(text);
    }

    if let Some(text) = node
        .query(ytq!(.text || .simpleText || .content))
        .and_then(|value| value.as_str())
    {
        return Some(text);
    }

    node.query(ytq!(.runs)).and_then(|runs| {
        let mut out = String::new();
        for item in runs.items() {
            if let Some(text) = item.text() {
                out.push_str(&text);
            }
        }
        if out.is_empty() {
            None
        } else {
            Some(out)
        }
    })
}

pub(crate) fn yt_continuation(node: &JsonNode<'_>) -> Option<String> {
    if let Some(token) = node
        .query(ytq!(
            .( .nextContinuationData || .nextRadioContinuationData ).continuation
            || ($root || .continuationEndpoint).continuationCommand.token
        ))
        .and_then(|value| value.as_str())
    {
        return Some(token);
    }

    for commands in [
        node.query(ytq!(.commandExecutorCommand.commands)),
        node.query(ytq!(.continuationEndpoint.commandExecutorCommand.commands)),
    ]
    .into_iter()
    .flatten()
    {
        for command in commands.items() {
            if let Some(token) = command
                .query(ytq!(.continuationCommand.token))
                .and_then(|value| value.as_str())
            {
                return Some(token);
            }
        }
    }

    None
}

pub(crate) fn yt_continuation_value(value: &JsonValue) -> Option<String> {
    let doc = JsonDoc::new(value_to_json_string(value));
    doc.with_root(|root| Ok(yt_continuation(&root)))
        .ok()
        .flatten()
}

pub(crate) fn tab_renderer<'a>(tab: &JsonNode<'a>) -> Option<JsonNode<'a>> {
    tab.query(ytq!(($root || .expandableTabRenderer).tabRenderer))
}

pub(crate) fn yt_tab_list_contents<'a>(tab: &JsonNode<'a>) -> Option<JsonNode<'a>> {
    tab_renderer(tab).and_then(|tr| {
        let rich = tr.query(ytq!(.content.richGridRenderer.contents));
        let section = tr.query(ytq!(.content.sectionListRenderer.contents));
        match (rich, section) {
            (Some(rich), Some(section))
                if rich.items().is_empty() && !section.items().is_empty() =>
            {
                Some(section)
            }
            (Some(rich), _) if !rich.items().is_empty() => Some(rich),
            (_, Some(section)) => Some(section),
            _ => None,
        }
    })
}

pub(crate) fn yt_selected_tab_list_items<'a>(browse: &JsonNode<'a>) -> Option<JsonNode<'a>> {
    let tabs = browse.query(ytq!(.tabs || .contents))?;
    for tab in tabs.items() {
        let selected = tab_renderer(&tab)
            .and_then(|tr| tr.query(ytq!(.selected)))
            .and_then(|node| node.as_bool())
            .unwrap_or(false);
        if selected {
            return yt_tab_list_contents(&tab);
        }
    }
    None
}

pub(crate) fn yt_first_tab<'a>(browse: &JsonNode<'a>) -> Option<JsonNode<'a>> {
    browse.query(ytq!(.tabs[0] || .contents[0]))
}

pub(crate) fn yt_single_column_browse<'a>(
    root: &JsonNode<'a>,
) -> Result<JsonNode<'a>, ExtractionError> {
    root.require(
        ytq!(.contents.singleColumnBrowseResultsRenderer),
        "single column browse results",
    )
}

pub(crate) fn yt_single_column_sections<'a>(
    root: &JsonNode<'a>,
) -> Result<JsonNode<'a>, ExtractionError> {
    let browse = yt_single_column_browse(root)?;
    if let Some(items) = yt_selected_tab_list_items(&browse) {
        return Ok(items);
    }
    let tab = yt_first_tab(&browse)
        .ok_or_else(|| ExtractionError::InvalidData("no tab in single column browse".into()))?;
    yt_tab_list_contents(&tab)
        .ok_or_else(|| ExtractionError::InvalidData("no section list contents".into()))
}

pub(crate) fn yt_two_column_list_items_from_browse<'a>(
    browse: &JsonNode<'a>,
) -> Option<JsonNode<'a>> {
    yt_selected_tab_list_items(browse).or_else(|| {
        browse.query(ytq!(.tabs || .contents)).and_then(|tabs| {
            tabs.items()
                .into_iter()
                .find_map(|tab| yt_tab_list_contents(&tab))
        })
    })
}

pub(crate) fn yt_two_column_list_items<'a>(
    root: &JsonNode<'a>,
) -> Result<JsonNode<'a>, ExtractionError> {
    let browse = root.require(
        ytq!(.contents.twoColumnBrowseResultsRenderer),
        "two column browse results",
    )?;
    yt_two_column_list_items_from_browse(&browse)
        .ok_or_else(|| ExtractionError::InvalidData("no list contents in tab".into()))
}

pub(crate) fn yt_search_primary_items<'a>(
    root: &JsonNode<'a>,
) -> Result<JsonNode<'a>, ExtractionError> {
    root.query(ytq!(
        .contents.twoColumnSearchResultsRenderer.primaryContents.(.sectionListRenderer || .richGridRenderer).contents
    ))
    .ok_or_else(|| ExtractionError::InvalidData("no search primary contents".into()))
}

pub(crate) fn yt_estimated_results(root: &JsonNode<'_>) -> Option<u64> {
    root.query(ytq!(.estimatedResults)).and_then(|node| {
        node.as_str()
            .and_then(|s| s.parse().ok())
            .or_else(|| node.as_u64())
    })
}

pub(crate) fn yt_response_visitor_data(root: &JsonNode<'_>) -> Option<String> {
    root.query(ytq!(.responseContext.visitorData))
        .and_then(|node| node.as_str())
}

pub(crate) fn yt_music_header_title(root: &JsonNode<'_>) -> Option<String> {
    root.query(ytq!(.header.musicHeaderRenderer.title))
        .and_then(|node| node.text())
}

pub(crate) fn yt_thumbnails(node: &JsonNode<'_>) -> Vec<Thumbnail> {
    node.query(ytq!(($root || .thumbnail).thumbnails || .sources))
        .map(|items| {
            items
                .items()
                .into_iter()
                .filter_map(|item| {
                    Some(Thumbnail {
                        url: item.query(ytq!(.url))?.as_str()?,
                        width: item
                            .query(ytq!(.width))
                            .and_then(|v| v.as_u64())
                            .unwrap_or_default() as u32,
                        height: item
                            .query(ytq!(.height))
                            .and_then(|v| v.as_u64())
                            .unwrap_or_default() as u32,
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Build a [`Query`] from a compact path expression at compile time.
///
/// Syntax:
/// - `.key` — key access
/// - `[index]` — array index
/// - `||` — top-level alternation (e.g. `.a || .b`)
/// - `.(.a || .b)` — sub-path alternation; expands to a cross-product
///   (e.g. `.prefix.(.a || .b).suffix` becomes `.prefix.a.suffix` and
///   `.prefix.b.suffix`)
/// - `$root` — inside a `(...)` group, represents the empty path. Useful when
///   one of the alternatives should be the root itself (e.g.
///   `($root || .continuationEndpoint).continuationCommand.token` expands to
///   `.continuationCommand.token` and
///   `.continuationEndpoint.continuationCommand.token`)
pub(crate) use rustypipe_derive::ytq;

/// Round-trip a `JsonValue` through a `JsonDoc` so `ytq!` queries can walk it.
/// This is the bridge used by manual `Deserialize` impls on response types.
/// Returns `None` if the `from_node` callback yields no value, mirroring the
/// `Option` semantics of `ytq!` queries.
pub(crate) fn round_trip<F, T>(value: &JsonValue, f: F) -> Option<T>
where
    F: for<'a> FnOnce(&JsonNode<'a>) -> Option<T>,
{
    let raw = value_to_json_string(value);
    let doc = JsonDoc::new(raw);
    doc.with_root(|root| Ok(f(&root))).ok().flatten()
}

// =====================================================================
// Accessor helpers (WS1, WS2, WS7)
// =====================================================================

/// Ergonomic getters for `JsonValue` (the owned, flexon `OwnedValue`).
///
/// These collapse the repeated `value.get("k").and_then(|v| v.as_str())...`
/// chains in manual `impl Deserialize` blocks. They never allocate unless
/// they succeed, and the result is owned (so it survives past the borrowed
/// `JsonValue`).
#[allow(dead_code)]
pub(crate) trait JsonGet {
    fn get_str(&self, key: &str) -> Option<String>;
    fn require_str(&self, key: &str) -> Result<String, ExtractionError>;
    fn get_bool(&self, key: &str) -> Option<bool>;
    fn get_u64(&self, key: &str) -> Option<u64>;
    fn get_u32(&self, key: &str) -> Option<u32>;
    fn get_object(&self, key: &str) -> Option<&JsonValue>;
    fn get_value(&self, key: &str) -> Option<JsonValue>;
}

#[allow(dead_code)]
impl JsonGet for JsonValue {
    fn get_str(&self, key: &str) -> Option<String> {
        self.get(key).and_then(|v| v.as_str()).map(str::to_owned)
    }

    fn require_str(&self, key: &str) -> Result<String, ExtractionError> {
        self.get_str(key)
            .ok_or_else(|| ExtractionError::InvalidData(
                format!("missing required string field `{key}`").into(),
            ))
    }

    fn get_bool(&self, key: &str) -> Option<bool> {
        self.get(key).and_then(|v| v.as_bool())
    }

    fn get_u64(&self, key: &str) -> Option<u64> {
        self.get(key).and_then(|v| v.as_u64())
    }

    fn get_u32(&self, key: &str) -> Option<u32> {
        self.get_u64(key).and_then(|n| u32::try_from(n).ok())
    }

    fn get_object(&self, key: &str) -> Option<&JsonValue> {
        self.get(key)
    }

    fn get_value(&self, key: &str) -> Option<JsonValue> {
        self.get(key).cloned()
    }
}

/// Generate a `Deserialize` impl that round-trips the input through
/// [`round_trip`] and calls `from_node` on the resulting `JsonNode`.
///
/// Use this for any type that exposes `fn from_node(&JsonNode) -> Option<Self>`.
/// The error message includes the type name for debuggability.
#[macro_export]
macro_rules! deserialize_through_node {
    ($t:ty) => {
        impl<'de> serde::Deserialize<'de> for $t {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: serde::Deserializer<'de>,
            {
                let value = $crate::json::JsonValue::deserialize(deserializer)?;
                $crate::json::round_trip(&value, |root| <$t>::from_node(root))
                    .ok_or_else(|| {
                        serde::de::Error::custom(
                            concat!("failed to deserialize ", stringify!($t), " from ytq! node")
                        )
                    })
            }
        }
    };
}

/// Define a string-keyed enum with a `Default`, `from_str` and `Deserialize`
/// impl. Variant-to-string mapping is given inline; everything else is
/// generated. Multiple `= "..."` strings may follow a variant name to
/// declare aliases for the same variant.
///
/// Syntax:
/// ```ignore
/// yt_string_enum! {
///     pub(crate) enum Quality {
///         Tiny = "tiny",
///         Medium = "medium",
///         Large = "large" | "huge",
///     }
///     default: Quality::Medium
/// }
/// ```
#[macro_export]
macro_rules! yt_string_enum {
    (
        $(#[$attr:meta])*
        $vis:vis enum $name:ident {
            $( $(#[$var_attr:meta])* $variant:ident = $first_str:literal $(| $alias:literal)* ),+ $(,)?
        }
        default: $default:expr
    ) => {
        $crate::yt_string_enum!(@build
            [$($attr)*], $vis, $name,
            [$($variant),+],
            [$($first_str $(| $alias)*),+],
            $default,
            /* fallback */ false
        );
    };
    (
        $(#[$attr:meta])*
        $vis:vis enum $name:ident {
            $( $(#[$var_attr:meta])* $variant:ident = $first_str:literal $(| $alias:literal)* ),+ $(,)?
        }
        default: $default:expr,
        fallback_to_default
    ) => {
        $crate::yt_string_enum!(@build
            [$($attr)*], $vis, $name,
            [$($variant),+],
            [$($first_str $(| $alias)*),+],
            $default,
            /* fallback */ true
        );
    };
    // Internal: actual code emission.
    (@build
        [$($attr:meta)*], $vis:vis, $name:ident,
        [$($variant:ident),+],
        [$($first_str:literal $(| $alias:literal)*),+],
        $default:expr,
        $fallback:literal
    ) => {
        $(#[$attr])*
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
        $vis enum $name {
            $($variant),+
        }

        impl $name {
            pub(crate) fn from_str(s: &str) -> Option<Self> {
                match s {
                    $($first_str $(| $alias)* => Some(Self::$variant),)+
                    _ => {
                        if $fallback {
                            return Some($default);
                        }
                        None
                    }
                }
            }
        }

        impl std::default::Default for $name {
            fn default() -> Self {
                $default
            }
        }

        impl<'de> serde::Deserialize<'de> for $name {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: serde::Deserializer<'de>,
            {
                let s = String::deserialize(deserializer)?;
                Self::from_str(&s).ok_or_else(|| {
                    serde::de::Error::custom(format!(
                        "unknown {}: {s}",
                        stringify!($name)
                    ))
                })
            }
        }
    };
}

/// Query the node at `$path` and deserialize the result as an
/// `AttributedText` (the new-style rich-text format) and convert it to
/// `TextComponents`. Returns `None` if the path or deserialize fails.
#[macro_export]
macro_rules! ytq_attributed_text {
    ($node:expr, $($tt:tt)+) => {{
        let node = match $node.query(crate::json::ytq!($($tt)+)) {
            Some(n) => n,
            None => return None,
        };
        $crate::serializer::text::AttributedText::from_node(&node)
    }};
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn query_macro_reads_nested_path() {
        let doc = JsonDoc::new(r#"{"a":{"b":[{"c":"ok"}]}}"#.to_owned());
        let res = doc
            .with_root(|root| Ok(root.query(ytq!(.a.b[0].c)).and_then(|value| value.as_str())))
            .unwrap();

        assert_eq!(res.as_deref(), Some("ok"));
    }

    #[test]
    fn text_helper_reads_runs() {
        let doc = JsonDoc::new(r#"{"runs":[{"text":"hello"},{"text":" world"}]}"#.to_owned());
        let res = doc.with_root(|root| Ok(yt_text(&root))).unwrap();

        assert_eq!(res.as_deref(), Some("hello world"));
    }

    #[test]
    fn query_macro_alternative_paths() {
        let doc = JsonDoc::new(r#"{"x":1,"y":2}"#.to_owned());
        let res = doc
            .with_root(|root| {
                Ok(root
                    .query(ytq!(.missing || .x))
                    .and_then(|value| value.as_u64()))
            })
            .unwrap();

        assert_eq!(res, Some(1));
    }

    #[test]
    fn query_macro_skips_null_branch() {
        let doc = JsonDoc::new(r#"{"a":null,"b":"ok"}"#.to_owned());
        let res = doc
            .with_root(|root| Ok(root.query(ytq!(.a || .b)).and_then(|value| value.as_str())))
            .unwrap();

        assert_eq!(res.as_deref(), Some("ok"));
    }

    #[test]
    fn query_macro_prefers_first_non_null_branch() {
        let doc = JsonDoc::new(r#"{"a":"first","b":"second"}"#.to_owned());
        let res = doc
            .with_root(|root| Ok(root.query(ytq!(.a || .b)).and_then(|value| value.as_str())))
            .unwrap();

        assert_eq!(res.as_deref(), Some("first"));
    }

    #[test]
    fn query_macro_group_matches_first_option() {
        // `.(.a || .b).c` expands to two paths: `.a.c` and `.b.c`.
        // For the JSON `{"a":{"c":"hit"}, "b":{}}`, the first option matches.
        let doc = JsonDoc::new(r#"{"a":{"c":"hit"},"b":{}}"#.to_owned());
        let res = doc
            .with_root(|root| {
                Ok(root
                    .query(ytq!(.(.a || .b).c))
                    .and_then(|value| value.as_str()))
            })
            .unwrap();

        assert_eq!(res.as_deref(), Some("hit"));
    }

    #[test]
    fn query_macro_group_falls_through_to_second_option() {
        // `.(.a || .b).c` should fall through to `.b.c` if `.a.c` is missing.
        let doc = JsonDoc::new(r#"{"a":{},"b":{"c":"hit"}}"#.to_owned());
        let res = doc
            .with_root(|root| {
                Ok(root
                    .query(ytq!(.(.a || .b).c))
                    .and_then(|value| value.as_str()))
            })
            .unwrap();

        assert_eq!(res.as_deref(), Some("hit"));
    }

    #[test]
    fn query_macro_group_with_index() {
        // `.(.a[0] || .b[0]).c` expands to `.a[0].c` and `.b[0].c`.
        let doc = JsonDoc::new(r#"{"a":[{}],"b":[{"c":"hit"}]}"#.to_owned());
        let res = doc
            .with_root(|root| {
                Ok(root
                    .query(ytq!(.(.a[0] || .b[0]).c))
                    .and_then(|value| value.as_str()))
            })
            .unwrap();

        assert_eq!(res.as_deref(), Some("hit"));
    }

    #[test]
    fn query_macro_multiple_groups() {
        // `.(.a || .b).(.c || .d)` expands to 4 paths.
        let doc = JsonDoc::new(r#"{"a":{"d":"hit"}}"#.to_owned());
        let res = doc
            .with_root(|root| {
                Ok(root
                    .query(ytq!(.(.a || .b).(.c || .d)))
                    .and_then(|value| value.as_str()))
            })
            .unwrap();

        assert_eq!(res.as_deref(), Some("hit"));
    }

    #[test]
    fn query_macro_group_middle_of_path() {
        // Group in the middle: `prefix.(.a || .b).suffix`.
        let doc = JsonDoc::new(r#"{"prefix":{"b":{"suffix":"hit"}}}"#.to_owned());
        let res = doc
            .with_root(|root| {
                Ok(root
                    .query(ytq!(.prefix.(.a || .b).suffix))
                    .and_then(|value| value.as_str()))
            })
            .unwrap();

        assert_eq!(res.as_deref(), Some("hit"));
    }

    #[test]
    fn query_macro_group_and_top_level() {
        // Mixing parenthesized group and top-level ||.
        // `.(.a || .b).c || .d` expands to 3 paths.
        let doc = JsonDoc::new(r#"{"d":"hit"}"#.to_owned());
        let res = doc
            .with_root(|root| {
                Ok(root
                    .query(ytq!(.(.a || .b).c || .d))
                    .and_then(|value| value.as_str()))
            })
            .unwrap();

        assert_eq!(res.as_deref(), Some("hit"));
    }

    #[test]
    fn query_macro_root_in_group_matches_first() {
        // `$root` inside a group means "the root itself".
        // `($root || .continuationEndpoint).continuationCommand.token` expands
        // to `.continuationCommand.token` and
        // `.continuationEndpoint.continuationCommand.token`.
        // For the JSON below, the first option matches.
        let doc = JsonDoc::new(r#"{"continuationCommand":{"token":"hit"}}"#.to_owned());
        let res = doc
            .with_root(|root| {
                Ok(root
                    .query(ytq!(($root || .continuationEndpoint).continuationCommand.token))
                    .and_then(|value| value.as_str()))
            })
            .unwrap();

        assert_eq!(res.as_deref(), Some("hit"));
    }

    #[test]
    fn query_macro_root_in_group_falls_through() {
        // `$root` should be the first branch; falls through to the second
        // if the first isn't present.
        let doc = JsonDoc::new(
            r#"{"continuationEndpoint":{"continuationCommand":{"token":"hit"}}}"#.to_owned(),
        );
        let res = doc
            .with_root(|root| {
                Ok(root
                    .query(ytq!(($root || .continuationEndpoint).continuationCommand.token))
                    .and_then(|value| value.as_str()))
            })
            .unwrap();

        assert_eq!(res.as_deref(), Some("hit"));
    }
}
