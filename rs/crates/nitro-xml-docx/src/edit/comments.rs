// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/docx/ui/comment-commands.ts (list ops) and
// model-level addComment substitute; changes listed in PROVENANCE.md.

use std::any::Any;
use std::sync::atomic::{AtomicU64, Ordering};

use nitro_xml_opc::edit::{ChangeKind, ChangeSet, Command, CommandRegistry, EditError};
use serde_json::{json, Map, Value};

use super::host::DocxHost;

static COMMENT_SERIAL: AtomicU64 = AtomicU64::new(0);

fn host_mut(host: &mut dyn Any) -> Result<&mut DocxHost, EditError> {
    host.downcast_mut::<DocxHost>()
        .ok_or_else(|| EditError::Other {
            message: "expected DocxHost".into(),
        })
}

fn next_comment_id(explicit: Option<&str>) -> String {
    if let Some(id) = explicit {
        return id.to_owned();
    }
    let n = COMMENT_SERIAL.fetch_add(1, Ordering::Relaxed);
    format!(
        "dve-comment-{}-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0),
        n
    )
}

fn comments_vec(host: &mut DocxHost) -> &mut Vec<Value> {
    if host.comments.is_none() {
        host.comments = Some(Vec::new());
    }
    host.comments.as_mut().unwrap()
}

pub fn delete_comment(comments: &[Value], id: &str) -> Vec<Value> {
    comments
        .iter()
        .filter(|c| {
            c.get("id").and_then(|v| v.as_str()) != Some(id)
                && c.get("parentId").and_then(|v| v.as_str()) != Some(id)
        })
        .cloned()
        .collect()
}

pub fn resolve_comment(comments: &[Value], id: &str, resolved: bool) -> Vec<Value> {
    comments
        .iter()
        .map(|c| {
            if c.get("id").and_then(|v| v.as_str()) == Some(id) {
                let mut obj = c.as_object().cloned().unwrap_or_default();
                obj.insert("resolved".into(), json!(resolved));
                Value::Object(obj)
            } else {
                c.clone()
            }
        })
        .collect()
}

pub fn reply_to_comment(
    comments: &[Value],
    parent_id: &str,
    author: &str,
    text: &str,
    id: Option<&str>,
) -> Vec<Value> {
    let new_id = next_comment_id(id);
    let mut row = Map::new();
    row.insert("id".into(), json!(new_id));
    row.insert("author".into(), json!(author));
    row.insert("text".into(), json!(text));
    row.insert("parentId".into(), json!(parent_id));
    let mut out = comments.to_vec();
    out.push(Value::Object(row));
    out
}

fn for_each_run_mut(blocks: &mut [Value], mut f: impl FnMut(&mut Value)) {
    for block in blocks.iter_mut() {
        let ty = block.get("type").and_then(|t| t.as_str()).unwrap_or("");
        if ty == "paragraph" {
            if let Some(runs) = block.get_mut("runs").and_then(|r| r.as_array_mut()) {
                for run in runs {
                    f(run);
                }
            }
        } else if ty == "table" {
            if let Some(rows) = block.get_mut("rows").and_then(|r| r.as_array_mut()) {
                for row in rows {
                    if let Some(cells) = row.as_array_mut() {
                        for cell in cells {
                            if let Some(paras) =
                                cell.get_mut("paragraphs").and_then(|p| p.as_array_mut())
                            {
                                for para in paras {
                                    if let Some(runs) =
                                        para.get_mut("runs").and_then(|r| r.as_array_mut())
                                    {
                                        for run in runs {
                                            f(run);
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

fn attach_comment_id(run: &mut Value, id: &str) {
    let mut ids: Vec<String> = run
        .get("commentIds")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(str::to_owned))
                .collect()
        })
        .unwrap_or_default();
    if !ids.contains(&id.to_owned()) {
        ids.push(id.to_owned());
    }
    ids.sort();
    ids.dedup();
    if let Some(obj) = run.as_object_mut() {
        obj.insert("commentIds".into(), json!(ids));
    }
}

fn remove_comment_anchor(run: &mut Value, id: &str) {
    let remaining: Vec<String> = run
        .get("commentIds")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(str::to_owned))
                .filter(|existing| existing != id)
                .collect()
        })
        .unwrap_or_default();
    if let Some(obj) = run.as_object_mut() {
        if remaining.is_empty() {
            obj.remove("commentIds");
        } else {
            obj.insert("commentIds".into(), json!(remaining));
        }
    }
}

pub fn add_comment(
    host: &mut DocxHost,
    block_index: usize,
    run_index: Option<usize>,
    author: &str,
    text: &str,
    id: Option<&str>,
) -> Result<(), EditError> {
    let ty = host
        .blocks
        .get(block_index)
        .and_then(|b| b.get("type"))
        .and_then(|t| t.as_str())
        .unwrap_or("");
    if ty != "paragraph" {
        return Err(EditError::Other {
            message: format!("block {block_index} is not a paragraph"),
        });
    }
    let comment_id = next_comment_id(id);
    let mut comment = Map::new();
    comment.insert("id".into(), json!(comment_id.clone()));
    comment.insert("author".into(), json!(author));
    comment.insert("text".into(), json!(text));
    comment.insert("resolved".into(), json!(false));
    comments_vec(host).push(Value::Object(comment));

    let block = host
        .blocks
        .get_mut(block_index)
        .ok_or_else(|| EditError::Other {
            message: format!("block {block_index} missing"),
        })?;
    if let Some(runs) = block.get_mut("runs").and_then(|r| r.as_array_mut()) {
        if let Some(ri) = run_index {
            let run = runs.get_mut(ri).ok_or_else(|| EditError::Other {
                message: format!("run {ri} missing"),
            })?;
            attach_comment_id(run, &comment_id);
        } else {
            for run in runs.iter_mut() {
                attach_comment_id(run, &comment_id);
            }
        }
    }
    host.mark_touched(block_index);
    host.comments_touched = true;
    Ok(())
}

pub fn register_comment_commands(registry: &mut CommandRegistry) {
    registry.register(
        Command::stub("addComment", "addComment").with_runner(|host, args| {
            let h = host_mut(host)?;
            let block_index = args
                .get("blockIndex")
                .and_then(|v| v.as_u64())
                .ok_or_else(|| EditError::Other {
                    message: "addComment missing blockIndex".into(),
                })? as usize;
            let run_index = args
                .get("runIndex")
                .and_then(|v| v.as_u64())
                .map(|n| n as usize);
            let author =
                args.get("author")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| EditError::Other {
                        message: "addComment missing author".into(),
                    })?;
            let text =
                args.get("text")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| EditError::Other {
                        message: "addComment missing text".into(),
                    })?;
            let id = args.get("id").and_then(|v| v.as_str());
            add_comment(h, block_index, run_index, author, text, id)?;
            let mut cs = ChangeSet::new(ChangeKind::Other, "addComment");
            cs.dirty_parts = vec!["word/document.xml".into(), "word/comments.xml".into()];
            Ok(cs)
        }),
    );
    registry.register(
        Command::stub("replyToComment", "replyToComment").with_runner(|host, args| {
            let h = host_mut(host)?;
            let parent_id = args
                .get("parentId")
                .and_then(|v| v.as_str())
                .ok_or_else(|| EditError::Other {
                    message: "replyToComment missing parentId".into(),
                })?;
            let author =
                args.get("author")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| EditError::Other {
                        message: "replyToComment missing author".into(),
                    })?;
            let text =
                args.get("text")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| EditError::Other {
                        message: "replyToComment missing text".into(),
                    })?;
            let id = args.get("id").and_then(|v| v.as_str());
            let comments = h.comments.clone().unwrap_or_default();
            if !comments
                .iter()
                .any(|c| c.get("id").and_then(|v| v.as_str()) == Some(parent_id))
            {
                return Err(EditError::Other {
                    message: format!("comment {parent_id} missing"),
                });
            }
            h.comments = Some(reply_to_comment(&comments, parent_id, author, text, id));
            h.comments_touched = true;
            let mut cs = ChangeSet::new(ChangeKind::Other, "replyToComment");
            cs.dirty_parts = vec!["word/comments.xml".into()];
            Ok(cs)
        }),
    );
    registry.register(
        Command::stub("resolveComment", "resolveComment").with_runner(|host, args| {
            let h = host_mut(host)?;
            let id = args
                .get("id")
                .and_then(|v| v.as_str())
                .ok_or_else(|| EditError::Other {
                    message: "resolveComment missing id".into(),
                })?;
            let resolved = args
                .get("resolved")
                .and_then(|v| v.as_bool())
                .unwrap_or(true);
            let comments = h.comments.clone().unwrap_or_default();
            if !comments
                .iter()
                .any(|c| c.get("id").and_then(|v| v.as_str()) == Some(id))
            {
                return Err(EditError::Other {
                    message: format!("comment {id} missing"),
                });
            }
            h.comments = Some(resolve_comment(&comments, id, resolved));
            h.comments_touched = true;
            let mut cs = ChangeSet::new(ChangeKind::Other, "resolveComment");
            cs.dirty_parts = vec![
                "word/comments.xml".into(),
                "word/commentsExtended.xml".into(),
            ];
            Ok(cs)
        }),
    );
    registry.register(
        Command::stub("deleteComment", "deleteComment").with_runner(|host, args| {
            let h = host_mut(host)?;
            let id = args
                .get("id")
                .and_then(|v| v.as_str())
                .ok_or_else(|| EditError::Other {
                    message: "deleteComment missing id".into(),
                })?;
            let comments = h.comments.clone().unwrap_or_default();
            if !comments
                .iter()
                .any(|c| c.get("id").and_then(|v| v.as_str()) == Some(id))
            {
                return Err(EditError::Other {
                    message: format!("comment {id} missing"),
                });
            }
            h.comments = Some(delete_comment(&comments, id));
            for_each_run_mut(&mut h.blocks, |run| remove_comment_anchor(run, id));
            h.mark_all_blocks_touched();
            h.comments_touched = true;
            let mut cs = ChangeSet::new(ChangeKind::Other, "deleteComment");
            cs.dirty_parts = vec!["word/document.xml".into(), "word/comments.xml".into()];
            Ok(cs)
        }),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn catches_resolve_comment_missing_id_errors() {
        // Catches: resolveComment on a missing thread id must fail like upstream TS (#82).
        let mut host = DocxHost {
            format: "docx".into(),
            blocks: vec![],
            numbering_catalog: None,
            paragraph_styles: None,
            character_styles: None,
            sections: None,
            page: None,
            even_and_odd_headers: false,
            comments: Some(vec![json!({ "id": "0", "author": "A", "text": "t" })]),
            comments_touched: false,
            touched: Default::default(),
        };
        let mut registry = CommandRegistry::new();
        register_comment_commands(&mut registry);
        let err = registry
            .run(
                "resolveComment",
                &mut host,
                &json!({ "id": "missing", "resolved": true }),
            )
            .unwrap_err();
        assert!(err.to_string().contains("missing"));
    }
}
