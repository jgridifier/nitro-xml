// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/xlsx/numfmt/input-date.ts; changes listed in PROVENANCE.md.

use std::sync::LazyLock;

use regex::Regex;

use crate::numfmt::date::{date_to_serial, UtcDateTime};

/// Parsed date/time typed into a cell.
#[derive(Debug, Clone, PartialEq)]
pub struct DateInput {
    pub value: f64,
    pub num_fmt: String,
}

const MONTH_NAMES: [&str; 12] = [
    "january",
    "february",
    "march",
    "april",
    "may",
    "june",
    "july",
    "august",
    "september",
    "october",
    "november",
    "december",
];

fn month_from_name(name: &str) -> Option<u32> {
    let lower = name.to_ascii_lowercase();
    if lower.len() < 3 {
        return None;
    }
    MONTH_NAMES.iter().enumerate().find_map(|(i, m)| {
        if m.starts_with(&lower) || (lower.len() > 3 && lower.as_str() == *m) {
            Some((i + 1) as u32)
        } else {
            None
        }
    })
}

/// Two-digit years: 00-29 are 20xx, 30-99 are 19xx (Excel's cut-over).
fn full_year(text: &str) -> i32 {
    let y: i32 = text.parse().unwrap_or(0);
    if text.len() > 2 {
        y
    } else if y < 30 {
        2000 + y
    } else {
        1900 + y
    }
}

/// Serial of a calendar date, or `None` when it is invalid or before the epoch.
pub fn serial_of(year: i32, month: u32, day: u32, date1904: bool) -> Option<f64> {
    if !(1..=12).contains(&month) || day < 1 || year > 9999 {
        return None;
    }
    if !date1904 && year == 1900 && month == 2 && day == 29 {
        return Some(60.0);
    }
    // Match JS Date.UTC(2000, month-1, day) then setUTCFullYear(year); reject rollover months.
    if !valid_utc_ymd(2000, month, day) || !valid_utc_ymd(year, month, day) {
        return None;
    }
    let date = UtcDateTime {
        year,
        month,
        day,
        hour: 0,
        minute: 0,
        second: 0,
        millis: 0,
    };
    let serial = date_to_serial(&date, date1904);
    let minimum = if date1904 { 0.0 } else { 1.0 };
    if serial < minimum {
        None
    } else {
        Some(serial)
    }
}

fn valid_utc_ymd(year: i32, month: u32, day: u32) -> bool {
    if !(1..=12).contains(&month) || day < 1 {
        return false;
    }
    day <= days_in_month(year, month)
}

fn days_in_month(year: i32, month: u32) -> u32 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            if is_leap_year(year) {
                29
            } else {
                28
            }
        }
        _ => 0,
    }
}

fn is_leap_year(year: i32) -> bool {
    (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
}

pub(crate) struct TimeInput {
    pub fraction: f64,
    pub format: String,
}

static TIME_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)^(\d{1,5}):(\d{1,2})(?::(\d{1,2}))?(?:\.(\d+))?\s*(am|pm|a|p)?$").unwrap()
});

/// `h:mm`, `h:mm:ss`, `h:mm AM`, `[h]` beyond 24 hours and `mm:ss.0`.
pub(crate) fn parse_time(text: &str) -> Option<TimeInput> {
    let caps = TIME_RE.captures(text)?;
    let a = caps.get(1).map(|m| m.as_str()).unwrap_or("0");
    let b = caps.get(2).map(|m| m.as_str()).unwrap_or("0");
    let c = caps.get(3).map(|m| m.as_str());
    let frac = caps.get(4).map(|m| m.as_str());
    let ampm = caps.get(5).map(|m| m.as_str());

    let mut hours: f64 = a.parse().ok()?;
    let mut minutes: f64 = b.parse().ok()?;
    let mut seconds: f64 = c.map(|s| s.parse().unwrap_or(0.0)).unwrap_or(0.0);
    let format: String;

    if let Some(frac_s) = frac {
        if ampm.is_some() {
            return None;
        }
        let seconds_part: f64 = if let Some(c_s) = c {
            format!("{c_s}.{frac_s}").parse().ok()?
        } else {
            format!("{b}.{frac_s}").parse().ok()?
        };
        // Excel picks `mm:ss.0` for every time with fractional seconds, even `10:30:00.5`.
        format = "mm:ss.0".into();
        if c.is_none() {
            minutes = hours;
            hours = 0.0;
        }
        seconds = seconds_part;
    } else if let Some(ampm) = ampm {
        if hours > 12.0 {
            return None;
        }
        let pm = ampm.to_ascii_lowercase().starts_with('p');
        hours = (hours % 12.0) + if pm { 12.0 } else { 0.0 };
        format = if c.is_none() {
            "h:mm AM/PM".into()
        } else {
            "h:mm:ss AM/PM".into()
        };
    } else if hours >= 24.0 {
        format = "[h]:mm:ss".into();
    } else {
        format = if c.is_none() {
            "h:mm".into()
        } else {
            "h:mm:ss".into()
        };
    }
    if minutes > 59.0 || seconds >= 60.0 {
        return None;
    }
    Some(TimeInput {
        fraction: (hours * 3600.0 + minutes * 60.0 + seconds) / 86_400.0,
        format,
    })
}

struct DayInput {
    year: i32,
    month: u32,
    day: u32,
    format: String,
}

fn current_year() -> i32 {
    #[cfg(all(unix, not(target_arch = "wasm32")))]
    {
        unsafe {
            let t = libc::time(core::ptr::null_mut());
            let mut tm = core::mem::MaybeUninit::<libc::tm>::uninit();
            if libc::localtime_r(&t, tm.as_mut_ptr()).is_null() {
                return utc_year_now();
            }
            1900 + tm.assume_init().tm_year
        }
    }
    #[cfg(not(all(unix, not(target_arch = "wasm32"))))]
    {
        utc_year_now()
    }
}

fn utc_year_now() -> i32 {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let days = secs.div_euclid(86_400);
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    y as i32
}

static RE_YMD: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(\d{4})[-/](\d{1,2})[-/](\d{1,2})$").unwrap());
// Upstream uses a backreference for the separator; the `regex` crate cannot, so match both
// forms and reject mixed separators in `parse_mdy`.
static RE_MDY_SLASH: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(\d{1,2})/(\d{1,2})/(\d{2}|\d{4})$").unwrap());
static RE_MDY_DASH: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(\d{1,2})-(\d{1,2})-(\d{2}|\d{4})$").unwrap());
static RE_MD: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^(\d{1,2})/(\d{1,2})$").unwrap());
static RE_D_MON: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)^(\d{1,2})[- ]([a-z]+)\.?(?:[- ,]+(\d{2}|\d{4}))?$").unwrap()
});
static RE_MON_D_Y: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)^([a-z]+)\.?[- ](\d{1,2}),?[- ](\d{4})$").unwrap());
static RE_MON_N: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)^([a-z]{3,})[- ]?(\d{1,2})$").unwrap());
static RE_MON_Y: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)^([a-z]+)\.?[- ](\d{4})$").unwrap());
static RE_TAIL_TIME: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)(?:\s{1,8}|T)(\d{1,2}:\d{1,2}(?::\d{1,2})?(?:\s{0,8}(?:am|pm|a|p))?)$")
        .unwrap()
});

fn parse_mdy(text: &str) -> Option<(u32, u32, String)> {
    let m = RE_MDY_SLASH
        .captures(text)
        .or_else(|| RE_MDY_DASH.captures(text))?;
    Some((m[1].parse().ok()?, m[2].parse().ok()?, m[3].to_string()))
}

fn parse_day(text: &str) -> Option<DayInput> {
    if let Some(m) = RE_YMD.captures(text) {
        return Some(DayInput {
            year: m[1].parse().ok()?,
            month: m[2].parse().ok()?,
            day: m[3].parse().ok()?,
            format: "yyyy-mm-dd".into(),
        });
    }
    if let Some((month, day, year)) = parse_mdy(text) {
        return Some(DayInput {
            year: full_year(&year),
            month,
            day,
            format: "m/d/yyyy".into(),
        });
    }
    if let Some(m) = RE_MD.captures(text) {
        return Some(DayInput {
            year: current_year(),
            month: m[1].parse().ok()?,
            day: m[2].parse().ok()?,
            format: "d-mmm".into(),
        });
    }
    if let Some(m) = RE_D_MON.captures(text) {
        let month = month_from_name(&m[2])?;
        if m.get(3).is_none() {
            return Some(DayInput {
                year: current_year(),
                month,
                day: m[1].parse().ok()?,
                format: "d-mmm".into(),
            });
        }
        return Some(DayInput {
            year: full_year(&m[3]),
            month,
            day: m[1].parse().ok()?,
            format: "d-mmm-yy".into(),
        });
    }
    if let Some(m) = RE_MON_D_Y.captures(text) {
        let month = month_from_name(&m[1])?;
        return Some(DayInput {
            year: m[3].parse().ok()?,
            month,
            day: m[2].parse().ok()?,
            format: "d-mmm-yy".into(),
        });
    }
    if let Some(m) = RE_MON_N.captures(text) {
        // `Jan 5`, `Jan-24`: a day of the current year when it exists, otherwise a two-digit year
        // (`Jan-32` is January 1932, `Feb-30` February 1930), as Excel reads them.
        let month = month_from_name(&m[1])?;
        let n: u32 = m[2].parse().ok()?;
        let year = current_year();
        if n >= 1 && serial_of(year, month, n, false).is_some() {
            return Some(DayInput {
                year,
                month,
                day: n,
                format: "d-mmm".into(),
            });
        }
        return Some(DayInput {
            year: full_year(&m[2]),
            month,
            day: 1,
            format: "mmm-yy".into(),
        });
    }
    if let Some(m) = RE_MON_Y.captures(text) {
        let month = month_from_name(&m[1])?;
        return Some(DayInput {
            year: m[2].parse().ok()?,
            month,
            day: 1,
            format: "mmm-yy".into(),
        });
    }
    None
}

/// Parses typed dates, times and date-times the way Excel's en-US entry does.
pub fn parse_date_time_input(text: &str, date1904: bool) -> Option<DateInput> {
    if let Some(time) = parse_time(text) {
        return Some(DateInput {
            value: time.fraction,
            num_fmt: time.format,
        });
    }
    let tail = RE_TAIL_TIME.captures(text);
    let split = tail.and_then(|caps| {
        let full = caps.get(0)?;
        if full.start() == 0 {
            return None;
        }
        Some((
            text[..full.start()].to_string(),
            caps.get(1)?.as_str().to_string(),
        ))
    });
    let day_text = split.as_ref().map(|(day, _)| day.as_str()).unwrap_or(text);
    let day = parse_day(day_text)?;
    let serial = serial_of(day.year, day.month, day.day, date1904)?;
    let Some((_, time_text)) = split.as_ref() else {
        return Some(DateInput {
            value: serial,
            num_fmt: day.format,
        });
    };
    let t = parse_time(time_text)?;
    if t.format.starts_with('[') {
        return None;
    }
    let day_format = if day.format == "d-mmm-yy" || day.format == "d-mmm" {
        "m/d/yyyy".to_string()
    } else {
        day.format
    };
    Some(DateInput {
        value: serial + t.fraction,
        num_fmt: format!("{day_format} {}", t.format),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn leap_day_1900_is_serial_60() {
        // Catches: Excel's fictitious 1900-02-29 must parse as serial 60, not be rejected.
        assert_eq!(serial_of(1900, 2, 29, false), Some(60.0));
    }

    #[test]
    fn jan_32_is_month_year_not_day() {
        // Catches: `Jan-32` is January 1932 (mmm-yy), not day 32 of the current year.
        let parsed = parse_date_time_input("Jan-32", false).expect("parse");
        assert_eq!(parsed.num_fmt, "mmm-yy");
        assert!((parsed.value - 11689.0).abs() < 1e-9);
    }
}
