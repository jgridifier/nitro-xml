// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/xlsx/numfmt/date.ts; changes listed in PROVENANCE.md.

use crate::numfmt::decimal::{js_int_string, js_round, pad_end, pad_start};
use crate::numfmt::types::{AmpmKind, DatePart, Token};

const MS_PER_DAY: i64 = 86_400_000;
/// Days from 1970-01-01 back to 1899-12-30 (Excel's 1900 epoch).
const EPOCH_1900_DAYS: i64 = -25_569;
/// Days from 1970-01-01 back to 1904-01-01.
const EPOCH_1904_DAYS: i64 = -24_107;

/// Largest serial Excel displays as a date (9999-12-31).
pub const MAX_DATE_SERIAL: f64 = 2_958_465.0;

/// Wall-clock time stored in UTC fields, matching JavaScript `Date` UTC accessors.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UtcDateTime {
    pub year: i32,
    pub month: u32,
    pub day: u32,
    pub hour: u32,
    pub minute: u32,
    pub second: u32,
    pub millis: u32,
}

/// The date a serial number stands for.
///
/// In the 1900 system serial 60 is Excel's fictitious 1900-02-29, which this helper cannot
/// represent: it maps to 1900-03-01 (as does serial 61). Serials 1-59 follow the real calendar
/// shifted by one day from 1899-12-31.
pub fn serial_to_date(serial: f64, date1904: bool) -> UtcDateTime {
    utc_from_unix_ms(serial_to_unix_ms(serial, date1904))
}

/// The serial number of a date (its fields are read as the wall-clock time).
pub fn date_to_serial(date: &UtcDateTime, date1904: bool) -> f64 {
    let ms = ms_from_utc(date) as f64;
    let day = MS_PER_DAY as f64;
    if date1904 {
        (ms - EPOCH_1904_DAYS as f64 * day) / day
    } else {
        let serial = (ms - EPOCH_1900_DAYS as f64 * day) / day;
        if serial < 61.0 {
            serial - 1.0
        } else {
            serial
        }
    }
}

fn serial_to_unix_ms(serial: f64, date1904: bool) -> f64 {
    let day = MS_PER_DAY as f64;
    if date1904 {
        EPOCH_1904_DAYS as f64 * day + js_round(serial * day)
    } else {
        let adjusted = if serial < 61.0 { serial + 1.0 } else { serial };
        EPOCH_1900_DAYS as f64 * day + js_round(adjusted * day)
    }
}

fn utc_from_unix_ms(ms: f64) -> UtcDateTime {
    let day = MS_PER_DAY as f64;
    let days_f = (ms / day).floor();
    let tod = ms - days_f * day;
    let days = days_f as i64;
    let (year, month, day_n) = civil_from_days(days);
    let tod_i = if tod.is_finite() {
        tod.round() as i64
    } else {
        0
    };
    let tod_i = tod_i.clamp(0, MS_PER_DAY - 1);
    UtcDateTime {
        year,
        month,
        day: day_n,
        hour: (tod_i / 3_600_000) as u32,
        minute: ((tod_i / 60_000) % 60) as u32,
        second: ((tod_i / 1_000) % 60) as u32,
        millis: (tod_i % 1_000) as u32,
    }
}

fn ms_from_utc(date: &UtcDateTime) -> i64 {
    days_from_civil(date.year, date.month, date.day) * MS_PER_DAY
        + date.hour as i64 * 3_600_000
        + date.minute as i64 * 60_000
        + date.second as i64 * 1_000
        + date.millis as i64
}

/// Howard Hinnant's civil-from-days, days since 1970-01-01.
fn civil_from_days(mut z: i64) -> (i32, u32, u32) {
    z += 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y as i32, m as u32, d as u32)
}

fn days_from_civil(y: i32, m: u32, d: u32) -> i64 {
    let mut y = i64::from(y);
    let m = i64::from(m);
    let d = i64::from(d);
    if m <= 2 {
        y -= 1;
    }
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = (y - era * 400) as u64;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy as u64;
    era * 146_097 + doe as i64 - 719_468
}

struct DateParts {
    year: i32,
    month: u32,
    day: u32,
    weekday: u32,
    hours: u32,
    minutes: u32,
    seconds: u32,
    subsec: f64,
    total_seconds: f64,
}

fn calendar(days: i64, date1904: bool) -> (i32, u32, u32) {
    if !date1904 {
        if days == 0 {
            return (1900, 1, 0);
        }
        if days == 60 {
            return (1900, 2, 29);
        }
    }
    let offset = if date1904 || days > 60 {
        days
    } else {
        days + 1
    };
    let epoch = if date1904 {
        EPOCH_1904_DAYS
    } else {
        EPOCH_1900_DAYS
    };
    civil_from_days(epoch + offset)
}

fn date_parts(serial: f64, date1904: bool, precision: usize) -> DateParts {
    let units_per_second = 10f64.powi(precision as i32);
    let units_per_day = 86_400.0 * units_per_second;
    let total = js_round(serial * units_per_day);
    let days_f = (total / units_per_day).floor();
    let mut rest = total - days_f * units_per_day;
    let subsec = rest % units_per_second;
    rest = (rest - subsec) / units_per_second;
    let days = days_f as i64;
    let (year, month, day) = calendar(days, date1904);
    let shift = if date1904 { 5 } else { 6 };
    DateParts {
        year,
        month,
        day,
        weekday: (days + shift).rem_euclid(7) as u32,
        hours: (rest / 3600.0).floor() as u32,
        minutes: ((rest / 60.0).floor() as u32) % 60,
        seconds: (rest % 60.0) as u32,
        subsec,
        total_seconds: (total / units_per_second).floor(),
    }
}

const MONTHS: [&str; 12] = [
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
];
const DAYS: [&str; 7] = [
    "Sunday",
    "Monday",
    "Tuesday",
    "Wednesday",
    "Thursday",
    "Friday",
    "Saturday",
];

fn two(n: u32) -> String {
    format!("{n:02}")
}

pub(crate) fn date_precision(tokens: &[Token]) -> usize {
    let mut p = 0;
    for t in tokens {
        if let Token::Subsec { digits } = t {
            p = p.max((*digits).min(3));
        }
    }
    p
}

pub(crate) fn is_elapsed_only(tokens: &[Token]) -> bool {
    let mut elapsed = 0;
    for t in tokens {
        match t {
            Token::Elapsed { .. } => elapsed += 1,
            Token::Date(_) | Token::Ampm(_) | Token::Subsec { .. } => return false,
            _ => {}
        }
    }
    elapsed == 1
}

pub(crate) fn render_elapsed_only(serial: f64, tokens: &[Token]) -> String {
    let seconds = js_round(serial.abs() * 86_400.0);
    let mut text = String::new();
    for tok in tokens {
        match tok {
            Token::Lit(v) => text.push_str(v),
            Token::Elapsed { unit, width } => {
                let amount = match unit {
                    'h' => (seconds / 3600.0).floor(),
                    'm' => (seconds / 60.0).floor(),
                    _ => seconds,
                };
                text.push_str(&pad_start(&js_int_string(amount), *width, '0'));
            }
            _ => {}
        }
    }
    text
}

/// Formats a non-negative serial with a date/time section; `None` when out of range.
pub(crate) fn render_date(serial: f64, tokens: &[Token], date1904: bool) -> Option<String> {
    // `!(serial >= 0)` is true for NaN as well as negatives; a plain `<` would let NaN through.
    if serial.partial_cmp(&0.0).is_none_or(|ord| ord.is_lt()) || serial >= MAX_DATE_SERIAL + 1.0 {
        return None;
    }
    let precision = date_precision(tokens);
    let p = date_parts(serial, date1904, precision);
    if p.year > 9999 {
        return None;
    }
    let twelve = tokens.iter().any(|t| matches!(t, Token::Ampm(_)));
    let hour12 = if p.hours.is_multiple_of(12) {
        12
    } else {
        p.hours % 12
    };
    let hour = if twelve { hour12 } else { p.hours };
    let mut text = String::new();
    for tok in tokens {
        match tok {
            Token::Lit(v) => text.push_str(v),
            Token::Ampm(kind) => {
                let pm = p.hours >= 12;
                text.push_str(match (kind, pm) {
                    (AmpmKind::AmPm, true) => "PM",
                    (AmpmKind::AmPm, false) => "AM",
                    (AmpmKind::Upper, true) => "P",
                    (AmpmKind::Upper, false) => "A",
                    (AmpmKind::Lower, true) => "p",
                    (AmpmKind::Lower, false) => "a",
                });
            }
            Token::Subsec { digits } => {
                let body = pad_start(&js_int_string(p.subsec), precision, '0');
                let sliced: String = body.chars().take(*digits).collect();
                text.push('.');
                text.push_str(&pad_end(&sliced, *digits, '0'));
            }
            Token::Elapsed { unit, width } => {
                let amount = match unit {
                    'h' => (p.total_seconds / 3600.0).floor(),
                    'm' => (p.total_seconds / 60.0).floor(),
                    _ => p.total_seconds,
                };
                text.push_str(&pad_start(&js_int_string(amount), *width, '0'));
            }
            Token::Date(part) => text.push_str(&date_part(*part, &p, hour)),
            _ => {}
        }
    }
    Some(text)
}

fn date_part(part: DatePart, p: &DateParts, hour: u32) -> String {
    match part {
        DatePart::Y => two((p.year % 100) as u32),
        DatePart::Yyyy | DatePart::E => p.year.to_string(),
        DatePart::G => String::new(),
        DatePart::B => two(((p.year + 543) % 100) as u32),
        DatePart::Bbbb => (p.year + 543).to_string(),
        DatePart::M => p.month.to_string(),
        DatePart::Mm => two(p.month),
        DatePart::Mmm => month_name(p.month).chars().take(3).collect(),
        DatePart::Mmmm => month_name(p.month).to_string(),
        DatePart::Mmmmm => month_name(p.month)
            .chars()
            .next()
            .map(|c| c.to_string())
            .unwrap_or_default(),
        DatePart::D => p.day.to_string(),
        DatePart::Dd => two(p.day),
        DatePart::Ddd => day_name(p.weekday).chars().take(3).collect(),
        DatePart::Dddd => day_name(p.weekday).to_string(),
        DatePart::H => hour.to_string(),
        DatePart::Hh => two(hour),
        DatePart::Min => p.minutes.to_string(),
        DatePart::Mmin => two(p.minutes),
        DatePart::S => p.seconds.to_string(),
        DatePart::Ss => two(p.seconds),
    }
}

fn month_name(month: u32) -> &'static str {
    MONTHS
        .get(month.wrapping_sub(1) as usize)
        .copied()
        .unwrap_or("")
}

fn day_name(weekday: u32) -> &'static str {
    DAYS.get(weekday as usize).copied().unwrap_or("")
}

#[cfg(test)]
mod tests {
    use super::*;

    // Catches: Excel's fictitious 1900-02-29 shifting serials after Feb 1900.
    #[test]
    fn fictitious_leap_day_shifts_serials_before_march_1900() {
        assert_eq!(
            (
                serial_to_date(1.0, false).year,
                serial_to_date(1.0, false).month,
                serial_to_date(1.0, false).day
            ),
            (1900, 1, 1)
        );
        assert_eq!(
            (
                serial_to_date(59.0, false).month,
                serial_to_date(59.0, false).day
            ),
            (2, 28)
        );
        // Serial 60 is the fictitious day. The UTC date cannot represent it, so it lands on March 1,
        // the same instant as serial 61, and does not round-trip.
        let mapped = serial_to_date(60.0, false);
        assert_eq!((mapped.year, mapped.month, mapped.day), (1900, 3, 1));
        assert_eq!(
            (
                serial_to_date(61.0, false).month,
                serial_to_date(61.0, false).day
            ),
            (3, 1)
        );
        assert_eq!(date_to_serial(&mapped, false), 61.0);
        let jan1 = UtcDateTime {
            year: 1900,
            month: 1,
            day: 1,
            hour: 0,
            minute: 0,
            second: 0,
            millis: 0,
        };
        assert_eq!(date_to_serial(&jan1, false), 1.0);
        let mar1 = UtcDateTime {
            year: 1900,
            month: 3,
            day: 1,
            hour: 0,
            minute: 0,
            second: 0,
            millis: 0,
        };
        assert_eq!(date_to_serial(&mar1, false), 61.0);
    }
}
