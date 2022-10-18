use time::{Date, Month, OffsetDateTime};

pub const fn month_from_n(n: u8) -> Option<Month> {
    match n {
        1 => Some(Month::January),
        2 => Some(Month::February),
        3 => Some(Month::March),
        4 => Some(Month::April),
        5 => Some(Month::May),
        6 => Some(Month::June),
        7 => Some(Month::July),
        8 => Some(Month::August),
        9 => Some(Month::September),
        10 => Some(Month::October),
        11 => Some(Month::November),
        12 => Some(Month::December),
        _ => None,
    }
}

/// Shift a date by the given number of months.
/// Ambiguous month-ends are shifted backwards as necessary.
pub fn shift_months(date: Date, months: i32) -> Date {
    let mut year = date.year() + (date.month() as i32 + months) / 12;
    let mut month = (date.month() as i32 + months) % 12;
    let mut day = date.day();

    if month < 1 {
        year -= 1;
        month += 12;
    }

    let month = month_from_n(month as u8).unwrap();
    let month_days = time::util::days_in_year_month(year, month);

    day = day.min(month_days);
    Date::from_calendar_date(year, month, day).unwrap()
}

/// Shift a date by the given number of years.
/// Ambiguous month-ends are shifted backwards as necessary.
pub fn shift_years(date: Date, years: i32) -> Date {
    shift_months(date, years * 12)
}

/// Get the current datetime without milli/micro/nanoseconds
pub fn now_sec() -> OffsetDateTime {
    OffsetDateTime::now_utc()
        .replace_millisecond(0)
        .unwrap()
        .replace_microsecond(0)
        .unwrap()
        .replace_nanosecond(0)
        .unwrap()
}
