pub mod text;

mod range;
mod vec_log_err;

pub use range::Range;
pub use vec_log_err::VecLogError;

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
