/// Returns an unwrapped Option if Some() otherwise returns the passed expression
macro_rules! some_or_bail {
    ($opt:expr, $ret:expr $(,)?) => {{
        match $opt {
            Some(stuff) => stuff,
            None => {
                return $ret;
            }
        }
    }};
}

/// Returns an unwrapped Result if Ok() otherwise returns the passed expression
macro_rules! ok_or_bail {
    ($result:expr, $ret:expr $(,)?) => {{
        match $result {
            Ok(stuff) => stuff,
            Err(_) => {
                return $ret;
            }
        }
    }};
}
