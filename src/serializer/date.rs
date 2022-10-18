use serde::{
    de::{self, Visitor},
    ser, Serialize,
};
use serde_with::{DeserializeAs, SerializeAs};
use time::{macros::format_description, Date};

const YMD_FORMAT: &[time::format_description::FormatItem] =
    format_description!("[year]-[month]-[day]");

pub struct DateYmd;

impl SerializeAs<Date> for DateYmd {
    fn serialize_as<S>(date: &Date, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        date.format(YMD_FORMAT)
            .map_err(ser::Error::custom)?
            .serialize(serializer)
    }
}

impl<'de> DeserializeAs<'de, Date> for DateYmd {
    fn deserialize_as<D>(deserializer: D) -> Result<Date, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct DateYmdVisitor;

        impl<'de> Visitor<'de> for DateYmdVisitor {
            type Value = Date;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("a YYYY-MM-DD formatted date")
            }

            fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                Date::parse(v, YMD_FORMAT).map_err(|_| {
                    de::Error::invalid_value(de::Unexpected::Str(v), &"a YYYY-MM-DD formatted date")
                })
            }
        }

        deserializer.deserialize_str(DateYmdVisitor)
    }
}
