# SPDX-License-Identifier: MIT
"""Catches: Python open/read/save round-trip failing for generated OOXML fixtures."""

from pathlib import Path

import nitro_xml

ROOT = Path(__file__).resolve().parents[2]
FIXTURES = ROOT / "fixtures" / "generated"


def test_xlsx_roundtrip():
    path = FIXTURES / "xlsx" / "cells_1k.xlsx"
    doc = nitro_xml.open_path(str(path))
    assert doc.format == "xlsx"
    before = doc.cell_text(0, "A1")
    assert before != ""
    saved = doc.save()
    again = nitro_xml.open_bytes(saved, "cells_1k.xlsx")
    assert again.format == "xlsx"
    assert again.cell_text(0, "A1") == before


def test_docx_roundtrip():
    path = FIXTURES / "docx" / "pages_10.docx"
    doc = nitro_xml.open_path(str(path))
    assert doc.format == "docx"
    before = doc.text()
    assert before.strip() != ""
    saved = doc.save()
    again = nitro_xml.open_bytes(saved, "pages_10.docx")
    assert again.format == "docx"
    assert again.text() == before


def test_pptx_roundtrip():
    path = FIXTURES / "pptx" / "slides_10.pptx"
    doc = nitro_xml.open_path(str(path))
    assert doc.format == "pptx"
    before = doc.text()
    assert before.strip() != ""
    saved = doc.save()
    again = nitro_xml.open_bytes(saved, "slides_10.pptx")
    assert again.format == "pptx"
    assert again.text() == before
