// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/xlsx/formula/functions/engineering.ts,
// complex.ts, convert.ts, and the ERF entries in distributions.ts; changes listed in PROVENANCE.md.
//! Engineering functions from BIN2DEC through OCT2HEX, including complex numbers,
//! CONVERT and the error function. ROMAN, ARABIC, BASE and DECIMAL stay Math & Trig.
#![allow(clippy::too_many_lines)]
#![allow(clippy::cognitive_complexity)]
#![allow(clippy::similar_names)]

use std::collections::HashMap;
use std::sync::OnceLock;

use regex::Regex;

use super::coerce::{finite, to_text};
use super::helpers::{int_of, num, scalar_of, str_of};
use super::host::CallCtx;
use super::ops::js_is_integer;
use super::spec::{gen_spec, num_spec, FunctionSpec, ParamKind};
use super::value::{ErrorCode, Fx, Scalar, Value};

const C: &str = "Engineering";
const ANY: &[ParamKind] = &[ParamKind::Any];
const ZERO: &[Option<f64>] = &[None, Some(0.0)];
const TWO_24: f64 = 16_777_216.0;
const TWO_48: f64 = 281_474_976_710_656.0;

fn is_blank(value: &Value) -> bool {
    matches!(value, Value::Scalar(Scalar::Blank))
}

fn ok_num(n: f64) -> Fx<Value> {
    Ok(Value::number(finite(n)?))
}

fn text_value(text: String) -> Value {
    Value::Scalar(Scalar::Text(text))
}

fn bits_of(radix: u32) -> i32 {
    match radix {
        2 => 10,
        8 => 30,
        16 => 40,
        _ => 0,
    }
}

fn parse_base(ctx: &CallCtx<'_>, value: &Value, radix: u32) -> Fx<f64> {
    if radix == 10 {
        return Ok(num(ctx, value)?.trunc());
    }
    let text = str_of(ctx, value)?.trim().to_uppercase();
    if text.is_empty() {
        return Ok(0.0);
    }
    let ok = text.len() <= 10
        && match radix {
            2 => text.chars().all(|ch| ch == '0' || ch == '1'),
            8 => text.chars().all(|ch| ('0'..='7').contains(&ch)),
            16 => text.chars().all(|ch| ch.is_ascii_hexdigit()),
            _ => false,
        };
    if !ok {
        return Err(ErrorCode::Num);
    }
    let mut n = i64::from_str_radix(&text, radix).map_err(|_| ErrorCode::Num)? as f64;
    let bits = bits_of(radix);
    if text.len() == 10 && n >= 2f64.powi(bits - 1) {
        n -= 2f64.powi(bits);
    }
    Ok(n)
}

fn format_base(n: f64, radix: u32, places: Option<f64>) -> Fx<String> {
    let bits = bits_of(radix);
    let limit = 2f64.powi(bits - 1);
    if n < -limit || n >= limit {
        return Err(ErrorCode::Num);
    }
    let text = if n < 0.0 {
        format_radix(2f64.powi(bits) + n, radix)
    } else {
        format_radix(n, radix)
    };
    let Some(places) = places else {
        return Ok(text);
    };
    let places = places.trunc();
    if places < text.len() as f64 || places > 10.0 {
        return Err(ErrorCode::Num);
    }
    Ok(format!("{:0>width$}", text, width = places as usize))
}

fn format_radix(n: f64, radix: u32) -> String {
    let mut n = n.trunc() as u64;
    if n == 0 {
        return "0".to_owned();
    }
    let mut out = String::new();
    while n > 0 {
        let digit = (n % u64::from(radix)) as u8;
        let ch = if digit < 10 {
            b'0' + digit
        } else {
            b'A' + (digit - 10)
        };
        out.push(ch as char);
        n /= u64::from(radix);
    }
    out.chars().rev().collect()
}

fn convert_base(ctx: &CallCtx<'_>, args: &[Value], from: u32, to: u32, to_dec: bool) -> Fx<Value> {
    let n = parse_base(ctx, &args[0], from)?;
    if to_dec {
        return ok_num(n);
    }
    let places = if args.len() > 1 && !is_blank(&args[1]) {
        Some(int_of(ctx, &args[1])?)
    } else {
        None
    };
    Ok(text_value(format_base(n, to, places)?))
}

fn fn_bin2dec(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    convert_base(ctx, args, 2, 10, true)
}
fn fn_bin2oct(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    convert_base(ctx, args, 2, 8, false)
}
fn fn_bin2hex(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    convert_base(ctx, args, 2, 16, false)
}
fn fn_oct2dec(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    convert_base(ctx, args, 8, 10, true)
}
fn fn_oct2bin(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    convert_base(ctx, args, 8, 2, false)
}
fn fn_oct2hex(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    convert_base(ctx, args, 8, 16, false)
}
fn fn_dec2bin(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    convert_base(ctx, args, 10, 2, false)
}
fn fn_dec2oct(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    convert_base(ctx, args, 10, 8, false)
}
fn fn_dec2hex(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    convert_base(ctx, args, 10, 16, false)
}
fn fn_hex2bin(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    convert_base(ctx, args, 16, 2, false)
}
fn fn_hex2oct(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    convert_base(ctx, args, 16, 8, false)
}
fn fn_hex2dec(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    convert_base(ctx, args, 16, 10, true)
}

#[derive(Clone, Copy)]
struct Complex {
    re: f64,
    im: f64,
    suffix: u8,
}

fn complex_patterns() -> &'static (Regex, Regex, Regex) {
    static RE: OnceLock<(Regex, Regex, Regex)> = OnceLock::new();
    RE.get_or_init(|| {
        let number = r"(?:[0-9]+\.?[0-9]*|\.[0-9]+)(?:[eE][+-]?[0-9]+)?";
        (
            Regex::new(&format!(r"^([+-]?{number})$")).expect("complex real"),
            Regex::new(&format!(r"^([+-]?(?:{number})?)([ij])$")).expect("complex imag"),
            Regex::new(&format!(r"^([+-]?{number})([+-])((?:{number})?)([ij])$")).expect("complex"),
        )
    })
}

fn coefficient(text: &str) -> f64 {
    if text.is_empty() || text == "+" {
        1.0
    } else if text == "-" {
        -1.0
    } else {
        text.parse().unwrap_or(f64::NAN)
    }
}

fn parse_complex(value: &Scalar) -> Fx<Complex> {
    match value {
        Scalar::Error(code) => return Err(*code),
        Scalar::Number(n) => {
            return Ok(Complex {
                re: *n,
                im: 0.0,
                suffix: b'i',
            });
        }
        Scalar::Blank => {
            return Ok(Complex {
                re: 0.0,
                im: 0.0,
                suffix: b'i',
            })
        }
        Scalar::Bool(_) | Scalar::Text(_) => {}
    }
    let text = to_text(value)?;
    if text.is_empty() {
        return Ok(Complex {
            re: 0.0,
            im: 0.0,
            suffix: b'i',
        });
    }
    let (real, imag, both) = complex_patterns();
    if let Some(caps) = real.captures(&text) {
        return Ok(Complex {
            re: coefficient(caps.get(1).map(|m| m.as_str()).unwrap_or("")),
            im: 0.0,
            suffix: b'i',
        });
    }
    if let Some(caps) = imag.captures(&text) {
        return Ok(Complex {
            re: 0.0,
            im: coefficient(caps.get(1).map(|m| m.as_str()).unwrap_or("")),
            suffix: if caps.get(2).map(|m| m.as_str()) == Some("j") {
                b'j'
            } else {
                b'i'
            },
        });
    }
    let Some(caps) = both.captures(&text) else {
        return Err(ErrorCode::Num);
    };
    let sign = if caps.get(2).map(|m| m.as_str()) == Some("-") {
        -1.0
    } else {
        1.0
    };
    Ok(Complex {
        re: coefficient(caps.get(1).map(|m| m.as_str()).unwrap_or("")),
        im: sign * coefficient(caps.get(3).map(|m| m.as_str()).unwrap_or("")),
        suffix: if caps.get(4).map(|m| m.as_str()) == Some("j") {
            b'j'
        } else {
            b'i'
        },
    })
}

fn complex_arg(_ctx: &CallCtx<'_>, value: &Value) -> Fx<Complex> {
    parse_complex(&scalar_of(value)?)
}

/// 15 significant digits, then Excel's `E+05` exponent form.
fn digits(n: f64) -> String {
    if !n.is_finite() || n == 0.0 {
        return "0".to_owned();
    }
    let rounded = super::text_num::round15(n);
    rewrite_exp(&crate::number_format::js_number_string(if rounded == 0.0 {
        0.0
    } else {
        rounded
    }))
}

fn rewrite_exp(text: &str) -> String {
    let Some(index) = text.find(['e', 'E']) else {
        return text.to_owned();
    };
    let (mant, rest) = text.split_at(index);
    let rest = &rest[1..];
    let (sign, digits) = if let Some(body) = rest.strip_prefix('+') {
        ("+", body)
    } else if let Some(body) = rest.strip_prefix('-') {
        ("-", body)
    } else {
        ("+", rest)
    };
    let padded = if digits.len() < 2 {
        format!("{digits:0>2}")
    } else {
        digits.to_owned()
    };
    format!("{mant}E{sign}{padded}")
}

fn format_complex(z: &Complex) -> String {
    let r = if z.re.abs() < 1e-15 * 1.0_f64.max(z.im.abs()) {
        0.0
    } else {
        z.re
    };
    let i = if z.im.abs() < 1e-15 * 1.0_f64.max(z.re.abs()) {
        0.0
    } else {
        z.im
    };
    if i == 0.0 {
        return digits(r);
    }
    let suffix = z.suffix as char;
    let imaginary = if i == 1.0 {
        suffix.to_string()
    } else if i == -1.0 {
        format!("-{suffix}")
    } else {
        format!("{}{suffix}", digits(i))
    };
    if r == 0.0 {
        return imaginary;
    }
    format!("{}{}{imaginary}", digits(r), if i > 0.0 { "+" } else { "" })
}

fn make(re: f64, im: f64, suffix: u8) -> Complex {
    Complex { re, im, suffix }
}

fn modulus(z: Complex) -> f64 {
    z.re.hypot(z.im)
}

fn argument(z: Complex) -> f64 {
    z.im.atan2(z.re)
}

fn polar(r: f64, theta: f64, suffix: u8) -> Complex {
    make(r * theta.cos(), r * theta.sin(), suffix)
}

fn divide(a: Complex, b: Complex) -> Fx<Complex> {
    let d = b.re * b.re + b.im * b.im;
    if d == 0.0 {
        return Err(ErrorCode::Num);
    }
    Ok(make(
        (a.re * b.re + a.im * b.im) / d,
        (a.im * b.re - a.re * b.im) / d,
        a.suffix,
    ))
}

fn one(z: Complex) -> Complex {
    make(1.0, 0.0, z.suffix)
}

fn integer_power(z: Complex, n: f64) -> Fx<Complex> {
    let mut result = one(z);
    let mut i = 0.0;
    let steps = n.abs();
    while i < steps {
        result = make(
            result.re * z.re - result.im * z.im,
            result.re * z.im + result.im * z.re,
            z.suffix,
        );
        i += 1.0;
    }
    if n < 0.0 {
        divide(one(z), result)
    } else {
        Ok(result)
    }
}

fn exp_c(z: Complex) -> Complex {
    polar(z.re.exp(), z.im, z.suffix)
}

fn ln_c(z: Complex) -> Fx<Complex> {
    if z.re == 0.0 && z.im == 0.0 {
        return Err(ErrorCode::Num);
    }
    Ok(make(modulus(z).ln(), argument(z), z.suffix))
}

fn scale(z: Complex, k: f64) -> Complex {
    make(z.re * k, z.im * k, z.suffix)
}

fn sin_c(z: Complex) -> Complex {
    make(z.re.sin() * z.im.cosh(), z.re.cos() * z.im.sinh(), z.suffix)
}

fn cos_c(z: Complex) -> Complex {
    make(
        z.re.cos() * z.im.cosh(),
        -z.re.sin() * z.im.sinh(),
        z.suffix,
    )
}

fn sinh_c(z: Complex) -> Complex {
    make(z.re.sinh() * z.im.cos(), z.re.cosh() * z.im.sin(), z.suffix)
}

fn cosh_c(z: Complex) -> Complex {
    make(z.re.cosh() * z.im.cos(), z.re.sinh() * z.im.sin(), z.suffix)
}

fn joined(a: Complex, b: Complex) -> Fx<u8> {
    if a.suffix != b.suffix && a.im != 0.0 && b.im != 0.0 {
        return Err(ErrorCode::Value);
    }
    Ok(if a.im != 0.0 { a.suffix } else { b.suffix })
}

enum ImOut {
    Num(f64),
    Z(Complex),
}

fn show(result: ImOut) -> Fx<Value> {
    match result {
        ImOut::Num(n) => ok_num(n),
        ImOut::Z(z) => Ok(text_value(format_complex(&z))),
    }
}

fn unary(ctx: &CallCtx<'_>, args: &[Value], f: fn(Complex) -> Fx<ImOut>) -> Fx<Value> {
    show(f(complex_arg(ctx, &args[0])?)?)
}

fn fn_complex(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    let suffix = if args.len() > 2 && !is_blank(&args[2]) {
        str_of(ctx, &args[2])?
    } else {
        "i".to_owned()
    };
    if suffix != "i" && suffix != "j" {
        return Err(ErrorCode::Value);
    }
    show(ImOut::Z(make(
        num(ctx, &args[0])?,
        num(ctx, &args[1])?,
        suffix.as_bytes()[0],
    )))
}

fn fn_imreal(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    unary(ctx, args, |z| Ok(ImOut::Num(z.re)))
}
fn fn_imaginary(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    unary(ctx, args, |z| Ok(ImOut::Num(z.im)))
}
fn fn_imabs(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    unary(ctx, args, |z| Ok(ImOut::Num(modulus(z))))
}
fn fn_imargument(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    unary(ctx, args, |z| {
        if z.re == 0.0 && z.im == 0.0 {
            Err(ErrorCode::Div0)
        } else {
            Ok(ImOut::Num(argument(z)))
        }
    })
}
fn fn_imconjugate(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    unary(ctx, args, |z| Ok(ImOut::Z(make(z.re, -z.im, z.suffix))))
}
fn fn_imexp(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    unary(ctx, args, |z| Ok(ImOut::Z(exp_c(z))))
}
fn fn_imln(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    unary(ctx, args, |z| Ok(ImOut::Z(ln_c(z)?)))
}
fn fn_imlog10(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    unary(ctx, args, |z| {
        Ok(ImOut::Z(scale(ln_c(z)?, 1.0 / std::f64::consts::LN_10)))
    })
}
fn fn_imlog2(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    unary(ctx, args, |z| {
        Ok(ImOut::Z(scale(ln_c(z)?, 1.0 / std::f64::consts::LN_2)))
    })
}
fn fn_imsqrt(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    unary(ctx, args, |z| {
        Ok(ImOut::Z(polar(
            modulus(z).sqrt(),
            argument(z) / 2.0,
            z.suffix,
        )))
    })
}
fn fn_imsin(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    unary(ctx, args, |z| Ok(ImOut::Z(sin_c(z))))
}
fn fn_imcos(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    unary(ctx, args, |z| Ok(ImOut::Z(cos_c(z))))
}
fn fn_imtan(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    unary(ctx, args, |z| Ok(ImOut::Z(divide(sin_c(z), cos_c(z))?)))
}
fn fn_imsec(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    unary(ctx, args, |z| Ok(ImOut::Z(divide(one(z), cos_c(z))?)))
}
fn fn_imcsc(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    unary(ctx, args, |z| Ok(ImOut::Z(divide(one(z), sin_c(z))?)))
}
fn fn_imcot(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    unary(ctx, args, |z| Ok(ImOut::Z(divide(cos_c(z), sin_c(z))?)))
}
fn fn_imsinh(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    unary(ctx, args, |z| Ok(ImOut::Z(sinh_c(z))))
}
fn fn_imcosh(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    unary(ctx, args, |z| Ok(ImOut::Z(cosh_c(z))))
}
fn fn_imsech(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    unary(ctx, args, |z| Ok(ImOut::Z(divide(one(z), cosh_c(z))?)))
}
fn fn_imcsch(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    unary(ctx, args, |z| Ok(ImOut::Z(divide(one(z), sinh_c(z))?)))
}

fn fn_impower(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    let z = complex_arg(ctx, &args[0])?;
    let n = num(ctx, &args[1])?;
    if z.re == 0.0 && z.im == 0.0 {
        return if n > 0.0 {
            Ok(text_value("0".to_owned()))
        } else {
            Err(ErrorCode::Num)
        };
    }
    let result = if js_is_integer(n) && n.abs() <= 64.0 {
        integer_power(z, n)?
    } else {
        polar(modulus(z).powf(n), argument(z) * n, z.suffix)
    };
    show(ImOut::Z(result))
}

fn fn_imdiv(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    let a = complex_arg(ctx, &args[0])?;
    let b = complex_arg(ctx, &args[1])?;
    let mut quot = divide(a, b)?;
    quot.suffix = joined(a, b)?;
    show(ImOut::Z(quot))
}

fn collect_complex(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Vec<Complex>> {
    let mut out = Vec::new();
    let mut error = None;
    for arg in args {
        ctx.for_each(arg, &mut |value, _| {
            if error.is_some() {
                return;
            }
            match parse_complex(value) {
                Ok(z) => out.push(z),
                Err(code) => error = Some(code),
            }
        });
        if let Some(code) = error {
            return Err(code);
        }
    }
    Ok(out)
}

fn fold_complex(
    ctx: &CallCtx<'_>,
    args: &[Value],
    identity: Complex,
    step: fn(Complex, Complex) -> Fx<Complex>,
) -> Fx<Value> {
    let list = collect_complex(ctx, args)?;
    let mut acc = list.first().copied().unwrap_or(identity);
    for next in list.iter().skip(1) {
        acc = step(acc, *next)?;
    }
    show(ImOut::Z(acc))
}

fn fn_imsum(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    fold_complex(ctx, args, make(0.0, 0.0, b'i'), |a, b| {
        Ok(make(a.re + b.re, a.im + b.im, joined(a, b)?))
    })
}

fn fn_improduct(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    fold_complex(ctx, args, make(1.0, 0.0, b'i'), |a, b| {
        Ok(make(
            a.re * b.re - a.im * b.im,
            a.re * b.im + a.im * b.re,
            joined(a, b)?,
        ))
    })
}

fn fn_imsub(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    let a = complex_arg(ctx, &args[0])?;
    let b = complex_arg(ctx, &args[1])?;
    show(ImOut::Z(make(a.re - b.re, a.im - b.im, joined(a, b)?)))
}

fn bit_arg(ctx: &CallCtx<'_>, value: &Value) -> Fx<f64> {
    let n = num(ctx, value)?;
    if n < 0.0 || n >= TWO_48 || !js_is_integer(n) {
        Err(ErrorCode::Num)
    } else {
        Ok(n)
    }
}

fn bit_op(a: f64, b: f64, op: fn(i64, i64) -> i64) -> f64 {
    let hi = op((a / TWO_24).floor() as i64, (b / TWO_24).floor() as i64);
    let lo = op((a % TWO_24) as i64, (b % TWO_24) as i64);
    hi as f64 * TWO_24 + lo as f64
}

fn fn_bitand(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    ok_num(bit_op(
        bit_arg(ctx, &args[0])?,
        bit_arg(ctx, &args[1])?,
        |x, y| x & y,
    ))
}
fn fn_bitor(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    ok_num(bit_op(
        bit_arg(ctx, &args[0])?,
        bit_arg(ctx, &args[1])?,
        |x, y| x | y,
    ))
}
fn fn_bitxor(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    ok_num(bit_op(
        bit_arg(ctx, &args[0])?,
        bit_arg(ctx, &args[1])?,
        |x, y| x ^ y,
    ))
}
fn fn_bitlshift(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    let out = (bit_arg(ctx, &args[0])? * 2f64.powf(int_of(ctx, &args[1])?)).floor();
    if out >= TWO_48 {
        Err(ErrorCode::Num)
    } else {
        ok_num(out)
    }
}
fn fn_bitrshift(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    let out = (bit_arg(ctx, &args[0])? / 2f64.powf(int_of(ctx, &args[1])?)).floor();
    if out >= TWO_48 {
        Err(ErrorCode::Num)
    } else {
        ok_num(out)
    }
}

fn fn_delta(a: &[f64]) -> Fx<f64> {
    Ok(if a[0] == a.get(1).copied().unwrap_or(0.0) {
        1.0
    } else {
        0.0
    })
}

fn fn_gestep(a: &[f64]) -> Fx<f64> {
    Ok(if a[0] >= a.get(1).copied().unwrap_or(0.0) {
        1.0
    } else {
        0.0
    })
}

fn fn_erf(a: &[f64]) -> Fx<f64> {
    let lower = super::dist::erf(a[0])?;
    if a.len() < 2 {
        Ok(lower)
    } else {
        Ok(super::dist::erf(a[1])? - lower)
    }
}

fn fn_erf_precise(a: &[f64]) -> Fx<f64> {
    super::dist::erf(a[0])
}

fn fn_erfc(a: &[f64]) -> Fx<f64> {
    Ok(1.0 - super::dist::erf(a[0])?)
}

struct Unit {
    dimension: &'static str,
    factor: f64,
    power: i32,
    /// 0 none, 1 metric, 2 binary (binary units also accept metric prefixes).
    prefix: u8,
}

fn units() -> &'static HashMap<&'static str, Unit> {
    static MAP: OnceLock<HashMap<&'static str, Unit>> = OnceLock::new();
    MAP.get_or_init(|| {
        let mut map = HashMap::new();
        let mut add = |dimension: &'static str,
                       names: &[&'static str],
                       factor: f64,
                       power: i32,
                       prefix: u8| {
            for name in names {
                map.insert(
                    *name,
                    Unit {
                        dimension,
                        factor,
                        power,
                        prefix,
                    },
                );
            }
        };
        add("mass", &["g"], 1.0, 1, 1);
        add("mass", &["sg"], 14593.9029372064, 1, 0);
        add("mass", &["lbm"], 453.59237, 1, 0);
        add("mass", &["u"], 1.6605390666e-24, 1, 1);
        add("mass", &["ozm"], 28.349523125, 1, 0);
        add("mass", &["grain"], 0.06479891, 1, 0);
        add("mass", &["cwt", "shweight"], 45359.237, 1, 0);
        add("mass", &["uk_cwt", "lcwt", "hweight"], 50802.34544, 1, 0);
        add("mass", &["stone"], 6350.29318, 1, 0);
        add("mass", &["ton"], 907184.74, 1, 0);
        add("mass", &["uk_ton", "LTON", "brton"], 1016046.9088, 1, 0);
        add("distance", &["m"], 1.0, 1, 1);
        add("distance", &["mi"], 1609.344, 1, 0);
        add("distance", &["Nmi"], 1852.0, 1, 0);
        add("distance", &["in"], 0.0254, 1, 0);
        add("distance", &["ft"], 0.3048, 1, 0);
        add("distance", &["yd"], 0.9144, 1, 0);
        add("distance", &["ang"], 1e-10, 1, 1);
        add("distance", &["ell"], 1.143, 1, 0);
        add("distance", &["ly"], 9_460_730_472_580_800.0, 1, 0);
        add(
            "distance",
            &["parsec", "pc"],
            30_856_775_814_913_670.0,
            1,
            1,
        );
        add("distance", &["Pica"], 0.0254 / 72.0, 1, 0);
        add("distance", &["pica"], 0.0254 / 6.0, 1, 0);
        add("distance", &["survey_mi"], 1609.3472186944, 1, 0);
        add("time", &["yr"], 31_557_600.0, 1, 0);
        add("time", &["day", "d"], 86_400.0, 1, 0);
        add("time", &["hr"], 3600.0, 1, 0);
        add("time", &["mn", "min"], 60.0, 1, 0);
        add("time", &["sec", "s"], 1.0, 1, 1);
        add("pressure", &["Pa", "p"], 1.0, 1, 1);
        add("pressure", &["atm", "at"], 101_325.0, 1, 1);
        add("pressure", &["mmHg"], 133.322, 1, 1);
        add("pressure", &["psi"], 6894.757293168, 1, 0);
        add("pressure", &["Torr"], 101_325.0 / 760.0, 1, 0);
        add("force", &["N"], 1.0, 1, 1);
        add("force", &["dyn", "dy"], 1e-5, 1, 1);
        add("force", &["lbf"], 4.4482216152605, 1, 0);
        add("force", &["pond"], 0.00980665, 1, 1);
        add("energy", &["J"], 1.0, 1, 1);
        add("energy", &["e"], 1e-7, 1, 1);
        add("energy", &["c"], 4.184, 1, 1);
        add("energy", &["cal"], 4.1868, 1, 1);
        add("energy", &["eV", "ev"], 1.602176487e-19, 1, 1);
        add("energy", &["HPh", "hh"], 2_684_519.537696172, 1, 0);
        add("energy", &["Wh", "wh"], 3600.0, 1, 1);
        add("energy", &["flb"], 1.3558179483314, 1, 0);
        add("energy", &["BTU", "btu"], 1055.05585262, 1, 0);
        add("power", &["HP", "h"], 745.69987158227, 1, 0);
        add("power", &["PS"], 735.49875, 1, 0);
        add("power", &["W", "w"], 1.0, 1, 1);
        add("magnetism", &["T"], 1.0, 1, 1);
        add("magnetism", &["ga"], 1e-4, 1, 1);
        add("volume", &["tsp"], 4.92892159375e-6, 1, 0);
        add("volume", &["tspm"], 5e-6, 1, 0);
        add("volume", &["tbs"], 1.478676478125e-5, 1, 0);
        add("volume", &["oz"], 2.95735295625e-5, 1, 0);
        add("volume", &["cup"], 2.365882365e-4, 1, 0);
        add("volume", &["pt", "us_pt"], 4.73176473e-4, 1, 0);
        add("volume", &["uk_pt"], 5.6826125e-4, 1, 0);
        add("volume", &["qt"], 9.46352946e-4, 1, 0);
        add("volume", &["uk_qt"], 1.1365225e-3, 1, 0);
        add("volume", &["gal"], 3.785411784e-3, 1, 0);
        add("volume", &["uk_gal"], 4.54609e-3, 1, 0);
        add("volume", &["l", "L", "lt"], 1e-3, 1, 1);
        add("volume", &["m3", "m^3"], 1.0, 3, 1);
        add("volume", &["ang3", "ang^3"], 1e-30, 3, 1);
        add("volume", &["barrel"], 0.158987294928, 1, 0);
        add("volume", &["bushel"], 0.03523907016688, 1, 0);
        add("volume", &["regton", "GRT"], 2.8316846592, 1, 0);
        add("volume", &["MTON"], 1.13267386368, 1, 0);
        add("volume", &["in3", "in^3"], 1.6387064e-5, 1, 0);
        add("volume", &["ft3", "ft^3"], 0.028316846592, 1, 0);
        add("volume", &["yd3", "yd^3"], 0.764554857984, 1, 0);
        add("volume", &["mi3", "mi^3"], 4_168_181_825.4405794, 1, 0);
        add("volume", &["Nmi3", "Nmi^3"], 6_352_182_208.0, 1, 0);
        add("volume", &["ly3", "ly^3"], 8.46786664623715e47, 1, 0);
        add("area", &["m2", "m^2"], 1.0, 2, 1);
        add("area", &["ang2", "ang^2"], 1e-20, 2, 1);
        add("area", &["ar"], 100.0, 1, 1);
        add("area", &["ha"], 10_000.0, 1, 1);
        add("area", &["uk_acre"], 4046.8564224, 1, 0);
        add("area", &["us_acre"], 4046.87260987425, 1, 0);
        add("area", &["ft2", "ft^2"], 0.09290304, 1, 0);
        add("area", &["in2", "in^2"], 6.4516e-4, 1, 0);
        add("area", &["yd2", "yd^2"], 0.83612736, 1, 0);
        add("area", &["mi2", "mi^2"], 2_589_988.110336, 1, 0);
        add("area", &["Nmi2", "Nmi^2"], 3_429_904.0, 1, 0);
        add("area", &["ly2", "ly^2"], 8.95054210748189e31, 1, 0);
        add("area", &["Morgen"], 2500.0, 1, 0);
        add("information", &["bit"], 1.0, 1, 2);
        add("information", &["byte"], 8.0, 1, 2);
        add("speed", &["m/s", "m/sec"], 1.0, 1, 1);
        add("speed", &["m/h", "m/hr"], 1.0 / 3600.0, 1, 1);
        add("speed", &["mph"], 0.44704, 1, 0);
        add("speed", &["kn"], 1852.0 / 3600.0, 1, 0);
        add("speed", &["admkn"], 0.514773333333333, 1, 0);
        map
    })
}

fn metric(prefix: &str) -> Option<f64> {
    Some(match prefix {
        "Y" => 1e24,
        "Z" => 1e21,
        "E" => 1e18,
        "P" => 1e15,
        "T" => 1e12,
        "G" => 1e9,
        "M" => 1e6,
        "k" => 1e3,
        "h" => 1e2,
        "e" => 1e1,
        "d" => 1e-1,
        "c" => 1e-2,
        "m" => 1e-3,
        "u" => 1e-6,
        "n" => 1e-9,
        "p" => 1e-12,
        "f" => 1e-15,
        "a" => 1e-18,
        "z" => 1e-21,
        "y" => 1e-24,
        _ => return None,
    })
}

fn binary_prefix(prefix: &str) -> Option<f64> {
    Some(match prefix {
        "Yi" => 2f64.powi(80),
        "Zi" => 2f64.powi(70),
        "Ei" => 2f64.powi(60),
        "Pi" => 2f64.powi(50),
        "Ti" => 2f64.powi(40),
        "Gi" => 2f64.powi(30),
        "Mi" => 2f64.powi(20),
        "ki" => 2f64.powi(10),
        _ => return None,
    })
}

fn resolve_unit(name: &str) -> Fx<(&'static str, f64)> {
    let map = units();
    if let Some(unit) = map.get(name) {
        return Ok((unit.dimension, unit.factor));
    }
    for length in [2usize, 1] {
        if name.len() <= length {
            continue;
        }
        let (prefix, rest) = name.split_at(length);
        let Some(unit) = map.get(rest) else { continue };
        if unit.prefix == 0 {
            continue;
        }
        let scale = if unit.prefix == 2 {
            binary_prefix(prefix)
        } else {
            None
        }
        .or_else(|| metric(prefix));
        let Some(scale) = scale else { continue };
        return Ok((unit.dimension, unit.factor * scale.powi(unit.power)));
    }
    Err(ErrorCode::Na)
}

fn temp_key(name: &str) -> Option<&'static str> {
    Some(match name {
        "C" | "cel" => "C",
        "F" | "fah" => "F",
        "K" | "kel" => "K",
        "Rank" => "Rank",
        "Reau" => "Reau",
        _ => return None,
    })
}

fn to_kelvin(key: &str, value: f64) -> f64 {
    match key {
        "C" => value + 273.15,
        "F" => (value + 459.67) * 5.0 / 9.0,
        "Rank" => value * 5.0 / 9.0,
        "Reau" => value * 5.0 / 4.0 + 273.15,
        _ => value,
    }
}

fn from_kelvin(key: &str, kelvin: f64) -> f64 {
    match key {
        "C" => kelvin - 273.15,
        "F" => kelvin * 9.0 / 5.0 - 459.67,
        "Rank" => kelvin * 9.0 / 5.0,
        "Reau" => (kelvin - 273.15) * 4.0 / 5.0,
        _ => kelvin,
    }
}

fn convert_units(value: f64, from: &str, to: &str) -> Fx<f64> {
    match (temp_key(from), temp_key(to)) {
        (Some(from), Some(to)) => Ok(from_kelvin(to, to_kelvin(from, value))),
        (None, None) => {
            let (dim_a, factor_a) = resolve_unit(from)?;
            let (dim_b, factor_b) = resolve_unit(to)?;
            if dim_a != dim_b {
                return Err(ErrorCode::Na);
            }
            Ok((value * factor_a) / factor_b)
        }
        _ => Err(ErrorCode::Na),
    }
}

fn fn_convert(ctx: &CallCtx<'_>, args: &[Value]) -> Fx<Value> {
    ok_num(convert_units(
        num(ctx, &args[0])?,
        &str_of(ctx, &args[1])?,
        &str_of(ctx, &args[2])?,
    )?)
}

pub(super) fn specs() -> Vec<FunctionSpec> {
    vec![
        gen_spec(
            "BIN2DEC",
            C,
            "BIN2DEC(number)",
            "Converts a bin number to dec.",
            1,
            1,
            &[],
            false,
            fn_bin2dec,
        ),
        gen_spec(
            "BIN2OCT",
            C,
            "BIN2OCT(number, [places])",
            "Converts a bin number to oct.",
            1,
            2,
            &[],
            false,
            fn_bin2oct,
        ),
        gen_spec(
            "BIN2HEX",
            C,
            "BIN2HEX(number, [places])",
            "Converts a bin number to hex.",
            1,
            2,
            &[],
            false,
            fn_bin2hex,
        ),
        gen_spec(
            "OCT2BIN",
            C,
            "OCT2BIN(number, [places])",
            "Converts an oct number to bin.",
            1,
            2,
            &[],
            false,
            fn_oct2bin,
        ),
        gen_spec(
            "OCT2DEC",
            C,
            "OCT2DEC(number)",
            "Converts an oct number to dec.",
            1,
            1,
            &[],
            false,
            fn_oct2dec,
        ),
        gen_spec(
            "OCT2HEX",
            C,
            "OCT2HEX(number, [places])",
            "Converts an oct number to hex.",
            1,
            2,
            &[],
            false,
            fn_oct2hex,
        ),
        gen_spec(
            "DEC2BIN",
            C,
            "DEC2BIN(number, [places])",
            "Converts a dec number to bin.",
            1,
            2,
            &[],
            false,
            fn_dec2bin,
        ),
        gen_spec(
            "DEC2OCT",
            C,
            "DEC2OCT(number, [places])",
            "Converts a dec number to oct.",
            1,
            2,
            &[],
            false,
            fn_dec2oct,
        ),
        gen_spec(
            "DEC2HEX",
            C,
            "DEC2HEX(number, [places])",
            "Converts a dec number to hex.",
            1,
            2,
            &[],
            false,
            fn_dec2hex,
        ),
        gen_spec(
            "HEX2BIN",
            C,
            "HEX2BIN(number, [places])",
            "Converts a hex number to bin.",
            1,
            2,
            &[],
            false,
            fn_hex2bin,
        ),
        gen_spec(
            "HEX2OCT",
            C,
            "HEX2OCT(number, [places])",
            "Converts a hex number to oct.",
            1,
            2,
            &[],
            false,
            fn_hex2oct,
        ),
        gen_spec(
            "HEX2DEC",
            C,
            "HEX2DEC(number)",
            "Converts a hex number to dec.",
            1,
            1,
            &[],
            false,
            fn_hex2dec,
        ),
        gen_spec(
            "COMPLEX",
            C,
            "COMPLEX(real_num, i_num, [suffix])",
            "Builds a complex number from its real and imaginary coefficients.",
            2,
            3,
            &[],
            false,
            fn_complex,
        ),
        gen_spec(
            "IMREAL",
            C,
            "IMREAL(inumber)",
            "The real coefficient of a complex number.",
            1,
            1,
            &[],
            false,
            fn_imreal,
        ),
        gen_spec(
            "IMAGINARY",
            C,
            "IMAGINARY(inumber)",
            "The imaginary coefficient of a complex number.",
            1,
            1,
            &[],
            false,
            fn_imaginary,
        ),
        gen_spec(
            "IMABS",
            C,
            "IMABS(inumber)",
            "The absolute value (modulus) of a complex number.",
            1,
            1,
            &[],
            false,
            fn_imabs,
        ),
        gen_spec(
            "IMARGUMENT",
            C,
            "IMARGUMENT(inumber)",
            "The argument theta, an angle in radians.",
            1,
            1,
            &[],
            false,
            fn_imargument,
        ),
        gen_spec(
            "IMCONJUGATE",
            C,
            "IMCONJUGATE(inumber)",
            "The complex conjugate.",
            1,
            1,
            &[],
            false,
            fn_imconjugate,
        ),
        gen_spec(
            "IMEXP",
            C,
            "IMEXP(inumber)",
            "The exponential of a complex number.",
            1,
            1,
            &[],
            false,
            fn_imexp,
        ),
        gen_spec(
            "IMLN",
            C,
            "IMLN(inumber)",
            "The natural logarithm of a complex number.",
            1,
            1,
            &[],
            false,
            fn_imln,
        ),
        gen_spec(
            "IMLOG10",
            C,
            "IMLOG10(inumber)",
            "The base-10 logarithm of a complex number.",
            1,
            1,
            &[],
            false,
            fn_imlog10,
        ),
        gen_spec(
            "IMLOG2",
            C,
            "IMLOG2(inumber)",
            "The base-2 logarithm of a complex number.",
            1,
            1,
            &[],
            false,
            fn_imlog2,
        ),
        gen_spec(
            "IMSQRT",
            C,
            "IMSQRT(inumber)",
            "The square root of a complex number.",
            1,
            1,
            &[],
            false,
            fn_imsqrt,
        ),
        gen_spec(
            "IMSIN",
            C,
            "IMSIN(inumber)",
            "The sine of a complex number.",
            1,
            1,
            &[],
            false,
            fn_imsin,
        ),
        gen_spec(
            "IMCOS",
            C,
            "IMCOS(inumber)",
            "The cosine of a complex number.",
            1,
            1,
            &[],
            false,
            fn_imcos,
        ),
        gen_spec(
            "IMTAN",
            C,
            "IMTAN(inumber)",
            "The tangent of a complex number.",
            1,
            1,
            &[],
            false,
            fn_imtan,
        ),
        gen_spec(
            "IMSEC",
            C,
            "IMSEC(inumber)",
            "The secant of a complex number.",
            1,
            1,
            &[],
            false,
            fn_imsec,
        ),
        gen_spec(
            "IMCSC",
            C,
            "IMCSC(inumber)",
            "The cosecant of a complex number.",
            1,
            1,
            &[],
            false,
            fn_imcsc,
        ),
        gen_spec(
            "IMCOT",
            C,
            "IMCOT(inumber)",
            "The cotangent of a complex number.",
            1,
            1,
            &[],
            false,
            fn_imcot,
        ),
        gen_spec(
            "IMSINH",
            C,
            "IMSINH(inumber)",
            "The hyperbolic sine of a complex number.",
            1,
            1,
            &[],
            false,
            fn_imsinh,
        ),
        gen_spec(
            "IMCOSH",
            C,
            "IMCOSH(inumber)",
            "The hyperbolic cosine of a complex number.",
            1,
            1,
            &[],
            false,
            fn_imcosh,
        ),
        gen_spec(
            "IMSECH",
            C,
            "IMSECH(inumber)",
            "The hyperbolic secant of a complex number.",
            1,
            1,
            &[],
            false,
            fn_imsech,
        ),
        gen_spec(
            "IMCSCH",
            C,
            "IMCSCH(inumber)",
            "The hyperbolic cosecant of a complex number.",
            1,
            1,
            &[],
            false,
            fn_imcsch,
        ),
        gen_spec(
            "IMPOWER",
            C,
            "IMPOWER(inumber, number)",
            "A complex number raised to a power.",
            2,
            2,
            &[],
            false,
            fn_impower,
        ),
        gen_spec(
            "IMDIV",
            C,
            "IMDIV(inumber1, inumber2)",
            "The quotient of two complex numbers.",
            2,
            2,
            &[],
            false,
            fn_imdiv,
        ),
        gen_spec(
            "IMSUM",
            C,
            "IMSUM(inumber1, [inumber2], ...)",
            "The sum of complex numbers.",
            1,
            255,
            ANY,
            false,
            fn_imsum,
        ),
        gen_spec(
            "IMPRODUCT",
            C,
            "IMPRODUCT(inumber1, [inumber2], ...)",
            "The product of complex numbers.",
            1,
            255,
            ANY,
            false,
            fn_improduct,
        ),
        gen_spec(
            "IMSUB",
            C,
            "IMSUB(inumber1, inumber2)",
            "The difference of two complex numbers.",
            2,
            2,
            &[],
            false,
            fn_imsub,
        ),
        gen_spec(
            "BITAND",
            C,
            "BITAND(number1, number2)",
            "Bitwise AND.",
            2,
            2,
            &[],
            false,
            fn_bitand,
        ),
        gen_spec(
            "BITOR",
            C,
            "BITOR(number1, number2)",
            "Bitwise OR.",
            2,
            2,
            &[],
            false,
            fn_bitor,
        ),
        gen_spec(
            "BITXOR",
            C,
            "BITXOR(number1, number2)",
            "Bitwise XOR.",
            2,
            2,
            &[],
            false,
            fn_bitxor,
        ),
        gen_spec(
            "BITLSHIFT",
            C,
            "BITLSHIFT(number, shift_amount)",
            "Shifts bits left.",
            2,
            2,
            &[],
            false,
            fn_bitlshift,
        ),
        gen_spec(
            "BITRSHIFT",
            C,
            "BITRSHIFT(number, shift_amount)",
            "Shifts bits right.",
            2,
            2,
            &[],
            false,
            fn_bitrshift,
        ),
        num_spec(
            "DELTA",
            C,
            "DELTA(number1, [number2])",
            "1 when two numbers are equal, else 0.",
            1,
            2,
            ZERO,
            fn_delta,
        ),
        num_spec(
            "GESTEP",
            C,
            "GESTEP(number, [step])",
            "1 when a number is at least step, else 0.",
            1,
            2,
            ZERO,
            fn_gestep,
        ),
        gen_spec(
            "CONVERT",
            C,
            "CONVERT(number, from_unit, to_unit)",
            "Converts a number from one measurement system to another.",
            3,
            3,
            &[],
            false,
            fn_convert,
        ),
        num_spec(
            "ERF",
            C,
            "ERF(lower_limit, [upper_limit])",
            "The error function.",
            1,
            2,
            &[],
            fn_erf,
        ),
        num_spec(
            "ERF.PRECISE",
            C,
            "ERF.PRECISE(x)",
            "The error function.",
            1,
            1,
            &[],
            fn_erf_precise,
        ),
        num_spec(
            "ERFC",
            C,
            "ERFC(x)",
            "The complementary error function.",
            1,
            1,
            &[],
            fn_erfc,
        ),
        num_spec(
            "ERFC.PRECISE",
            C,
            "ERFC.PRECISE(x)",
            "The complementary error function.",
            1,
            1,
            &[],
            fn_erfc,
        ),
    ]
}
