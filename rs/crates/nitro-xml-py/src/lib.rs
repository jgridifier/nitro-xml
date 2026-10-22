// SPDX-License-Identifier: MIT
//! PyO3 bindings: open OOXML packages, read cell/text, lossless save.

#![allow(clippy::useless_conversion)] // PyO3 `PyResult<T>` vs `Result<T, PyErr>` in exported signatures.

use nitro_xml_docx::{save_docx, DocxEditor};
use nitro_xml_pptx::edit::host::EditableElement;
use nitro_xml_pptx::editor::PptxEditor;
use nitro_xml_xlsx::address::parse_address;
use nitro_xml_xlsx::editor::EditorSession;
use pyo3::exceptions::{PyIOError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::PyBytes;
fn py_value_err(message: String) -> pyo3::PyErr {
    PyValueError::new_err(message)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum OfficeFormat {
    Xlsx,
    Docx,
    Pptx,
}

fn format_from_name(name: &str) -> Result<OfficeFormat, String> {
    let ext = name.rsplit('.').next().unwrap_or("").to_ascii_lowercase();
    match ext.as_str() {
        "xlsx" | "xlsm" | "xltx" | "xltm" => Ok(OfficeFormat::Xlsx),
        "docx" | "docm" | "dotx" | "dotm" => Ok(OfficeFormat::Docx),
        "pptx" | "pptm" | "potx" | "potm" => Ok(OfficeFormat::Pptx),
        _ => Err(format!("unsupported extension in name: {name}")),
    }
}

fn paragraph_plain_text(block: &serde_json::Value) -> String {
    block
        .get("runs")
        .and_then(|r| r.as_array())
        .map(|rs| {
            rs.iter()
                .map(|r| r.get("text").and_then(|t| t.as_str()).unwrap_or(""))
                .collect::<String>()
        })
        .unwrap_or_default()
}

fn format_label(fmt: OfficeFormat) -> &'static str {
    match fmt {
        OfficeFormat::Xlsx => "xlsx",
        OfficeFormat::Docx => "docx",
        OfficeFormat::Pptx => "pptx",
    }
}

enum DocumentState {
    Xlsx(Box<EditorSession>),
    Docx(Box<DocxEditor>),
    Pptx(Box<PptxEditor>),
}

impl DocumentState {
    fn format(&self) -> OfficeFormat {
        match self {
            DocumentState::Xlsx(_) => OfficeFormat::Xlsx,
            DocumentState::Docx(_) => OfficeFormat::Docx,
            DocumentState::Pptx(_) => OfficeFormat::Pptx,
        }
    }

    fn open(bytes: Vec<u8>, fmt: OfficeFormat) -> Result<Self, String> {
        match fmt {
            OfficeFormat::Xlsx => {
                EditorSession::open(&bytes).map(|s| DocumentState::Xlsx(Box::new(s)))
            }
            OfficeFormat::Docx => {
                DocxEditor::open(&bytes).map(|s| DocumentState::Docx(Box::new(s)))
            }
            OfficeFormat::Pptx => {
                PptxEditor::open(&bytes).map(|s| DocumentState::Pptx(Box::new(s)))
            }
        }
    }

    fn save_xlsx_docx(&self, original: &[u8]) -> Result<Vec<u8>, String> {
        match self {
            DocumentState::Xlsx(session) => session.save(original),
            DocumentState::Docx(editor) => save_docx(&editor.session.host, original),
            DocumentState::Pptx(_) => unreachable!(),
        }
    }

    fn save_pptx(&mut self) -> Result<Vec<u8>, String> {
        match self {
            DocumentState::Pptx(editor) => editor.save(),
            _ => unreachable!(),
        }
    }

    fn cell_text(&self, sheet_index: u32, address: &str) -> Result<String, String> {
        let session = match self {
            DocumentState::Xlsx(s) => s,
            _ => return Err("cell_text is only supported for xlsx".into()),
        };
        let addr = parse_address(address).ok_or_else(|| format!("bad cell address: {address}"))?;
        Ok(session.input_text(sheet_index, addr.row, addr.col))
    }

    fn text(&self) -> Result<String, String> {
        match self {
            DocumentState::Docx(editor) => Ok(docx_plain_text(&editor.session.host)),
            DocumentState::Pptx(editor) => Ok(pptx_plain_text(&editor.session.host)),
            DocumentState::Xlsx(_) => Err("text() is only supported for docx and pptx".into()),
        }
    }
}

fn docx_plain_text(host: &nitro_xml_docx::edit::host::DocxHost) -> String {
    let mut lines = Vec::new();
    for block in &host.blocks {
        match block.get("type").and_then(|t| t.as_str()) {
            Some("paragraph") => {
                let t = paragraph_plain_text(block);
                if !t.is_empty() {
                    lines.push(t);
                }
            }
            Some("table") => {
                if let Some(rows) = block.get("rows").and_then(|r| r.as_array()) {
                    for row in rows {
                        if let Some(cells) = row.get("cells").and_then(|c| c.as_array()) {
                            for cell in cells {
                                if let Some(paras) =
                                    cell.get("paragraphs").and_then(|p| p.as_array())
                                {
                                    for para in paras {
                                        let t = paragraph_plain_text(para);
                                        if !t.is_empty() {
                                            lines.push(t);
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            _ => {}
        }
    }
    lines.join("\n")
}

fn pptx_element_text(el: &EditableElement, out: &mut Vec<String>) {
    if let Some(t) = &el.text {
        let trimmed = t.trim();
        if !trimmed.is_empty() {
            out.push(trimmed.to_string());
        }
    }
    if let Some(table) = &el.table {
        for row in &table.rows {
            for cell in &row.cells {
                let trimmed = cell.text.trim();
                if !trimmed.is_empty() {
                    out.push(trimmed.to_string());
                }
            }
        }
    }
    for child in &el.children {
        pptx_element_text(child, out);
    }
}

fn pptx_plain_text(host: &nitro_xml_pptx::edit::host::PptxHost) -> String {
    let mut parts = Vec::new();
    for slide in &host.slides {
        for el in &slide.elements {
            pptx_element_text(el, &mut parts);
        }
    }
    parts.join("\n")
}

#[pyclass]
struct Document {
    original: Vec<u8>,
    format: OfficeFormat,
    state: DocumentState,
}

#[pymethods]
#[allow(clippy::useless_conversion)] // PyO3 maps `Result<T, PyErr>` to `PyResult<T>` in signatures.
impl Document {
    #[getter]
    fn format(&self) -> &'static str {
        format_label(self.format)
    }

    fn cell_text(&self, sheet_index: u32, address: &str) -> PyResult<String> {
        self.state
            .cell_text(sheet_index, address)
            .map_err(py_value_err)
    }

    fn text(&self) -> PyResult<String> {
        self.state.text().map_err(py_value_err)
    }

    fn save<'py>(&mut self, py: Python<'py>) -> PyResult<Bound<'py, PyBytes>> {
        let out = match self.state.format() {
            OfficeFormat::Pptx => self.state.save_pptx(),
            _ => self.state.save_xlsx_docx(&self.original),
        }
        .map_err(py_value_err)?;
        if matches!(self.format, OfficeFormat::Pptx) {
            self.original = out.clone();
        }
        Ok(PyBytes::new(py, &out))
    }
}

fn open_bytes_impl(data: Vec<u8>, name: &str) -> PyResult<Document> {
    let format = format_from_name(name).map_err(py_value_err)?;
    let state = DocumentState::open(data.clone(), format).map_err(py_value_err)?;
    Ok(Document {
        original: data,
        format,
        state,
    })
}

#[allow(clippy::useless_conversion)]
#[pyfunction]
fn open_bytes(data: &[u8], name: &str) -> PyResult<Document> {
    open_bytes_impl(data.to_vec(), name)
}

#[allow(clippy::useless_conversion)]
#[pyfunction]
fn open_path(path: &str) -> PyResult<Document> {
    let data = std::fs::read(path).map_err(|e| PyIOError::new_err(format!("read {path}: {e}")))?;
    let name = std::path::Path::new(path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(path);
    open_bytes_impl(data, name)
}

#[pymodule(name = "_native")]
fn nitro_xml_native(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<Document>()?;
    m.add_function(wrap_pyfunction!(open_bytes, m)?)?;
    m.add_function(wrap_pyfunction!(open_path, m)?)?;
    Ok(())
}
