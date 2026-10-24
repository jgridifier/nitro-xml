// SPDX-License-Identifier: MIT
//! Paragraph text insert/replace, split and join on the docx edit host (#78).
//!
//! Script op `setParagraphText` matches `parity/edit/apply.mjs`. Split/join feed
//! the #61 rich surface host path (no ProseMirror). Table cell text is #79.

use std::any::Any;

use nitro_xml_opc::edit::{ChangeKind, ChangeSet, Command, CommandRegistry, EditError};
use serde_json::{json, Value};

use super::host::{make_run, DocxHost};

fn host_mut(host: &mut dyn Any) -> Result<&mut DocxHost, EditError> {
    host.downcast_mut::<DocxHost>()
        .ok_or_else(|| EditError::Other {
            message: "expected DocxHost".into(),
        })
}

fn block_index(args: &Value) -> Result<usize, EditError> {
    args.get("blockIndex")
        .and_then(|v| v.as_u64())
        .map(|n| n as usize)
        .ok_or_else(|| EditError::Other {
            message: "missing blockIndex".into(),
        })
}

/// Replace a paragraph's runs with a single styled run (TS `setParagraphText`).
pub fn set_paragraph_text(
    host: &mut DocxHost,
    block_index: usize,
    text: &str,
    attrs: &serde_json::Map<String, Value>,
) -> Result<(), EditError> {
    let block = host
        .paragraph_mut(block_index)
        .map_err(|e| EditError::Other { message: e })?;
    let run = make_run(text, attrs);
    if let Some(obj) = block.as_object_mut() {
        obj.insert("runs".into(), json!([run]));
    }
    host.mark_touched(block_index);
    Ok(())
}

/// Split a top-level paragraph at a UTF-8 byte offset into two blocks.
pub fn split_paragraph(
    host: &mut DocxHost,
    block_index: usize,
    offset: usize,
) -> Result<(), EditError> {
    host.ensure_base_blocks();
    let block = host
        .paragraph(block_index)
        .map_err(|e| EditError::Other { message: e })?;
    let text: String = block
        .get("runs")
        .and_then(|r| r.as_array())
        .map(|rs| {
            rs.iter()
                .map(|r| r.get("text").and_then(|t| t.as_str()).unwrap_or(""))
                .collect()
        })
        .unwrap_or_default();
    let offset = offset.min(text.len());
    if !text.is_char_boundary(offset) {
        return Err(EditError::Other {
            message: "split offset not on char boundary".into(),
        });
    }
    let (left, right) = text.split_at(offset);
    let id = block
        .get("id")
        .and_then(|v| v.as_str())
        .unwrap_or("p")
        .to_owned();
    let align = block.get("align").cloned();
    let style = block.get("style").cloned();
    let numbering = block.get("numbering").cloned();

    let mut left_para = json!({
        "type": "paragraph",
        "id": id,
        "runs": [{ "text": left }],
    });
    let mut right_para = json!({
        "type": "paragraph",
        "id": format!("{id}-split"),
        "runs": [{ "text": right }],
    });
    for (para, extra) in [(&mut left_para, align.clone()), (&mut right_para, align)] {
        if let (Some(obj), Some(a)) = (para.as_object_mut(), extra) {
            obj.insert("align".into(), a);
        }
    }
    if let (Some(obj), Some(s)) = (left_para.as_object_mut(), style.clone()) {
        obj.insert("style".into(), s);
    }
    if let (Some(obj), Some(s)) = (right_para.as_object_mut(), style) {
        obj.insert("style".into(), s);
    }
    if let (Some(obj), Some(n)) = (left_para.as_object_mut(), numbering.clone()) {
        obj.insert("numbering".into(), n);
    }
    if let (Some(obj), Some(n)) = (right_para.as_object_mut(), numbering) {
        obj.insert("numbering".into(), n);
    }

    host.blocks[block_index] = left_para;
    host.blocks.insert(block_index + 1, right_para);
    host.mark_touched(block_index);
    host.mark_touched(block_index + 1);
    Ok(())
}

/// Join paragraph `block_index + 1` into `block_index` (top-level paragraphs only).
pub fn join_paragraph(host: &mut DocxHost, block_index: usize) -> Result<(), EditError> {
    host.ensure_base_blocks();
    if block_index + 1 >= host.blocks.len() {
        return Err(EditError::Other {
            message: "no next paragraph to join".into(),
        });
    }
    let left_ty = host.blocks[block_index]
        .get("type")
        .and_then(|t| t.as_str())
        .unwrap_or("");
    let right_ty = host.blocks[block_index + 1]
        .get("type")
        .and_then(|t| t.as_str())
        .unwrap_or("");
    if left_ty != "paragraph" || right_ty != "paragraph" {
        return Err(EditError::Other {
            message: "join requires adjacent paragraphs".into(),
        });
    }
    let right = host.blocks.remove(block_index + 1);
    let right_runs = right
        .get("runs")
        .and_then(|r| r.as_array())
        .cloned()
        .unwrap_or_default();
    let left = &mut host.blocks[block_index];
    if let Some(obj) = left.as_object_mut() {
        let runs = obj
            .entry("runs")
            .or_insert_with(|| json!([]))
            .as_array_mut()
            .ok_or_else(|| EditError::Other {
                message: "runs not array".into(),
            })?;
        runs.extend(right_runs);
    }
    host.mark_touched(block_index);
    Ok(())
}

fn run_text(run: &Value) -> Option<&str> {
    run.get("text").and_then(|t| t.as_str())
}

fn runs_mut(block: &mut Value) -> Result<&mut Vec<Value>, EditError> {
    block
        .as_object_mut()
        .and_then(|o| o.get_mut("runs"))
        .and_then(|r| r.as_array_mut())
        .ok_or_else(|| EditError::Other {
            message: "paragraph has no runs".into(),
        })
}

/// Insert `text` at a UTF-8 byte `offset` of the paragraph plain text, extending the run that
/// holds the character before the caret so run formatting is kept (Word behaviour).
pub fn insert_text(
    host: &mut DocxHost,
    block_index: usize,
    offset: usize,
    text: &str,
) -> Result<(), EditError> {
    let block = host
        .paragraph_mut(block_index)
        .map_err(|e| EditError::Other { message: e })?;
    if block.get("runs").and_then(|r| r.as_array()).is_none() {
        if let Some(obj) = block.as_object_mut() {
            obj.insert("runs".into(), json!([]));
        }
    }
    let runs = runs_mut(block)?;
    let mut start = 0usize;
    let mut target: Option<(usize, usize)> = None;
    for (i, run) in runs.iter().enumerate() {
        let Some(t) = run_text(run) else { continue };
        if target.is_none() {
            target = Some((i, 0));
        }
        if offset > start && offset <= start + t.len() {
            target = Some((i, offset - start));
            break;
        }
        if offset <= start {
            break;
        }
        // Past this run: remember its end as the fallback (caret beyond the text).
        target = Some((i, t.len()));
        start += t.len();
    }
    match target {
        Some((i, at)) => {
            let t = run_text(&runs[i]).unwrap_or("");
            if !t.is_char_boundary(at) {
                return Err(EditError::Other {
                    message: "insert offset not on char boundary".into(),
                });
            }
            let mut next = String::with_capacity(t.len() + text.len());
            next.push_str(&t[..at]);
            next.push_str(text);
            next.push_str(&t[at..]);
            runs[i]["text"] = json!(next);
        }
        None => runs.push(json!({ "text": text })),
    }
    host.mark_touched(block_index);
    Ok(())
}

/// Delete the byte range `start..end` of the paragraph plain text across runs. Runs that become
/// empty are dropped (one empty run is kept so the paragraph keeps its formatting).
pub fn delete_text(
    host: &mut DocxHost,
    block_index: usize,
    start: usize,
    end: usize,
) -> Result<(), EditError> {
    let block = host
        .paragraph_mut(block_index)
        .map_err(|e| EditError::Other { message: e })?;
    let runs = runs_mut(block)?;
    let mut pos = 0usize;
    let mut emptied: Vec<usize> = Vec::new();
    for (i, run) in runs.iter_mut().enumerate() {
        let Some(t) = run_text(run) else { continue };
        let (rs, re) = (pos, pos + t.len());
        pos = re;
        let (a, b) = (start.max(rs), end.min(re));
        if a >= b {
            continue;
        }
        let (la, lb) = (a - rs, b - rs);
        if !t.is_char_boundary(la) || !t.is_char_boundary(lb) {
            return Err(EditError::Other {
                message: "delete range not on char boundary".into(),
            });
        }
        let next = format!("{}{}", &t[..la], &t[lb..]);
        if next.is_empty() {
            emptied.push(i);
        }
        run["text"] = json!(next);
    }
    let text_runs = runs.iter().filter(|r| run_text(r).is_some()).count();
    if emptied.len() >= text_runs {
        emptied.remove(0);
    }
    for i in emptied.into_iter().rev() {
        runs.remove(i);
    }
    host.mark_touched(block_index);
    Ok(())
}

fn unique_block_id(host: &DocxHost, base: &str) -> String {
    let taken = |id: &str| {
        host.blocks
            .iter()
            .any(|b| b.get("id").and_then(|v| v.as_str()) == Some(id))
    };
    let mut n = 1usize;
    loop {
        let id = format!("{base}-s{n}");
        if !taken(&id) {
            return id;
        }
        n += 1;
    }
}

/// Split a top-level paragraph at a byte offset, keeping run formatting and paragraph
/// properties on both halves (Enter).
pub fn split_paragraph_runs(
    host: &mut DocxHost,
    block_index: usize,
    offset: usize,
) -> Result<(), EditError> {
    host.ensure_base_blocks();
    let block = host
        .paragraph(block_index)
        .map_err(|e| EditError::Other { message: e })?
        .clone();
    let runs = block
        .get("runs")
        .and_then(|r| r.as_array())
        .cloned()
        .unwrap_or_default();
    let mut left: Vec<Value> = Vec::new();
    let mut right: Vec<Value> = Vec::new();
    let mut pos = 0usize;
    for run in runs {
        let Some(t) = run_text(&run).map(str::to_owned) else {
            if pos >= offset {
                right.push(run);
            } else {
                left.push(run);
            }
            continue;
        };
        let (rs, re) = (pos, pos + t.len());
        pos = re;
        if re <= offset {
            left.push(run);
        } else if rs >= offset {
            right.push(run);
        } else {
            let at = offset - rs;
            if !t.is_char_boundary(at) {
                return Err(EditError::Other {
                    message: "split offset not on char boundary".into(),
                });
            }
            let mut l = run.clone();
            l["text"] = json!(&t[..at]);
            let mut r = run;
            r["text"] = json!(&t[at..]);
            left.push(l);
            right.push(r);
        }
    }
    // An empty half still carries the neighbouring run's formatting.
    let template = left.last().or(right.first()).cloned();
    for half in [&mut left, &mut right] {
        if half.is_empty() {
            let mut run = template.clone().unwrap_or_else(|| json!({}));
            run["text"] = json!("");
            half.push(run);
        }
    }
    let id = block
        .get("id")
        .and_then(|v| v.as_str())
        .unwrap_or("p")
        .to_owned();
    let new_id = unique_block_id(host, &id);
    let mut right_para = block;
    right_para["id"] = json!(new_id);
    right_para["runs"] = json!(right);
    host.blocks[block_index]["runs"] = json!(left);
    host.blocks.insert(block_index + 1, right_para);
    host.mark_touched(block_index);
    host.mark_touched(block_index + 1);
    Ok(())
}

pub fn register_text_commands(registry: &mut CommandRegistry) {
    registry.register(
        Command::stub("setParagraphText", "setParagraphText").with_runner(|host, args| {
            let h = host_mut(host)?;
            let bi = block_index(args)?;
            let text =
                args.get("text")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| EditError::Other {
                        message: "setParagraphText missing text".into(),
                    })?;
            let attrs = args.as_object().cloned().unwrap_or_default();
            set_paragraph_text(h, bi, text, &attrs)?;
            let mut cs = ChangeSet::new(ChangeKind::Other, format!("setParagraphText {bi}"));
            cs.dirty_parts = vec!["word/document.xml".into()];
            Ok(cs)
        }),
    );
    registry.register(
        Command::stub("insertText", "insertText").with_runner(|host, args| {
            let h = host_mut(host)?;
            let bi = block_index(args)?;
            let offset = usize_arg(args, "offset")?;
            let text =
                args.get("text")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| EditError::Other {
                        message: "insertText missing text".into(),
                    })?;
            insert_text(h, bi, offset, text)?;
            Ok(text_change(format!("insertText {bi}")))
        }),
    );
    registry.register(
        Command::stub("deleteText", "deleteText").with_runner(|host, args| {
            let h = host_mut(host)?;
            let bi = block_index(args)?;
            delete_text(h, bi, usize_arg(args, "start")?, usize_arg(args, "end")?)?;
            Ok(text_change(format!("deleteText {bi}")))
        }),
    );
    registry.register(
        Command::stub("splitParagraph", "splitParagraph").with_runner(|host, args| {
            let h = host_mut(host)?;
            let bi = block_index(args)?;
            split_paragraph_runs(h, bi, usize_arg(args, "offset")?)?;
            Ok(text_change(format!("splitParagraph {bi}")))
        }),
    );
    registry.register(
        Command::stub("joinParagraphs", "joinParagraphs").with_runner(|host, args| {
            let h = host_mut(host)?;
            let bi = block_index(args)?;
            join_paragraph(h, bi)?;
            Ok(text_change(format!("joinParagraphs {bi}")))
        }),
    );
}

fn usize_arg(args: &Value, key: &str) -> Result<usize, EditError> {
    args.get(key)
        .and_then(|v| v.as_u64())
        .map(|n| n as usize)
        .ok_or_else(|| EditError::Other {
            message: format!("missing {key}"),
        })
}

fn text_change(label: String) -> ChangeSet {
    let mut cs = ChangeSet::new(ChangeKind::Other, label);
    cs.dirty_parts = vec!["word/document.xml".into()];
    cs
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn catches_set_paragraph_text_on_table_block_is_rejected() {
        // Catches: setParagraphText silently mutating a table as if it were a paragraph (#78 negative).
        let mut host = DocxHost::stub(vec![json!({ "type": "table", "rows": [] })]);
        let err = set_paragraph_text(&mut host, 0, "x", &Default::default()).unwrap_err();
        assert!(err.to_string().contains("not a paragraph"));
    }

    fn para(runs: Value) -> Value {
        json!({ "type": "paragraph", "id": "p", "runs": runs })
    }

    fn texts(host: &DocxHost, i: usize) -> Vec<String> {
        host.blocks[i]["runs"]
            .as_array()
            .unwrap()
            .iter()
            .map(|r| r["text"].as_str().unwrap().to_owned())
            .collect()
    }

    #[test]
    fn catches_typing_flattening_mixed_format_runs() {
        // Catches: typing into a bold+plain paragraph rewriting it as one run (lost run formatting).
        let mut host = DocxHost::stub(vec![para(json!([
            { "text": "Bold", "bold": true },
            { "text": " plain" }
        ]))]);
        insert_text(&mut host, 0, 4, "!").unwrap();
        assert_eq!(texts(&host, 0), vec!["Bold!", " plain"]);
        assert_eq!(host.blocks[0]["runs"][0]["bold"], json!(true));
        insert_text(&mut host, 0, 0, ">").unwrap();
        assert_eq!(texts(&host, 0)[0], ">Bold!");
    }

    #[test]
    fn catches_delete_across_runs_leaving_empty_runs_or_wrong_text() {
        // Catches: Backspace/Delete over a run boundary removing the wrong span or leaving empty runs.
        let mut host = DocxHost::stub(vec![para(json!([
            { "text": "ab", "bold": true },
            { "text": "cd" },
            { "text": "ef", "italic": true }
        ]))]);
        delete_text(&mut host, 0, 1, 5).unwrap();
        assert_eq!(texts(&host, 0), vec!["a", "f"]);
        delete_text(&mut host, 0, 0, 2).unwrap();
        assert_eq!(
            texts(&host, 0).len(),
            1,
            "keeps one run so the paragraph keeps its format"
        );
    }

    #[test]
    fn catches_enter_dropping_run_formatting_or_reusing_block_ids() {
        // Catches: Enter flattening both halves and giving every split the same "-split" id.
        let mut host = DocxHost::stub(vec![para(json!([
            { "text": "ab", "bold": true },
            { "text": "cd" }
        ]))]);
        split_paragraph_runs(&mut host, 0, 1).unwrap();
        split_paragraph_runs(&mut host, 1, 0).unwrap();
        assert_eq!(texts(&host, 0), vec!["a"]);
        assert_eq!(texts(&host, 1), vec![""]);
        assert_eq!(texts(&host, 2), vec!["b", "cd"]);
        assert_eq!(host.blocks[2]["runs"][0]["bold"], json!(true));
        let ids: std::collections::HashSet<_> = host
            .blocks
            .iter()
            .map(|b| b["id"].as_str().unwrap().to_owned())
            .collect();
        assert_eq!(ids.len(), 3);
    }
}
