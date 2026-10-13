// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/xlsx/formula/functions/datetime.ts,
// workdays.ts and date-calc.ts; changes listed in PROVENANCE.md.
//! Date & time functions, including NETWORKDAYS and WORKDAY.
//! `serial_arg`, `add_months`, `days360` and `year_frac` are shared with financial functions.
#![allow(clippy::too_many_lines)]
#![allow(clippy::cognitive_complexity)]

use std::collections::HashSet;

use super::host::CallCtx;
use super::spec::{gen_spec, FunctionSpec, ParamKind};
use super::text_num::{
    days_in_month, is_leap_year, month_start_serial, parse_number_text, serial_seconds,
    serial_to_ymd, weekday_of, ymd_to_serial, Ymd,
};
use super::value::{ErrorCode, Fx, Scalar, Value};

const C: &str = "Date & Time";
const MAX_SERIAL: f64 = 2_958_465.0;

pub fn specs() -> Vec<FunctionSpec> {
    vec![
        gen_spec(
            "NETWORKDAYS",
            C,
            "NETWORKDAYS(start_date, end_date, [holidays])",
            "Whole working days between two dates.",
            2,
            3,
            &[ParamKind::Value, ParamKind::Value, ParamKind::Any],
            false,
            |ctx, args| {
                network_days(
                    ctx,
                    num(ctx, &args[0])?,
                    num(ctx, &args[1])?,
                    HashSet::from([0, 6]),
                    holiday_set(ctx, args.get(2))?,
                )
            },
        ),
        gen_spec(
            "NETWORKDAYS.INTL",
            C,
            "NETWORKDAYS.INTL(start_date, end_date, [weekend], [holidays])",
            "Working days between dates with custom weekends.",
            2,
            4,
            &[
                ParamKind::Value,
                ParamKind::Value,
                ParamKind::Value,
                ParamKind::Any,
            ],
            false,
            |ctx, args| {
                network_days(
                    ctx,
                    num(ctx, &args[0])?,
                    num(ctx, &args[1])?,
                    weekend_days(ctx, args.get(2))?,
                    holiday_set(ctx, args.get(3))?,
                )
            },
        ),
        gen_spec(
            "WORKDAY",
            C,
            "WORKDAY(start_date, days, [holidays])",
            "The date a number of working days away.",
            2,
            3,
            &[ParamKind::Value, ParamKind::Value, ParamKind::Any],
            false,
            |ctx, args| {
                workday(
                    ctx,
                    num(ctx, &args[0])?,
                    num(ctx, &args[1])?,
                    HashSet::from([0, 6]),
                    holiday_set(ctx, args.get(2))?,
                )
            },
        ),
        gen_spec(
            "WORKDAY.INTL",
            C,
            "WORKDAY.INTL(start_date, days, [weekend], [holidays])",
            "The date a number of working days away with custom weekends.",
            2,
            4,
            &[
                ParamKind::Value,
                ParamKind::Value,
                ParamKind::Value,
                ParamKind::Any,
            ],
            false,
            |ctx, args| {
                workday(
                    ctx,
                    num(ctx, &args[0])?,
                    num(ctx, &args[1])?,
                    weekend_days(ctx, args.get(2))?,
                    holiday_set(ctx, args.get(3))?,
                )
            },
        ),
        gen_spec(
            "DATE",
            C,
            "DATE(year, month, day)",
            "The serial number of a date.",
            3,
            3,
            &[],
            false,
            |ctx, args| {
                Ok(Value::number(date_serial(
                    num(ctx, &args[0])?,
                    num(ctx, &args[1])?,
                    num(ctx, &args[2])?,
                    ctx.date1904,
                )?))
            },
        ),
        gen_spec(
            "DATEVALUE",
            C,
            "DATEVALUE(date_text)",
            "Converts date text to a serial number.",
            1,
            1,
            &[],
            false,
            |ctx, args| date_value(ctx, &args[0]),
        ),
        gen_spec(
            "TIMEVALUE",
            C,
            "TIMEVALUE(time_text)",
            "Converts time text to a fraction of a day.",
            1,
            1,
            &[],
            false,
            |ctx, args| time_value(ctx, &args[0]),
        ),
        gen_spec(
            "DAY",
            C,
            "DAY(serial_number)",
            "The day of the month.",
            1,
            1,
            &[],
            false,
            |ctx, args| {
                Ok(Value::number(
                    serial_to_ymd(serial_arg(ctx, &args[0])?, ctx.date1904).day as f64,
                ))
            },
        ),
        gen_spec(
            "MONTH",
            C,
            "MONTH(serial_number)",
            "The month (1-12).",
            1,
            1,
            &[],
            false,
            |ctx, args| {
                Ok(Value::number(
                    serial_to_ymd(serial_arg(ctx, &args[0])?, ctx.date1904).month as f64,
                ))
            },
        ),
        gen_spec(
            "YEAR",
            C,
            "YEAR(serial_number)",
            "The year.",
            1,
            1,
            &[],
            false,
            |ctx, args| {
                Ok(Value::number(
                    serial_to_ymd(serial_arg(ctx, &args[0])?, ctx.date1904).year as f64,
                ))
            },
        ),
        gen_spec(
            "HOUR",
            C,
            "HOUR(serial_number)",
            "The hour (0-23).",
            1,
            1,
            &[],
            false,
            |ctx, args| {
                Ok(Value::number(
                    (serial_seconds(serial_arg(ctx, &args[0])?) / 3600) as f64,
                ))
            },
        ),
        gen_spec(
            "MINUTE",
            C,
            "MINUTE(serial_number)",
            "The minute (0-59).",
            1,
            1,
            &[],
            false,
            |ctx, args| {
                Ok(Value::number(
                    ((serial_seconds(serial_arg(ctx, &args[0])?) / 60) % 60) as f64,
                ))
            },
        ),
        gen_spec(
            "SECOND",
            C,
            "SECOND(serial_number)",
            "The second (0-59).",
            1,
            1,
            &[],
            false,
            |ctx, args| {
                Ok(Value::number(
                    (serial_seconds(serial_arg(ctx, &args[0])?) % 60) as f64,
                ))
            },
        ),
        gen_spec(
            "TIME",
            C,
            "TIME(hour, minute, second)",
            "The fraction of a day for a time.",
            3,
            3,
            &[],
            false,
            time_fn,
        ),
        gen_spec(
            "NOW",
            C,
            "NOW()",
            "The current date and time.",
            0,
            0,
            &[],
            true,
            |ctx, _| Ok(Value::number(ctx.host.now_serial())),
        ),
        gen_spec(
            "TODAY",
            C,
            "TODAY()",
            "The current date.",
            0,
            0,
            &[],
            true,
            |ctx, _| Ok(Value::number(ctx.host.now_serial().floor())),
        ),
        gen_spec(
            "WEEKDAY",
            C,
            "WEEKDAY(serial_number, [return_type])",
            "The day of the week.",
            1,
            2,
            &[],
            false,
            |ctx, args| {
                Ok(Value::number(weekday(
                    serial_arg(ctx, &args[0])?,
                    opt_num(ctx, args, 1, 1.0)?.trunc(),
                    ctx.date1904,
                )?))
            },
        ),
        gen_spec(
            "WEEKNUM",
            C,
            "WEEKNUM(serial_number, [return_type])",
            "The week number of the year.",
            1,
            2,
            &[],
            false,
            |ctx, args| {
                Ok(Value::number(week_num(
                    serial_arg(ctx, &args[0])?,
                    opt_num(ctx, args, 1, 1.0)?.trunc(),
                    ctx.date1904,
                )?))
            },
        ),
        gen_spec(
            "ISOWEEKNUM",
            C,
            "ISOWEEKNUM(date)",
            "The ISO week number of the year.",
            1,
            1,
            &[],
            false,
            |ctx, args| {
                Ok(Value::number(iso_week(
                    serial_arg(ctx, &args[0])?,
                    ctx.date1904,
                )))
            },
        ),
        gen_spec(
            "EDATE",
            C,
            "EDATE(start_date, months)",
            "The date a number of months away.",
            2,
            2,
            &[],
            false,
            |ctx, args| {
                Ok(Value::number(add_months(
                    num(ctx, &args[0])?,
                    num(ctx, &args[1])?,
                    ctx.date1904,
                    false,
                )?))
            },
        ),
        gen_spec(
            "EOMONTH",
            C,
            "EOMONTH(start_date, months)",
            "The last day of a month a number of months away.",
            2,
            2,
            &[],
            false,
            |ctx, args| {
                Ok(Value::number(add_months(
                    num(ctx, &args[0])?,
                    num(ctx, &args[1])?,
                    ctx.date1904,
                    true,
                )?))
            },
        ),
        gen_spec(
            "DATEDIF",
            C,
            "DATEDIF(start_date, end_date, unit)",
            "The difference between dates in years, months or days.",
            3,
            3,
            &[],
            false,
            |ctx, args| {
                Ok(Value::number(datedif(
                    serial_arg(ctx, &args[0])?,
                    serial_arg(ctx, &args[1])?,
                    super::helpers::str_of(ctx, &args[2])?,
                    ctx.date1904,
                )?))
            },
        ),
        gen_spec(
            "DAYS",
            C,
            "DAYS(end_date, start_date)",
            "The number of days between two dates.",
            2,
            2,
            &[],
            false,
            |ctx, args| {
                Ok(Value::number(
                    serial_arg(ctx, &args[0])?.floor() - serial_arg(ctx, &args[1])?.floor(),
                ))
            },
        ),
        gen_spec(
            "DAYS360",
            C,
            "DAYS360(start_date, end_date, [method])",
            "Days between dates on a 360-day year.",
            2,
            3,
            &[],
            false,
            |ctx, args| {
                let european = args.len() > 2 && num(ctx, &args[2])? != 0.0;
                Ok(Value::number(days360(
                    serial_arg(ctx, &args[0])?,
                    serial_arg(ctx, &args[1])?,
                    european,
                    ctx.date1904,
                )))
            },
        ),
        gen_spec(
            "YEARFRAC",
            C,
            "YEARFRAC(start_date, end_date, [basis])",
            "The fraction of a year between dates.",
            2,
            3,
            &[],
            false,
            |ctx, args| {
                Ok(Value::number(year_frac(
                    serial_arg(ctx, &args[0])?,
                    serial_arg(ctx, &args[1])?,
                    opt_num(ctx, args, 2, 0.0)?.trunc(),
                    ctx.date1904,
                )?))
            },
        ),
    ]
}

fn num(ctx: &CallCtx<'_>, value: &Value) -> Fx<f64> {
    super::helpers::num(ctx, value)
}

fn opt_num(ctx: &CallCtx<'_>, args: &[Value], index: usize, fallback: f64) -> Fx<f64> {
    if index < args.len() {
        num(ctx, &args[index])
    } else {
        Ok(fallback)
    }
}

pub(crate) fn serial_arg(ctx: &CallCtx<'_>, value: &Value) -> Fx<f64> {
    let n = num(ctx, value)?;
    if n < 0.0 || n > MAX_SERIAL + 0.99999999 {
        return Err(ErrorCode::Num);
    }
    Ok(n)
}

fn date_serial(year: f64, month: f64, day: f64, date1904: bool) -> Fx<f64> {
    let mut y = year.trunc() as i32;
    if !(0..10_000).contains(&y) {
        return Err(ErrorCode::Num);
    }
    if y < 1900 {
        y += 1900;
    }
    match ymd_to_serial(y, month.trunc() as i32, day.trunc() as i32, date1904) {
        Some(serial) if serial <= MAX_SERIAL => Ok(serial),
        _ => Err(ErrorCode::Num),
    }
}

fn date_value(ctx: &CallCtx<'_>, value: &Value) -> Fx<Value> {
    let scalar = super::helpers::scalar_of(value)?;
    let Scalar::Text(text) = scalar else {
        return match scalar {
            Scalar::Error(code) => Ok(Value::error(code)),
            _ => Err(ErrorCode::Value),
        };
    };
    let parsed = parse_number_text(&text, ctx.date1904);
    let plain = plain_number_re().is_match(text.trim());
    match parsed {
        Some(n) if !plain => Ok(Value::number(n.floor())),
        _ => Err(ErrorCode::Value),
    }
}

fn plain_number_re() -> &'static regex::Regex {
    static RE: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    RE.get_or_init(|| regex::Regex::new(r"^[+-]?[\d.,\s$]+%?$").expect("datevalue pattern"))
}

fn time_value(ctx: &CallCtx<'_>, value: &Value) -> Fx<Value> {
    let scalar = super::helpers::scalar_of(value)?;
    let Scalar::Text(text) = scalar else {
        return match scalar {
            Scalar::Error(code) => Ok(Value::error(code)),
            _ => Err(ErrorCode::Value),
        };
    };
    let parsed = parse_number_text(&text, ctx.date1904);
    if parsed.is_none() || !time_marker_re().is_match(&text) {
        return Err(ErrorCode::Value);
    }
    let parsed = parsed.unwrap_or(0.0);
    Ok(Value::number(parsed - parsed.floor()))
}

fn time_marker_re() -> &'static regex::Regex {
    static RE: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    RE.get_or_init(|| regex::Regex::new(r"(?i):|am|pm").expect("timevalue pattern"))
}

fn time_fn(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    let h = num(ctx, &args[0])?.trunc();
    let m = num(ctx, &args[1])?.trunc();
    let s = num(ctx, &args[2])?.trunc();
    if h > 32_767.0 || m > 32_767.0 || s > 32_767.0 {
        return Err(ErrorCode::Num);
    }
    let total = h * 3600.0 + m * 60.0 + s;
    if total < 0.0 {
        return Err(ErrorCode::Num);
    }
    Ok(Value::number(total % 86_400.0 / 86_400.0))
}

pub(crate) fn add_months(serial: f64, months: f64, date1904: bool, end_of_month: bool) -> Fx<f64> {
    let serial = bounded(serial)?;
    let Ymd { year, month, day } = serial_to_ymd(serial, date1904);
    let total = i64::from(year) * 12 + i64::from(month - 1) + months.trunc() as i64;
    let y = total.div_euclid(12) as i32;
    let m = (total % 12) as i32 + 1;
    let dim = days_in_month(y, m);
    let d = if end_of_month { dim } else { day.min(dim) };
    ymd_to_serial(y, m, d, date1904).ok_or(ErrorCode::Num)
}

fn bounded(serial: f64) -> Fx<f64> {
    if serial < 0.0 || serial > MAX_SERIAL + 0.99999999 {
        Err(ErrorCode::Num)
    } else {
        Ok(serial)
    }
}

fn weekday(serial: f64, kind: f64, date1904: bool) -> Fx<f64> {
    let dow = weekday_of(serial, date1904);
    if kind == 3.0 {
        return Ok(f64::from((dow + 6) % 7));
    }
    let start = match kind as i32 {
        1 => 0,
        2 | 11 => 1,
        12 => 2,
        13 => 3,
        14 => 4,
        15 => 5,
        16 => 6,
        17 => 0,
        _ => return Err(ErrorCode::Num),
    };
    Ok(f64::from((dow - start + 7).rem_euclid(7) + 1))
}

fn iso_week(serial: f64, date1904: bool) -> f64 {
    let days = serial.floor() as i64;
    let thursday = days - i64::from((weekday_of(serial, date1904) + 6) % 7) + 3;
    let year = if !date1904 && thursday < 1 {
        1899
    } else {
        serial_to_ymd(thursday as f64, date1904).year
    };
    ((thursday - month_start_serial(year, 0, date1904)) / 7 + 1) as f64
}

fn week_num(serial: f64, kind: f64, date1904: bool) -> Fx<f64> {
    if kind == 21.0 {
        return Ok(iso_week(serial, date1904));
    }
    let start = match kind as i32 {
        1 => 0,
        2 | 11 => 1,
        12 => 2,
        13 => 3,
        14 => 4,
        15 => 5,
        16 => 6,
        17 => 0,
        _ => return Err(ErrorCode::Num),
    };
    let year = serial_to_ymd(serial, date1904).year;
    let jan1 = ymd_to_serial(year, 1, 1, date1904).unwrap_or(0.0);
    let offset = (weekday_of(jan1, date1904) - start + 7).rem_euclid(7);
    Ok(((serial.floor() - jan1 + f64::from(offset)) / 7.0).floor() + 1.0)
}

fn datedif(start: f64, end: f64, unit: String, date1904: bool) -> Fx<f64> {
    if start > end {
        return Err(ErrorCode::Num);
    }
    let a = serial_to_ymd(start, date1904);
    let b = serial_to_ymd(end, date1904);
    let mut months = (b.year - a.year) * 12 + (b.month - a.month);
    if b.day < a.day {
        months -= 1;
    }
    let value = match unit.to_ascii_uppercase().as_str() {
        "Y" => months / 12,
        "M" => months,
        "D" => (end.floor() - start.floor()) as i32,
        "YM" => months.rem_euclid(12),
        "MD" => {
            if b.day >= a.day {
                b.day - a.day
            } else {
                let pm = if b.month == 1 { 12 } else { b.month - 1 };
                let py = if b.month == 1 { b.year - 1 } else { b.year };
                days_in_month(py, pm) - a.day + b.day
            }
        }
        "YD" => {
            let mut y = b.year;
            let mut anniversary = ymd_to_serial(y, a.month, a.day, date1904).unwrap_or(0.0);
            if anniversary > end.floor() {
                y -= 1;
                anniversary = ymd_to_serial(y, a.month, a.day, date1904).unwrap_or(0.0);
            }
            (end.floor() - anniversary) as i32
        }
        _ => return Err(ErrorCode::Num),
    };
    Ok(f64::from(value))
}

pub(crate) fn days360(start: f64, end: f64, european: bool, date1904: bool) -> f64 {
    let a = serial_to_ymd(start, date1904);
    let b = serial_to_ymd(end, date1904);
    let mut d1 = a.day;
    let mut d2 = b.day;
    if european {
        if d1 == 31 {
            d1 = 30;
        }
        if d2 == 31 {
            d2 = 30;
        }
    } else {
        let last_feb = |y: i32, m: i32, d: i32| m == 2 && d == days_in_month(y, 2);
        if last_feb(a.year, a.month, d1) {
            d1 = 30;
        }
        if d1 == 31 {
            d1 = 30;
        }
        if d2 == 31 && d1 >= 30 {
            d2 = 30;
        }
    }
    f64::from((b.year - a.year) * 360 + (b.month - a.month) * 30 + (d2 - d1))
}

fn year_frac_30(a: Ymd, b: Ymd) -> f64 {
    let last_feb = |d: Ymd| d.month == 2 && d.day == days_in_month(d.year, 2);
    let mut d1 = a.day;
    let mut d2 = b.day;
    if d1 == 31 && d2 == 31 {
        d1 = 30;
        d2 = 30;
    } else if d1 == 31 {
        d1 = 30;
    } else if d1 == 30 && d2 == 31 {
        d2 = 30;
    } else if last_feb(a) && last_feb(b) {
        d1 = 30;
        d2 = 30;
    } else if last_feb(a) {
        d1 = 30;
    }
    f64::from((b.year - a.year) * 360 + (b.month - a.month) * 30 + (d2 - d1)) / 360.0
}

pub(crate) fn year_frac(start: f64, end: f64, basis: f64, date1904: bool) -> Fx<f64> {
    let mut start = start.floor();
    let mut end = end.floor();
    if start > end {
        std::mem::swap(&mut start, &mut end);
    }
    let a = serial_to_ymd(start, date1904);
    let b = serial_to_ymd(end, date1904);
    match basis as i32 {
        0 => Ok(year_frac_30(a, b)),
        1 => {
            if a.year == b.year {
                return Ok((end - start) / if is_leap_year(a.year) { 366.0 } else { 365.0 });
            }
            let one_year = b.year == a.year + 1
                && (b.month < a.month || (b.month == a.month && b.day <= a.day));
            if one_year {
                let feb29 = (is_leap_year(a.year) && a.month < 3)
                    || (is_leap_year(b.year) && (b.month > 2 || (b.month == 2 && b.day == 29)));
                return Ok((end - start) / if feb29 { 366.0 } else { 365.0 });
            }
            let mut total = 0.0;
            for y in a.year..=b.year {
                total += if is_leap_year(y) { 366.0 } else { 365.0 };
            }
            Ok((end - start) / (total / f64::from(b.year - a.year + 1)))
        }
        2 => Ok((end - start) / 360.0),
        3 => Ok((end - start) / 365.0),
        4 => Ok(days360(start, end, true, date1904) / 360.0),
        _ => Err(ErrorCode::Num),
    }
}

fn weekend_days(ctx: &CallCtx<'_>, value: Option<&Value>) -> Fx<HashSet<i32>> {
    let Some(value) = value else {
        return Ok(HashSet::from([0, 6]));
    };
    if matches!(value, Value::Scalar(Scalar::Blank)) {
        return Ok(HashSet::from([0, 6]));
    }
    let scalar = super::helpers::scalar_of(value)?;
    if matches!(scalar, Scalar::Blank) {
        return Ok(HashSet::from([0, 6]));
    }
    if let Scalar::Text(text) = &scalar {
        if !weekend_mask_re().is_match(text) {
            return Err(ErrorCode::Value);
        }
        let mut days = HashSet::new();
        for (index, ch) in text.chars().enumerate() {
            if ch == '1' {
                days.insert((index as i32 + 1) % 7);
            }
        }
        return Ok(days);
    }
    let code = num(ctx, value)?.trunc() as i32;
    if (1..=7).contains(&code) {
        let first = (code + 5) % 7;
        return Ok(HashSet::from([first, (first + 1) % 7]));
    }
    if (11..=17).contains(&code) {
        return Ok(HashSet::from([code - 11]));
    }
    Err(ErrorCode::Num)
}

fn weekend_mask_re() -> &'static regex::Regex {
    static RE: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    RE.get_or_init(|| regex::Regex::new(r"^[01]{7}$").expect("weekend mask"))
}

fn holiday_set(ctx: &CallCtx<'_>, value: Option<&Value>) -> Fx<HashSet<i64>> {
    let mut out = HashSet::new();
    let Some(value) = value else {
        return Ok(out);
    };
    if matches!(value, Value::Scalar(Scalar::Blank)) {
        return Ok(out);
    }
    let mut error = None;
    ctx.for_each(value, &mut |scalar, _| {
        if error.is_some() {
            return;
        }
        if let Scalar::Error(code) = scalar {
            error = Some(*code);
            return;
        }
        if let Scalar::Number(n) = scalar {
            out.insert(n.floor() as i64);
        } else if !matches!(scalar, Scalar::Blank) {
            match super::coerce::to_number(scalar, ctx.date1904) {
                Ok(n) => out.insert(n.floor() as i64),
                Err(code) => {
                    error = Some(code);
                    false
                }
            };
        }
    });
    if let Some(code) = error {
        return Err(code);
    }
    Ok(out)
}

fn network_days(
    ctx: &CallCtx<'_>,
    start: f64,
    end: f64,
    weekend: HashSet<i32>,
    holidays: HashSet<i64>,
) -> Fx<Value> {
    let a = start.floor() as i64;
    let b = end.floor() as i64;
    let step = if a <= b { 1 } else { -1 };
    let mut count = 0i64;
    let mut d = a;
    loop {
        if !weekend.contains(&weekday_of(d as f64, ctx.date1904)) && !holidays.contains(&d) {
            count += 1;
        }
        if d == b {
            break;
        }
        d += step;
    }
    Ok(Value::number((count * step) as f64))
}

fn workday(
    ctx: &CallCtx<'_>,
    start: f64,
    days: f64,
    weekend: HashSet<i32>,
    holidays: HashSet<i64>,
) -> Fx<Value> {
    let mut d = start.floor() as i64;
    let mut remaining = days.trunc() as i64;
    let step = if remaining >= 0 { 1 } else { -1 };
    if weekend.len() >= 7 {
        return Err(ErrorCode::Value);
    }
    while remaining != 0 {
        d += step;
        if d < 0 {
            return Err(ErrorCode::Num);
        }
        if !weekend.contains(&weekday_of(d as f64, ctx.date1904)) && !holidays.contains(&d) {
            remaining -= step;
        }
    }
    Ok(Value::number(d as f64))
}
