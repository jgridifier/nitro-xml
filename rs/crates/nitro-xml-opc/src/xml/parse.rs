// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/xml/xml.ts (parseXml: DTD/ENTITY rejection,
// error-on-malformed, single root) and src/core/xml/lite.ts (entity / attribute normalisation);
// changes listed in PROVENANCE.md.
//! Single-pass streaming parse over `quick-xml` `NsReader` into an owned tree.

use std::borrow::Cow;

use quick_xml::events::Event;
use quick_xml::name::ResolveResult;
use quick_xml::reader::NsReader;

use super::decode::{decode_entities, normalize_attribute_value, normalize_line_endings};
use super::error::XmlError;
use super::tree::{Attr, Document, Element, Node};

/// Parse OOXML part bytes the way upstream `parseXml` does after `TextDecoder('utf-8')`.
pub fn parse_xml_bytes(bytes: &[u8]) -> Result<Document, XmlError> {
    let text = decode_utf8(bytes);
    parse_xml_str(&text)
}

/// Parse an XML string (already Unicode). A leading BOM is dropped.
pub fn parse_xml_str(xml: &str) -> Result<Document, XmlError> {
    parse_xml_str_with_options(xml, ParseOptions::default())
}

/// Hard cap on element nesting. Upstream has no depth budget; this beats that by failing closed
/// before the owned parse stack can grow without bound. Set above the parity gate's crafted
/// 2 000-deep accept case (`tools/model-compare/gates/make_xml_cases.py` `deep.xml`) and the
/// iterative [`Element`](super::tree::Element) `Drop` that clears 2 000-deep trees.
pub const MAX_XML_DEPTH: usize = 4096;

/// Options for [`parse_xml_str_with_options`].
#[derive(Clone, Copy, Debug)]
pub struct ParseOptions {
    /// When true (default), attributes are sorted for stable dumps; chart fragment `buildXml`
    /// parity needs document order (`false`).
    pub sort_attributes: bool,
    /// Maximum element nesting depth (default [`MAX_XML_DEPTH`]). `0` disables the check.
    pub max_depth: usize,
}

impl Default for ParseOptions {
    fn default() -> Self {
        Self {
            sort_attributes: true,
            max_depth: MAX_XML_DEPTH,
        }
    }
}

/// Parse with explicit options (chart parts use `sort_attributes: false` for `sourceXml` parity).
pub fn parse_xml_str_with_options(xml: &str, options: ParseOptions) -> Result<Document, XmlError> {
    let xml = xml.strip_prefix('\u{feff}').unwrap_or(xml);
    // Parity with parseXml: any DOCTYPE / ENTITY substring is rejected before parsing.
    // Entity *expansion* is off: only predefined + numeric refs are decoded later.
    if contains_dtd_or_entity(xml) {
        return Err(XmlError::new(
            "dtd",
            "XML with DTD or entity declarations is not supported",
        ));
    }
    let xml = normalize_line_endings(xml);
    parse_normalized(&xml, options)
}

fn decode_utf8(bytes: &[u8]) -> Cow<'_, str> {
    let bytes = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(bytes);
    match std::str::from_utf8(bytes) {
        Ok(s) => Cow::Borrowed(s),
        Err(_) => Cow::Owned(String::from_utf8_lossy(bytes).into_owned()),
    }
}

fn contains_dtd_or_entity(xml: &str) -> bool {
    let lower = xml.as_bytes();
    // Case-insensitive search for <!DOCTYPE or <!ENTITY (upstream `/<!DOCTYPE|<!ENTITY/i`).
    let n = lower.len();
    let mut i = 0;
    while i + 9 <= n {
        if lower[i] == b'<' && lower[i + 1] == b'!' {
            let rest = &lower[i + 2..];
            if starts_with_ignore_ascii_case(rest, b"DOCTYPE")
                || starts_with_ignore_ascii_case(rest, b"ENTITY")
            {
                return true;
            }
        }
        i += 1;
    }
    false
}

fn starts_with_ignore_ascii_case(hay: &[u8], needle: &[u8]) -> bool {
    hay.len() >= needle.len()
        && hay[..needle.len()]
            .iter()
            .zip(needle.iter())
            .all(|(a, b)| a.eq_ignore_ascii_case(b))
}

fn parse_normalized(xml: &str, options: ParseOptions) -> Result<Document, XmlError> {
    let sort_attributes = options.sort_attributes;
    let max_depth = options.max_depth;
    let mut reader = NsReader::from_str(xml);
    let config = reader.config_mut();
    config.trim_text(false);
    config.check_end_names = true;
    config.expand_empty_elements = false;

    let mut stack: Vec<Element> = Vec::new();
    let mut root: Option<Element> = None;
    let mut pending_text: Option<String> = None;
    let mut seen_root_end = false;

    loop {
        let (resolve, event) = reader
            .read_resolved_event()
            .map_err(|e| XmlError::new("parse", e.to_string()))?;

        match event {
            Event::Start(e) => {
                if seen_root_end && stack.is_empty() {
                    return Err(XmlError::new("parse", "only one root element allowed"));
                }
                if max_depth > 0 && stack.len() >= max_depth {
                    return Err(XmlError::new(
                        "depth",
                        format!("XML nesting exceeds the depth limit ({max_depth})"),
                    ));
                }
                let ns = resolve_element_ns(resolve)?;
                let local = local_name_str(e.local_name().as_ref())?;
                let attrs = collect_attrs(&reader, &e, sort_attributes)?;
                let el = Element::new(ns, local, attrs);
                flush_text(&mut stack, &mut root, &mut pending_text)?;
                stack.push(el);
            }
            Event::Empty(e) => {
                if seen_root_end && stack.is_empty() {
                    return Err(XmlError::new("parse", "only one root element allowed"));
                }
                // Empty elements still occupy one nesting level under their parent.
                if max_depth > 0 && stack.len() >= max_depth {
                    return Err(XmlError::new(
                        "depth",
                        format!("XML nesting exceeds the depth limit ({max_depth})"),
                    ));
                }
                let ns = resolve_element_ns(resolve)?;
                let local = local_name_str(e.local_name().as_ref())?;
                let attrs = collect_attrs(&reader, &e, sort_attributes)?;
                let el = Element::new(ns, local, attrs);
                let at_document = stack.is_empty();
                push_child(&mut stack, &mut root, &mut pending_text, Node::Element(el))?;
                if at_document {
                    seen_root_end = true;
                }
            }
            Event::End(e) => {
                let local = local_name_str(e.local_name().as_ref())?;
                let mut finished = stack
                    .pop()
                    .ok_or_else(|| XmlError::new("parse", "unexpected end tag"))?;
                if finished.local != local {
                    return Err(XmlError::new(
                        "parse",
                        format!("end tag mismatch: {} != {}", finished.local, local),
                    ));
                }
                if let Some(text) = pending_text.take() {
                    finished.children.push(Node::Text(text));
                }
                if stack.is_empty() {
                    if root.is_some() {
                        return Err(XmlError::new("parse", "only one root element allowed"));
                    }
                    root = Some(finished);
                    seen_root_end = true;
                } else {
                    let parent = stack
                        .last_mut()
                        .ok_or_else(|| XmlError::new("parse", "internal stack error"))?;
                    parent.children.push(Node::Element(finished));
                }
            }
            Event::Text(t) => {
                let raw = std::str::from_utf8(t.as_ref())
                    .map_err(|_| XmlError::new("parse", "text is not utf-8"))?;
                // After the root, only whitespace is allowed (prolog trailing / document trailing).
                if seen_root_end && stack.is_empty() {
                    if !raw.chars().all(|c| matches!(c, ' ' | '\t' | '\n' | '\r')) {
                        return Err(XmlError::new("parse", "trailing content after root"));
                    }
                    continue;
                }
                if stack.is_empty() && root.is_none() {
                    // Leading whitespace before the root.
                    if raw.chars().all(|c| matches!(c, ' ' | '\t' | '\n' | '\r')) {
                        continue;
                    }
                    return Err(XmlError::new("parse", "text before root element"));
                }
                // quick-xml ≥0.41 emits entities as GeneralRef; Text is usually entity-free, but
                // decode_entities still handles any leftover &...; forms.
                let decoded = decode_entities(raw)
                    .ok_or_else(|| XmlError::new("parse", "unknown or malformed entity"))?;
                append_text(&mut pending_text, &decoded);
            }
            Event::GeneralRef(r) => {
                if seen_root_end && stack.is_empty() {
                    return Err(XmlError::new("parse", "trailing content after root"));
                }
                if stack.is_empty() && root.is_none() {
                    return Err(XmlError::new("parse", "text before root element"));
                }
                let body = std::str::from_utf8(r.as_ref())
                    .map_err(|_| XmlError::new("parse", "entity is not utf-8"))?;
                // Body is the inside of &...; (e.g. "amp", "#x41").
                let decoded = decode_entities(&format!("&{body};"))
                    .ok_or_else(|| XmlError::new("parse", "unknown or malformed entity"))?;
                append_text(&mut pending_text, &decoded);
            }
            Event::CData(c) => {
                if seen_root_end && stack.is_empty() {
                    return Err(XmlError::new("parse", "trailing content after root"));
                }
                if stack.is_empty() {
                    return Err(XmlError::new("parse", "cdata before root element"));
                }
                let raw = std::str::from_utf8(c.as_ref())
                    .map_err(|_| XmlError::new("parse", "cdata is not utf-8"))?;
                match &mut pending_text {
                    Some(buf) => buf.push_str(raw),
                    None => pending_text = Some(raw.to_owned()),
                }
            }
            Event::Comment(_) | Event::PI(_) | Event::Decl(_) | Event::DocType(_) => {
                // Prolog / comments / PIs are dropped. DocType is also rejected by the regex above.
            }
            Event::Eof => break,
        }
    }

    if !stack.is_empty() {
        return Err(XmlError::new("parse", "unclosed element"));
    }
    let root = root.ok_or_else(|| XmlError::new("parse", "missing root element"))?;
    Ok(Document { root })
}

fn append_text(pending: &mut Option<String>, text: &str) {
    match pending {
        Some(buf) => buf.push_str(text),
        None => *pending = Some(text.to_owned()),
    }
}

fn resolve_element_ns(resolve: ResolveResult<'_>) -> Result<Option<String>, XmlError> {
    match resolve {
        ResolveResult::Bound(ns) => {
            let s = std::str::from_utf8(ns.as_ref())
                .map_err(|_| XmlError::new("parse", "namespace uri is not utf-8"))?;
            Ok(Some(s.to_owned()))
        }
        ResolveResult::Unbound => Ok(None),
        ResolveResult::Unknown(prefix) => Err(XmlError::new(
            "parse",
            format!(
                "undeclared element prefix {}",
                String::from_utf8_lossy(&prefix)
            ),
        )),
    }
}

fn local_name_str(local: &[u8]) -> Result<String, XmlError> {
    std::str::from_utf8(local)
        .map(|s| s.to_owned())
        .map_err(|_| XmlError::new("parse", "name is not utf-8"))
}

fn collect_attrs<R: std::io::BufRead>(
    reader: &NsReader<R>,
    start: &quick_xml::events::BytesStart<'_>,
    sort_attributes: bool,
) -> Result<Vec<Attr>, XmlError> {
    let mut attrs = Vec::new();
    let mut seen: Vec<(Option<String>, String)> = Vec::new();
    for attr in start.attributes() {
        let attr = attr.map_err(|e| XmlError::new("parse", e.to_string()))?;
        let key = attr.key;
        let has_prefix = key.prefix().is_some();
        let is_xmlns_default = {
            let local = key.local_name();
            !has_prefix && local.as_ref() == b"xmlns"
        };
        let is_xmlns_prefixed = key.prefix().is_some_and(|p| p.as_ref() == b"xmlns");
        // Skip xmlns / xmlns:* declarations (not attributes in the dump / DOM sense used here).
        if is_xmlns_default || is_xmlns_prefixed {
            continue;
        }
        let raw = std::str::from_utf8(attr.value.as_ref())
            .map_err(|_| XmlError::new("parse", "attribute value is not utf-8"))?;
        if raw.contains('<') {
            return Err(XmlError::new(
                "parse",
                "unescaped '<' not allowed in attribute values",
            ));
        }
        let value = normalize_attribute_value(raw)
            .ok_or_else(|| XmlError::new("parse", "unknown or malformed entity in attribute"))?;

        let (resolve, local_name) = reader.resolver().resolve_attribute(key);
        let ns = match resolve {
            ResolveResult::Bound(ns) => {
                let s = std::str::from_utf8(ns.as_ref())
                    .map_err(|_| XmlError::new("parse", "attribute namespace uri is not utf-8"))?;
                Some(s.to_owned())
            }
            ResolveResult::Unbound => None,
            ResolveResult::Unknown(prefix) => {
                return Err(XmlError::new(
                    "parse",
                    format!(
                        "undeclared attribute prefix {}",
                        String::from_utf8_lossy(&prefix)
                    ),
                ));
            }
        };
        let local = std::str::from_utf8(local_name.as_ref())
            .map_err(|_| XmlError::new("parse", "attribute local name is not utf-8"))?
            .to_owned();

        let key_pair = (ns.clone(), local.clone());
        if seen.iter().any(|s| s == &key_pair) {
            return Err(XmlError::new(
                "parse",
                format!("duplicate attribute {local}"),
            ));
        }
        seen.push(key_pair);
        attrs.push(Attr { ns, local, value });
    }
    if sort_attributes {
        attrs.sort_by(|a, b| {
            let ak = (a.ns.as_deref().unwrap_or(""), a.local.as_str());
            let bk = (b.ns.as_deref().unwrap_or(""), b.local.as_str());
            ak.cmp(&bk)
        });
    }
    Ok(attrs)
}

fn flush_text(
    stack: &mut [Element],
    root: &mut Option<Element>,
    pending: &mut Option<String>,
) -> Result<(), XmlError> {
    let Some(text) = pending.take() else {
        return Ok(());
    };
    if let Some(top) = stack.last_mut() {
        top.children.push(Node::Text(text));
        Ok(())
    } else if root.is_some() {
        Err(XmlError::new("parse", "text after root element"))
    } else {
        Err(XmlError::new("parse", "text before root element"))
    }
}

fn push_child(
    stack: &mut [Element],
    root: &mut Option<Element>,
    pending: &mut Option<String>,
    child: Node,
) -> Result<(), XmlError> {
    flush_text(stack, root, pending)?;
    if let Some(top) = stack.last_mut() {
        top.children.push(child);
        Ok(())
    } else if root.is_none() {
        match child {
            Node::Element(el) => {
                *root = Some(el);
                Ok(())
            }
            Node::Text(_) => Err(XmlError::new("parse", "text before root element")),
        }
    } else {
        Err(XmlError::new("parse", "only one root element allowed"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catches_naive_dtd_reject_even_inside_cdata_like_upstream() {
        // Catches: parseXml uses a whole-string /<!DOCTYPE|<!ENTITY/i test, so a DOCTYPE substring
        // inside CDATA still rejects (not a real DTD parse).
        let err = parse_xml_str("<a><![CDATA[<!DOCTYPE]]></a>").unwrap_err();
        assert_eq!(err.kind(), "dtd");
    }

    #[test]
    fn catches_unbounded_element_nesting_without_depth_cap() {
        // Catches: a deep nesting bomb growing the owned Element stack without a depth budget.
        let depth = MAX_XML_DEPTH + 1;
        let mut xml = String::new();
        for _ in 0..depth {
            xml.push_str("<a>");
        }
        for _ in 0..depth {
            xml.push_str("</a>");
        }
        let err = parse_xml_str(&xml).unwrap_err();
        assert_eq!(err.kind(), "depth");
    }

    #[test]
    fn catches_lt_in_attribute_value_rejected() {
        // Catches: quick-xml accepts a literal '<' in attribute values; xmldom rejects it.
        assert!(parse_xml_str(r#"<a b="<"/>"#).is_err());
    }
}
