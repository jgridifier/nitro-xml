// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/xlsx/formula/text-number.ts and the date-serial
// helpers it calls (ymdToSerial / monthStartSerial); changes listed in PROVENANCE.md.
//! Text-to-number coercion and Excel's General number-to-text conversion.

use std::sync::OnceLock;

use regex::Regex;

const MONTHS: [&str; 12] = [
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

fn cached<'a>(slot: &'a OnceLock<Regex>, pattern: &str) -> &'a Regex {
    slot.get_or_init(|| Regex::new(pattern).expect("formula text pattern"))
}

fn number_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    cached(
        &RE,
        r"(?i)^([0-9]+(?:,[0-9]{3,})+|[0-9]*)(\.[0-9]*)?(?:[eE]([+-]?[0-9]+))?$",
    )
}

fn time_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    cached(
        &RE,
        r"(?i)^([0-9]{1,4})(?::([0-9]+))?(?::([0-9]+(?:\.[0-9]*)?))?(?:\s+(am|pm|a|p))?$",
    )
}

fn fraction_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    cached(&RE, r"^([+-]?)(?:([0-9]+)\s+)?([0-9]+)/([0-9]+)$")
}

fn signed_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    cached(&RE, r"^([+-]?)\s*\$?\s*([+-]?)\s*")
}

fn date_ymd() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    cached(&RE, r"^([0-9]{4})[-/]([0-9]{1,2})[-/]([0-9]{1,2})$")
}

fn date_mdy() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    cached(&RE, r"^([0-9]{1,2})/([0-9]{1,2})(?:/([0-9]{2}|[0-9]{4}))?$")
}

fn date_dmon() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    cached(
        &RE,
        r"(?i)^([0-9]{1,2})[-\s]([a-z]+)\.?(?:[-\s,]+([0-9]{2}|[0-9]{4}))?$",
    )
}

fn date_mond() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    cached(
        &RE,
        r"(?i)^([a-z]+)\.?[-\s]([0-9]{1,2})(?:,?\s*([0-9]{4}))?$",
    )
}

fn date_mony() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    cached(&RE, r"(?i)^([a-z]+)\.?[-\s]([0-9]{4})$")
}

fn ampm_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    cached(&RE, r"(?i)^(.*\S)\s+(\S+\s*(?:am|pm))$")
}

fn month_from_name(name: &str) -> Option<i32> {
    let lower = name.to_lowercase();
    if lower.len() < 3 {
        return None;
    }
    let index = MONTHS
        .iter()
        .position(|full| lower.starts_with(&full[..3]))?;
    let full = MONTHS[index];
    if lower.len() == 3 || full.starts_with(&lower) || lower == "sept" {
        Some(index as i32 + 1)
    } else {
        None
    }
}

fn parse_plain_number(text: &str) -> Option<f64> {
    let caps = number_re().captures(text)?;
    let int_part = caps
        .get(1)
        .map(|m| m.as_str())
        .unwrap_or("")
        .replace(',', "");
    let frac = caps.get(2).map(|m| m.as_str()).unwrap_or("");
    if int_part.is_empty() && frac.len() < 2 {
        return None;
    }
    let exp = caps
        .get(3)
        .map(|m| format!("e{}", m.as_str()))
        .unwrap_or_default();
    let frac = if frac == "." { "" } else { frac };
    let body = if int_part.is_empty() {
        format!("0{frac}{exp}")
    } else {
        format!("{int_part}{frac}{exp}")
    };
    let value: f64 = body.parse().ok()?;
    value.is_finite().then_some(value)
}

fn parse_time(text: &str) -> Option<f64> {
    let caps = time_re().captures(text)?;
    let mut hours: f64 = caps.get(1)?.as_str().parse().ok()?;
    let minutes: f64 = match caps.get(2) {
        Some(m) => m.as_str().parse().ok()?,
        None => 0.0,
    };
    let seconds: f64 = match caps.get(3) {
        Some(m) => m.as_str().parse().ok()?,
        None => 0.0,
    };
    let meridiem = caps.get(4).map(|m| m.as_str().to_lowercase());
    if caps.get(2).is_none() && (meridiem.is_none() || caps.get(3).is_some()) {
        return None;
    }
    if let Some(meridiem) = meridiem {
        if hours > 12.0 {
            return None;
        }
        if hours == 12.0 {
            hours = 0.0;
        }
        if meridiem.starts_with('p') {
            hours += 12.0;
        }
    }
    Some((hours * 3600.0 + minutes * 60.0 + seconds) / 86_400.0)
}

pub(crate) fn is_leap_year(year: i32) -> bool {
    year % 4 == 0 && (year % 100 != 0 || year % 400 == 0)
}

pub(crate) fn days_in_month(year: i32, month: i32) -> i32 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if is_leap_year(year) => 29,
        2 => 28,
        _ => 0,
    }
}

/// Days since the Unix epoch (Howard Hinnant), for a Gregorian civil date.
fn days_from_civil(mut year: i32, month: i32, day: i32) -> i64 {
    year -= i32::from(month <= 2);
    let era = if year >= 0 { year } else { year - 399 } / 400;
    let yoe = (year - era * 400) as i64;
    let month_adj = if month > 2 { month - 3 } else { month + 9 };
    let doy = (153 * month_adj as i64 + 2) / 5 + day as i64 - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era as i64 * 146097 + doe - 719468
}

fn epoch_days(date1904: bool) -> i64 {
    if date1904 {
        days_from_civil(1904, 1, 1)
    } else {
        days_from_civil(1899, 12, 30)
    }
}

pub(crate) fn month_start_serial(year: i32, month0: i32, date1904: bool) -> i64 {
    let days = days_from_civil(year, month0 + 1, 1) - epoch_days(date1904);
    if !date1904 && days < 61 {
        days - 1
    } else {
        days
    }
}

/// The serial of a calendar date, or `None` outside Excel's range.
pub fn ymd_to_serial(year: i32, month: i32, day: i32, date1904: bool) -> Option<f64> {
    let total = year as i64 * 12 + (month as i64 - 1);
    // `div_euclid` matches `Math.floor(total / 12)` for negative totals.
    let y = total.div_euclid(12) as i32;
    if y > 9999 {
        return None;
    }
    let month0 = (total - y as i64 * 12) as i32;
    let serial = month_start_serial(y, month0, date1904) + day as i64 - 1;
    let limit = month_start_serial(10000, 0, date1904);
    if serial < 0 || serial >= limit {
        return None;
    }
    Some(serial as f64)
}

fn serial_or_none(year: i32, month: i32, day: i32, date1904: bool) -> Option<f64> {
    if !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return None;
    }
    if year < if date1904 { 1904 } else { 1900 } {
        return None;
    }
    let phantom = year == 1900 && month == 2 && day == 29;
    if !phantom && day > days_in_month(year, month) {
        return None;
    }
    ymd_to_serial(year, month, day, date1904)
}

fn full_year(text: &str) -> i32 {
    let year: i32 = text.parse().unwrap_or(0);
    if text.len() > 2 {
        return year;
    }
    if year < 30 {
        2000 + year
    } else {
        1900 + year
    }
}

fn local_year() -> i32 {
    #[cfg(unix)]
    {
        // SAFETY: `time` and `localtime_r` are the libc clock. `tm` is a local
        // zeroed `struct tm`; `localtime_r` only writes through that pointer.
        unsafe {
            let t = libc::time(std::ptr::null_mut());
            let mut tm = std::mem::zeroed();
            if libc::localtime_r(&t, &mut tm).is_null() {
                1970
            } else {
                tm.tm_year + 1900
            }
        }
    }
    #[cfg(not(unix))]
    {
        1970
    }
}

fn parse_date(text: &str, date1904: bool) -> Option<f64> {
    let year_now = local_year();
    if let Some(caps) = date_ymd().captures(text) {
        return serial_or_none(
            caps[1].parse().ok()?,
            caps[2].parse().ok()?,
            caps[3].parse().ok()?,
            date1904,
        );
    }
    if let Some(caps) = date_mdy().captures(text) {
        let year = caps
            .get(3)
            .map(|m| full_year(m.as_str()))
            .unwrap_or(year_now);
        return serial_or_none(year, caps[1].parse().ok()?, caps[2].parse().ok()?, date1904);
    }
    if let Some(caps) = date_dmon().captures(text) {
        let month = month_from_name(caps.get(2)?.as_str())?;
        let year = caps
            .get(3)
            .map(|m| full_year(m.as_str()))
            .unwrap_or(year_now);
        return serial_or_none(year, month, caps[1].parse().ok()?, date1904);
    }
    if let Some(caps) = date_mond().captures(text) {
        let month = month_from_name(caps.get(1)?.as_str())?;
        let year = caps
            .get(3)
            .map(|m| m.as_str().parse().ok())
            .unwrap_or(Some(year_now))?;
        return serial_or_none(year, month, caps[2].parse().ok()?, date1904);
    }
    if let Some(caps) = date_mony().captures(text) {
        let month = month_from_name(caps.get(1)?.as_str())?;
        return serial_or_none(caps[2].parse().ok()?, month, 1, date1904);
    }
    None
}

/// The number a text converts to in arithmetic, or `None` when Excel reports `#VALUE!`.
pub fn parse_number_text(input: &str, date1904: bool) -> Option<f64> {
    let mut text = input.trim().to_owned();
    if text.is_empty() {
        return None;
    }
    let mut scale = 1.0;
    if text.ends_with('%') {
        scale = 0.01;
        text = text[..text.len() - 1].trim_end().to_owned();
    }
    let mut sign = 1.0;
    if text.starts_with('(') && text.ends_with(')') {
        sign = -1.0;
        text = text[1..text.len() - 1].trim().to_owned();
    }
    if let Some(caps) = signed_re().captures(&text) {
        let sign_text = format!(
            "{}{}",
            caps.get(1).map(|m| m.as_str()).unwrap_or(""),
            caps.get(2).map(|m| m.as_str()).unwrap_or("")
        );
        if sign_text.len() <= 1 {
            let matched = caps.get(0).map(|m| m.len()).unwrap_or(0);
            let body = &text[matched..];
            if let Some(plain) = parse_plain_number(body) {
                let signed = if sign_text == "-" { -1.0 } else { 1.0 };
                return Some(signed * sign * plain * scale);
            }
        }
    }
    if scale != 1.0 || sign != 1.0 {
        return None;
    }
    if let Some(caps) = fraction_re().captures(&text) {
        if caps.get(2).is_some() {
            let den: f64 = caps[4].parse().ok()?;
            if den == 0.0 {
                return None;
            }
            let whole: f64 = caps[2].parse().ok()?;
            let num: f64 = caps[3].parse().ok()?;
            let value = whole + num / den;
            return Some(if &caps[1] == "-" { -value } else { value });
        }
    }
    if let Some(time) = parse_time(&text) {
        return Some(time);
    }
    if let Some(date) = parse_date(&text, date1904) {
        return Some(date);
    }
    if let Some(space) = text.rfind(' ') {
        if space > 0 {
            let date_part = parse_date(text[..space].trim(), date1904);
            let time_part = parse_time(text[space + 1..].trim());
            if let (Some(date), Some(time)) = (date_part, time_part) {
                return Some(date + time);
            }
            if let Some(caps) = ampm_re().captures(&text) {
                let date = parse_date(caps.get(1).map(|m| m.as_str()).unwrap_or(""), date1904);
                let time = parse_time(caps.get(2).map(|m| m.as_str()).unwrap_or(""));
                if let (Some(date), Some(time)) = (date, time) {
                    return Some(date + time);
                }
            }
        }
    }
    None
}

/// Rounds to 15 significant digits, the precision Excel displays and compares with.
/// `Number(value.toExponential(14))` in JavaScript; Rust's `{:.14e}` matches those digits.
pub fn round15(value: f64) -> f64 {
    if value == 0.0 || !value.is_finite() {
        return value;
    }
    let text = format!("{value:.14e}");
    text.parse::<f64>().unwrap_or(value)
}

/// Excel's General conversion of a number to text.
pub fn number_to_text(value: f64) -> String {
    if value == 0.0 {
        return "0".to_owned();
    }
    if !value.is_finite() {
        return "#NUM!".to_owned();
    }
    let rounded = round15(value);
    let negative = rounded < 0.0;
    let abs = rounded.abs();
    let text = format!("{abs:.14e}");
    let (mantissa, exp_text) = text.split_once('e').unwrap_or(("0", "0"));
    let exponent: i32 = exp_text.parse().unwrap_or(0);
    let digits = mantissa.replace('.', "");
    let digits = digits.trim_end_matches('0');
    let digits = if digits.is_empty() { "0" } else { digits };
    let plain = if exponent >= 0 {
        let exp = exponent as usize;
        if digits.len() <= exp + 1 {
            format!("{digits}{}", "0".repeat(exp + 1 - digits.len()))
        } else {
            format!("{}.{}", &digits[..=exp], &digits[exp + 1..])
        }
    } else {
        let zeros = (-exponent - 1) as usize;
        format!("0.{}{digits}", "0".repeat(zeros))
    };
    if plain.len() <= 20 {
        return if negative { format!("-{plain}") } else { plain };
    }
    let mant = if digits.len() > 1 {
        format!("{}.{}", &digits[..1], &digits[1..])
    } else {
        digits.to_owned()
    };
    let exp_abs = exponent.unsigned_abs();
    let exp = format!("{}{exp_abs:02}", if exponent < 0 { "-" } else { "+" });
    format!("{}{mant}E{exp}", if negative { "-" } else { "" })
}

/// Calendar date of a serial. Serial 0 is 1900-01-00 and serial 60 is 1900-02-29.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Ymd {
    pub year: i32,
    pub month: i32,
    pub day: i32,
}

/// Inverse of [`days_from_civil`] (Howard Hinnant).
fn civil_from_days(mut z: i64) -> Ymd {
    z += 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = (z - era * 146097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = if m <= 2 { y + 1 } else { y };
    Ymd {
        year: year as i32,
        month: m as i32,
        day: d as i32,
    }
}

pub(crate) fn serial_to_ymd(serial: f64, date1904: bool) -> Ymd {
    let days = serial.floor() as i64;
    if date1904 {
        return civil_from_days(epoch_days(true) + days);
    }
    if days == 0 {
        return Ymd {
            year: 1900,
            month: 1,
            day: 0,
        };
    }
    if days == 60 {
        return Ymd {
            year: 1900,
            month: 2,
            day: 29,
        };
    }
    let adjusted = if days < 60 { days + 1 } else { days };
    civil_from_days(epoch_days(false) + adjusted)
}

/// Day of the week, 0 = Sunday. Serial 1 in the 1900 system is a Sunday.
pub(crate) fn weekday_of(serial: f64, date1904: bool) -> i32 {
    let days = serial.floor() as i64;
    let n = if date1904 { days + 5 } else { days - 1 };
    n.rem_euclid(7) as i32
}

/// Whole seconds in the fractional day. An exact half second rounds to even.
pub(crate) fn serial_seconds(serial: f64) -> i64 {
    let seconds = (serial - serial.floor()) * 86_400.0;
    let whole = seconds.floor();
    let rest = seconds - whole;
    let rounded = if rest > 0.5 || (rest == 0.5 && (whole as i64) % 2 == 1) {
        whole + 1.0
    } else {
        whole
    };
    (rounded as i64).rem_euclid(86_400)
}
