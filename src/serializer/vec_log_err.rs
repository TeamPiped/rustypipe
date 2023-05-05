use std::{fmt, marker::PhantomData};

use serde::{
    de::{IgnoredAny, SeqAccess, Visitor},
    Deserialize,
};
use serde_with::{de::DeserializeAsWrap, DeserializeAs};

use super::MapResult;

/// Deserializes a list of arbitrary items into a `MapResult`,
/// creating warnings for items that could not be deserialized.
///
/// This is similar to `VecSkipError`, but it does not silently ignore
/// faulty items.
pub struct VecLogError<T>(PhantomData<T>);

impl<'de, T, U> DeserializeAs<'de, MapResult<Vec<T>>> for VecLogError<U>
where
    U: DeserializeAs<'de, T>,
{
    fn deserialize_as<D>(deserializer: D) -> Result<MapResult<Vec<T>>, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(serde::Deserialize)]
        #[serde(
            untagged,
            bound(deserialize = "DeserializeAsWrap<T, TAs>: Deserialize<'de>")
        )]
        enum GoodOrError<'a, T, TAs>
        where
            TAs: DeserializeAs<'a, T>,
        {
            Good(DeserializeAsWrap<T, TAs>),
            Error(serde_json::value::Value),
            #[serde(skip)]
            _JustAMarkerForTheLifetime(PhantomData<&'a u32>),
        }

        struct SeqVisitor<T, U> {
            marker: PhantomData<T>,
            marker2: PhantomData<U>,
        }

        impl<'de, T, U> Visitor<'de> for SeqVisitor<T, U>
        where
            U: DeserializeAs<'de, T>,
        {
            type Value = MapResult<Vec<T>>;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("a sequence")
            }

            fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
            where
                A: SeqAccess<'de>,
            {
                let mut values = Vec::with_capacity(seq.size_hint().unwrap_or_default());
                let mut warnings = Vec::new();

                while let Some(value) = seq.next_element()? {
                    match value {
                        GoodOrError::<T, U>::Good(value) => {
                            values.push(value.into_inner());
                        }
                        GoodOrError::<T, U>::Error(value) => {
                            warnings.push(format!(
                                "error deserializing item: {}",
                                serde_json::to_string(&value).unwrap_or_default()
                            ));
                        }
                        _ => {}
                    }
                }
                Ok(MapResult {
                    c: values,
                    warnings,
                })
            }
        }

        let visitor = SeqVisitor::<T, U> {
            marker: PhantomData,
            marker2: PhantomData,
        };
        deserializer.deserialize_seq(visitor)
    }
}

/// Reimplementation of VecSkipError using a type wrapper
/// to allow use with generics
pub struct VecSkipErrorWrap<T>(pub Vec<T>);

impl<'de, T> Deserialize<'de> for VecSkipErrorWrap<T>
where
    T: Deserialize<'de>,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(serde::Deserialize)]
        #[serde(untagged)]
        enum GoodOrError<T> {
            Good(T),
            Error(IgnoredAny),
        }

        struct SeqVisitor<T>(PhantomData<T>);

        impl<'de, T> Visitor<'de> for SeqVisitor<T>
        where
            T: Deserialize<'de>,
        {
            type Value = VecSkipErrorWrap<T>;

            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("a sequence")
            }

            fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
            where
                A: SeqAccess<'de>,
            {
                let mut values = Vec::with_capacity(seq.size_hint().unwrap_or_default());

                while let Some(value) = seq.next_element()? {
                    match value {
                        GoodOrError::<T>::Good(value) => {
                            values.push(value);
                        }
                        GoodOrError::<T>::Error(_) => {}
                    }
                }
                Ok(VecSkipErrorWrap(values))
            }
        }

        deserializer.deserialize_seq(SeqVisitor(PhantomData::<T>))
    }
}

#[cfg(test)]
mod tests {
    use serde::Deserialize;
    use serde_with::serde_as;

    use crate::serializer::MapResult;

    #[serde_as]
    #[derive(Debug, Deserialize)]
    #[allow(dead_code)]
    struct S {
        #[serde_as(as = "crate::serializer::VecLogError<_>")]
        items: MapResult<Vec<Item>>,
    }

    #[derive(Debug, Deserialize)]
    #[allow(dead_code)]
    struct Item {
        name: String,
    }

    #[test]
    fn test() {
        let json = r#"{"items": [{"name": "i1"}, {"xyz": "i2"}, {"name": "i3"}, {"namra": "i4"}]}"#;

        let res = serde_json::from_str::<S>(json).unwrap();

        insta::assert_debug_snapshot!(res, @r###"
        S {
            items: [
                Item {
                    name: "i1",
                },
                Item {
                    name: "i3",
                },
            ],
        }
        "###);

        insta::assert_debug_snapshot!(res.items.warnings, @r###"
        [
            "error deserializing item: {\"xyz\":\"i2\"}",
            "error deserializing item: {\"namra\":\"i4\"}",
        ]
        "###);
    }
}
