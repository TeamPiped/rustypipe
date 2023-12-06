pub mod text;

mod range;
mod vec_log_err;

pub use range::Range;
pub use vec_log_err::VecSkipErrorWrap;

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
            serde_json::from_str::<E>(r#"{"Apple": {"red": true}}"#).unwrap(),
            E::Apple { red: true }
        );
        assert_eq!(
            serde_json::from_str::<E>(r#"{"Lemon": {"yellow": true}}"#).unwrap(),
            E::None
        );
        assert!(serde_json::from_str::<E>(r#"{"Apple": {"yellow": true}}"#).is_err());
    }
}
