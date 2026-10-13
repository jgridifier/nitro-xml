// SPDX-License-Identifier: MIT
//! 1,000-op edit sessions with save / reopen / undo invariants (#90).

use nitro_xml_opc::edit::{
    parse_edit_script, replay_script, CommandRegistry, EditHistory, EditHost, EditSession,
};
use serde::Serialize;
use serde_json::{json, Value};

pub const OPS: usize = 1000;
const HISTORY_LIMIT: usize = 100;
const PRNG_SEED: u32 = 0x0090_0090;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Xlsx,
    Docx,
    Pptx,
}

impl Format {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Xlsx => "xlsx",
            Self::Docx => "docx",
            Self::Pptx => "pptx",
        }
    }
}

#[derive(Debug, Serialize)]
pub struct FormatResult {
    pub format: String,
    pub ok: bool,
    pub ops_attempted: usize,
    pub ops_applied: usize,
    pub undos: usize,
    pub redos: usize,
    pub history_commits: usize,
    pub error: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct HarnessReport {
    pub ops: usize,
    pub history_limit: usize,
    pub negative_control_ok: bool,
    pub formats: Vec<FormatResult>,
}

pub struct Fixtures {
    pub xlsx: Vec<u8>,
    pub docx: Vec<u8>,
    pub pptx: Vec<u8>,
}

pub fn run_all(fixtures: &Fixtures) -> HarnessReport {
    let negative_control_ok = run_negative_controls(fixtures).is_ok();
    let formats = [
        run_format(Format::Xlsx, &fixtures.xlsx),
        run_format(Format::Docx, &fixtures.docx),
        run_format(Format::Pptx, &fixtures.pptx),
    ]
    .into_iter()
    .collect();
    HarnessReport {
        ops: OPS,
        history_limit: HISTORY_LIMIT,
        negative_control_ok,
        formats,
    }
}

pub fn run_negative_controls(fixtures: &Fixtures) -> Result<(), String> {
    for (format, bytes) in [
        (Format::Xlsx, &fixtures.xlsx),
        (Format::Docx, &fixtures.docx),
        (Format::Pptx, &fixtures.pptx),
    ] {
        if !negative_control_host_drift(format, bytes)? {
            return Err(format!(
                "negative control for {} did not detect host snapshot drift",
                format.as_str()
            ));
        }
        let start = open_snapshot(format, bytes)?;
        let wrong = json!({ "deliberately": "wrong" });
        if verify_undo_walk(&start, &[wrong], &start).is_ok() {
            return Err(format!(
                "negative control for {} did not detect undo-stack mismatch",
                format.as_str()
            ));
        }
    }
    Ok(())
}

fn negative_control_host_drift(format: Format, fixture: &[u8]) -> Result<bool, String> {
    match format {
        Format::Xlsx => {
            use nitro_xml_xlsx::edit::XlsxHost;
            let mut host = XlsxHost::open(fixture).map_err(|e| e.to_string())?;
            let start = host.undo_parity_snapshot();
            if let Some(sheet) = host.sheets.first_mut() {
                sheet.name = "NEG-CONTROL".into();
            }
            Ok(host.undo_parity_snapshot() != start)
        }
        Format::Docx => {
            use nitro_xml_docx::edit::DocxHost;
            let mut host = DocxHost::open(fixture).map_err(|e| e.to_string())?;
            let start = host.undo_parity_snapshot();
            if let Some(block) = host.blocks.first_mut() {
                if let Some(runs) = block.get_mut("runs").and_then(|r| r.as_array_mut()) {
                    if let Some(run) = runs.first_mut() {
                        if let Some(obj) = run.as_object_mut() {
                            obj.insert("text".into(), json!("NEG-CONTROL"));
                        }
                    }
                }
            }
            Ok(host.undo_parity_snapshot() != start)
        }
        Format::Pptx => {
            use nitro_xml_pptx::edit::PptxHost;
            let mut host = PptxHost::open(fixture).map_err(|e| e.to_string())?;
            let start = host.undo_parity_snapshot();
            if let Ok(slide) = host.slide_mut(0) {
                if let Some(el) = slide.elements.first_mut() {
                    el.text = Some("NEG-CONTROL".into());
                }
            }
            Ok(host.undo_parity_snapshot() != start)
        }
    }
}

fn run_format(format: Format, fixture: &[u8]) -> FormatResult {
    match run_format_inner(format, fixture) {
        Ok(stats) => FormatResult {
            format: format.as_str().into(),
            ok: true,
            ops_attempted: stats.ops_attempted,
            ops_applied: stats.ops_applied,
            undos: stats.undos,
            redos: stats.redos,
            history_commits: stats.history_commits,
            error: None,
        },
        Err(e) => FormatResult {
            format: format.as_str().into(),
            ok: false,
            ops_attempted: OPS,
            ops_applied: 0,
            undos: 0,
            redos: 0,
            history_commits: 0,
            error: Some(e),
        },
    }
}

#[derive(Debug)]
struct SessionStats {
    ops_attempted: usize,
    ops_applied: usize,
    undos: usize,
    redos: usize,
    history_commits: usize,
}

fn run_format_inner(format: Format, fixture: &[u8]) -> Result<SessionStats, String> {
    let data = seeded_bytes(PRNG_SEED);
    match format {
        Format::Xlsx => run_xlsx(fixture, &data),
        Format::Docx => run_docx(fixture, &data),
        Format::Pptx => run_pptx(fixture, &data),
    }
}

fn seeded_bytes(seed: u32) -> Vec<u8> {
    let mut s = seed;
    let mut out = Vec::with_capacity(OPS * 4);
    for _ in 0..OPS * 4 {
        s ^= s << 13;
        s ^= s >> 17;
        s ^= s << 5;
        out.push((s & 0xff) as u8);
    }
    out
}

fn run_xlsx(fixture: &[u8], data: &[u8]) -> Result<SessionStats, String> {
    use nitro_xml_xlsx::edit::{
        register_cell_commands, register_shift_commands, register_structure_commands, XlsxHost,
    };
    use nitro_xml_xlsx::{load_workbook, save_xlsx};

    let host = XlsxHost::open(fixture).map_err(|e| e.to_string())?;
    let mut registry = CommandRegistry::with_schema_stubs();
    register_cell_commands(&mut registry);
    register_structure_commands(&mut registry);
    register_shift_commands(&mut registry);
    let mut session = EditSession::new(host, EditHistory::new());
    let start = session.host.undo_parity_snapshot();
    let sheet_count = session.host.sheets.len();

    let (stats, checkpoints) = apply_stream(
        Format::Xlsx,
        data,
        &mut session,
        &registry,
        &start,
        |b0, b1, b2, b3| match b0 % 4 {
            0 => Some(json!({
                "op": "setCellInput",
                "sheet": 0,
                "row": (b1 as u32) % 8,
                "col": (b2 as u32) % 8,
                "text": format!("x{b3}"),
            })),
            1 => Some(json!({
                "op": "setCellValue",
                "sheet": 0,
                "row": (b1 as u32) % 8,
                "col": (b2 as u32) % 8,
                "value": (b3 as i64) % 100,
            })),
            _ => None,
        },
    )?;

    assert_history_limit(&session)?;

    let saved = save_xlsx(&session.host, fixture).map_err(|e| e.to_string())?;
    let reopened = load_workbook(&saved).map_err(|e| e.to_string())?;
    if reopened.sheets.len() != sheet_count {
        return Err(format!(
            "xlsx reopen sheet count {} != {sheet_count}",
            reopened.sheets.len()
        ));
    }

    drain_undo_stack(&mut session, &start, &checkpoints, stats.ops_applied)?;
    Ok(stats)
}

fn run_docx(fixture: &[u8], data: &[u8]) -> Result<SessionStats, String> {
    use nitro_xml_docx::edit::{register_docx_command_registry, DocxHost};
    use nitro_xml_docx::save_docx;

    let host = DocxHost::open(fixture).map_err(|e| e.to_string())?;
    let mut registry = CommandRegistry::with_schema_stubs();
    register_docx_command_registry(&mut registry);
    let mut session = EditSession::new(host, EditHistory::new());
    let start = session.host.undo_parity_snapshot();
    let block_count = session.host.blocks.len().max(1);

    let (stats, checkpoints) = apply_stream(
        Format::Docx,
        data,
        &mut session,
        &registry,
        &start,
        |b0, b1, b2, b3| match b0 % 4 {
            0 => Some(json!({
                "op": "setParagraphText",
                "blockIndex": (b1 as u32) % (block_count as u32),
                "text": format!("d{b2}-{b3}"),
            })),
            1 => Some(json!({
                "op": "setRunFormatting",
                "blockIndex": (b1 as u32) % (block_count as u32),
                "runIndex": 0,
                "bold": b2 % 2 == 0,
            })),
            _ => None,
        },
    )?;

    assert_history_limit(&session)?;
    let expected_blocks = session.host.blocks.len();

    let saved = save_docx(&session.host, fixture).map_err(|e| e.to_string())?;
    let reopened = DocxHost::open(&saved).map_err(|e| e.to_string())?;
    if reopened.blocks.len() != expected_blocks {
        return Err(format!(
            "docx reopen block count {} != {expected_blocks}",
            reopened.blocks.len()
        ));
    }

    drain_undo_stack(&mut session, &start, &checkpoints, stats.ops_applied)?;
    Ok(stats)
}

fn run_pptx(fixture: &[u8], data: &[u8]) -> Result<SessionStats, String> {
    use nitro_xml_pptx::edit::{register_shape_commands, PptxHost};
    use nitro_xml_pptx::{save_pptx, Presentation};

    let host = PptxHost::open(fixture).map_err(|e| e.to_string())?;
    let mut registry = CommandRegistry::with_schema_stubs();
    register_shape_commands(&mut registry);
    let mut session = EditSession::new(host, EditHistory::new());
    let start = session.host.undo_parity_snapshot();
    let slide_count = session.host.slides.len();

    let (stats, checkpoints) = apply_stream(
        Format::Pptx,
        data,
        &mut session,
        &registry,
        &start,
        |b0, b1, b2, _b3| match b0 % 4 {
            0 => Some(json!({
                "op": "setShapeText",
                "slideIndex": 0,
                "elementIndex": 0,
                "text": format!("p{b1}{b2}"),
            })),
            1 => Some(json!({
                "op": "patchShapeGeometry",
                "slideIndex": 0,
                "elementIndex": 0,
                "x": 10.0 + f64::from(b1),
                "y": 10.0 + f64::from(b2),
            })),
            _ => None,
        },
    )?;

    assert_history_limit(&session)?;

    let saved = save_pptx(&session.host, fixture).map_err(|e| e.to_string())?;
    let pres = Presentation::open(&saved).map_err(|e| e.to_string())?;
    if pres.slide_count() != slide_count {
        return Err(format!(
            "pptx reopen slide count {} != {slide_count}",
            pres.slide_count()
        ));
    }

    drain_undo_stack(&mut session, &start, &checkpoints, stats.ops_applied)?;
    Ok(stats)
}

fn assert_history_limit<H: EditHost>(session: &EditSession<H, EditHistory>) -> Result<(), String> {
    let commits = session.history.committed().len();
    if commits > HISTORY_LIMIT {
        return Err(format!(
            "history commits {commits} exceeds TS limit {HISTORY_LIMIT}"
        ));
    }
    Ok(())
}

fn apply_stream<H, F>(
    format: Format,
    data: &[u8],
    session: &mut EditSession<H, EditHistory>,
    registry: &CommandRegistry,
    start: &Value,
    mut make_op: F,
) -> Result<(SessionStats, Vec<Value>), String>
where
    H: EditHost + UndoSnap,
    F: FnMut(u8, u8, u8, u8) -> Option<Value>,
{
    let mut stats = SessionStats {
        ops_attempted: 0,
        ops_applied: 0,
        undos: 0,
        redos: 0,
        history_commits: 0,
    };
    let mut checkpoints: Vec<Value> = Vec::new();

    let chunks = data.chunks_exact(4).take(OPS);
    for (op_index, chunk) in chunks.enumerate() {
        let (b0, b1, b2, b3) = (chunk[0], chunk[1], chunk[2], chunk[3]);
        stats.ops_attempted += 1;

        match b0 % 10 {
            0 | 1 => {
                if session.can_undo() {
                    session.undo();
                    stats.undos += 1;
                    if checkpoints.pop().is_none() {
                        return Err("checkpoint/history desync on undo".into());
                    }
                    if let Some(expected) = checkpoints.last() {
                        if session.host.snap() != *expected {
                            return Err(format!(
                                "undo host snapshot does not match checkpoint tail at op {op_index}"
                            ));
                        }
                    }
                }
                continue;
            }
            2 | 3 => {
                if session.can_redo() {
                    session.redo();
                    stats.redos += 1;
                    let snap = session.host.snap();
                    checkpoints.push(snap);
                    if checkpoints
                        .last()
                        .is_some_and(|last| session.host.snap() != *last)
                    {
                        return Err("undo_parity_snapshot unstable immediately after redo".into());
                    }
                    trim_checkpoints(&mut checkpoints);
                }
                continue;
            }
            _ => {}
        }

        let Some(op) = make_op(b0, b1, b2, b3) else {
            continue;
        };
        let script = json!({
            "id": "long-session",
            "format": format.as_str(),
            "fixture": "long-session",
            "ops": [op],
        });
        let script_json = serde_json::to_string(&script).map_err(|e| e.to_string())?;
        let parsed = parse_edit_script(&script_json).map_err(|e| e.to_string())?;
        let result = replay_script(&parsed, registry, session);
        if result.status == "ok" {
            for op in &result.ops {
                if op.status == "ok" {
                    stats.ops_applied += 1;
                    let snap = session.host.snap();
                    checkpoints.push(snap);
                    trim_checkpoints(&mut checkpoints);
                }
            }
        }
    }

    stats.history_commits = session.history.committed().len();
    if checkpoints.len() != session.history.committed().len() {
        return Err(format!(
            "checkpoint stack len {} != history commits {} (applied {})",
            checkpoints.len(),
            session.history.committed().len(),
            stats.ops_applied
        ));
    }
    Ok((stats, checkpoints))
}

fn trim_checkpoints(checkpoints: &mut Vec<Value>) {
    while checkpoints.len() > HISTORY_LIMIT {
        checkpoints.remove(0);
    }
}

fn drain_undo_stack<H: EditHost + UndoSnap>(
    session: &mut EditSession<H, EditHistory>,
    start: &Value,
    checkpoints: &[Value],
    ops_applied: usize,
) -> Result<(), String> {
    verify_undo_walk(start, checkpoints, &session.host.snap())?;

    let mut tail = checkpoints.to_vec();
    while session.can_undo() {
        session.undo();
        if tail.is_empty() {
            return Err("undo with empty checkpoint stack".into());
        }
        tail.pop();
        if let Some(expected) = tail.last() {
            if session.host.snap() != *expected {
                return Err("undo-parity snapshot drift on undo step".into());
            }
        }
    }

    if ops_applied <= HISTORY_LIMIT && session.host.snap() != *start {
        return Err("undo-all did not restore open snapshot".into());
    }

    let snap_before = session.host.snap();
    if session.undo().is_some() {
        return Err("undo past empty stack returned a step".into());
    }
    if session.host.snap() != snap_before {
        return Err("undo on empty stack mutated host".into());
    }
    Ok(())
}

fn verify_undo_walk(start: &Value, after_commits: &[Value], current: &Value) -> Result<(), String> {
    if after_commits.is_empty() {
        if current != start {
            return Err("snapshot drift with empty commit stack".into());
        }
        return Ok(());
    }
    if *current != after_commits[after_commits.len() - 1] {
        return Err("current snapshot does not match last commit checkpoint".into());
    }
    Ok(())
}

trait UndoSnap {
    fn snap(&self) -> Value;
}

impl UndoSnap for nitro_xml_xlsx::edit::XlsxHost {
    fn snap(&self) -> Value {
        self.undo_parity_snapshot()
    }
}
impl UndoSnap for nitro_xml_docx::edit::DocxHost {
    fn snap(&self) -> Value {
        self.undo_parity_snapshot()
    }
}
impl UndoSnap for nitro_xml_pptx::edit::PptxHost {
    fn snap(&self) -> Value {
        self.undo_parity_snapshot()
    }
}

fn open_snapshot(format: Format, fixture: &[u8]) -> Result<Value, String> {
    match format {
        Format::Xlsx => {
            use nitro_xml_xlsx::edit::XlsxHost;
            Ok(XlsxHost::open(fixture)
                .map_err(|e| e.to_string())?
                .undo_parity_snapshot())
        }
        Format::Docx => {
            use nitro_xml_docx::edit::DocxHost;
            Ok(DocxHost::open(fixture)
                .map_err(|e| e.to_string())?
                .undo_parity_snapshot())
        }
        Format::Pptx => {
            use nitro_xml_pptx::edit::PptxHost;
            Ok(PptxHost::open(fixture)
                .map_err(|e| e.to_string())?
                .undo_parity_snapshot())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nitro_xml_opc::edit::{CommandRegistry, EditHistory, EditSession};
    use nitro_xml_xlsx::edit::{
        register_cell_commands, register_shift_commands, register_structure_commands, XlsxHost,
    };
    use std::path::PathBuf;

    #[test]
    fn catches_single_commit_undo_matches_checkpoint() {
        use nitro_xml_opc::edit::{
            parse_edit_script, replay_script, CommandRegistry, EditHistory, EditSession,
        };
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
        let bytes = std::fs::read(root.join("fixtures/generated/xlsx/cells_1k.xlsx")).unwrap();
        let host = XlsxHost::open(&bytes).unwrap();
        let mut registry = CommandRegistry::with_schema_stubs();
        register_cell_commands(&mut registry);
        register_structure_commands(&mut registry);
        register_shift_commands(&mut registry);
        let mut session = EditSession::new(host, EditHistory::new());
        let start = session.host.undo_parity_snapshot();
        let script = json!({
            "id": "one",
            "format": "xlsx",
            "fixture": "t",
            "ops": [{"op": "setCellInput", "sheet": 0, "row": 1, "col": 1, "text": "z"}]
        });
        let parsed = parse_edit_script(&serde_json::to_string(&script).unwrap()).unwrap();
        let report = replay_script(&parsed, &registry, &mut session);
        assert_eq!(report.status, "ok");
        let after = session.host.undo_parity_snapshot();
        session.undo();
        let undone = session.host.undo_parity_snapshot();
        assert_eq!(undone, start, "undo should restore start");
        session.redo();
        assert_eq!(session.host.undo_parity_snapshot(), after);
    }

    #[test]
    fn catches_long_session_xlsx_end_to_end() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
        let bytes = std::fs::read(root.join("fixtures/generated/xlsx/cells_1k.xlsx")).unwrap();
        run_xlsx(&bytes, &seeded_bytes(PRNG_SEED)).expect("xlsx long session");
    }

    #[test]
    fn catches_long_session_xlsx_apply_stream() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
        let bytes = std::fs::read(root.join("fixtures/generated/xlsx/cells_1k.xlsx")).unwrap();
        let data = seeded_bytes(PRNG_SEED);
        let host = XlsxHost::open(&bytes).unwrap();
        let mut registry = CommandRegistry::with_schema_stubs();
        register_cell_commands(&mut registry);
        register_structure_commands(&mut registry);
        register_shift_commands(&mut registry);
        let mut session = EditSession::new(host, EditHistory::new());
        let start = session.host.undo_parity_snapshot();
        let (stats, _cps) = apply_stream(
            Format::Xlsx,
            &data,
            &mut session,
            &registry,
            &start,
            |b0, b1, b2, b3| match b0 % 4 {
                0 => Some(json!({
                    "op": "setCellInput",
                    "sheet": 0,
                    "row": (b1 as u32) % 8,
                    "col": (b2 as u32) % 8,
                    "text": format!("x{b3}"),
                })),
                1 => Some(json!({
                    "op": "setCellValue",
                    "sheet": 0,
                    "row": (b1 as u32) % 8,
                    "col": (b2 as u32) % 8,
                    "value": (b3 as i64) % 100,
                })),
                _ => None,
            },
        )
        .expect("apply_stream failed");
        assert!(stats.ops_applied > 0);
        verify_undo_walk(&start, &_cps, &session.host.snap()).expect("end state vs checkpoints");
        drain_undo_stack(&mut session, &start, &_cps, stats.ops_applied).expect("drain undo");
    }
}
