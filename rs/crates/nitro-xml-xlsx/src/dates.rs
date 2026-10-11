// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/xlsx/read/dates.ts; changes listed in PROVENANCE.md.
//! ISO date/time strings in `t="d"` cells → Excel serial.

use crate::numfmt::{date_to_serial, UtcDateTime};

const DAY_MS: f64 = 86_400_000.0;
const EPOCH_1900: f64 = -2_208_988_800_000.0; // Date.UTC(1899, 11, 30)

/// Serial for an ISO 8601 date (`t="d"` cells), read as wall time.
pub fn iso_to_serial(iso: &str, date1904: bool) -> Option<f64> {
    let trimmed = iso.trim();
    if let Some(serial) = parse_iso_datetime(trimmed) {
        let ms = serial.0;
        if !date1904 && ms < EPOCH_1900 + DAY_MS {
            return Some((ms - EPOCH_1900) / DAY_MS);
        }
        return Some(date_to_serial(&serial.1, date1904));
    }
    parse_time_only(trimmed)
}

fn parse_iso_datetime(s: &str) -> Option<(f64, UtcDateTime)> {
    if s.len() < 10 || s.as_bytes().get(4) != Some(&b'-') || s.as_bytes().get(7) != Some(&b'-') {
        return None;
    }
    let year: i32 = s[0..4].parse().ok()?;
    let month: u32 = s[5..7].parse().ok()?;
    let day: u32 = s[8..10].parse().ok()?;
    let mut hour = 0u32;
    let mut minute = 0u32;
    let mut second = 0u32;
    let mut millis = 0u32;
    let rest = s.get(10..).unwrap_or("");
    if let Some(t) = rest.strip_prefix('T') {
        let (h, m, sec, ms, _) = parse_time_part(t)?;
        hour = h;
        minute = m;
        second = sec;
        millis = ms;
    } else if !rest.is_empty()
        && !rest.starts_with('Z')
        && !rest.starts_with('+')
        && !rest.starts_with('-')
    {
        return None;
    }
    let dt = UtcDateTime {
        year,
        month,
        day,
        hour,
        minute,
        second,
        millis,
    };
    Some((utc_ms(year, month, day, hour, minute, second, millis), dt))
}

fn parse_time_part(t: &str) -> Option<(u32, u32, u32, u32, usize)> {
    if t.len() < 5 || t.as_bytes().get(2) != Some(&b':') {
        return None;
    }
    let hour: u32 = t[0..2].parse().ok()?;
    let minute: u32 = t[3..5].parse().ok()?;
    let mut second = 0u32;
    let mut millis = 0u32;
    let mut end = 5;
    if t.len() > 5 && t.as_bytes()[5] == b':' {
        let sec_end = t[6..]
            .find(|c: char| !c.is_ascii_digit())
            .map(|i| 6 + i)
            .unwrap_or(t.len());
        second = t[6..sec_end].parse().ok()?;
        end = sec_end;
        if t.as_bytes().get(end) == Some(&b'.') {
            let frac_start = end + 1;
            let frac_end = t[frac_start..]
                .find(|c: char| !c.is_ascii_digit())
                .map(|i| frac_start + i)
                .unwrap_or(t.len());
            let frac = &t[frac_start..frac_end];
            millis = (format!("0.{frac}").parse::<f64>().unwrap_or(0.0) * 1000.0).round() as u32;
            end = frac_end;
        }
    }
    Some((hour, minute, second, millis, end))
}

fn parse_time_only(s: &str) -> Option<f64> {
    let (h, m, sec, ms, consumed) = parse_time_part(s)?;
    if consumed != s.len() && !s[consumed..].is_empty() {
        return None;
    }
    let seconds = h as f64 * 3600.0 + m as f64 * 60.0 + sec as f64;
    Some((seconds + ms as f64 / 1000.0) / 86_400.0)
}

fn utc_ms(
    year: i32,
    month: u32,
    day: u32,
    hour: u32,
    minute: u32,
    second: u32,
    millis: u32,
) -> f64 {
    let days = civil_to_days(year, month, day);
    let tod =
        hour as i64 * 3_600_000 + minute as i64 * 60_000 + second as i64 * 1000 + millis as i64;
    days as f64 * DAY_MS + tod as f64
}

fn civil_to_days(year: i32, month: u32, day: u32) -> i64 {
    let y = year as i64;
    let m = month as i64;
    let d = day as i64;
    let y_adj = y - if m <= 2 { 1 } else { 0 };
    let era = if y_adj >= 0 { y_adj } else { y_adj - 399 } / 400;
    let yoe = y_adj - era * 400;
    let doy = (153 * (m + if m > 2 { -3 } else { 9 }) + 2) / 5 + d - 1;
    yoe * 365 + yoe / 4 - yoe / 100 + doy + era * 146097 + 719468
}
