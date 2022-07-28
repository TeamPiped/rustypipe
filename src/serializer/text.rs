use serde::{Deserialize, Deserializer};
use serde_with::{serde_as, DeserializeAs};

/// The YouTube API has multiple ways of outputting text. This deserializer
/// is an attempt to unify them.
///
/// ```json
/// {
///   "text": "Hello World"
/// }
/// ```
///
/// ```json
/// {
///   "simpleText": "Hello World"
/// }
/// ```
///
/// Multiple "runs" of text should be joined with spaces
/// ```json
/// {
///   "runs": [
///     {"text": "Hello"},
///     {"text": "World"},
///   ]
/// }
/// ```
///

#[serde_as]
#[derive(Deserialize)]
#[serde(untagged)]
pub enum Text {
    Simple {
        #[serde(alias = "simpleText")]
        text: String,
    },
    Multiple {
        #[serde_as(as = "Vec<crate::serializer::text::Text>")]
        runs: Vec<String>,
    },
}

impl<'de> DeserializeAs<'de, String> for Text {
    fn deserialize_as<D>(deserializer: D) -> Result<String, D::Error>
    where
        D: Deserializer<'de>,
    {
        let text = Text::deserialize(deserializer)?;
        match text {
            Text::Simple { text } => Ok(text),
            Text::Multiple { runs } => Ok(runs.join("")),
        }
    }
}

#[cfg(test)]
mod tests {
    use rstest::rstest;
    use serde::Deserialize;
    use serde_with::serde_as;

    #[rstest]
    #[case(
        r#"{
            "txt": {
                "text": "Hello World"
            }
        }"#,
        "Hello World"
    )]
    #[case(
        r#"{
            "txt": {
                "simpleText": "Hello World"
            }
        }"#,
        "Hello World"
    )]
    #[case(
        r#"{
            "txt": {
                "runs": [
                    {
                        "text": "Abo für "
                    },
                    {
                     "text": "MBCkpop"
                    },
                    {
                        "text": " beenden?"
                    }
                ]
            }
        }"#,
        "Abo für MBCkpop beenden?"
    )]
    fn t_deserialize(#[case] test_json: &str, #[case] exp: &str) {
        #[serde_as]
        #[derive(Deserialize)]
        struct S {
            #[serde_as(as = "crate::serializer::text::Text")]
            txt: String,
        }

        let res = serde_json::from_str::<S>(&test_json).unwrap();
        assert_eq!(res.txt, exp)
    }
}
