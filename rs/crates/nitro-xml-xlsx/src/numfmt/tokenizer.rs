// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/xlsx/numfmt/tokenizer.ts; changes listed in PROVENANCE.md.

use crate::numfmt::types::{
    AmpmKind, Condition, ConditionOp, DatePart, Section, SectionKind, Token,
};

const NAMED_COLORS: &[(&str, &str)] = &[
    ("black", "#000000"),
    ("white", "#FFFFFF"),
    ("red", "#FF0000"),
    ("green", "#00FF00"),
    ("blue", "#0000FF"),
    ("yellow", "#FFFF00"),
    ("magenta", "#FF00FF"),
    ("cyan", "#00FFFF"),
];

/// The default 56-colour palette `[Color1]`..`[Color56]` refer to.
const PALETTE_56: [&str; 56] = [
    "000000", "FFFFFF", "FF0000", "00FF00", "0000FF", "FFFF00", "FF00FF", "00FFFF", "800000",
    "008000", "000080", "808000", "800080", "008080", "C0C0C0", "808080", "9999FF", "993366",
    "FFFFCC", "CCFFFF", "660066", "FF8080", "0066CC", "CCCCFF", "000080", "FF00FF", "FFFF00",
    "00FFFF", "800080", "800000", "008080", "0000FF", "00CCFF", "CCFFFF", "CCFFCC", "FFFF99",
    "99CCFF", "FF99CC", "CC99FF", "FFCC99", "3366FF", "33CCCC", "99CC00", "FFCC00", "FF9900",
    "FF6600", "666699", "969696", "003366", "339966", "003300", "333300", "993300", "993366",
    "333399", "333333",
];

/// Splits a format code into its `;` separated sections (quotes, escapes and brackets respected).
pub(crate) fn split_sections(format: &str) -> Vec<String> {
    let cs: Vec<char> = format.chars().collect();
    let mut sections = Vec::new();
    let mut current = String::new();
    let mut i = 0;
    while i < cs.len() {
        let ch = cs[i];
        if ch == '"' {
            let end = find_char(&cs, '"', i + 1);
            let stop = end.unwrap_or(cs.len());
            current.extend(cs[i..stop].iter().copied());
            if end.is_some() {
                current.push('"');
                i = stop + 1;
            } else {
                i = stop;
            }
        } else if ch == '\\' || ch == '_' || ch == '*' {
            current.push(ch);
            if i + 1 < cs.len() {
                current.push(cs[i + 1]);
            }
            i += 2;
        } else if ch == '[' {
            let end = find_char(&cs, ']', i + 1);
            let stop = end.unwrap_or(cs.len());
            current.extend(cs[i..stop].iter().copied());
            if end.is_some() {
                current.push(']');
                i = stop + 1;
            } else {
                i = stop;
            }
        } else if ch == ';' {
            sections.push(std::mem::take(&mut current));
            i += 1;
        } else {
            current.push(ch);
            i += 1;
        }
    }
    sections.push(current);
    sections
}

fn find_char(cs: &[char], ch: char, from: usize) -> Option<usize> {
    cs[from..].iter().position(|&c| c == ch).map(|p| p + from)
}

struct Bracket {
    color: Option<String>,
    condition: Option<Condition>,
    token: Option<Token>,
}

fn parse_bracket(content: &str) -> Bracket {
    let lower = content.to_ascii_lowercase();
    if let Some((_, hex)) = NAMED_COLORS.iter().find(|(name, _)| *name == lower) {
        return Bracket {
            color: Some((*hex).to_string()),
            condition: None,
            token: None,
        };
    }
    if let Some(color) = indexed_color(&lower) {
        return Bracket {
            color: Some(color),
            condition: None,
            token: None,
        };
    }
    if let Some(condition) = parse_condition(content) {
        return Bracket {
            color: None,
            condition: Some(condition),
            token: None,
        };
    }
    if let Some(token) = parse_elapsed(content) {
        return Bracket {
            color: None,
            condition: None,
            token: Some(token),
        };
    }
    if let Some(rest) = content.strip_prefix('$') {
        let symbol = match rest.find('-') {
            Some(dash) => &rest[..dash],
            None => rest,
        };
        if !symbol.is_empty() {
            return Bracket {
                color: None,
                condition: None,
                token: Some(Token::Lit(symbol.to_string())),
            };
        }
    }
    Bracket {
        color: None,
        condition: None,
        token: None,
    }
}

fn indexed_color(lower: &str) -> Option<String> {
    let rest = lower.strip_prefix("color")?;
    let rest = rest.trim_start_matches(char::is_whitespace);
    if rest.is_empty() || !rest.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let n: usize = rest.parse().ok()?;
    let hex = PALETTE_56.get(n.checked_sub(1)?)?;
    Some(format!("#{hex}"))
}

fn parse_condition(content: &str) -> Option<Condition> {
    let cs: Vec<char> = content.chars().collect();
    if cs.is_empty() {
        return None;
    }
    let (op, mut i) = if content.starts_with("<=") {
        (ConditionOp::Le, 2)
    } else if content.starts_with(">=") {
        (ConditionOp::Ge, 2)
    } else if content.starts_with("<>") {
        (ConditionOp::Ne, 2)
    } else if content.starts_with('<') {
        (ConditionOp::Lt, 1)
    } else if content.starts_with('>') {
        (ConditionOp::Gt, 1)
    } else if content.starts_with('=') {
        (ConditionOp::Eq, 1)
    } else {
        return None;
    };
    while i < cs.len() && cs[i].is_whitespace() {
        i += 1;
    }
    let rest: String = cs[i..].iter().collect();
    if !is_number_token(&rest) {
        return None;
    }
    let value = rest.parse::<f64>().ok()?;
    Some(Condition { op, value })
}

fn is_number_token(s: &str) -> bool {
    let mut cs = s.chars().peekable();
    if matches!(cs.peek(), Some('+' | '-')) {
        cs.next();
    }
    if cs.peek() == Some(&'.') {
        cs.next();
        let mut frac = false;
        while matches!(cs.peek(), Some(c) if c.is_ascii_digit()) {
            cs.next();
            frac = true;
        }
        if !frac {
            return false;
        }
    } else {
        let mut saw = false;
        while matches!(cs.peek(), Some(c) if c.is_ascii_digit()) {
            cs.next();
            saw = true;
        }
        if !saw {
            return false;
        }
        if cs.peek() == Some(&'.') {
            cs.next();
            while matches!(cs.peek(), Some(c) if c.is_ascii_digit()) {
                cs.next();
            }
        }
    }
    if matches!(cs.peek(), Some('e' | 'E')) {
        cs.next();
        if matches!(cs.peek(), Some('+' | '-')) {
            cs.next();
        }
        let mut exp = false;
        while matches!(cs.peek(), Some(c) if c.is_ascii_digit()) {
            cs.next();
            exp = true;
        }
        if !exp {
            return false;
        }
    }
    cs.next().is_none()
}

fn parse_elapsed(content: &str) -> Option<Token> {
    let lower: String = content.chars().map(|c| c.to_ascii_lowercase()).collect();
    let unit = lower.chars().next()?;
    if !matches!(unit, 'h' | 'm' | 's') || !lower.chars().all(|c| c == unit) {
        return None;
    }
    Some(Token::Elapsed {
        unit,
        width: lower.chars().count(),
    })
}

fn run_length(cs: &[char], i: usize, lower_ch: char) -> usize {
    let mut n = 0;
    while i + n < cs.len() && cs[i + n].to_ascii_lowercase() == lower_ch {
        n += 1;
    }
    n
}

fn date_part_for(ch: char, n: usize) -> DatePart {
    match ch {
        'y' => {
            if n <= 2 {
                DatePart::Y
            } else {
                DatePart::Yyyy
            }
        }
        'b' => {
            if n <= 2 {
                DatePart::B
            } else {
                DatePart::Bbbb
            }
        }
        'm' => match n {
            1 => DatePart::M,
            2 => DatePart::Mm,
            3 => DatePart::Mmm,
            4 => DatePart::Mmmm,
            5 => DatePart::Mmmmm,
            _ => DatePart::Mmmm,
        },
        'd' => match n.min(4) {
            1 => DatePart::D,
            2 => DatePart::Dd,
            3 => DatePart::Ddd,
            _ => DatePart::Dddd,
        },
        'h' => {
            if n == 1 {
                DatePart::H
            } else {
                DatePart::Hh
            }
        }
        _ => {
            if n == 1 {
                DatePart::S
            } else {
                DatePart::Ss
            }
        }
    }
}

fn slice_eq_ignore(cs: &[char], i: usize, lit: &str) -> bool {
    let lit: Vec<char> = lit.chars().collect();
    if i + lit.len() > cs.len() {
        return false;
    }
    cs[i..i + lit.len()]
        .iter()
        .zip(lit.iter())
        .all(|(a, b)| a.eq_ignore_ascii_case(b))
}

fn push_lit(tokens: &mut Vec<Token>, v: &str) {
    if let Some(Token::Lit(existing)) = tokens.last_mut() {
        existing.push_str(v);
    } else if !v.is_empty() {
        tokens.push(Token::Lit(v.to_string()));
    }
}

/// Tokenizes one section and extracts its colour and condition.
pub(crate) fn tokenize_section(src: &str) -> Section {
    let cs: Vec<char> = src.chars().collect();
    let mut tokens = Vec::new();
    let mut color = None;
    let mut condition = None;
    let mut i = 0;
    while i < cs.len() {
        let ch = cs[i];
        let lower = ch.to_ascii_lowercase();
        if ch == '"' {
            let end = find_char(&cs, '"', i + 1);
            let stop = end.unwrap_or(cs.len());
            let lit: String = cs[i + 1..stop].iter().collect();
            push_lit(&mut tokens, &lit);
            i = stop + 1;
        } else if ch == '\\' {
            if i + 1 < cs.len() {
                push_lit(&mut tokens, &cs[i + 1].to_string());
            }
            i += 2;
        } else if ch == '_' {
            push_lit(&mut tokens, " ");
            i += 2;
        } else if ch == '*' {
            i += 2;
        } else if ch == '[' {
            let end = find_char(&cs, ']', i + 1);
            let stop = end.unwrap_or(cs.len());
            let inner: String = cs[i + 1..stop].iter().collect();
            let res = parse_bracket(&inner);
            if res.color.is_some() {
                color = res.color;
            }
            if res.condition.is_some() {
                condition = res.condition;
            }
            if let Some(Token::Lit(v)) = res.token {
                push_lit(&mut tokens, &v);
            } else if let Some(tok) = res.token {
                tokens.push(tok);
            }
            i = stop + 1;
        } else if ch == '0' || ch == '#' || ch == '?' {
            tokens.push(Token::Digit(ch));
            i += 1;
        } else if ('1'..='9').contains(&ch) {
            tokens.push(Token::Num(ch.to_string()));
            i += 1;
        } else if ch == '.' {
            tokens.push(Token::Point);
            i += 1;
        } else if ch == ',' {
            tokens.push(Token::Comma);
            i += 1;
        } else if ch == '%' {
            tokens.push(Token::Percent);
            i += 1;
        } else if ch == '/' {
            tokens.push(Token::Slash);
            i += 1;
        } else if ch == '@' {
            tokens.push(Token::Text);
            i += 1;
        } else if lower == 'e' && matches!(cs.get(i + 1), Some('+' | '-')) {
            tokens.push(Token::Exp {
                plus: cs[i + 1] == '+',
            });
            i += 2;
        } else if lower == 'g' && slice_eq_ignore(&cs, i, "general") {
            tokens.push(Token::General);
            i += 7;
        } else if lower == 'a' && slice_eq_ignore(&cs, i, "am/pm") {
            tokens.push(Token::Ampm(AmpmKind::AmPm));
            i += 5;
        } else if lower == 'a' && slice_eq_ignore(&cs, i, "a/p") {
            tokens.push(Token::Ampm(if ch == 'A' {
                AmpmKind::Upper
            } else {
                AmpmKind::Lower
            }));
            i += 3;
        } else if lower == 'e' || lower == 'g' {
            let n = run_length(&cs, i, lower);
            tokens.push(Token::Date(if lower == 'e' {
                DatePart::E
            } else {
                DatePart::G
            }));
            i += n;
        } else if "ymdhsb".contains(lower) {
            let n = run_length(&cs, i, lower);
            tokens.push(Token::Date(date_part_for(lower, n)));
            i += n;
        } else {
            push_lit(&mut tokens, &ch.to_string());
            i += 1;
        }
    }
    let kind = classify(&tokens);
    Section {
        tokens: if kind == SectionKind::Date {
            normalize_date_tokens(tokens)
        } else {
            tokens
        },
        kind,
        color,
        condition,
    }
}

fn classify(tokens: &[Token]) -> SectionKind {
    if tokens.iter().any(|t| matches!(t, Token::Text)) {
        SectionKind::Text
    } else if tokens
        .iter()
        .any(|t| matches!(t, Token::Date(_) | Token::Elapsed { .. } | Token::Ampm(_)))
    {
        SectionKind::Date
    } else if tokens.iter().any(|t| matches!(t, Token::General)) {
        SectionKind::General
    } else {
        SectionKind::Number
    }
}

fn is_hour(t: &Token) -> bool {
    matches!(t, Token::Date(DatePart::H | DatePart::Hh))
        || matches!(t, Token::Elapsed { unit: 'h', .. })
}

fn is_second(t: &Token) -> bool {
    matches!(t, Token::Date(DatePart::S | DatePart::Ss))
        || matches!(t, Token::Elapsed { unit: 's', .. })
}

/// Resolves month/minute ambiguity, sub-second digits, and turns number-only tokens into literals.
fn normalize_date_tokens(tokens: Vec<Token>) -> Vec<Token> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < tokens.len() {
        match &tokens[i] {
            Token::Point => {
                let mut n = 0;
                while matches!(tokens.get(i + 1 + n), Some(Token::Digit('0'))) {
                    n += 1;
                }
                if n > 0 {
                    out.push(Token::Subsec { digits: n });
                    i += n;
                } else {
                    push_lit(&mut out, ".");
                }
            }
            Token::Digit(c) => push_lit(&mut out, &c.to_string()),
            Token::Num(v) => push_lit(&mut out, v),
            Token::Comma => push_lit(&mut out, ","),
            Token::Percent => push_lit(&mut out, "%"),
            Token::Slash => push_lit(&mut out, "/"),
            Token::Exp { plus } => push_lit(&mut out, if *plus { "E+" } else { "E-" }),
            Token::General => push_lit(&mut out, "General"),
            Token::Lit(v) => push_lit(&mut out, v),
            other => out.push(other.clone()),
        }
        i += 1;
    }
    let time_idx: Vec<usize> = out
        .iter()
        .enumerate()
        .filter_map(|(i, t)| matches!(t, Token::Date(_) | Token::Elapsed { .. }).then_some(i))
        .collect();
    for k in 0..time_idx.len() {
        let part = match &out[time_idx[k]] {
            Token::Date(p) => *p,
            _ => continue,
        };
        if part != DatePart::M && part != DatePart::Mm {
            continue;
        }
        let prev_hour = k > 0 && is_hour(&out[time_idx[k - 1]]);
        let next_sec = k + 1 < time_idx.len() && is_second(&out[time_idx[k + 1]]);
        if prev_hour || next_sec {
            if let Token::Date(p) = &mut out[time_idx[k]] {
                *p = if part == DatePart::M {
                    DatePart::Min
                } else {
                    DatePart::Mmin
                };
            }
        }
    }
    out
}
