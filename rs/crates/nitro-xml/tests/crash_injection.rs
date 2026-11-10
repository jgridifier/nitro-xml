// SPDX-License-Identifier: MIT
//! Crash injection for the Rust editor (#30, #62): a real worker process is
//! aborted mid-edit, then the journal is replayed onto the original bytes.
//!
//! The test binary re-executes itself as the worker (`NITRO_CRASH_WORKER`), so
//! the kill is a genuine `SIGABRT` of a separate process, not a dropped value.

use std::collections::BTreeMap;
use std::fs;
use std::io::Write;
use std::os::unix::process::ExitStatusExt;
use std::path::{Path, PathBuf};
use std::process::Command;

use nitro_xml_opc::edit::{
    parse_edit_script, replay_script, CommandRegistry, EditHistory, EditSession,
};
use nitro_xml_opc::zip::ZipPackage;
use serde_json::{json, Value};

const SIGABRT: i32 = 6;

#[derive(Clone, Copy, Debug)]
enum Format {
    Xlsx,
    Docx,
    Pptx,
}

impl Format {
    const ALL: [Format; 3] = [Format::Xlsx, Format::Docx, Format::Pptx];

    fn name(self) -> &'static str {
        match self {
            Format::Xlsx => "xlsx",
            Format::Docx => "docx",
            Format::Pptx => "pptx",
        }
    }

    fn parse(s: &str) -> Format {
        Format::ALL
            .into_iter()
            .find(|f| f.name() == s)
            .expect("format")
    }

    /// (edit script id, fixture path), the same scripts `tests/crash/run.mjs` uses.
    fn script(self) -> (&'static str, &'static str) {
        match self {
            Format::Xlsx => (
                "xlsx/xlsx-multi-sheet-ops",
                "fixtures/generated/xlsx/cells_1k.xlsx",
            ),
            Format::Docx => (
                "docx/docx-text-and-format",
                "fixtures/generated/docx/painter_line_numbers.docx",
            ),
            Format::Pptx => (
                "pptx/pptx-text-and-style",
                "fixtures/generated/pptx/slides_10.pptx",
            ),
        }
    }

    /// Parts a save of these ops may rewrite; every other part must be carried
    /// through byte-identical. xlsx regenerates its modeled parts (workbook,
    /// sheets, shared strings, styles, rels; ADR 0003) and carries the rest.
    fn may_change(self, part: &str) -> bool {
        match self {
            Format::Xlsx => {
                part == "xl/workbook.xml"
                    || part == "_rels/.rels"
                    || part == "xl/sharedStrings.xml"
                    || part == "xl/styles.xml"
                    || part == "xl/_rels/workbook.xml.rels"
                    || part == "[Content_Types].xml"
                    || part == "docProps/app.xml"
                    || part.starts_with("xl/worksheets/")
            }
            Format::Docx => part == "word/document.xml",
            Format::Pptx => part == "ppt/slides/slide1.xml",
        }
    }
}

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..")
}

fn load_ops(format: Format) -> (Vec<Value>, Vec<u8>) {
    let (script, fixture) = format.script();
    let root = repo_root();
    let json: Value = serde_json::from_slice(
        &fs::read(root.join("edits").join(format!("{script}.json"))).unwrap(),
    )
    .unwrap();
    let ops = json["ops"].as_array().unwrap().clone();
    (ops, fs::read(root.join(fixture)).unwrap())
}

/// One live editing session over any of the three formats.
#[allow(clippy::large_enum_variant)]
enum Doc {
    Xlsx(
        EditSession<nitro_xml_xlsx::edit::XlsxHost, EditHistory>,
        CommandRegistry,
    ),
    Docx(
        EditSession<nitro_xml_docx::edit::DocxHost, EditHistory>,
        CommandRegistry,
    ),
    Pptx(
        EditSession<nitro_xml_pptx::edit::PptxHost, EditHistory>,
        CommandRegistry,
    ),
}

impl Doc {
    fn open(format: Format, bytes: &[u8]) -> Doc {
        let mut reg = CommandRegistry::with_schema_stubs();
        match format {
            Format::Xlsx => {
                use nitro_xml_xlsx::edit::*;
                register_cell_commands(&mut reg);
                register_structure_commands(&mut reg);
                register_shift_commands(&mut reg);
                Doc::Xlsx(
                    EditSession::new(XlsxHost::open(bytes).unwrap(), EditHistory::new()),
                    reg,
                )
            }
            Format::Docx => {
                use nitro_xml_docx::edit::*;
                register_docx_command_registry(&mut reg);
                Doc::Docx(
                    EditSession::new(DocxHost::open(bytes).unwrap(), EditHistory::new()),
                    reg,
                )
            }
            Format::Pptx => {
                use nitro_xml_pptx::edit::*;
                register_shape_commands(&mut reg);
                Doc::Pptx(
                    EditSession::new(PptxHost::open(bytes).unwrap(), EditHistory::new()),
                    reg,
                )
            }
        }
    }

    fn apply(&mut self, format: Format, op: &Value) {
        let script =
            json!({ "id": "crash", "format": format.name(), "fixture": "crash", "ops": [op] });
        let parsed = parse_edit_script(&script.to_string()).unwrap();
        let report = match self {
            Doc::Xlsx(s, r) => replay_script(&parsed, r, s),
            Doc::Docx(s, r) => replay_script(&parsed, r, s),
            Doc::Pptx(s, r) => replay_script(&parsed, r, s),
        };
        assert_eq!(report.status, "ok", "op {op} did not apply: {report:?}");
    }

    fn snapshot(&self) -> Value {
        match self {
            Doc::Xlsx(s, _) => s.host.undo_parity_snapshot(),
            Doc::Docx(s, _) => s.host.undo_parity_snapshot(),
            Doc::Pptx(s, _) => s.host.undo_parity_snapshot(),
        }
    }

    fn undo_all(&mut self) -> usize {
        let mut n = 0;
        macro_rules! drain {
            ($s:expr) => {
                while $s.can_undo() {
                    $s.undo();
                    n += 1;
                }
            };
        }
        match self {
            Doc::Xlsx(s, _) => drain!(s),
            Doc::Docx(s, _) => drain!(s),
            Doc::Pptx(s, _) => drain!(s),
        }
        n
    }

    fn save(&self, original: &[u8]) -> Vec<u8> {
        match self {
            Doc::Xlsx(s, _) => nitro_xml_xlsx::save_xlsx(&s.host, original).unwrap(),
            Doc::Docx(s, _) => nitro_xml_docx::save_docx(&s.host, original).unwrap(),
            Doc::Pptx(s, _) => nitro_xml_pptx::write::save_pptx(&s.host, original).unwrap(),
        }
    }
}

fn parts(bytes: &[u8]) -> BTreeMap<String, Vec<u8>> {
    let mut zip = ZipPackage::open(bytes).unwrap();
    let names: Vec<String> = zip.part_names().map(str::to_owned).collect();
    names
        .into_iter()
        .map(|n| {
            let data = zip.read(&n).unwrap().unwrap_or_default();
            (n, data)
        })
        .collect()
}

/// Worker body. Commits ops to an fsynced journal and an autosave, then dies
/// with the next op applied in memory but only half-written to the journal.
fn run_worker(spec: &str) -> ! {
    let mut it = spec.split('|');
    let format = Format::parse(it.next().unwrap());
    let dir = PathBuf::from(it.next().unwrap());
    let committed: usize = it.next().unwrap().parse().unwrap();

    let (ops, original) = load_ops(format);
    let mut doc = Doc::open(format, &original);
    let mut journal = fs::File::create(dir.join("journal.ndjson")).unwrap();

    for op in &ops[..committed] {
        doc.apply(format, op);
        writeln!(journal, "{op}").unwrap();
        journal.sync_all().unwrap();
        let tmp = dir.join("autosave.tmp");
        fs::write(&tmp, doc.save(&original)).unwrap();
        fs::rename(&tmp, dir.join("autosave.bin")).unwrap();
    }

    // Mid-edit: in-memory change applied, journal record torn, then killed.
    let next = ops[committed].to_string();
    doc.apply(format, &ops[committed]);
    journal
        .write_all(&next.as_bytes()[..next.len() / 2])
        .unwrap();
    journal.sync_all().unwrap();
    std::process::abort();
}

/// Entry point of the re-executed worker; a no-op in a normal test run.
#[test]
fn crash_worker_entry() {
    if let Ok(spec) = std::env::var("NITRO_CRASH_WORKER") {
        run_worker(&spec);
    }
}

fn spawn_worker(format: Format, dir: &Path, committed: usize) {
    let status = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "crash_worker_entry", "--test-threads=1"])
        .env(
            "NITRO_CRASH_WORKER",
            format!("{}|{}|{committed}", format.name(), dir.display()),
        )
        .status()
        .unwrap();
    assert_eq!(
        status.signal(),
        Some(SIGABRT),
        "{format:?} worker must die by SIGABRT mid-edit, got {status:?}"
    );
}

/// Journal records that were fully written; the torn tail has no newline.
fn read_journal(dir: &Path) -> Vec<Value> {
    let text = fs::read_to_string(dir.join("journal.ndjson")).unwrap();
    let complete = &text[..text.rfind('\n').map_or(0, |i| i + 1)];
    complete
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect()
}

fn assert_untouched_parts_identical(
    format: Format,
    committed: usize,
    original: &[u8],
    saved: &[u8],
) {
    let (before, after) = (parts(original), parts(saved));
    // The xlsx writer always emits a theme part even when the source had none.
    let added: Vec<&String> = after.keys().filter(|k| !before.contains_key(*k)).collect();
    for name in &added {
        assert!(
            format.may_change(name)
                || matches!(
                    (format, name.as_str()),
                    (Format::Xlsx, "xl/theme/theme1.xml")
                ),
            "{format:?}: save added unexpected part {name}"
        );
    }
    let mut carried = 0;
    for (name, data) in &before {
        if format.may_change(name) {
            continue;
        }
        assert!(
            after.get(name) == Some(data),
            "{format:?}: untouched part {name} not byte-identical (committed={})",
            committed
        );
        carried += 1;
    }
    assert!(carried > 0, "{format:?}: nothing was carried through");
}

// Catches: a worker kill mid-edit losing committed edits, leaking the
// half-applied edit into the recovered document, or the saved bytes rewriting
// parts the edits never touched.
#[test]
fn killed_worker_replays_committed_edits_only() {
    for format in Format::ALL {
        let (ops, original) = load_ops(format);
        let pristine = Doc::open(format, &original).snapshot();
        for committed in 0..ops.len() {
            let dir = std::env::temp_dir().join(format!(
                "nitro-crash-{}-{}-{committed}",
                format.name(),
                std::process::id()
            ));
            let _ = fs::remove_dir_all(&dir);
            fs::create_dir_all(&dir).unwrap();
            spawn_worker(format, &dir, committed);

            // Reopen and replay the journal onto the original bytes.
            let journal = read_journal(&dir);
            assert_eq!(
                journal.len(),
                committed,
                "{format:?}: torn tail must be dropped"
            );
            let mut recovered = Doc::open(format, &original);
            for op in &journal {
                recovered.apply(format, op);
            }

            // Recovered == exactly the pre-crash committed edits.
            let mut expected = Doc::open(format, &original);
            for op in &ops[..committed] {
                expected.apply(format, op);
            }
            assert_eq!(
                recovered.snapshot(),
                expected.snapshot(),
                "{format:?} after {committed} ops"
            );
            let mut leaked = Doc::open(format, &original);
            for op in &ops[..=committed] {
                leaked.apply(format, op);
            }
            assert_ne!(
                recovered.snapshot(),
                leaked.snapshot(),
                "{format:?}: uncommitted op leaked"
            );

            // Undo history is rebuilt by the replay.
            assert_eq!(
                recovered.undo_all(),
                committed,
                "{format:?}: one undo step per replayed op"
            );
            assert_eq!(
                recovered.snapshot(),
                pristine,
                "{format:?}: undo-all must reach the start"
            );

            // The last autosave, written before the kill, reopens to the same
            // document and carries untouched parts through unchanged.
            if committed > 0 {
                let saved = fs::read(dir.join("autosave.bin")).unwrap();
                assert_eq!(
                    Doc::open(format, &saved).snapshot(),
                    expected.snapshot(),
                    "{format:?} autosave"
                );
                assert_untouched_parts_identical(format, committed, &original, &saved);
                assert_eq!(
                    saved,
                    expected.save(&original),
                    "{format:?}: autosave == save of replayed doc"
                );
            }
            let _ = fs::remove_dir_all(&dir);
        }
    }
}
