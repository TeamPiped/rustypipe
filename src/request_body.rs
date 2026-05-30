use serde::Serialize;
use serde_json::{Map, Value};

pub(crate) fn to_value<T>(value: T) -> Value
where
    T: Serialize,
{
    serde_json::to_value(value).expect("request body value should serialize")
}

pub(crate) fn insert_value<T>(map: &mut Map<String, Value>, key: &str, value: T)
where
    T: Serialize,
{
    map.insert(key.to_owned(), to_value(value));
}

pub(crate) fn insert_optional_value<T>(map: &mut Map<String, Value>, key: &str, value: Option<T>)
where
    T: Serialize,
{
    if let Some(value) = value {
        insert_value(map, key, value);
    }
}

pub(crate) fn extend_object<T>(map: &mut Map<String, Value>, value: T)
where
    T: Serialize,
{
    let Value::Object(other) = to_value(value) else {
        panic!("request body merge value must serialize to an object");
    };
    map.extend(other);
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
        let mut map = ::serde_json::Map::new();
        $crate::request_body::__ytbody_entries!(map; $($entries)*);
        ::serde_json::Value::Object(map)
    }};
}

pub(crate) use __ytbody_entries;
pub(crate) use ytbody;
