use once_cell::sync::Lazy;

use fancy_regex::Regex;
use serde::de::{Deserialize, Deserializer, Error, Unexpected};
use serde::ser::{Serialize, Serializer};

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct MimeType {
    pub mime: String,
    pub codecs: Vec<String>,
}

pub fn deserialize<'de, D>(deserializer: D) -> Result<MimeType, D::Error>
where
    D: Deserializer<'de>,
{
    static PATTERN: Lazy<Regex> =
        Lazy::new(|| Regex::new(r#"(\w+/\w+);\scodecs="([a-zA-Z-0-9.,\s]*)""#).unwrap());

    // deserializing into a &str gives back an error
    let s = String::deserialize(deserializer)?;

    let captures = PATTERN.captures(&s).ok().flatten().ok_or_else(|| {
        D::Error::invalid_value(
            Unexpected::Str(&s),
            &"a valid mime type with the format <TYPE>/<SUBTYPE>",
        )
    })?;
    let mime = captures.get(1).unwrap().as_str().to_owned();
    let codecs = captures
        .get(2)
        .unwrap()
        .as_str()
        .split(", ")
        .map(str::to_owned)
        .collect::<Vec<String>>();

    Ok(MimeType {
        mime,
        codecs,
    })
}

pub fn serialize<S>(mime_type: &MimeType, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    let mut s = format!(r#"{}; codecs=""#, mime_type.mime,);

    for codec in mime_type.codecs.iter() {
        s.push_str(codec);
        s.push(',');
        s.push(' ');
    }

    s.pop();
    s.pop();
    s.push('"');
    s.serialize(serializer)
}

#[cfg(test)]
mod tests {
    use rstest::rstest;
    use serde::{Serialize, Deserialize};
    use super::*;

    #[derive(Serialize, Deserialize)]
    struct S {
        #[serde(with = "crate::serializer::mime_type")]
        mime: MimeType,
    }

    #[rstest]
    #[case(
        r#"{"mime": "video/mp4; codecs=\"avc1.42001E, mp4a.40.2\""}"#,
        MimeType {
            mime: "video/mp4".to_owned(),
            codecs: vec!["avc1.42001E".to_owned(), "mp4a.40.2".to_owned()],
        }
    )]
    #[case(
        r#"{"mime": "video/webm; codecs=\"vp9\""}"#,
        MimeType {
            mime: "video/webm".to_owned(),
            codecs: vec!["vp9".to_owned()],
        }
    )]
    #[case(
        r#"{"mime": "audio/webm; codecs=\"opus\""}"#,
        MimeType {
            mime: "audio/webm".to_owned(),
            codecs: vec!["opus".to_owned()],
        }
    )]
    fn t_deserialize(#[case] test_json: &str, #[case] exp: MimeType) {
        let res = serde_json::from_str::<S>(&test_json).unwrap();
        assert_eq!(res.mime, exp)
    }

    #[rstest]
    fn t_serialize() {
        let s = S {
            mime: MimeType {
                mime: "video/webm".to_owned(),
                codecs: vec!["av01.0.08M.08".to_owned()],
            },
        };

        let json = serde_json::to_string(&s).unwrap();
        assert_eq!(json, r#"{"mime":"video/webm; codecs=\"av01.0.08M.08\""}"#)
    }
}
