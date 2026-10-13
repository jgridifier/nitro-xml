// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/xlsx/read/formula-text.ts
// (stripFuturePrefixes, addFuturePrefixes); changes listed in PROVENANCE.md.
//! Formula text normalization. The model keeps plain names; the writer adds `_xlfn.` prefixes.

/// Removes `_xlfn.` / `_xlws.` function prefixes (and turns `ANCHORARRAY(A1)` into `A1#`).
pub fn strip_future_prefixes(formula: &str) -> String {
    if !formula.contains("_xl") {
        return formula.to_owned();
    }
    let plain = rewrite_formula(formula, |token, next, out| {
        if !next.starts_with('(') {
            return false;
        }
        let stripped = strip_xl_prefix(token);
        if stripped == token {
            return false;
        }
        out.push_str(stripped);
        true
    });
    from_stored_syntax(&plain)
}

/// Post-2007 functions. Excel stores them with an `_xlfn.` prefix (`FILTER` and `SORT`
/// with `_xlfn._xlws.`). Names are uppercase and sorted for binary search.
const FUTURE_FUNCTIONS: &[&str] = &[
    "ACOT",
    "ACOTH",
    "AGGREGATE",
    "ANCHORARRAY",
    "ARABIC",
    "ARRAYTOTEXT",
    "BASE",
    "BETA.DIST",
    "BETA.INV",
    "BINOM.DIST",
    "BINOM.DIST.RANGE",
    "BINOM.INV",
    "BITAND",
    "BITLSHIFT",
    "BITOR",
    "BITRSHIFT",
    "BITXOR",
    "BYCOL",
    "BYROW",
    "CEILING.MATH",
    "CEILING.PRECISE",
    "CHISQ.DIST",
    "CHISQ.DIST.RT",
    "CHISQ.INV",
    "CHISQ.INV.RT",
    "CHISQ.TEST",
    "CHOOSECOLS",
    "CHOOSEROWS",
    "COMBINA",
    "CONCAT",
    "CONFIDENCE.NORM",
    "CONFIDENCE.T",
    "COT",
    "COTH",
    "COVARIANCE.P",
    "COVARIANCE.S",
    "CSC",
    "CSCH",
    "DAYS",
    "DECIMAL",
    "DROP",
    "ENCODEURL",
    "ERF.PRECISE",
    "ERFC.PRECISE",
    "EXPAND",
    "EXPON.DIST",
    "F.DIST",
    "F.DIST.RT",
    "F.INV",
    "F.INV.RT",
    "F.TEST",
    "FIELDVALUE",
    "FILTER",
    "FILTERXML",
    "FLOOR.MATH",
    "FLOOR.PRECISE",
    "FORECAST.ETS",
    "FORECAST.ETS.CONFINT",
    "FORECAST.ETS.SEASONALITY",
    "FORECAST.ETS.STAT",
    "FORECAST.LINEAR",
    "FORMULATEXT",
    "GAMMA",
    "GAMMA.DIST",
    "GAMMA.INV",
    "GAMMALN.PRECISE",
    "GAUSS",
    "GROUPBY",
    "HSTACK",
    "HYPGEOM.DIST",
    "IFNA",
    "IFS",
    "IMAGE",
    "IMCOSH",
    "IMCOT",
    "IMCSC",
    "IMCSCH",
    "IMSEC",
    "IMSECH",
    "IMSINH",
    "IMTAN",
    "ISFORMULA",
    "ISOMITTED",
    "ISOWEEKNUM",
    "LAMBDA",
    "LET",
    "LOGNORM.DIST",
    "LOGNORM.INV",
    "MAKEARRAY",
    "MAP",
    "MAXIFS",
    "MINIFS",
    "MODE.MULT",
    "MODE.SNGL",
    "MUNIT",
    "NEGBINOM.DIST",
    "NETWORKDAYS.INTL",
    "NORM.DIST",
    "NORM.INV",
    "NORM.S.DIST",
    "NORM.S.INV",
    "NUMBERVALUE",
    "PDURATION",
    "PERCENTILE.EXC",
    "PERCENTILE.INC",
    "PERCENTOF",
    "PERCENTRANK.EXC",
    "PERCENTRANK.INC",
    "PERMUTATIONA",
    "PHI",
    "PIVOTBY",
    "POISSON.DIST",
    "QUARTILE.EXC",
    "QUARTILE.INC",
    "RANDARRAY",
    "RANK.AVG",
    "RANK.EQ",
    "REDUCE",
    "REGEXEXTRACT",
    "REGEXREPLACE",
    "REGEXTEST",
    "RRI",
    "SCAN",
    "SEC",
    "SECH",
    "SEQUENCE",
    "SHEET",
    "SHEETS",
    "SINGLE",
    "SKEW.P",
    "SORT",
    "SORTBY",
    "STDEV.P",
    "STDEV.S",
    "STOCKHISTORY",
    "SWITCH",
    "T.DIST",
    "T.DIST.2T",
    "T.DIST.RT",
    "T.INV",
    "T.INV.2T",
    "T.TEST",
    "TAKE",
    "TEXTAFTER",
    "TEXTBEFORE",
    "TEXTJOIN",
    "TEXTSPLIT",
    "TOCOL",
    "TOROW",
    "TRIMRANGE",
    "UNICHAR",
    "UNICODE",
    "UNIQUE",
    "VALUETOTEXT",
    "VAR.P",
    "VAR.S",
    "VSTACK",
    "WEBSERVICE",
    "WEIBULL.DIST",
    "WORKDAY.INTL",
    "WRAPCOLS",
    "WRAPROWS",
    "XLOOKUP",
    "XMATCH",
    "XOR",
];

/// Adds `_xlfn.` prefixes to post-2007 functions, as Excel stores them.
///
/// `FILTER` and `SORT` are written `_xlfn._xlws.`. Spill (`A1#`) and implicit
/// intersection (`@`) stay as typed; upstream `toStoredSyntax` is not applied here.
pub fn add_future_prefixes(formula: &str) -> String {
    rewrite_formula(formula, |token, next, out| {
        if !next.starts_with('(') {
            return false;
        }
        let upper = token.to_ascii_uppercase();
        if FUTURE_FUNCTIONS.binary_search(&upper.as_str()).is_err() {
            return false;
        }
        if upper == "FILTER" || upper == "SORT" {
            out.push_str("_xlfn._xlws.");
        } else {
            out.push_str("_xlfn.");
        }
        out.push_str(token);
        true
    })
}

fn strip_xl_prefix(token: &str) -> &str {
    let mut s = token;
    if s.len() >= 6 && s[..6].eq_ignore_ascii_case("_xlfn.") {
        s = &s[6..];
    }
    if s.len() >= 6 && s[..6].eq_ignore_ascii_case("_xlws.") {
        s = &s[6..];
    }
    s
}

fn rewrite_formula(
    formula: &str,
    mut visit: impl FnMut(&str, &str, &mut String) -> bool,
) -> String {
    let chars: Vec<char> = formula.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < chars.len() {
        let ch = chars[i];
        if ch == '"' || ch == '\'' {
            let quote = ch;
            let mut j = i + 1;
            while j < chars.len() {
                if chars[j] == quote {
                    if j + 1 < chars.len() && chars[j + 1] == quote {
                        j += 2;
                        continue;
                    }
                    j += 1;
                    break;
                }
                j += 1;
            }
            out.extend(chars[i..j].iter());
            i = j;
            continue;
        }
        if ch == '[' {
            let mut depth = 0i32;
            let mut j = i;
            while j < chars.len() {
                if chars[j] == '[' {
                    depth += 1;
                } else if chars[j] == ']' {
                    depth -= 1;
                    if depth == 0 {
                        j += 1;
                        break;
                    }
                }
                j += 1;
            }
            out.extend(chars[i..j].iter());
            i = j;
            continue;
        }
        if ch.is_ascii_alphanumeric() || matches!(ch, '_' | '$' | '\\' | '.') {
            let mut j = i;
            while j < chars.len() {
                let c = chars[j];
                if c.is_ascii_alphanumeric() || matches!(c, '_' | '$' | '\\' | '.' | '?') {
                    j += 1;
                } else {
                    break;
                }
            }
            let token: String = chars[i..j].iter().collect();
            let next: String = chars[j..].iter().collect();
            if !visit(&token, &next, &mut out) {
                out.push_str(&token);
            }
            i = j;
            continue;
        }
        out.push(ch);
        i += 1;
    }
    out
}

/// Minimal `ANCHORARRAY(x)` → `x#` and `SINGLE(x)` → `@x`.
fn from_stored_syntax(formula: &str) -> String {
    let upper = formula.to_ascii_uppercase();
    if !upper.contains("ANCHORARRAY") && !upper.contains("SINGLE") {
        return formula.to_owned();
    }
    replace_call(formula, "ANCHORARRAY", |inner| format!("{inner}#"))
        .pipe(|s| replace_call(&s, "SINGLE", |inner| format!("@{inner}")))
}

trait Pipe: Sized {
    fn pipe<R>(self, f: impl FnOnce(Self) -> R) -> R;
}
impl<T> Pipe for T {
    fn pipe<R>(self, f: impl FnOnce(Self) -> R) -> R {
        f(self)
    }
}

fn replace_call(formula: &str, name: &str, rewrite: impl Fn(&str) -> String) -> String {
    let chars: Vec<char> = formula.chars().collect();
    let name_chars: Vec<char> = name.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < chars.len() {
        if i + name_chars.len() < chars.len()
            && chars[i..i + name_chars.len()]
                .iter()
                .zip(name_chars.iter())
                .all(|(a, b)| a.eq_ignore_ascii_case(b))
            && chars.get(i + name_chars.len()) == Some(&'(')
        {
            let start_inner = i + name_chars.len() + 1;
            let mut depth = 1i32;
            let mut j = start_inner;
            while j < chars.len() && depth > 0 {
                if chars[j] == '(' {
                    depth += 1;
                } else if chars[j] == ')' {
                    depth -= 1;
                }
                if depth > 0 {
                    j += 1;
                }
            }
            if depth == 0 {
                let inner: String = chars[start_inner..j].iter().collect();
                out.push_str(&rewrite(&inner));
                i = j + 1;
                continue;
            }
        }
        out.push(chars[i]);
        i += 1;
    }
    out
}
