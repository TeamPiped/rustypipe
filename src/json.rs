use std::fmt;

use flexon::{pointer::JsonPointer, LazyValue, Value as FlexValue};
use serde::de::DeserializeOwned;

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

    pub(crate) fn body(&self) -> &str {
        &self.body
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
            serde_json::to_string(v).map_err(|e| ExtractionError::InvalidData(e.to_string().into()))?
        } else if let Some(v) = value.as_bool() {
            if v { "true".to_owned() } else { "false".to_owned() }
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
            self.doc.resolve_value(&path).ok().map(|_| Self {
                doc: self.doc,
                path,
            })
        })
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
        serde_json::from_str(&raw).map_err(|e| ExtractionError::InvalidData(e.to_string().into()))
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
}

pub(crate) fn yt_text(node: &JsonNode<'_>) -> Option<String> {
    if let Some(text) = node.as_str() {
        return Some(text);
    }

    for key in [
        Query::first_of(&[&[PathSegment::Key("text")]]),
        Query::first_of(&[&[PathSegment::Key("simpleText")]]),
        Query::first_of(&[&[PathSegment::Key("content")]]),
    ] {
        if let Some(text) = node.query(key).and_then(|value| value.as_str()) {
            return Some(text);
        }
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
    node.first_of(&[
        ytq!(.nextContinuationData.continuation),
        ytq!(.nextRadioContinuationData.continuation),
        ytq!(.continuationEndpoint.continuationCommand.token),
        ytq!(.continuationEndpoint.commandExecutorCommand.commands[0].continuationCommand.token),
    ])
    .and_then(|value| value.as_str())
}

pub(crate) fn tab_renderer<'a>(tab: &JsonNode<'a>) -> Option<JsonNode<'a>> {
    tab.first_of(&[
        ytq!(.tabRenderer),
        ytq!(.expandableTabRenderer.tabRenderer),
    ])
}

pub(crate) fn yt_tab_list_contents<'a>(tab: &JsonNode<'a>) -> Option<JsonNode<'a>> {
    tab_renderer(tab).and_then(|tr| {
        let rich = tr.query(ytq!(.content.richGridRenderer.contents));
        let section = tr.query(ytq!(.content.sectionListRenderer.contents));
        match (rich, section) {
            (Some(rich), Some(section)) if rich.items().is_empty() && !section.items().is_empty() => {
                Some(section)
            }
            (Some(rich), _) if !rich.items().is_empty() => Some(rich),
            (_, Some(section)) => Some(section),
            _ => None,
        }
    })
}

pub(crate) fn yt_selected_tab_list_items<'a>(browse: &JsonNode<'a>) -> Option<JsonNode<'a>> {
    let tabs = browse.first_of(&[ytq!(.tabs), ytq!(.contents)])?;
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
    browse.first_of(&[ytq!(.tabs[0]), ytq!(.contents[0])])
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
    let tab = yt_first_tab(&browse).ok_or_else(|| {
        ExtractionError::InvalidData("no tab in single column browse".into())
    })?;
    yt_tab_list_contents(&tab).ok_or_else(|| {
        ExtractionError::InvalidData("no section list contents".into())
    })
}

pub(crate) fn yt_two_column_list_items_from_browse<'a>(
    browse: &JsonNode<'a>,
) -> Option<JsonNode<'a>> {
    yt_selected_tab_list_items(browse).or_else(|| {
        browse
            .first_of(&[ytq!(.tabs), ytq!(.contents)])
            .and_then(|tabs| tabs.items().into_iter().find_map(|tab| yt_tab_list_contents(&tab)))
    })
}

pub(crate) fn yt_two_column_list_items<'a>(
    root: &JsonNode<'a>,
) -> Result<JsonNode<'a>, ExtractionError> {
    let browse = root.require(
        ytq!(.contents.twoColumnBrowseResultsRenderer),
        "two column browse results",
    )?;
    yt_two_column_list_items_from_browse(&browse).ok_or_else(|| {
        ExtractionError::InvalidData("no list contents in tab".into())
    })
}

pub(crate) fn yt_search_primary_items<'a>(
    root: &JsonNode<'a>,
) -> Result<JsonNode<'a>, ExtractionError> {
    root.first_of(&[
        ytq!(.contents.twoColumnSearchResultsRenderer.primaryContents.sectionListRenderer.contents),
        ytq!(.contents.twoColumnSearchResultsRenderer.primaryContents.richGridRenderer.contents),
    ])
    .ok_or_else(|| ExtractionError::InvalidData("no search primary contents".into()))
}

pub(crate) fn yt_estimated_results(root: &JsonNode<'_>) -> Option<u64> {
    root.query(ytq!(.estimatedResults))
        .and_then(|node| node.as_str().and_then(|s| s.parse().ok()).or_else(|| node.as_u64()))
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
    node.first_of(&[ytq!(.thumbnails), ytq!(.sources), ytq!(.thumbnail.thumbnails)])
        .map(|items| {
            items.items()
                .into_iter()
                .filter_map(|item| {
                    Some(Thumbnail {
                        url: item.query(ytq!(.url))?.as_str()?,
                        width: item.query(ytq!(.width)).and_then(|v| v.as_u64()).unwrap_or_default()
                            as u32,
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

macro_rules! __yt_path {
    (@build [$($out:expr,)*]) => {
        &[$($out,)*]
    };
    (@build [$($out:expr,)*] . $key:ident $($rest:tt)*) => {
        $crate::json::__yt_path!(@next [$($out,)* $crate::json::PathSegment::Key(stringify!($key)),] $($rest)*)
    };
    (@next [$($out:expr,)*]) => {
        &[$($out,)*]
    };
    (@next [$($out:expr,)*] [ $idx:literal ] $($rest:tt)*) => {
        $crate::json::__yt_path!(@next [$($out,)* $crate::json::PathSegment::Index($idx),] $($rest)*)
    };
    (@next [$($out:expr,)*] . $key:ident $($rest:tt)*) => {
        $crate::json::__yt_path!(@next [$($out,)* $crate::json::PathSegment::Key(stringify!($key)),] $($rest)*)
    };
}

pub(crate) use __yt_path;

macro_rules! ytq {
    ($($path:tt)+) => {
        $crate::json::Query::first_of(&[$crate::json::__yt_path!(@build [] $($path)+)])
    };
}

pub(crate) use ytq;

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
}
