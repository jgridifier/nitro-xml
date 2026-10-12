// SPDX-License-Identifier: MIT
//! Pull direct child elements out of a part as raw XML slices (carry unmodelled markup).

/// Direct children of the first element, as `(local name, raw element xml)`.
pub fn direct_child_xml(xml: &str) -> Vec<(String, String)> {
    let chars: Vec<char> = xml.chars().collect();
    let mut i = 0;
    while i < chars.len() && chars[i].is_whitespace() {
        i += 1;
    }
    if i + 1 < chars.len() && chars[i] == '<' && chars[i + 1] == '?' {
        if let Some(end) = find_seq(&chars, i, &['?', '>']) {
            i = end;
        }
    }
    while i < chars.len() && chars[i].is_whitespace() {
        i += 1;
    }
    if i >= chars.len() || chars[i] != '<' {
        return Vec::new();
    }
    let Some(root_end) = end_of_start_tag(&chars, i) else {
        return Vec::new();
    };
    if chars.get(root_end.saturating_sub(2)) == Some(&'/') {
        return Vec::new();
    }
    let mut out = Vec::new();
    let mut i = root_end;
    while i < chars.len() {
        while i < chars.len() && chars[i].is_whitespace() {
            i += 1;
        }
        if i >= chars.len() || chars[i] != '<' {
            break;
        }
        if chars.get(i + 1) == Some(&'/') {
            break;
        }
        if chars.get(i + 1) == Some(&'!') {
            if let Some(end) = skip_comment_or_decl(&chars, i) {
                i = end;
                continue;
            }
            break;
        }
        if chars.get(i + 1) == Some(&'?') {
            if let Some(end) = find_seq(&chars, i, &['?', '>']) {
                i = end;
                continue;
            }
            break;
        }
        let start = i;
        let Some(name) = element_local_name(&chars, i) else {
            break;
        };
        let Some(tag_end) = end_of_start_tag(&chars, i) else {
            break;
        };
        let self_closing = tag_end >= 2 && chars[tag_end - 2] == '/';
        let end = if self_closing {
            tag_end
        } else {
            match find_element_end(&chars, tag_end, &name) {
                Some(end) => end,
                None => break,
            }
        };
        let raw: String = chars[start..end].iter().collect();
        out.push((name, raw));
        i = end;
    }
    out
}

fn element_local_name(chars: &[char], lt: usize) -> Option<String> {
    if chars.get(lt) != Some(&'<') {
        return None;
    }
    let mut i = lt + 1;
    let start = i;
    while i < chars.len() && is_name_char(chars[i]) {
        i += 1;
    }
    if i == start {
        return None;
    }
    let token: String = chars[start..i].iter().collect();
    Some(match token.rsplit_once(':') {
        Some((_, local)) => local.to_owned(),
        None => token,
    })
}

fn is_name_char(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || ch == '_' || ch == ':' || ch == '-' || ch == '.'
}

fn end_of_start_tag(chars: &[char], lt: usize) -> Option<usize> {
    let mut i = lt + 1;
    let mut quote: Option<char> = None;
    while i < chars.len() {
        let ch = chars[i];
        if let Some(q) = quote {
            if ch == q {
                quote = None;
            }
            i += 1;
            continue;
        }
        match ch {
            '"' | '\'' => {
                quote = Some(ch);
                i += 1;
            }
            '>' => return Some(i + 1),
            _ => i += 1,
        }
    }
    None
}

fn find_element_end(chars: &[char], mut i: usize, local: &str) -> Option<usize> {
    let mut depth = 1i32;
    while i < chars.len() {
        if chars[i] != '<' {
            i += 1;
            continue;
        }
        if chars.get(i + 1) == Some(&'!') {
            i = skip_comment_or_decl(chars, i).unwrap_or(chars.len());
            continue;
        }
        if chars.get(i + 1) == Some(&'?') {
            i = find_seq(chars, i, &['?', '>']).unwrap_or(chars.len());
            continue;
        }
        if chars.get(i + 1) == Some(&'/') {
            let name = element_local_name_from_close(chars, i)?;
            let end = end_of_start_tag(chars, i)?;
            if name == local {
                depth -= 1;
                if depth == 0 {
                    return Some(end);
                }
            }
            i = end;
            continue;
        }
        let name = element_local_name(chars, i)?;
        let end = end_of_start_tag(chars, i)?;
        let self_closing = end >= 2 && chars[end - 2] == '/';
        if !self_closing && name == local {
            depth += 1;
        }
        i = end;
    }
    None
}

fn element_local_name_from_close(chars: &[char], lt: usize) -> Option<String> {
    if chars.get(lt) != Some(&'<') || chars.get(lt + 1) != Some(&'/') {
        return None;
    }
    let mut i = lt + 2;
    let start = i;
    while i < chars.len() && is_name_char(chars[i]) {
        i += 1;
    }
    if i == start {
        return None;
    }
    let token: String = chars[start..i].iter().collect();
    Some(match token.rsplit_once(':') {
        Some((_, local)) => local.to_owned(),
        None => token,
    })
}

fn skip_comment_or_decl(chars: &[char], i: usize) -> Option<usize> {
    if chars.get(i + 1) == Some(&'!') && chars.get(i + 2) == Some(&'-') {
        return find_seq(chars, i, &['-', '-', '>']);
    }
    end_of_start_tag(chars, i)
}

fn find_seq(chars: &[char], from: usize, seq: &[char]) -> Option<usize> {
    if seq.is_empty() || from >= chars.len() {
        return None;
    }
    let mut i = from;
    while i + seq.len() <= chars.len() {
        if chars[i..i + seq.len()] == *seq {
            return Some(i + seq.len());
        }
        i += 1;
    }
    None
}
