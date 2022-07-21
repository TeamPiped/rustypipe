/// Returns an unwrapped Option if Some() otherwise returns the passed expression
#[allow(unused_macros)]
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

/// Returns an unwrapped Option if Some() otherwise continues a loop.
#[allow(unused_macros)]
macro_rules! some_or_continue {
    ($opt:expr $(,)?) => {{
        match $opt {
            Some(stuff) => stuff,
            None => {
                continue;
            }
        }
    }};
}

/// Returns an unwrapped Result if Ok() otherwise returns the passed expression
#[allow(unused_macros)]
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

#[allow(unused_macros)]
macro_rules! ok_or_continue {
    ($opt:expr $(,)?) => {{
        match $opt {
            Ok(stuff) => stuff,
            Err(_) => {
                continue;
            }
        }
    }};
}
