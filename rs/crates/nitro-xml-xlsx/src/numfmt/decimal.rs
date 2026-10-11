// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/xlsx/numfmt/decimal.ts; changes listed in PROVENANCE.md.

/// Significant digits of a non-negative value: `value = 0.d × 10^point`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Digits {
    /// Significant digits without leading or trailing zeros (`""` for zero).
    pub d: String,
    /// Count of digits before the decimal point.
    pub point: i32,
}

pub(crate) fn zero_digits() -> Digits {
    Digits {
        d: String::new(),
        point: 0,
    }
}

/// Decimal digits of `|x|` at 15 significant digits, matching JavaScript `toExponential(14)`.
pub(crate) fn to_digits(x: f64) -> Digits {
    let abs = x.abs();
    if abs == 0.0 || !abs.is_finite() {
        return zero_digits();
    }
    let (digits, exp) = exp_15(abs);
    trim(Digits {
        d: digits,
        point: exp + 1,
    })
}

/// 15-digit significand and the base-10 exponent of `toExponential(14)`.
///
/// Rust's default `{:.14e}` breaks ties to even. JavaScript breaks them away from zero, so this
/// formats 40 digits (enough that a tie at digit 15 is visible) and rounds half away from zero.
fn exp_15(abs: f64) -> (String, i32) {
    let s = format!("{abs:.40e}");
    let pos = s.find(['e', 'E']).expect("exponential form");
    let mant = &s[..pos];
    let exp: i32 = s[pos + 1..].parse().expect("exponent");
    let mut exp = exp;
    let raw: Vec<u8> = mant.bytes().filter(|c| *c != b'.').collect();
    let mut head: Vec<u8> = raw.iter().copied().take(15).collect();
    while head.len() < 15 {
        head.push(b'0');
    }
    let next = raw.get(15).copied().unwrap_or(b'0');
    if next >= b'5' {
        let mut i = head.len() as i32 - 1;
        while i >= 0 && head[i as usize] == b'9' {
            head[i as usize] = b'0';
            i -= 1;
        }
        if i < 0 {
            head.insert(0, b'1');
            head.truncate(15);
            exp += 1;
        } else {
            head[i as usize] += 1;
        }
    }
    (String::from_utf8(head).expect("digits"), exp)
}

fn trim(v: Digits) -> Digits {
    let bytes = v.d.as_bytes();
    let mut lead = 0;
    while lead < bytes.len() && bytes[lead] == b'0' {
        lead += 1;
    }
    let mut d = v.d[lead..].to_string();
    let point = v.point - lead as i32;
    let mut end = d.len();
    while end > 0 && d.as_bytes()[end - 1] == b'0' {
        end -= 1;
    }
    d.truncate(end);
    if d.is_empty() {
        zero_digits()
    } else {
        Digits { d, point }
    }
}

/// Rounds to `keep` significant digits (half away from zero).
pub(crate) fn round_significant(v: &Digits, keep: i32) -> Digits {
    if keep >= v.d.len() as i32 {
        return v.clone();
    }
    if keep < 0 {
        return zero_digits();
    }
    let keep = keep as usize;
    let round_up = v.d.as_bytes().get(keep).copied().unwrap_or(b'0') >= b'5';
    let mut head = v.d.as_bytes()[..keep].to_vec();
    let mut point = v.point;
    if round_up {
        let mut i = head.len() as i32 - 1;
        while i >= 0 && head[i as usize] == b'9' {
            head[i as usize] = b'0';
            i -= 1;
        }
        if i < 0 {
            head.insert(0, b'1');
            point += 1;
        } else {
            head[i as usize] += 1;
        }
    }
    trim(Digits {
        d: String::from_utf8(head).expect("digits"),
        point,
    })
}

/// Rounds to `decimals` places after the point.
pub(crate) fn round_decimals(v: &Digits, decimals: i32) -> Digits {
    round_significant(v, v.point + decimals)
}

/// Multiplies by `10^n` exactly on the digit representation.
pub(crate) fn shift(v: &Digits, n: i32) -> Digits {
    if v.d.is_empty() {
        v.clone()
    } else {
        Digits {
            d: v.d.clone(),
            point: v.point + n,
        }
    }
}

/// Integer digits (no leading zeros, `""` when the integer is zero).
pub(crate) fn integer_part(v: &Digits) -> String {
    if v.point <= 0 {
        return String::new();
    }
    let p = v.point as usize;
    let mut s: String = v.d.chars().take(p).collect();
    if s.len() < p {
        s.extend(std::iter::repeat_n('0', p - s.len()));
    }
    s
}

/// Exactly `decimals` fractional digits (truncated; round first).
pub(crate) fn fraction_part(v: &Digits, decimals: i32) -> String {
    if decimals <= 0 {
        return String::new();
    }
    let decimals = decimals as usize;
    let frac = if v.point >= 0 {
        let p = v.point as usize;
        if p >= v.d.len() {
            String::new()
        } else {
            v.d[p..].to_string()
        }
    } else {
        let zeros = (-v.point) as usize;
        format!("{}{}", "0".repeat(zeros), v.d)
    };
    let mut out: String = frac.chars().take(decimals).collect();
    if out.len() < decimals {
        out.extend(std::iter::repeat_n('0', decimals - out.len()));
    }
    out
}

pub(crate) fn is_zero(v: &Digits) -> bool {
    v.d.is_empty()
}

/// Plain decimal string of a non-negative value, trailing zeros removed.
pub(crate) fn plain(v: &Digits) -> String {
    if v.d.is_empty() {
        return "0".into();
    }
    let int_raw = integer_part(v);
    let int = if int_raw.is_empty() {
        "0"
    } else {
        int_raw.as_str()
    };
    let frac_len = (v.d.len() as i32 - v.point).max(0);
    let frac = fraction_part(v, frac_len);
    if frac.is_empty() {
        int.to_string()
    } else {
        format!("{int}.{frac}")
    }
}

pub(crate) fn pad_start(s: &str, width: usize, ch: char) -> String {
    let n = s.chars().count();
    if n >= width {
        return s.to_string();
    }
    let mut out = String::with_capacity(s.len() + width - n);
    for _ in 0..(width - n) {
        out.push(ch);
    }
    out.push_str(s);
    out
}

pub(crate) fn pad_end(s: &str, width: usize, ch: char) -> String {
    let n = s.chars().count();
    if n >= width {
        return s.to_string();
    }
    let mut out = String::with_capacity(s.len() + width - n);
    out.push_str(s);
    for _ in 0..(width - n) {
        out.push(ch);
    }
    out
}

/// `Math.round`: half towards +∞, already-integral values unchanged.
pub(crate) fn js_round(n: f64) -> f64 {
    if !n.is_finite() || n.fract() == 0.0 {
        return n;
    }
    if n > 0.0 && n < 0.5 {
        return 0.0;
    }
    if (-0.5..0.0).contains(&n) {
        return -0.0;
    }
    let f = n.floor();
    let frac = n - f;
    if frac >= 0.5 {
        f + 1.0
    } else {
        f
    }
}

/// JavaScript `String(n)` for an integer-valued number (fraction numerators, elapsed counts).
pub fn js_int_string(n: f64) -> String {
    if !n.is_finite() {
        if n.is_nan() {
            return "NaN".into();
        }
        return if n.is_sign_positive() {
            "Infinity".into()
        } else {
            "-Infinity".into()
        };
    }
    if n == 0.0 {
        return "0".into();
    }
    let neg = n.is_sign_negative();
    let abs = n.abs();
    let plain = format!("{abs}");
    let digits = if plain.bytes().all(|c| c.is_ascii_digit()) {
        plain
    } else {
        digits_of_abs(abs, &plain)
    };
    let body = if abs >= 1e21 {
        exp_from_plain_int(&digits)
    } else {
        digits
    };
    if neg {
        format!("-{body}")
    } else {
        body
    }
}

fn digits_of_abs(abs: f64, plain: &str) -> String {
    if plain.bytes().all(|c| c.is_ascii_digit()) {
        return plain.to_string();
    }
    // Rust switched to scientific notation. Rebuild the integer digits from the exponent form.
    if let Some(pos) = plain.find(['e', 'E']) {
        let mant = &plain[..pos];
        let exp: i32 = plain[pos + 1..].parse().unwrap_or(0);
        let digits: String = mant.chars().filter(|c| *c != '.').collect();
        let point = exp + 1;
        if point <= 0 {
            return "0".into();
        }
        let mut s: String = digits.chars().take(point as usize).collect();
        if (s.len() as i32) < point {
            s.extend(std::iter::repeat_n('0', (point as usize) - s.len()));
        }
        return s;
    }
    format!("{abs:.0}")
}

fn exp_from_plain_int(plain: &str) -> String {
    let digits = plain.trim_end_matches('0');
    if digits.is_empty() {
        return "0".into();
    }
    let exp = plain.len() - 1;
    let mut s = String::new();
    s.push(digits.as_bytes()[0] as char);
    if digits.len() > 1 {
        s.push('.');
        s.push_str(&digits[1..]);
    }
    s.push_str("e+");
    s.push_str(&exp.to_string());
    s
}
