pub mod text;

mod range;
mod vec_log_err;

pub use range::Range;

use std::fmt::Debug;

/// This represents a result from a deserializing/mapping operation.
/// It holds the desired content (`c`) and a list of warning messages,
/// if there occurred minor error during the deserializing or mapping
/// (e.g. certain list items could not be deserialized).
#[derive(Clone)]
pub struct MapResult<T> {
    pub c: T,
    pub warnings: Vec<String>,
}

impl<T> Debug for MapResult<T>
where
    T: Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.c.fmt(f)
    }
}

impl<T> Default for MapResult<T>
where
    T: Default,
{
    fn default() -> Self {
        Self {
            c: Default::default(),
            warnings: Vec::new(),
        }
    }
}

impl<T> MapResult<T> {
    /// Move `other.c` into `self` and append `other.warnings` to `self.warnings`.
    pub fn merge(&mut self, other: MapResult<T>) {
        self.c = other.c;
        self.warnings.extend(other.warnings);
    }
}

impl<T> MapResult<Vec<T>> {
    /// Append the items of `other` to `self` and merge warnings.
    pub fn extend_vec(&mut self, other: MapResult<Vec<T>>) {
        self.c.extend(other.c);
        self.warnings.extend(other.warnings);
    }
}

impl<T> MapResult<Option<T>> {
    /// If `other.c` is `Some`, replace `self.c` and merge warnings. Otherwise
    /// keep `self` unchanged.
    #[allow(dead_code)]
    pub fn or_some(mut self, other: MapResult<Option<T>>) -> Self {
        if other.c.is_some() {
            self.c = other.c;
        }
        self.warnings.extend(other.warnings);
        self
    }
}

/// Accumulator for the common pattern of building a `Vec<T>` mapper alongside
/// a single continuation token and a warning list.
///
/// Used by endpoint mappers that walk a JSON tree and call out to renderers
/// that produce `MapResult<T>` results.
pub(crate) struct ItemsAccumulator<T> {
    pub items: Vec<T>,
    pub warnings: Vec<String>,
    pub ctoken: Option<String>,
}

impl<T> Default for ItemsAccumulator<T> {
    fn default() -> Self {
        Self {
            items: Vec::new(),
            warnings: Vec::new(),
            ctoken: None,
        }
    }
}

impl<T> ItemsAccumulator<T> {
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a `MapResult<T>` to the accumulator; if `new_ctoken` is `Some` and
    /// the existing ctoken is `None`, store it.
    #[allow(dead_code)]
    pub fn add_mapped(&mut self, mapped: MapResult<T>, new_ctoken: Option<String>) {
        self.items.push(mapped.c);
        self.warnings.extend(mapped.warnings);
        if self.ctoken.is_none() {
            self.ctoken = new_ctoken;
        }
    }

    /// Add a `MapResult<Vec<T>>` to the accumulator; if `new_ctoken` is `Some`
    /// and the existing ctoken is `None`, store it.
    pub fn add_mapped_vec(&mut self, mapped: MapResult<Vec<T>>, new_ctoken: Option<String>) {
        self.items.extend(mapped.c);
        self.warnings.extend(mapped.warnings);
        if self.ctoken.is_none() {
            self.ctoken = new_ctoken;
        }
    }

    /// Push a single warning string.
    #[allow(dead_code)]
    pub fn add_warning(&mut self, w: String) {
        self.warnings.push(w);
    }

    /// Add a warning from a sub-mapper's warnings Vec, leaving the inner data
    /// untouched (used when the inner data is already consumed inline).
    #[allow(dead_code)]
    pub fn add_warnings(&mut self, mut warnings: Vec<String>) {
        self.warnings.append(&mut warnings);
    }

    /// Finalise the accumulator into the canonical
    /// `(MapResult<Vec<T>>, Option<String>)` pair.
    pub fn finish(self) -> (MapResult<Vec<T>>, Option<String>) {
        (
            MapResult {
                c: self.items,
                warnings: self.warnings,
            },
            self.ctoken,
        )
    }
}

#[cfg(test)]
mod tests {
    use serde::Deserialize;
    use serde_with::rust::deserialize_ignore_any;

    use super::*;

    #[derive(Debug, Deserialize, PartialEq)]
    enum E {
        Apple {
            red: bool,
        },
        Banana {
            yellow: bool,
        },
        #[serde(other, deserialize_with = "deserialize_ignore_any")]
        None,
    }

    #[test]
    fn t_ignore_any() {
        assert_eq!(
            flexon::from_str::<E>(r#"{"Apple": {"red": true}}"#).unwrap(),
            E::Apple { red: true }
        );
        assert_eq!(
            flexon::from_str::<E>(r#"{"Lemon": {"yellow": true}}"#).unwrap(),
            E::None
        );
        assert!(flexon::from_str::<E>(r#"{"Apple": {"yellow": true}}"#).is_err());
    }
}
