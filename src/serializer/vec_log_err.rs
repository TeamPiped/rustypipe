use std::{fmt, marker::PhantomData};

use crate::json::JsonValue;

use serde::{
    de::{SeqAccess, Visitor},
    Deserialize,
};

use super::MapResult;

/// Deserializes a list of arbitrary items into a `MapResult`,
/// creating warnings for items that could not be deserialized.
///
/// This is similar to `VecSkipError`, but it does not silently ignore
/// faulty items.
impl<'de, T> Deserialize<'de> for MapResult<Vec<T>>
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
            Error(JsonValue),
        }

        struct SeqVisitor<T>(PhantomData<T>);

        impl<'de, T> Visitor<'de> for SeqVisitor<T>
        where
            T: Deserialize<'de>,
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
                        GoodOrError::<T>::Good(value) => {
                            values.push(value);
                        }
                        GoodOrError::<T>::Error(value) => {
                            warnings.push(format!(
                                "error deserializing item: {}",
                                flexon::to_string(&value).unwrap_or_default()
                            ));
                        }
                    }
                }
                Ok(MapResult {
                    c: values,
                    warnings,
                })
            }
        }

        deserializer.deserialize_seq(SeqVisitor(PhantomData::<T>))
    }
}

#[cfg(test)]
mod tests {
    use serde::Deserialize;

    use crate::serializer::MapResult;

    #[derive(Debug, Deserialize)]
    #[allow(dead_code)]
    struct SLog {
        items: MapResult<Vec<Item>>,
    }

    #[derive(Debug, Deserialize)]
    #[allow(dead_code)]
    struct Item {
        name: String,
    }

    const JSON: &str =
        r#"{"items": [{"name": "i1"}, {"xyz": "i2"}, {"name": "i3"}, {"namra": "i4"}]}"#;

    #[test]
    fn log_error() {
        let res = flexon::from_str::<SLog>(JSON).unwrap();

        insta::assert_debug_snapshot!(res, @r###"
        SLog {
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
