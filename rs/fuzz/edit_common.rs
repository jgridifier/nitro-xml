// SPDX-License-Identifier: MIT
//! Save-path edit fuzz harness (#89): random command sequences, undo-to-start,
//! save and reopen. Shared by `edit_xlsx` / `edit_docx` / `edit_pptx` and `edit_ci`.

use nitro_xml_opc::edit::{
    parse_edit_script, replay_script, CommandRegistry, EditHistory, EditHost, EditSession,
};
use serde_json::{json, Value};

/// Cap ops per fuzz input so CI stays short (not a long campaign).
pub const MAX_OPS: usize = 12;
/// Deterministic CI sequences per format (seeded PRNG over the same decoder).
pub const CI_SEQUENCES: u32 = 24;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Xlsx = 0,
    Docx = 1,
    Pptx = 2,
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

#[derive(Debug, Default)]
pub struct SequenceReport {
    pub format: Format,
    pub ops_attempted: usize,
    pub ops_applied: usize,
    pub undos: usize,
    pub redos: usize,
}

impl Default for Format {
    fn default() -> Self {
        Self::Xlsx
    }
}

/// Decode `data` into a short command stream and exercise one format.
/// Panics on undo-to-start drift or save/reopen failure (libFuzzer signal).
pub fn fuzz_format(format: Format, fixture: &[u8], data: &[u8]) {
    run_format(format, fixture, data).unwrap_or_else(|e| {
        panic!("edit fuzz {}: {e}", format.as_str());
    });
}

/// CI entry: run many short seeded sequences; return Err on the first failure.
pub fn ci_run_format(format: Format, fixture: &[u8]) -> Result<Vec<SequenceReport>, String> {
    let mut reports = Vec::with_capacity(CI_SEQUENCES as usize);
    for seed in 0..CI_SEQUENCES {
        let data = seeded_bytes(seed, format);
        reports.push(run_format(format, fixture, &data)?);
    }
    Ok(reports)
}

/// Negative control: prove the harness detects undo-to-start breakage.
pub fn negative_control(format: Format, fixture: &[u8]) -> Result<(), String> {
    let diverged = force_diverge_without_history(format, fixture)?;
    if diverged {
        Ok(())
    } else {
        Err(format!(
            "negative control for {} did not detect undo-to-start breakage",
            format.as_str()
        ))
    }
}

fn seeded_bytes(seed: u32, format: Format) -> Vec<u8> {
    let mut s = seed
        .wrapping_mul(0x9E37_79B9)
        .wrapping_add(format as u32 + 1);
    let mut out = Vec::with_capacity(MAX_OPS * 4);
    for _ in 0..MAX_OPS * 4 {
        s ^= s << 13;
        s ^= s >> 17;
        s ^= s << 5;
        out.push((s & 0xff) as u8);
    }
    out
}

fn run_format(format: Format, fixture: &[u8], data: &[u8]) -> Result<SequenceReport, String> {
    match format {
        Format::Xlsx => run_xlsx(fixture, data),
        Format::Docx => run_docx(fixture, data),
        Format::Pptx => run_pptx(fixture, data),
    }
}

fn force_diverge_without_history(format: Format, fixture: &[u8]) -> Result<bool, String> {
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
            // Mutate projected run text so undo_parity_snapshot diverges.
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

fn run_xlsx(fixture: &[u8], data: &[u8]) -> Result<SequenceReport, String> {
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

    let mut report = SequenceReport {
        format: Format::Xlsx,
        ..SequenceReport::default()
    };

    apply_stream(
        Format::Xlsx,
        data,
        &mut session,
        &registry,
        &mut report,
        |b0, b1, b2, b3| match b0 % 5 {
            0 | 1 => Some(json!({
                "op": "setCellInput",
                "sheet": 0,
                "row": (b1 as u32) % 8,
                "col": (b2 as u32) % 8,
                "text": format!("f{b3}"),
            })),
            2 => Some(json!({
                "op": "setCellValue",
                "sheet": 0,
                "row": (b1 as u32) % 8,
                "col": (b2 as u32) % 8,
                "value": (b3 as i64) % 100,
            })),
            _ => None,
        },
        |b0| b0 % 5 == 3,
        |b0| b0 % 5 == 4,
    )?;

    undo_all_to_start(&mut session, &start, "xlsx")?;
    let saved = save_xlsx(&session.host, fixture).map_err(|e| e.to_string())?;
    if saved.as_slice() != fixture {
        return Err("xlsx save after undo-to-start is not byte-identical (gate a)".into());
    }
    let reopened = load_workbook(&saved).map_err(|e| e.to_string())?;
    if reopened.sheets.len() != sheet_count {
        return Err(format!(
            "xlsx reopen sheet count {} != {sheet_count}",
            reopened.sheets.len()
        ));
    }
    Ok(report)
}

fn run_docx(fixture: &[u8], data: &[u8]) -> Result<SequenceReport, String> {
    use nitro_xml_docx::edit::{register_docx_command_registry, DocxHost};
    use nitro_xml_docx::save_docx;

    let host = DocxHost::open(fixture).map_err(|e| e.to_string())?;
    let mut registry = CommandRegistry::with_schema_stubs();
    register_docx_command_registry(&mut registry);
    let mut session = EditSession::new(host, EditHistory::new());
    let start = session.host.undo_parity_snapshot();
    let block_count = session.host.blocks.len().max(1);

    let mut report = SequenceReport {
        format: Format::Docx,
        ..SequenceReport::default()
    };

    apply_stream(
        Format::Docx,
        data,
        &mut session,
        &registry,
        &mut report,
        |b0, b1, b2, b3| match b0 % 5 {
            0 | 1 => Some(json!({
                "op": "setParagraphText",
                "blockIndex": (b1 as u32) % (block_count as u32),
                "text": format!("d{b2}-{b3}"),
            })),
            2 => Some(json!({
                "op": "setRunFormatting",
                "blockIndex": (b1 as u32) % (block_count as u32),
                "runIndex": 0,
                "bold": b2 % 2 == 0,
            })),
            _ => None,
        },
        |b0| b0 % 5 == 3,
        |b0| b0 % 5 == 4,
    )?;

    let expected_blocks = session.host.blocks.len();
    undo_all_to_start(&mut session, &start, "docx")?;
    let saved = save_docx(&session.host, fixture).map_err(|e| e.to_string())?;
    if saved.as_slice() != fixture {
        return Err("docx save after undo-to-start is not byte-identical (gate a)".into());
    }
    let reopened = DocxHost::open(&saved).map_err(|e| e.to_string())?;
    if reopened.blocks.len() != expected_blocks {
        return Err(format!(
            "docx reopen block count {} != {expected_blocks}",
            reopened.blocks.len()
        ));
    }
    Ok(report)
}

fn run_pptx(fixture: &[u8], data: &[u8]) -> Result<SequenceReport, String> {
    use nitro_xml_pptx::edit::{register_shape_commands, PptxHost};
    use nitro_xml_pptx::write::save_pptx;
    use nitro_xml_pptx::Presentation;

    let host = PptxHost::open(fixture).map_err(|e| e.to_string())?;
    let mut registry = CommandRegistry::with_schema_stubs();
    register_shape_commands(&mut registry);
    let mut session = EditSession::new(host, EditHistory::new());
    let start = session.host.undo_parity_snapshot();
    let slide_count = session.host.slides.len().max(1);

    let mut report = SequenceReport {
        format: Format::Pptx,
        ..SequenceReport::default()
    };

    apply_stream(
        Format::Pptx,
        data,
        &mut session,
        &registry,
        &mut report,
        |b0, b1, b2, b3| match b0 % 5 {
            0 | 1 => Some(json!({
                "op": "setShapeText",
                "slideIndex": (b1 as u32) % (slide_count as u32),
                "elementIndex": 0,
                "text": format!("p{b2}{b3}"),
            })),
            2 => Some(json!({
                "op": "patchShapeGeometry",
                "slideIndex": (b1 as u32) % (slide_count as u32),
                "elementIndex": 0,
                "x": 10.0 + f64::from(b2),
                "y": 10.0 + f64::from(b3),
            })),
            _ => None,
        },
        |b0| b0 % 5 == 3,
        |b0| b0 % 5 == 4,
    )?;

    undo_all_to_start(&mut session, &start, "pptx")?;
    let saved = save_pptx(&session.host, fixture).map_err(|e| e.to_string())?;
    if saved.as_slice() != fixture {
        return Err("pptx save after undo-to-start is not byte-identical (gate a)".into());
    }
    let pres = Presentation::open(&saved).map_err(|e| e.to_string())?;
    if pres.slide_count() != slide_count {
        return Err(format!(
            "pptx reopen slide count {} != {slide_count}",
            pres.slide_count()
        ));
    }
    Ok(report)
}

fn undo_all_to_start<H>(
    session: &mut EditSession<H, EditHistory>,
    start: &Value,
    label: &str,
) -> Result<(), String>
where
    H: EditHost + UndoSnap,
{
    while session.can_undo() {
        session.undo();
    }
    let now = session.host.snap();
    if now != *start {
        return Err(format!("{label} undo-to-start invariant broken"));
    }
    Ok(())
}

/// Hosts that expose the undo-parity JSON snapshot used by gate (c).
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

fn apply_stream<H, F, U, R>(
    format: Format,
    data: &[u8],
    session: &mut EditSession<H, EditHistory>,
    registry: &CommandRegistry,
    report: &mut SequenceReport,
    mut make_op: F,
    is_undo: U,
    is_redo: R,
) -> Result<(), String>
where
    H: EditHost,
    F: FnMut(u8, u8, u8, u8) -> Option<Value>,
    U: Fn(u8) -> bool,
    R: Fn(u8) -> bool,
{
    let chunks = data.chunks_exact(4).take(MAX_OPS);
    for chunk in chunks {
        let (b0, b1, b2, b3) = (chunk[0], chunk[1], chunk[2], chunk[3]);
        report.ops_attempted += 1;
        if is_undo(b0) {
            if session.can_undo() {
                session.undo();
                report.undos += 1;
            }
            continue;
        }
        if is_redo(b0) {
            if session.can_redo() {
                session.redo();
                report.redos += 1;
            }
            continue;
        }
        let Some(op) = make_op(b0, b1, b2, b3) else {
            continue;
        };
        let script = json!({
            "id": "edit-fuzz",
            "format": format.as_str(),
            "fixture": "fuzz",
            "ops": [op],
        });
        let script_json = serde_json::to_string(&script).map_err(|e| e.to_string())?;
        let parsed = parse_edit_script(&script_json).map_err(|e| e.to_string())?;
        let before = session.history.committed().len();
        let result = replay_script(&parsed, registry, session);
        if result.status == "ok" && session.history.committed().len() > before {
            report.ops_applied += 1;
        }
    }
    Ok(())
}
