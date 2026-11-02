#!/usr/bin/env python3
# SPDX-License-Identifier: MIT
"""Real-content L/XL workload generators for xlsx / docx / pptx (issue #117).

L/XL tier size comes from content a parser and renderer must process, never from an
unreferenced filler part: sheets, rows, cells and shared strings (xlsx); pages, paragraphs,
tables and referenced images (docx); slides with text and referenced pictures (pptx).

Determinism: every random draw comes from ``random.Random(f"{SEED}:{label}")`` (string seeds are
hashed with SHA-512, stable across Python versions), and ``write_ooxml`` fixes member order, zip
timestamps and compression level, so two runs are byte-identical.
"""

from __future__ import annotations

import random
from pathlib import Path
from typing import Iterator

from ooxml_zip import app_props, content_types, core_props, noise_png, rels_root, write_ooxml

SEED = 4  # issue #4; same seed as tiers.py

_NS_R = "http://schemas.openxmlformats.org/officeDocument/2006/relationships"
_REL = "http://schemas.openxmlformats.org/officeDocument/2006/relationships"
_XML = '<?xml version="1.0" encoding="UTF-8" standalone="yes"?>'
_DEFAULTS_PNG = [
    ("rels", "application/vnd.openxmlformats-package.relationships+xml"),
    ("xml", "application/xml"),
    ("png", "image/png"),
]

_vocab_cache: list[str] | None = None


def _rng(label: str) -> random.Random:
    return random.Random(f"{SEED}:{label}")


def _vocab() -> list[str]:
    """4096 pseudo-words (letters only, so XML-safe) shared by every generator."""
    global _vocab_cache
    if _vocab_cache is None:
        rng = _rng("vocab")
        syl = ["ka", "lo", "mi", "ra", "ten", "sol", "vek", "dar", "un", "pho", "ne", "qui", "bar", "zel", "ith", "mon"]
        words: set[str] = set()
        while len(words) < 4096:
            words.add("".join(rng.choices(syl, k=rng.randint(2, 4))))
        _vocab_cache = sorted(words)
    return _vocab_cache


def _words(rng: random.Random, n: int) -> str:
    return " ".join(rng.choices(_vocab(), k=n))


def _col_names(n: int) -> list[str]:
    out = []
    for i in range(1, n + 1):
        s, k = "", i
        while k:
            k, r = divmod(k - 1, 26)
            s = chr(65 + r) + s
        out.append(s)
    return out


def _xml_part(name: str, ctype: str) -> tuple[str, str]:
    return (name, ctype)


# --------------------------------------------------------------------------- xlsx


def _sheet_chunks(rng: random.Random, rows: int, cols: int, sst_n: int, *, first: bool) -> Iterator[bytes]:
    names = _col_names(cols)
    head = (
        f'{_XML}<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" '
        f'xmlns:r="{_NS_R}"><dimension ref="A1:{names[-1]}{rows}"/>'
        '<sheetViews><sheetView workbookViewId="0"/></sheetViews><sheetData>'
    )
    yield head.encode()
    batch: list[str] = []
    for r in range(1, rows + 1):
        cells = []
        for c in range(cols):
            x = rng.random()
            ref = f"{names[c]}{r}"
            if c == 0 or x < 0.25:
                cells.append(f'<c r="{ref}" t="s"><v>{rng.randrange(sst_n)}</v></c>')
            elif x < 0.4:
                cells.append(f'<c r="{ref}"><v>{rng.randrange(1_000_000)}</v></c>')
            else:
                cells.append(f'<c r="{ref}"><v>{round(rng.uniform(0, 1e6), 2)}</v></c>')
        batch.append(f'<row r="{r}">{"".join(cells)}</row>')
        if len(batch) == 500:
            yield "".join(batch).encode()
            batch = []
    if batch:
        yield "".join(batch).encode()
    tail = "</sheetData>"
    if first:
        tail += '<mergeCells count="1"><mergeCell ref="A1:B2"/></mergeCells>'
        tail += (
            '<conditionalFormatting sqref="B1:T10"><cfRule type="colorScale" priority="1"><colorScale>'
            '<cfvo type="min"/><cfvo type="max"/><color rgb="FFF8696B"/><color rgb="FF63BE7B"/>'
            "</colorScale></cfRule></conditionalFormatting>"
        )
    tail += (
        '<printOptions headings="1" gridLines="1"/>'
        '<pageMargins left="0.7" right="0.7" top="0.75" bottom="0.75" header="0.3" footer="0.3"/>'
        '<pageSetup orientation="portrait"/></worksheet>'
    )
    yield tail.encode()


def _sst_chunks(rng: random.Random, n: int) -> Iterator[bytes]:
    yield (f'{_XML}<sst xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" uniqueCount="{n}">').encode()
    batch = []
    for _ in range(n):
        batch.append(f"<si><t>{_words(rng, rng.randint(1, 4))}</t></si>")
        if len(batch) == 2000:
            yield "".join(batch).encode()
            batch = []
    if batch:
        yield "".join(batch).encode()
    yield b"</sst>"


def gen_xlsx_workload(path: Path, *, label: str, sheets: int, rows: int, cols: int = 20, sst_n: int) -> None:
    """``sheets`` worksheets of ``rows`` x ``cols`` cells (sheet i+1 shares one string table)."""
    rng = _rng(f"xlsx:{label}")
    ct = [
        ("/xl/workbook.xml", "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"),
        ("/xl/sharedStrings.xml", "application/vnd.openxmlformats-officedocument.spreadsheetml.sharedStrings+xml"),
        ("/xl/styles.xml", "application/vnd.openxmlformats-officedocument.spreadsheetml.styles+xml"),
        ("/docProps/core.xml", "application/vnd.openxmlformats-package.core-properties+xml"),
        ("/docProps/app.xml", "application/vnd.openxmlformats-officedocument.extended-properties+xml"),
    ]
    sheet_els, rels = [], []
    members: dict = {}
    for i in range(1, sheets + 1):
        ct.append((f"/xl/worksheets/sheet{i}.xml", "application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"))
        sheet_els.append(f'<sheet name="Sheet{i}" sheetId="{i}" r:id="rId{i}"/>')
        rels.append(f'<Relationship Id="rId{i}" Type="{_REL}/worksheet" Target="worksheets/sheet{i}.xml"/>')
        members[f"xl/worksheets/sheet{i}.xml"] = _sheet_chunks(_rng(f"xlsx:{label}:sheet{i}"), rows, cols, sst_n, first=i == 1)
    rels.append(f'<Relationship Id="rId{sheets + 1}" Type="{_REL}/sharedStrings" Target="sharedStrings.xml"/>')
    rels.append(f'<Relationship Id="rId{sheets + 2}" Type="{_REL}/styles" Target="styles.xml"/>')
    members.update(
        {
            "[Content_Types].xml": content_types(ct),
            "_rels/.rels": rels_root("xl/workbook.xml", f"{_REL}/officeDocument"),
            "xl/workbook.xml": (
                f'{_XML}<workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="{_NS_R}">'
                f'<sheets>{"".join(sheet_els)}</sheets></workbook>'
            ).encode(),
            "xl/_rels/workbook.xml.rels": (
                f'{_XML}<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">'
                f'{"".join(rels)}</Relationships>'
            ).encode(),
            "xl/sharedStrings.xml": _sst_chunks(_rng(f"xlsx:{label}:sst"), sst_n),
            "xl/styles.xml": (
                f'{_XML}<styleSheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">'
                '<fonts count="1"><font><sz val="11"/><name val="Calibri"/></font></fonts>'
                '<fills count="2"><fill><patternFill patternType="none"/></fill>'
                '<fill><patternFill patternType="gray125"/></fill></fills>'
                '<borders count="1"><border/></borders><cellStyleXfs count="1"><xf/></cellStyleXfs>'
                '<cellXfs count="1"><xf xfId="0"/></cellXfs></styleSheet>'
            ).encode(),
            "docProps/core.xml": core_props(f"xlsx {sheets} sheets x {rows} rows x {cols} cols"),
            "docProps/app.xml": app_props("nitro-xml fixture generator"),
        }
    )
    write_ooxml(path, members)


# --------------------------------------------------------------------------- docx


def _docx_body(
    rng: random.Random, pages: int, paras: int, words: int, table_every: int, image_every: int, n_images: int
) -> Iterator[bytes]:
    yield (
        f'{_XML}<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" '
        f'xmlns:r="{_NS_R}" xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing" '
        'xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" '
        'xmlns:pic="http://schemas.openxmlformats.org/drawingml/2006/picture"><w:body>'
    ).encode()
    doc_pr = 0
    for p in range(1, pages + 1):
        out = [
            f'<w:p><w:pPr><w:pStyle w:val="Heading1"/>{"<w:pageBreakBefore/>" if p > 1 else ""}</w:pPr>'
            f'<w:r><w:t>Section {p}: {_words(rng, 4)}</w:t></w:r></w:p>'
        ]
        for _ in range(paras):
            out.append(
                f'<w:p><w:r><w:t xml:space="preserve">{_words(rng, words)} </w:t></w:r>'
                f'<w:r><w:rPr><w:b/></w:rPr><w:t xml:space="preserve">{_words(rng, 3)} </w:t></w:r>'
                f'<w:r><w:t>{_words(rng, words // 2)}</w:t></w:r></w:p>'
            )
        if table_every and p % table_every == 0:
            out.append(
                '<w:tbl><w:tblPr><w:tblStyle w:val="TableGrid"/><w:tblW w:w="0" w:type="auto"/></w:tblPr>'
                '<w:tblGrid>' + '<w:gridCol w:w="2000"/>' * 4 + "</w:tblGrid>"
            )
            for _r in range(5):
                out.append(
                    "<w:tr>"
                    + "".join(
                        f'<w:tc><w:p><w:r><w:t>{_words(rng, 2) if c == 0 else round(rng.uniform(0, 1e5), 2)}</w:t></w:r></w:p></w:tc>'
                        for c in range(4)
                    )
                    + "</w:tr>"
                )
            out.append("</w:tbl><w:p/>")
        if image_every and p % image_every == 0:
            doc_pr += 1
            rid = f"rId{100 + (doc_pr - 1) % n_images}"
            out.append(
                '<w:p><w:r><w:drawing><wp:inline distT="0" distB="0" distL="0" distR="0">'
                f'<wp:extent cx="1828800" cy="1828800"/><wp:docPr id="{doc_pr}" name="Picture {doc_pr}"/>'
                '<a:graphic><a:graphicData uri="http://schemas.openxmlformats.org/drawingml/2006/picture"><pic:pic>'
                f'<pic:nvPicPr><pic:cNvPr id="{doc_pr}" name="image{doc_pr}.png"/><pic:cNvPicPr/></pic:nvPicPr>'
                f'<pic:blipFill><a:blip r:embed="{rid}"/><a:stretch><a:fillRect/></a:stretch></pic:blipFill>'
                '<pic:spPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="1828800" cy="1828800"/></a:xfrm>'
                '<a:prstGeom prst="rect"><a:avLst/></a:prstGeom></pic:spPr></pic:pic>'
                "</a:graphicData></a:graphic></wp:inline></w:drawing></w:r></w:p>"
            )
        yield "".join(out).encode()
    yield (
        '<w:sectPr><w:pgSz w:w="12240" w:h="15840"/><w:pgMar w:top="1440" w:right="1440" '
        'w:bottom="1440" w:left="1440" w:header="720" w:footer="720"/></w:sectPr></w:body></w:document>'
    ).encode()


def gen_docx_workload(
    path: Path,
    *,
    label: str,
    pages: int,
    paras: int = 5,
    words: int = 60,
    table_every: int = 4,
    image_every: int = 10,
    n_images: int = 40,
    image_px: int = 96,
) -> None:
    """``pages`` page-break-separated sections of body text, a table every ``table_every`` pages and
    an inline picture every ``image_every`` pages drawn from a pool of ``n_images`` distinct PNGs."""
    img_rng = _rng(f"docx:{label}:img")
    members: dict = {}
    rels = [f'<Relationship Id="rId1" Type="{_REL}/styles" Target="styles.xml"/>']
    n_used = min(n_images, pages // image_every) if image_every else 0
    for k in range(n_used):
        members[f"word/media/image{k + 1}.png"] = noise_png(image_px, image_px, img_rng)
        rels.append(f'<Relationship Id="rId{100 + k}" Type="{_REL}/image" Target="media/image{k + 1}.png"/>')
    members.update(
        {
            "[Content_Types].xml": content_types(
                [
                    ("/word/document.xml", "application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"),
                    ("/word/styles.xml", "application/vnd.openxmlformats-officedocument.wordprocessingml.styles+xml"),
                    ("/docProps/core.xml", "application/vnd.openxmlformats-package.core-properties+xml"),
                    ("/docProps/app.xml", "application/vnd.openxmlformats-officedocument.extended-properties+xml"),
                ],
                defaults=_DEFAULTS_PNG,
            ),
            "_rels/.rels": rels_root("word/document.xml", f"{_REL}/officeDocument"),
            "word/document.xml": _docx_body(
                _rng(f"docx:{label}:body"), pages, paras, words, table_every, image_every, max(n_used, 1)
            ),
            "word/_rels/document.xml.rels": (
                f'{_XML}<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">'
                f'{"".join(rels)}</Relationships>'
            ).encode(),
            "word/styles.xml": (
                f'{_XML}<w:styles xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">'
                '<w:style w:type="paragraph" w:default="1" w:styleId="Normal"><w:name w:val="Normal"/></w:style>'
                '<w:style w:type="paragraph" w:styleId="Heading1"><w:name w:val="heading 1"/><w:basedOn w:val="Normal"/>'
                '<w:rPr><w:b/><w:sz w:val="32"/></w:rPr></w:style>'
                '<w:style w:type="table" w:styleId="TableGrid"><w:name w:val="Table Grid"/><w:tblPr><w:tblBorders>'
                '<w:top w:val="single" w:sz="4"/><w:left w:val="single" w:sz="4"/><w:bottom w:val="single" w:sz="4"/>'
                '<w:right w:val="single" w:sz="4"/><w:insideH w:val="single" w:sz="4"/><w:insideV w:val="single" w:sz="4"/>'
                "</w:tblBorders></w:tblPr></w:style></w:styles>"
            ).encode(),
            "docProps/core.xml": core_props(f"docx pages={pages}"),
            "docProps/app.xml": app_props("nitro-xml fixture generator", f"<Pages>{pages}</Pages>"),
        }
    )
    write_ooxml(path, members)


# --------------------------------------------------------------------------- pptx

_PPT_NS = (
    'xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" '
    f'xmlns:r="{_NS_R}" xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main"'
)


def _slide_xml(rng: random.Random, i: int, bullets: int, words: int) -> bytes:
    body = "".join(f"<a:p><a:r><a:rPr lang=\"en-US\"/><a:t>{_words(rng, words)}</a:t></a:r></a:p>" for _ in range(bullets))
    return (
        f'{_XML}<p:sld {_PPT_NS}><p:cSld><p:spTree><p:nvGrpSpPr><p:cNvPr id="1" name=""/><p:cNvGrpSpPr/><p:nvPr/>'
        "</p:nvGrpSpPr><p:grpSpPr/>"
        '<p:sp><p:nvSpPr><p:cNvPr id="2" name="Title"/><p:cNvSpPr txBox="1"/><p:nvPr/></p:nvSpPr>'
        '<p:spPr><a:xfrm><a:off x="457200" y="274638"/><a:ext cx="11277600" cy="868362"/></a:xfrm>'
        '<a:prstGeom prst="rect"><a:avLst/></a:prstGeom></p:spPr>'
        f'<p:txBody><a:bodyPr/><a:lstStyle/><a:p><a:r><a:rPr lang="en-US" sz="3200" b="1"/><a:t>Slide {i}: {_words(rng, 4)}</a:t></a:r></a:p></p:txBody></p:sp>'
        '<p:sp><p:nvSpPr><p:cNvPr id="3" name="Body"/><p:cNvSpPr txBox="1"/><p:nvPr/></p:nvSpPr>'
        '<p:spPr><a:xfrm><a:off x="457200" y="1371600"/><a:ext cx="6400800" cy="4800600"/></a:xfrm>'
        '<a:prstGeom prst="rect"><a:avLst/></a:prstGeom></p:spPr>'
        f"<p:txBody><a:bodyPr/><a:lstStyle/>{body}</p:txBody></p:sp>"
        '<p:pic><p:nvPicPr><p:cNvPr id="4" name="Picture 3"/><p:cNvPicPr/><p:nvPr/></p:nvPicPr>'
        '<p:blipFill><a:blip r:embed="rId2"/><a:stretch><a:fillRect/></a:stretch></p:blipFill>'
        '<p:spPr><a:xfrm><a:off x="7239000" y="1371600"/><a:ext cx="4495800" cy="4495800"/></a:xfrm>'
        '<a:prstGeom prst="rect"><a:avLst/></a:prstGeom></p:spPr></p:pic>'
        "</p:spTree></p:cSld><p:clrMapOvr><a:masterClrMapping/></p:clrMapOvr></p:sld>"
    ).encode()


def gen_pptx_workload(
    path: Path,
    *,
    label: str,
    slides: int,
    bullets: int = 6,
    words: int = 14,
    n_images: int = 60,
    image_px: int = 128,
) -> None:
    """``slides`` slides, each with a title, ``bullets`` text paragraphs and a picture that references
    one of ``n_images`` distinct PNGs in ppt/media."""
    # Reuse the committed S/M scaffolding (master, layout, theme) so L/XL differ only in content.
    import tiers

    scaffold = tiers.pptx_scaffold()
    img_rng = _rng(f"pptx:{label}:img")
    rng = _rng(f"pptx:{label}:slides")
    n_images = min(n_images, slides)
    members: dict = dict(scaffold["members"])
    overrides = list(scaffold["overrides"])
    pres_rels = list(scaffold["pres_rels"])
    sld_ids = []
    for k in range(1, n_images + 1):
        members[f"ppt/media/image{k}.png"] = noise_png(image_px, image_px, img_rng)
    for i in range(1, slides + 1):
        overrides.append((f"/ppt/slides/slide{i}.xml", "application/vnd.openxmlformats-officedocument.presentationml.slide+xml"))
        sld_ids.append(f'<p:sldId id="{255 + i}" r:id="rId{i + 2}"/>')
        pres_rels.append(f'<Relationship Id="rId{i + 2}" Type="{_REL}/slide" Target="slides/slide{i}.xml"/>')
        members[f"ppt/slides/slide{i}.xml"] = _slide_xml(rng, i, bullets, words)
        members[f"ppt/slides/_rels/slide{i}.xml.rels"] = (
            f'{_XML}<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">'
            f'<Relationship Id="rId1" Type="{_REL}/slideLayout" Target="../slideLayouts/slideLayout1.xml"/>'
            f'<Relationship Id="rId2" Type="{_REL}/image" Target="../media/image{(i - 1) % n_images + 1}.png"/>'
            "</Relationships>"
        ).encode()
    members["[Content_Types].xml"] = content_types(overrides, defaults=_DEFAULTS_PNG)
    members["ppt/presentation.xml"] = (
        f'{_XML}<p:presentation {_PPT_NS}>'
        '<p:sldMasterIdLst><p:sldMasterId id="2147483648" r:id="rId1"/></p:sldMasterIdLst>'
        f'<p:sldIdLst>{"".join(sld_ids)}</p:sldIdLst>'
        '<p:sldSz cx="12192000" cy="6858000"/><p:notesSz cx="6858000" cy="9144000"/></p:presentation>'
    ).encode()
    members["ppt/_rels/presentation.xml.rels"] = (
        f'{_XML}<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">'
        + "".join(pres_rels)
        + "</Relationships>"
    ).encode()
    members["docProps/core.xml"] = core_props(f"pptx slides={slides}")
    members["docProps/app.xml"] = app_props("nitro-xml fixture generator", f"<Slides>{slides}</Slides>")
    write_ooxml(path, members)
