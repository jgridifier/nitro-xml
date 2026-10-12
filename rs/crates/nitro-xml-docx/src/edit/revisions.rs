// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/docx/revision-commands.ts; changes listed in
// PROVENANCE.md.

use std::any::Any;

use nitro_xml_opc::edit::{ChangeKind, ChangeSet, Command, CommandRegistry, EditError};
use serde_json::{json, Value};

use super::host::DocxHost;
use super::restore_format::{restore_paragraph_formatting, restore_run_formatting};

fn host_mut(host: &mut dyn Any) -> Result<&mut DocxHost, EditError> {
    host.downcast_mut::<DocxHost>()
        .ok_or_else(|| EditError::Other {
            message: "expected DocxHost".into(),
        })
}

#[derive(Clone, Debug)]
pub(crate) struct RevisionEntry {
    id: String,
    kind: String,
    paragraph_id: String,
    run_index: Option<usize>,
    move_name: Option<String>,
}

fn paragraphs_of(blocks: &[Value]) -> Vec<Value> {
    let mut paragraphs = Vec::new();
    for block in blocks {
        let ty = block.get("type").and_then(|t| t.as_str()).unwrap_or("");
        if ty == "paragraph" {
            paragraphs.push(block.clone());
        } else if ty == "table" {
            let empty_rows: &[Value] = &[];
            for row in block
                .get("rows")
                .and_then(|r| r.as_array())
                .map(|a| a.as_slice())
                .unwrap_or(empty_rows)
            {
                for cell in row.as_array().unwrap_or(&vec![]) {
                    let empty_paras: &[Value] = &[];
                    for p in cell
                        .get("paragraphs")
                        .and_then(|p| p.as_array())
                        .map(|a| a.as_slice())
                        .unwrap_or(empty_paras)
                    {
                        paragraphs.push(p.clone());
                    }
                }
            }
        }
    }
    paragraphs
}

fn document_paragraphs(blocks: &[Value]) -> Vec<Value> {
    paragraphs_of(blocks)
}

pub(crate) fn list_revisions(blocks: &[Value]) -> Vec<RevisionEntry> {
    let mut entries = Vec::new();
    for paragraph in document_paragraphs(blocks) {
        let pid = paragraph
            .get("id")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_owned();
        if let Some(mr) = paragraph.get("markRevision") {
            entries.push(revision_entry(mr, &pid, None));
        }
        if let Some(fr) = paragraph.get("formatRevision") {
            entries.push(revision_entry(fr, &pid, None));
        }
        if let Some(runs) = paragraph.get("runs").and_then(|r| r.as_array()) {
            for (run_index, run) in runs.iter().enumerate() {
                if let Some(rev) = run.get("revision") {
                    entries.push(revision_entry(rev, &pid, Some(run_index)));
                }
                if let Some(rev) = run.get("formatRevision") {
                    entries.push(revision_entry(rev, &pid, Some(run_index)));
                }
            }
        }
    }
    entries
}

fn revision_entry(rev: &Value, paragraph_id: &str, run_index: Option<usize>) -> RevisionEntry {
    let move_name = rev
        .get("move")
        .and_then(|m| m.get("name"))
        .and_then(|n| n.as_str())
        .map(str::to_owned);
    RevisionEntry {
        id: rev
            .get("id")
            .and_then(|v| v.as_str())
            .unwrap_or("0")
            .to_owned(),
        kind: rev
            .get("kind")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_owned(),
        paragraph_id: paragraph_id.to_owned(),
        run_index,
        move_name,
    }
}

pub(crate) fn find_revision(blocks: &[Value], id: &str) -> Option<RevisionEntry> {
    list_revisions(blocks).into_iter().find(|e| e.id == id)
}

fn matching_run_indices(paragraph: &Value, id: &str) -> Vec<usize> {
    paragraph
        .get("runs")
        .and_then(|r| r.as_array())
        .map(|runs| {
            runs.iter()
                .enumerate()
                .filter(|(_, run)| {
                    run.get("revision")
                        .and_then(|r| r.get("id"))
                        .and_then(|v| v.as_str())
                        == Some(id)
                        || run
                            .get("formatRevision")
                            .and_then(|r| r.get("id"))
                            .and_then(|v| v.as_str())
                            == Some(id)
                })
                .map(|(i, _)| i)
                .collect()
        })
        .unwrap_or_default()
}

fn merge_into_next(blocks: &mut Vec<Value>, paragraph_id: &str) -> Result<(), String> {
    let index = blocks
        .iter()
        .position(|b| {
            b.get("type").and_then(|t| t.as_str()) == Some("paragraph")
                && b.get("id").and_then(|id| id.as_str()) == Some(paragraph_id)
        })
        .ok_or_else(|| format!("paragraph {paragraph_id} missing"))?;
    let next = blocks.get(index + 1);
    if next.is_none()
        || next.and_then(|b| b.get("type")).and_then(|t| t.as_str()) != Some("paragraph")
    {
        return Err(format!(
            "Cannot resolve the paragraph mark revision on paragraph {paragraph_id}: there is no following paragraph to merge with (merging across a table boundary is not supported)."
        ));
    }
    let current = blocks[index].clone();
    let next = blocks[index + 1].clone();
    let current_runs = current
        .get("runs")
        .and_then(|r| r.as_array())
        .cloned()
        .unwrap_or_default();
    let next_runs = next
        .get("runs")
        .and_then(|r| r.as_array())
        .cloned()
        .unwrap_or_default();
    let mut merged = next.clone();
    if let Some(obj) = merged.as_object_mut() {
        obj.insert("runs".into(), json!([current_runs, next_runs].concat()));
    }
    blocks[index] = merged;
    blocks.remove(index + 1);
    Ok(())
}

fn paragraph_has_mark(paragraph: &Value, id: &str) -> bool {
    paragraph
        .get("markRevision")
        .and_then(|r| r.get("id"))
        .and_then(|v| v.as_str())
        == Some(id)
}

fn clear_mark_revision(paragraph: &mut Value, id: &str) {
    if paragraph_has_mark(paragraph, id) {
        if let Some(obj) = paragraph.as_object_mut() {
            obj.remove("markRevision");
        }
    }
}

fn resolve_paragraph_mark(
    blocks: &mut Vec<Value>,
    id: &str,
    keep_break: bool,
) -> Result<(), String> {
    if blocks.iter().any(|b| {
        b.get("type").and_then(|t| t.as_str()) == Some("paragraph") && paragraph_has_mark(b, id)
    }) {
        if keep_break {
            for p in blocks.iter_mut() {
                if p.get("type").and_then(|t| t.as_str()) == Some("paragraph") {
                    clear_mark_revision(p, id);
                }
            }
            return Ok(());
        }
        let paragraph_id = blocks
            .iter()
            .find(|p| {
                p.get("type").and_then(|t| t.as_str()) == Some("paragraph")
                    && paragraph_has_mark(p, id)
            })
            .and_then(|p| p.get("id"))
            .and_then(|v| v.as_str())
            .ok_or_else(|| format!("No paragraph mark revision with id {id} was found."))?
            .to_owned();
        merge_into_next(blocks, &paragraph_id)?;
        return Ok(());
    }
    for block in blocks.iter_mut() {
        if block.get("type").and_then(|t| t.as_str()) != Some("table") {
            continue;
        }
        if let Some(rows) = block.get_mut("rows").and_then(|r| r.as_array_mut()) {
            for row in rows {
                if let Some(cells) = row.as_array_mut() {
                    for cell in cells {
                        if let Some(paras) =
                            cell.get_mut("paragraphs").and_then(|p| p.as_array_mut())
                        {
                            if !paras.iter().any(|p| paragraph_has_mark(p, id)) {
                                continue;
                            }
                            if keep_break {
                                for p in paras.iter_mut() {
                                    clear_mark_revision(p, id);
                                }
                                return Ok(());
                            }
                            let paragraph_id = paras
                                .iter()
                                .find(|p| paragraph_has_mark(p, id))
                                .and_then(|p| p.get("id"))
                                .and_then(|v| v.as_str())
                                .ok_or_else(|| {
                                    format!("No paragraph mark revision with id {id} was found.")
                                })?
                                .to_owned();
                            merge_into_next(paras, &paragraph_id)?;
                            return Ok(());
                        }
                    }
                }
            }
        }
    }
    Err(format!(
        "No paragraph mark revision with id {id} was found."
    ))
}

pub fn linked_revision_ids(blocks: &[Value], id: &str) -> Vec<String> {
    let entries = list_revisions(blocks);
    let move_name = entries
        .iter()
        .find(|e| e.id == id)
        .and_then(|e| e.move_name.clone());
    if move_name.is_none() {
        return vec![id.to_owned()];
    }
    let name = move_name.unwrap();
    let ids: Vec<String> = entries
        .iter()
        .filter(|e| {
            (e.kind == "moveFrom" || e.kind == "moveTo") && e.move_name.as_deref() == Some(&name)
        })
        .map(|e| e.id.clone())
        .collect();
    let mut all = vec![id.to_owned()];
    all.extend(ids);
    all.sort();
    all.dedup();
    all
}

fn for_each_paragraph_mut(blocks: &mut [Value], mut f: impl FnMut(&mut Value)) {
    for block in blocks.iter_mut() {
        let ty = block.get("type").and_then(|t| t.as_str()).unwrap_or("");
        if ty == "paragraph" {
            f(block);
        } else if ty == "table" {
            if let Some(rows) = block.get_mut("rows").and_then(|r| r.as_array_mut()) {
                for row in rows {
                    if let Some(cells) = row.as_array_mut() {
                        for cell in cells {
                            if let Some(paras) =
                                cell.get_mut("paragraphs").and_then(|p| p.as_array_mut())
                            {
                                for p in paras {
                                    f(p);
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

fn update_runs(
    blocks: &mut [Value],
    id: &str,
    mut apply: impl FnMut(&mut Value, usize) -> Result<(), String>,
) -> Result<(), String> {
    let mut found = false;
    let mut err: Option<String> = None;
    for_each_paragraph_mut(blocks, |paragraph| {
        if err.is_some() {
            return;
        }
        let indices = matching_run_indices(paragraph, id);
        if indices.is_empty() {
            return;
        }
        found = true;
        for index in indices.into_iter().rev() {
            if let Err(e) = apply(paragraph, index) {
                err = Some(e);
                return;
            }
        }
        if paragraph
            .get("runs")
            .and_then(|r| r.as_array())
            .map(|r| r.is_empty())
            .unwrap_or(true)
        {
            if let Some(obj) = paragraph.as_object_mut() {
                obj.insert("runs".into(), json!([{ "text": "" }]));
            }
        }
    });
    if let Some(e) = err {
        return Err(e);
    }
    if found {
        Ok(())
    } else {
        Err(format!("No run revision with id {id} was found."))
    }
}

fn clear_run_revision(paragraph: &mut Value, run_index: usize) {
    if let Some(run) = paragraph
        .get_mut("runs")
        .and_then(|r| r.as_array_mut())
        .and_then(|rs| rs.get_mut(run_index))
    {
        if let Some(obj) = run.as_object_mut() {
            obj.remove("revision");
        }
    }
}

fn accept_one(blocks: &mut Vec<Value>, id: &str) -> Result<(), String> {
    let entry =
        find_revision(blocks, id).ok_or_else(|| format!("No revision with id {id} was found."))?;
    if entry.kind == "formatChange" {
        return update_runs(blocks, id, |paragraph, run_index| {
            if let Some(run) = paragraph
                .get_mut("runs")
                .and_then(|r| r.as_array_mut())
                .and_then(|rs| rs.get_mut(run_index))
            {
                if run
                    .get("formatRevision")
                    .and_then(|r| r.get("id"))
                    .and_then(|v| v.as_str())
                    == Some(id)
                {
                    if let Some(obj) = run.as_object_mut() {
                        obj.remove("formatRevision");
                    }
                } else if let Some(obj) = run.as_object_mut() {
                    obj.remove("revision");
                }
            }
            Ok(())
        });
    }
    if entry.kind == "paragraphChange" {
        let mut done = false;
        for_each_paragraph_mut(blocks, |paragraph| {
            if paragraph
                .get("formatRevision")
                .and_then(|r| r.get("id"))
                .and_then(|v| v.as_str())
                == Some(id)
            {
                if let Some(obj) = paragraph.as_object_mut() {
                    obj.remove("formatRevision");
                }
                done = true;
            }
        });
        return if done {
            Ok(())
        } else {
            Err(format!("No revision with id {id} was found."))
        };
    }
    if entry.run_index.is_none() {
        let keep = entry.kind == "insert";
        return resolve_paragraph_mark(blocks, id, keep);
    }
    if entry.kind == "insert" || entry.kind == "moveTo" {
        return update_runs(blocks, id, |paragraph, run_index| {
            clear_run_revision(paragraph, run_index);
            Ok(())
        });
    }
    update_runs(blocks, id, |paragraph, run_index| {
        if let Some(runs) = paragraph.get_mut("runs").and_then(|r| r.as_array_mut()) {
            runs.remove(run_index);
        }
        Ok(())
    })
}

fn reject_one(blocks: &mut Vec<Value>, id: &str) -> Result<(), String> {
    let entry =
        find_revision(blocks, id).ok_or_else(|| format!("No revision with id {id} was found."))?;
    if entry.kind == "formatChange" {
        return update_runs(blocks, id, |paragraph, run_index| {
            if let Some(run) = paragraph
                .get_mut("runs")
                .and_then(|r| r.as_array_mut())
                .and_then(|rs| rs.get_mut(run_index))
            {
                restore_run_formatting(run)?;
            }
            Ok(())
        });
    }
    if entry.kind == "paragraphChange" {
        let mut done = false;
        let mut err: Option<String> = None;
        for_each_paragraph_mut(blocks, |paragraph| {
            if paragraph
                .get("formatRevision")
                .and_then(|r| r.get("id"))
                .and_then(|v| v.as_str())
                == Some(id)
            {
                if let Err(e) = restore_paragraph_formatting(paragraph) {
                    err = Some(e);
                } else {
                    done = true;
                }
            }
        });
        if let Some(e) = err {
            return Err(e);
        }
        return if done {
            Ok(())
        } else {
            Err(format!("No revision with id {id} was found."))
        };
    }
    if entry.run_index.is_none() {
        let keep = entry.kind == "delete";
        return resolve_paragraph_mark(blocks, id, keep);
    }
    if entry.kind == "insert" || entry.kind == "moveTo" {
        return update_runs(blocks, id, |paragraph, run_index| {
            if let Some(runs) = paragraph.get_mut("runs").and_then(|r| r.as_array_mut()) {
                runs.remove(run_index);
            }
            Ok(())
        });
    }
    update_runs(blocks, id, |paragraph, run_index| {
        clear_run_revision(paragraph, run_index);
        Ok(())
    })
}

fn resolve_linked(
    blocks: &mut Vec<Value>,
    id: &str,
    resolve: fn(&mut Vec<Value>, &str) -> Result<(), String>,
) -> Result<(), String> {
    if find_revision(blocks, id).is_none() {
        return Err(format!("No revision with id {id} was found."));
    }
    for linked in linked_revision_ids(blocks, id) {
        while find_revision(blocks, &linked).is_some() {
            resolve(blocks, &linked)?;
        }
    }
    Ok(())
}

pub fn accept_revision(host: &mut DocxHost, id: &str) -> Result<(), EditError> {
    resolve_linked(&mut host.blocks, id, accept_one)
        .map_err(|e| EditError::Other { message: e })?;
    host.mark_all_blocks_touched();
    Ok(())
}

pub fn reject_revision(host: &mut DocxHost, id: &str) -> Result<(), EditError> {
    resolve_linked(&mut host.blocks, id, reject_one)
        .map_err(|e| EditError::Other { message: e })?;
    host.mark_all_blocks_touched();
    Ok(())
}

fn apply_all(
    host: &mut DocxHost,
    apply: fn(&mut DocxHost, &str) -> Result<(), EditError>,
) -> Result<(), EditError> {
    let entries = list_revisions(&host.blocks);
    for entry in entries.into_iter().rev() {
        if find_revision(&host.blocks, &entry.id).is_some() {
            apply(host, &entry.id)?;
        }
    }
    Ok(())
}

pub fn accept_all_revisions(host: &mut DocxHost) -> Result<(), EditError> {
    apply_all(host, accept_revision)
}

pub fn reject_all_revisions(host: &mut DocxHost) -> Result<(), EditError> {
    apply_all(host, reject_revision)
}

pub fn register_revision_commands(registry: &mut CommandRegistry) {
    registry.register(
        Command::stub("acceptRevision", "acceptRevision").with_runner(|host, args| {
            let h = host_mut(host)?;
            let id = args
                .get("id")
                .and_then(|v| v.as_str())
                .ok_or_else(|| EditError::Other {
                    message: "acceptRevision missing id".into(),
                })?;
            accept_revision(h, id)?;
            let mut cs = ChangeSet::new(ChangeKind::Other, format!("acceptRevision {id}"));
            cs.dirty_parts = vec!["word/document.xml".into()];
            Ok(cs)
        }),
    );
    registry.register(
        Command::stub("rejectRevision", "rejectRevision").with_runner(|host, args| {
            let h = host_mut(host)?;
            let id = args
                .get("id")
                .and_then(|v| v.as_str())
                .ok_or_else(|| EditError::Other {
                    message: "rejectRevision missing id".into(),
                })?;
            reject_revision(h, id)?;
            let mut cs = ChangeSet::new(ChangeKind::Other, format!("rejectRevision {id}"));
            cs.dirty_parts = vec!["word/document.xml".into()];
            Ok(cs)
        }),
    );
    registry.register(
        Command::stub("acceptAllRevisions", "acceptAllRevisions").with_runner(|host, _args| {
            let h = host_mut(host)?;
            accept_all_revisions(h)?;
            let mut cs = ChangeSet::new(ChangeKind::Other, "acceptAllRevisions");
            cs.dirty_parts = vec!["word/document.xml".into()];
            Ok(cs)
        }),
    );
    registry.register(
        Command::stub("rejectAllRevisions", "rejectAllRevisions").with_runner(|host, _args| {
            let h = host_mut(host)?;
            reject_all_revisions(h)?;
            let mut cs = ChangeSet::new(ChangeKind::Other, "rejectAllRevisions");
            cs.dirty_parts = vec!["word/document.xml".into()];
            Ok(cs)
        }),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::path::PathBuf;

    #[test]
    fn delins_fixture_lists_revisions() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
        let manifest = std::fs::read_to_string(root.join("fixtures/manifest.json")).unwrap();
        let v: Value = serde_json::from_str(&manifest).unwrap();
        let row = v["fixtures"]
            .as_array()
            .unwrap()
            .iter()
            .find(|f| f["id"] == "real-docx-poi-delins")
            .unwrap();
        let bytes = std::fs::read(root.join(row["path"].as_str().unwrap())).unwrap();
        let loaded = crate::body::read_package(&bytes).unwrap();
        let revs = list_revisions(&loaded.model.blocks);
        assert!(!revs.is_empty(), "poi-delins should expose revisions");
    }

    #[test]
    fn catches_accept_revision_unknown_id_errors() {
        // Catches: acceptRevision with a missing revision id must fail like upstream TS (#82).
        let mut host = DocxHost::stub(vec![json!({
            "type": "paragraph",
            "id": "p0",
            "runs": [{ "text": "hi" }]
        })]);
        let err = accept_revision(&mut host, "missing").unwrap_err();
        assert!(err.to_string().contains("No revision"));
    }
}
