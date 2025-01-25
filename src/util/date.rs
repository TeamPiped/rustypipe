use time::{Date, Duration, Month, OffsetDateTime};

use crate::error::Error;

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

/// Gets the current timezone from the system.
///
/// Currently only supported for Windows, Unix, and WASM targets.
///
/// # Errors
/// Returns an [Error](enum@Error) if the timezone cannot be determined.
pub fn local_timezone_name() -> Result<String, Error> {
    #[cfg(unix)]
    {
        use std::path::Path;
        let path = Path::new("/etc/localtime");
        let realpath = std::fs::read_link(path)
            .map_err(|_| Error::Other("could not read localtime".into()))?;
        // The part of the path we're interested in cannot contain non unicode characters.
        return realpath
            .to_str()
            .and_then(|s| s.split("/zoneinfo/").last())
            .map(str::to_owned)
            .ok_or_else(|| {
                Error::Other(format!("could not parse zoneinfo path: {realpath:?}").into())
            });
    }

    #[cfg(windows)]
    {
        unsafe {
            use windows_sys::Win32::System::Time::GetDynamicTimeZoneInformation;
            use windows_sys::Win32::System::Time::DYNAMIC_TIME_ZONE_INFORMATION;
            let mut data: DYNAMIC_TIME_ZONE_INFORMATION = std::mem::zeroed();
            let res = GetDynamicTimeZoneInformation(&mut data as _);
            if res > 2 {
                return Err(Error::Other("local timezone could not be read".into()));
            } else {
                let win_name_utf16 = &data.TimeZoneKeyName;
                let mut len: usize = 0;
                while win_name_utf16[len] != 0x0 {
                    len += 1;
                }
                if len == 0 {
                    return Err(Error::Other("local timezone could not be read".into()));
                }
                return String::from_utf16(&win_name_utf16[..len])
                    .map_err(|_| Error::Other("local timezone is invalid UTF16".into()));
            }
        }
    }

    #[allow(unreachable_code)]
    Err(Error::Other("local timezone unsupported".into()))
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

    #[test]
    fn local_timezone_name() {
        super::local_timezone_name().unwrap();
    }
}
