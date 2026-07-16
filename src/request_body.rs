use serde::{Serialize, Serializer};

use crate::json::{json_from_str, json_to_string, JsonValue};

#[derive(Clone, Debug)]
pub(crate) struct RequestBody(String);

impl RequestBody {
    pub(crate) fn into_string(self) -> String {
        self.0
    }
}

impl Serialize for RequestBody {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let value: JsonValue = json_from_str(&self.0).map_err(serde::ser::Error::custom)?;
        value.serialize(serializer)
    }
}

pub(crate) fn to_raw<T>(value: T) -> String
where
    T: Serialize,
{
    json_to_string(&value).expect("request body value should serialize")
}

pub(crate) fn from_items(items: Vec<(String, String)>) -> RequestBody {
    let values = items.into_iter().map(|(key, raw)| {
        (
            key,
            json_from_str::<JsonValue>(&raw).expect("request body value should parse"),
        )
    });
    RequestBody(
        json_to_string(&crate::json::object_value(values)).expect("request body should serialize"),
    )
}

pub(crate) fn insert_value<T>(items: &mut Vec<(String, String)>, key: &str, value: T)
where
    T: Serialize,
{
    items.push((key.to_owned(), to_raw(value)));
}

pub(crate) fn insert_optional_value<T>(
    items: &mut Vec<(String, String)>,
    key: &str,
    value: Option<T>,
) where
    T: Serialize,
{
    if let Some(value) = value {
        insert_value(items, key, value);
    }
}

pub(crate) fn extend_object<T>(items: &mut Vec<(String, String)>, value: T)
where
    T: Serialize,
{
    let JsonValue::Object(other) =
        json_from_str::<JsonValue>(&to_raw(value)).expect("request body merge value should parse")
    else {
        panic!("request body merge value must serialize to an object");
    };
    items.extend(
        other
            .as_slice()
            .iter()
            .map(|(key, value)| (key.as_str().to_owned(), json_to_string(value).unwrap())),
    );
}

macro_rules! __ytbody_entries {
    ($map:ident;) => {};
    ($map:ident; .. $value:expr $(, $($rest:tt)*)?) => {{
        $crate::request_body::extend_object(&mut $map, $value);
        $crate::request_body::__ytbody_entries!($map; $($($rest)*)?);
    }};
    ($map:ident; ? $key:ident : $value:expr $(, $($rest:tt)*)?) => {{
        $crate::request_body::insert_optional_value(&mut $map, stringify!($key), $value);
        $crate::request_body::__ytbody_entries!($map; $($($rest)*)?);
    }};
    ($map:ident; ? $key:literal : $value:expr $(, $($rest:tt)*)?) => {{
        $crate::request_body::insert_optional_value(&mut $map, $key, $value);
        $crate::request_body::__ytbody_entries!($map; $($($rest)*)?);
    }};
    ($map:ident; $key:ident : $value:expr $(, $($rest:tt)*)?) => {{
        $crate::request_body::insert_value(&mut $map, stringify!($key), $value);
        $crate::request_body::__ytbody_entries!($map; $($($rest)*)?);
    }};
    ($map:ident; $key:literal : $value:expr $(, $($rest:tt)*)?) => {{
        $crate::request_body::insert_value(&mut $map, $key, $value);
        $crate::request_body::__ytbody_entries!($map; $($($rest)*)?);
    }};
}

macro_rules! ytbody {
    ({ $($entries:tt)* }) => {{
        let mut map = Vec::new();
        $crate::request_body::__ytbody_entries!(map; $($entries)*);
        $crate::request_body::from_items(map)
    }};
}

pub(crate) use __ytbody_entries;
pub(crate) use ytbody;
