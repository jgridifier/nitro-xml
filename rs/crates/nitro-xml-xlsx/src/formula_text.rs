// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/xlsx/read/formula-text.ts (stripFuturePrefixes);
// changes listed in PROVENANCE.md.
//! Formula text normalization on read.

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
