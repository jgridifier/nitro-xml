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
from tiers import RECIPES, gen_xlsx_cells, gen_docx_pages, gen_pptx_slides  # noqa: E402
from workloads import gen_docx_workload, gen_pptx_workload, gen_xlsx_workload  # noqa: E402


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

    def test_large_tiers_have_no_unreferenced_parts(self) -> None:
        # Catches: L/XL size coming from an orphan filler part (pad.bin) or an image no document part references.
        import re
        import zipfile

        with tempfile.TemporaryDirectory() as td:
            x, d, p = Path(td) / "x.xlsx", Path(td) / "d.docx", Path(td) / "p.pptx"
            gen_xlsx_workload(x, label="t", sheets=2, rows=30, sst_n=40)
            gen_docx_workload(d, label="t", pages=30, n_images=3, image_px=8)
            gen_pptx_workload(p, label="t", slides=6, n_images=3, image_px=8)
            for path, media_re in ((d, r"media/"), (p, r"media/")):
                z = zipfile.ZipFile(path)
                rels = "".join(z.read(n).decode() for n in z.namelist() if n.endswith(".rels"))
                for name in z.namelist():
                    if re.search(media_re, name):
                        self.assertIn(name.rsplit("/", 1)[1], rels, f"{name} is not referenced by any relationship")
            for path in (x, d, p):
                self.assertFalse([n for n in zipfile.ZipFile(path).namelist() if "pad" in n])
            self.assertEqual(zipfile.ZipFile(x).read("xl/worksheets/sheet2.xml").count(b"<row "), 30)

    def test_large_tier_recipes_are_real_content_and_deterministic(self) -> None:
        # Catches: an on-demand L/XL recipe silently falling back to pad= filler, or two builds hashing differently.
        for rid, r in RECIPES.items():
            if r.get("on_demand"):
                self.assertIn("real-content", r["features"], rid)
        with tempfile.TemporaryDirectory() as td:
            a, b = Path(td) / "a.docx", Path(td) / "b.docx"
            gen_docx_workload(a, label="t", pages=25, n_images=2, image_px=8)
            gen_docx_workload(b, label="t", pages=25, n_images=2, image_px=8)
            self.assertEqual(a.read_bytes(), b.read_bytes())


if __name__ == "__main__":
    raise SystemExit(unittest.main())
