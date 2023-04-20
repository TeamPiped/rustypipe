use time::{Date, Month, OffsetDateTime};

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

    let month = Month::try_from(month as u8).unwrap();
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
