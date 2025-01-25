use time::{Date, Duration, Month, OffsetDateTime};

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
    let month_days = month.length(year);

    day = day.min(month_days);
    Date::from_calendar_date(year, month, day).unwrap()
}

/// Shift a date by the given number of years.
/// Ambiguous month-ends are shifted backwards as necessary.
pub fn shift_years(date: Date, years: i32) -> Date {
    shift_months(date, years * 12)
}

/// Shift a date to the monday of its week, plus/minus the given amount of weeks
pub fn shift_weeks_monday(date: Date, weeks: i32) -> Date {
    let d = date + Duration::weeks(weeks.into());
    Date::from_iso_week_date(d.year(), d.iso_week(), time::Weekday::Monday).unwrap()
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

#[cfg(test)]
mod tests {
    use rstest::rstest;
    use time::{macros::date, Date};

    #[rstest]
    #[case::this_week(date!(2025-01-17), 0, date!(2025-01-13))]
    #[case::last_week(date!(2025-01-17), -1, date!(2025-01-06))]
    #[case::last_month(date!(2025-01-17), -4, date!(2024-12-16))]
    fn shift_weeks_monday(#[case] date: Date, #[case] weeks: i32, #[case] expect: Date) {
        let res = super::shift_weeks_monday(date, weeks);
        assert_eq!(res, expect);
    }
}
