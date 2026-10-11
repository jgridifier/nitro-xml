#!/usr/bin/env python3
# SPDX-License-Identifier: MIT
"""Generate MIT-licensed Arabic/Hebrew bidi fixtures for #44."""

from __future__ import annotations

import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]
sys.path.insert(0, str(ROOT / "fixtures" / "gen"))

from ooxml_zip import app_props, content_types, core_props, rels_root, sha256_file, write_ooxml  # noqa: E402

# Logical-order strings (storage order). Copy must preserve these as-is.
AR = "مرحبا بالعالم"
HE = "שלום עולם"
MIXED = "Hello שלום world مرحبا"
AR_NUM = "السعر 1234 ريال"
HE_NUM = "מחיר 56 ₪"


def _styles() -> bytes:
    return (
        '<?xml version="1.0" encoding="UTF-8" standalone="yes"?>'
        '<styleSheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">'
        '<fonts count="1"><font><sz val="11"/><name val="Calibri"/></font></fonts>'
        '<fills count="2"><fill><patternFill patternType="none"/></fill>'
        '<fill><patternFill patternType="gray125"/></fill></fills>'
        '<borders count="1"><border/></borders>'
        '<cellStyleXfs count="1"><xf/></cellStyleXfs>'
        '<cellXfs count="1"><xf xfId="0"/></cellXfs>'
        "</styleSheet>"
    ).encode("utf-8")


def _xlsx(path: Path, *, title: str, strings: list[str], rtl_sheet: bool) -> None:
    sst = (
        '<?xml version="1.0" encoding="UTF-8" standalone="yes"?>'
        f'<sst xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" '
        f'count="{len(strings)}" uniqueCount="{len(strings)}">'
        + "".join(f"<si><t xml:space=\"preserve\">{_xml_escape(s)}</t></si>" for s in strings)
        + "</sst>"
    ).encode("utf-8")
    cells = []
    for i, _ in enumerate(strings):
        col = chr(ord("A") + i)
        cells.append(f'<c r="{col}1" t="s"><v>{i}</v></c>')
    rtl_attr = ' rightToLeft="1"' if rtl_sheet else ""
    sheet = (
        '<?xml version="1.0" encoding="UTF-8" standalone="yes"?>'
        '<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" '
        'xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">'
        f'<sheetViews><sheetView workbookViewId="0"{rtl_attr}/></sheetViews>'
        f'<sheetData><row r="1">{"".join(cells)}</row></sheetData>'
        "</worksheet>"
    ).encode("utf-8")
    workbook = (
        '<?xml version="1.0" encoding="UTF-8" standalone="yes"?>'
        '<workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" '
        'xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">'
        f'<sheets><sheet name="{"RTL" if rtl_sheet else "Bidi"}" sheetId="1" r:id="rId1"/></sheets>'
        "</workbook>"
    ).encode("utf-8")
    wb_rels = (
        '<?xml version="1.0" encoding="UTF-8" standalone="yes"?>'
        '<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">'
        '<Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet1.xml"/>'
        '<Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/sharedStrings" Target="sharedStrings.xml"/>'
        '<Relationship Id="rId3" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles" Target="styles.xml"/>'
        "</Relationships>"
    ).encode("utf-8")
    write_ooxml(
        path,
        {
            "[Content_Types].xml": content_types(
                [
                    ("/xl/workbook.xml", "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"),
                    ("/xl/worksheets/sheet1.xml", "application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"),
                    ("/xl/sharedStrings.xml", "application/vnd.openxmlformats-officedocument.spreadsheetml.sharedStrings+xml"),
                    ("/xl/styles.xml", "application/vnd.openxmlformats-officedocument.spreadsheetml.styles+xml"),
                    ("/docProps/core.xml", "application/vnd.openxmlformats-package.core-properties+xml"),
                    ("/docProps/app.xml", "application/vnd.openxmlformats-officedocument.extended-properties+xml"),
                ]
            ),
            "_rels/.rels": rels_root(
                "xl/workbook.xml",
                "http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument",
            ),
            "xl/workbook.xml": workbook,
            "xl/_rels/workbook.xml.rels": wb_rels,
            "xl/worksheets/sheet1.xml": sheet,
            "xl/sharedStrings.xml": sst,
            "xl/styles.xml": _styles(),
            "docProps/core.xml": core_props(title),
            "docProps/app.xml": app_props("nitro-xml bidi fixture generator"),
        },
    )


def _xml_escape(s: str) -> str:
    return (
        s.replace("&", "&amp;")
        .replace("<", "&lt;")
        .replace(">", "&gt;")
        .replace('"', "&quot;")
    )


def _pptx(path: Path) -> None:
    """One slide with RTL Arabic/Hebrew paragraphs and a mixed LTR/RTL run."""
    from tiers import gen_pptx_slides
    import zipfile

    tmp = path.with_suffix(".tmp.pptx")
    gen_pptx_slides(tmp, 1, pad=0)
    with zipfile.ZipFile(tmp, "r") as zin:
        members = {n: zin.read(n) for n in zin.namelist()}
    tmp.unlink(missing_ok=True)

    slide = (
        '<?xml version="1.0" encoding="UTF-8" standalone="yes"?>'
        '<p:sld xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" '
        'xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" '
        'xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main">'
        "<p:cSld><p:spTree>"
        '<p:nvGrpSpPr><p:cNvPr id="1" name=""/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr><p:grpSpPr/>'
        '<p:sp><p:nvSpPr><p:cNvPr id="2" name="Title"/><p:cNvSpPr txBox="1"/><p:nvPr/></p:nvSpPr>'
        '<p:spPr><a:xfrm><a:off x="457200" y="457200"/><a:ext cx="8229600" cy="1371600"/></a:xfrm>'
        '<a:prstGeom prst="rect"><a:avLst/></a:prstGeom></p:spPr>'
        "<p:txBody><a:bodyPr/><a:lstStyle/>"
        f'<a:p><a:pPr algn="r" rtl="1"/><a:r><a:rPr lang="ar-SA"/><a:t>{_xml_escape(AR)}</a:t></a:r></a:p>'
        f'<a:p><a:pPr algn="r" rtl="1"/><a:r><a:rPr lang="he-IL"/><a:t>{_xml_escape(HE)}</a:t></a:r></a:p>'
        f'<a:p><a:pPr algn="l"/><a:r><a:rPr lang="en-US"/><a:t>Hello </a:t></a:r>'
        f'<a:r><a:rPr lang="he-IL"/><a:t>{_xml_escape(HE.split()[0])}</a:t></a:r>'
        f'<a:r><a:rPr lang="en-US"/><a:t> world </a:t></a:r>'
        f'<a:r><a:rPr lang="ar-SA"/><a:t>{_xml_escape(AR.split()[0])}</a:t></a:r></a:p>'
        "</p:txBody></p:sp></p:spTree></p:cSld>"
        '<p:clrMapOvr><a:masterClrMapping/></p:clrMapOvr></p:sld>'
    ).encode("utf-8")
    members["ppt/slides/slide1.xml"] = slide
    members["docProps/core.xml"] = core_props("bidi arabic hebrew slides")
    write_ooxml(path, members)


def main() -> int:
    out = HERE
    mixed = out / "arabic-hebrew-mixed.xlsx"
    rtl = out / "rtl-sheet.xlsx"
    pptx = out / "arabic-hebrew-slides.pptx"
    _xlsx(
        mixed,
        title="bidi mixed arabic hebrew",
        strings=[AR, HE, MIXED, AR_NUM, HE_NUM],
        rtl_sheet=False,
    )
    _xlsx(
        rtl,
        title="bidi rtl sheet",
        strings=[AR, HE, MIXED],
        rtl_sheet=True,
    )
    _pptx(pptx)
    for p in (mixed, rtl, pptx):
        print(f"{p.relative_to(ROOT)} sha256={sha256_file(p)} bytes={p.stat().st_size}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
