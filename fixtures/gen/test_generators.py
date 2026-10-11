#!/usr/bin/env python3
# SPDX-License-Identifier: MIT
"""Unit tests for fixture generators (non-obvious invariants only)."""

from __future__ import annotations

import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

from ooxml_zip import sha256_bytes, write_ooxml  # noqa: E402
from tiers import gen_xlsx_cells, gen_docx_pages, gen_pptx_slides  # noqa: E402


class GeneratorInvariants(unittest.TestCase):
    def test_two_runs_byte_identical(self) -> None:
        # Catches: zip member order or DOS timestamps drifting between runs so "fixed seed" fixtures are not reproducible.
        with tempfile.TemporaryDirectory() as td:
            a = Path(td) / "a.xlsx"
            b = Path(td) / "b.xlsx"
            gen_xlsx_cells(a, 200, merges=True, shared_extra=3, pad=1024)
            gen_xlsx_cells(b, 200, merges=True, shared_extra=3, pad=1024)
            self.assertEqual(a.read_bytes(), b.read_bytes())
            self.assertEqual(sha256_bytes(a.read_bytes()), sha256_bytes(b.read_bytes()))

            c = Path(td) / "c.docx"
            d = Path(td) / "d.docx"
            gen_docx_pages(c, 3, tables=True, images=True, pad=512)
            gen_docx_pages(d, 3, tables=True, images=True, pad=512)
            self.assertEqual(c.read_bytes(), d.read_bytes())

            e = Path(td) / "e.pptx"
            f = Path(td) / "f.pptx"
            gen_pptx_slides(e, 4, pad=256)
            gen_pptx_slides(f, 4, pad=256)
            self.assertEqual(e.read_bytes(), f.read_bytes())

    def test_sorted_members_beat_insertion_order(self) -> None:
        # Catches: writing OOXML parts in dict insertion order so two logically identical packages hash differently.
        with tempfile.TemporaryDirectory() as td:
            p1 = Path(td) / "1.zip"
            p2 = Path(td) / "2.zip"
            m1 = {"b.xml": b"<b/>", "a.xml": b"<a/>"}
            m2 = {"a.xml": b"<a/>", "b.xml": b"<b/>"}
            write_ooxml(p1, m1)
            write_ooxml(p2, m2)
            self.assertEqual(p1.read_bytes(), p2.read_bytes())


if __name__ == "__main__":
    raise SystemExit(unittest.main())
