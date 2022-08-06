use std::marker::PhantomData;

use serde::{de::Visitor, Deserialize, Deserializer};
use serde_with::{serde_as, DeserializeAs, rust::maps_duplicate_key_is_error::deserialize};

/// ```json
/// {
///   itemSectionRenderer": {
///     "contents": [
///       {
///         "playlistVideoListRenderer": {
///           "contents": [
///             {
///               "playlistVideoRenderer": { ... }
///             },
///             {
///               "playlistVideoRenderer": { ... }
///             },
///           }
///         }
///       }
///     ]
///   }
/// }
/// ```
///
/// Renderer names:
///
/// 1 content element:
/// - tabRenderer > content
///
/// 1 content element (array):
/// - twoColumnBrowseResultsRenderer > tabs
/// - sectionListRenderer > contents
/// - itemSectionRenderer > contents
///
/// n content elements:
/// - playlistVideoListRenderer > contents

#[serde_as]
#[derive(Deserialize)]
#[serde(untagged, bound = "for<'de2> T: Deserialize<'de2>")]
pub enum Renderer<T> where for<'de2> T: Deserialize<'de2> {
    Single {
        #[serde_as(as = "crate::serializer::renderer::Renderer<T>")]
        content: T,
    },
    Multiple {
        #[serde(alias = "tabs")]
        #[serde_as(as = "crate::serializer::renderer::Renderer<T>")]
        contents: Vec<T>,
    },
    Content {
        #[serde(flatten)]
        inner: T,
    },
}

// pub struct Renderer<T>(PhantomData<T>);

impl<'de, T> DeserializeAs<'de, T> for Renderer<T> {
    fn deserialize_as<D>(deserializer: D) -> Result<T, D::Error>
    where
        D: Deserializer<'de>,
    {
        todo!()
    }
}

impl<'de, T> DeserializeAs<'de, Vec<T>> for Renderer<T> {
    fn deserialize_as<D>(deserializer: D) -> Result<Vec<T>, D::Error>
    where
        D: Deserializer<'de>,
    {
        todo!()
    }
}

/*
struct RendererVisitor<T, U>(PhantomData<T>, PhantomData<U>);

impl<'de, T, U> Visitor<'de> for RendererVisitor<T, U>
where
    U: DeserializeAs<'de, T>,
{
    type Value = T;

    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        formatter.write_str("a yt renderer")
    }

    fn visit_newtype_struct<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
        where
            D: Deserializer<'de>, {

    }
}
*/
