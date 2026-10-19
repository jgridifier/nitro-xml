#!/usr/bin/env python3
# SPDX-License-Identifier: MIT
"""Deterministic size-tier generators for xlsx / docx / pptx (fixed seed)."""

from __future__ import annotations

from pathlib import Path

from ooxml_zip import (
    app_props,
    content_types,
    core_props,
    rels_root,
    tiny_png,
    write_ooxml,
)

SEED = 4  # issue #4
ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / "generated"


def _pad_blob(label: str, nbytes: int) -> bytes:
    """Deterministic padding derived from seed+label (not random.Random state drift)."""
    import hashlib

    out = bytearray()
    counter = 0
    while len(out) < nbytes:
        out.extend(hashlib.sha256(f"{SEED}:{label}:{counter}".encode()).digest())
        counter += 1
    return bytes(out[:nbytes])


def gen_xlsx_cells(path: Path, n_cells: int, *, merges: bool, shared_extra: int, pad: int) -> None:
    """Sheet with n_cells numeric+string values, optional merges and shared-string padding."""
    # Lay out as 20 columns.
    cols = 20
    rows = (n_cells + cols - 1) // cols
    # shared strings: one per row label + pad
    sst_parts = [f"<si><t>row-{r}</t></si>" for r in range(rows)]
    for i in range(shared_extra):
        sst_parts.append(f"<si><t>pad-{i:06d}</t></si>")
    sst = (
        '<?xml version="1.0" encoding="UTF-8" standalone="yes"?>'
        f'<sst xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" '
        f'count="{len(sst_parts)}" uniqueCount="{len(sst_parts)}">'
        + "".join(sst_parts)
        + "</sst>"
    ).encode("utf-8")

    sheet_rows = []
    cell_i = 0
    for r in range(1, rows + 1):
        cells = [f'<c r="A{r}" t="s"><v>{r-1}</v></c>']
        for c in range(2, cols + 1):
            if cell_i >= n_cells:
                break
            col = _col(c)
            cells.append(f'<c r="{col}{r}"><v>{(r * c) % 997}</v></c>')
            cell_i += 1
        sheet_rows.append(f'<row r="{r}">{"".join(cells)}</row>')

    merge_xml = ""
    if merges and rows >= 2:
        merge_xml = '<mergeCells count="1"><mergeCell ref="A1:B2"/></mergeCells>'

    # Conditional formatting (colour scale) over A1:T10 — exercises X3 input.
    cf = ""
    if rows >= 1:
        cf = (
            '<conditionalFormatting sqref="B1:T10">'
            '<cfRule type="colorScale" priority="1"><colorScale>'
            '<cfvo type="min"/><cfvo type="max"/>'
            '<color rgb="FFF8696B"/><color rgb="FF63BE7B"/>'
            "</colorScale></cfRule></conditionalFormatting>"
        )

    # Excel always writes `<dimension>`; L/XL first-view uses it to stop inflate at the viewport.
    last_col = _col(cols)
    dimension = f'<dimension ref="A1:{last_col}{rows}"/>'
    sheet = (
        '<?xml version="1.0" encoding="UTF-8" standalone="yes"?>'
        '<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" '
        'xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">'
        f'{dimension}'
        f'<sheetViews><sheetView workbookViewId="0"/></sheetViews>'
        f'<sheetData>{"".join(sheet_rows)}</sheetData>{merge_xml}{cf}'
        '<pageMargins left="0.7" right="0.7" top="0.75" bottom="0.75" header="0.3" footer="0.3"/>'
        '<pageSetup orientation="portrait" fitToWidth="1" fitToHeight="1"/>'
        '<printOptions headings="1" gridLines="1"/>'
        "</worksheet>"
    ).encode("utf-8")

    styles = (
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

    workbook = (
        '<?xml version="1.0" encoding="UTF-8" standalone="yes"?>'
        '<workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" '
        'xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">'
        '<sheets><sheet name="Sheet1" sheetId="1" r:id="rId1"/></sheets>'
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

    members = {
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
        "xl/styles.xml": styles,
        "docProps/core.xml": core_props(f"xlsx cells={n_cells}"),
        "docProps/app.xml": app_props("nitro-xml fixture generator"),
    }
    if pad:
        members["xl/media/pad.bin"] = _pad_blob(f"xlsx-{n_cells}", pad)
        # media not referenced — size only; Content_Types default covers bin via Override skip
        members["[Content_Types].xml"] = content_types(
            [
                ("/xl/workbook.xml", "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"),
                ("/xl/worksheets/sheet1.xml", "application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"),
                ("/xl/sharedStrings.xml", "application/vnd.openxmlformats-officedocument.spreadsheetml.sharedStrings+xml"),
                ("/xl/styles.xml", "application/vnd.openxmlformats-officedocument.spreadsheetml.styles+xml"),
                ("/docProps/core.xml", "application/vnd.openxmlformats-package.core-properties+xml"),
                ("/docProps/app.xml", "application/vnd.openxmlformats-officedocument.extended-properties+xml"),
                ("/xl/media/pad.bin", "application/octet-stream"),
            ],
            defaults=[
                ("rels", "application/vnd.openxmlformats-package.relationships+xml"),
                ("xml", "application/xml"),
                ("bin", "application/octet-stream"),
            ],
        )
    write_ooxml(path, members)


def _col(n: int) -> str:
    s = ""
    while n:
        n, r = divmod(n - 1, 26)
        s = chr(65 + r) + s
    return s


def gen_docx_pages(path: Path, n_pages: int, *, tables: bool, images: bool, pad: int) -> None:
    """Approximate n_pages via page breaks; optional table and tiny image."""
    paras = []
    for i in range(n_pages):
        paras.append(f"<w:p><w:r><w:t>Page {i+1} seed={SEED}</w:t></w:r></w:p>")
        if i + 1 < n_pages:
            paras.append('<w:p><w:r><w:br w:type="page"/></w:r></w:p>')
    if tables:
        paras.append(
            "<w:tbl><w:tblPr/><w:tblGrid><w:gridCol w:w=\"2000\"/><w:gridCol w:w=\"2000\"/></w:tblGrid>"
            "<w:tr><w:tc><w:p><w:r><w:t>A</w:t></w:r></w:p></w:tc>"
            "<w:tc><w:p><w:r><w:t>B</w:t></w:r></w:p></w:tc></w:tr></w:tbl>"
        )
    body_extra = ""
    rels_extra = ""
    overrides = [
        ("/word/document.xml", "application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"),
        ("/word/styles.xml", "application/vnd.openxmlformats-officedocument.wordprocessingml.styles+xml"),
        ("/docProps/core.xml", "application/vnd.openxmlformats-package.core-properties+xml"),
        ("/docProps/app.xml", "application/vnd.openxmlformats-officedocument.extended-properties+xml"),
    ]
    members: dict[str, bytes] = {}
    if images:
        members["word/media/image1.png"] = tiny_png()
        body_extra = (
            '<w:p><w:r><w:drawing><wp:inline xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing" '
            'distT="0" distB="0" distL="0" distR="0">'
            '<wp:extent cx="914400" cy="914400"/><wp:docPr id="1" name="img"/>'
            '<a:graphic xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main">'
            '<a:graphicData uri="http://schemas.openxmlformats.org/drawingml/2006/picture">'
            '<pic:pic xmlns:pic="http://schemas.openxmlformats.org/drawingml/2006/picture">'
            '<pic:nvPicPr><pic:cNvPr id="0" name="image1.png"/><pic:cNvPicPr/></pic:nvPicPr>'
            '<pic:blipFill><a:blip r:embed="rId2"/><a:stretch><a:fillRect/></a:stretch></pic:blipFill>'
            '<pic:spPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="914400" cy="914400"/></a:xfrm>'
            '<a:prstGeom prst="rect"><a:avLst/></a:prstGeom></pic:spPr></pic:pic>'
            "</a:graphicData></a:graphic></wp:inline></w:drawing></w:r></w:p>"
        )
        rels_extra = (
            '<Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image" Target="media/image1.png"/>'
        )
        overrides.append(("/word/media/image1.png", "image/png"))

    sect = (
        "<w:sectPr>"
        '<w:pgSz w:w="12240" w:h="15840"/>'
        '<w:pgMar w:top="1440" w:right="1440" w:bottom="1440" w:left="1440" w:header="720" w:footer="720"/>'
        "</w:sectPr>"
    )
    document = (
        '<?xml version="1.0" encoding="UTF-8" standalone="yes"?>'
        '<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" '
        'xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">'
        f'<w:body>{"".join(paras)}{body_extra}{sect}</w:body></w:document>'
    ).encode("utf-8")

    styles = (
        '<?xml version="1.0" encoding="UTF-8" standalone="yes"?>'
        '<w:styles xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">'
        '<w:style w:type="paragraph" w:default="1" w:styleId="Normal"><w:name w:val="Normal"/></w:style>'
        "</w:styles>"
    ).encode("utf-8")

    doc_rels = (
        '<?xml version="1.0" encoding="UTF-8" standalone="yes"?>'
        '<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">'
        '<Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles" Target="styles.xml"/>'
        f"{rels_extra}</Relationships>"
    ).encode("utf-8")

    members.update(
        {
            "[Content_Types].xml": content_types(overrides),
            "_rels/.rels": rels_root(
                "word/document.xml",
                "http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument",
            ),
            "word/document.xml": document,
            "word/_rels/document.xml.rels": doc_rels,
            "word/styles.xml": styles,
            "docProps/core.xml": core_props(f"docx pages={n_pages}"),
            "docProps/app.xml": app_props("nitro-xml fixture generator"),
        }
    )
    if pad:
        members["word/media/pad.bin"] = _pad_blob(f"docx-{n_pages}", pad)
        overrides = list(overrides) + [("/word/media/pad.bin", "application/octet-stream")]
        members["[Content_Types].xml"] = content_types(
            overrides,
            defaults=[
                ("rels", "application/vnd.openxmlformats-package.relationships+xml"),
                ("xml", "application/xml"),
                ("bin", "application/octet-stream"),
                ("png", "image/png"),
            ],
        )
    write_ooxml(path, members)


def gen_pptx_slides(path: Path, n_slides: int, *, pad: int) -> None:
    """n_slides of title text; optional binary pad for L/XL tiers."""
    overrides = [
        ("/ppt/presentation.xml", "application/vnd.openxmlformats-officedocument.presentationml.presentation.main+xml"),
        ("/ppt/slideMasters/slideMaster1.xml", "application/vnd.openxmlformats-officedocument.presentationml.slideMaster+xml"),
        ("/ppt/slideLayouts/slideLayout1.xml", "application/vnd.openxmlformats-officedocument.presentationml.slideLayout+xml"),
        ("/ppt/theme/theme1.xml", "application/vnd.openxmlformats-officedocument.theme+xml"),
        ("/docProps/core.xml", "application/vnd.openxmlformats-package.core-properties+xml"),
        ("/docProps/app.xml", "application/vnd.openxmlformats-officedocument.extended-properties+xml"),
    ]
    sld_id_lst = []
    pres_rels = [
        '<Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideMaster" Target="slideMasters/slideMaster1.xml"/>',
        '<Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/theme" Target="theme/theme1.xml"/>',
    ]
    members: dict[str, bytes] = {}
    for i in range(1, n_slides + 1):
        overrides.append(
            (f"/ppt/slides/slide{i}.xml", "application/vnd.openxmlformats-officedocument.presentationml.slide+xml")
        )
        rid = f"rId{i+2}"
        sld_id_lst.append(f'<p:sldId id="{255+i}" r:id="{rid}"/>')
        pres_rels.append(
            f'<Relationship Id="{rid}" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slide" Target="slides/slide{i}.xml"/>'
        )
        members[f"ppt/slides/slide{i}.xml"] = (
            '<?xml version="1.0" encoding="UTF-8" standalone="yes"?>'
            '<p:sld xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" '
            'xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" '
            'xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main">'
            "<p:cSld><p:spTree><p:nvGrpSpPr><p:cNvPr id=\"1\" name=\"\"/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr>"
            "<p:grpSpPr/><p:sp><p:nvSpPr><p:cNvPr id=\"2\" name=\"Title\"/><p:cNvSpPr txBox=\"1\"/><p:nvPr/>"
            f"</p:nvSpPr><p:spPr/><p:txBody><a:bodyPr/><a:lstStyle/><a:p><a:r><a:t>Slide {i}</a:t></a:r></a:p>"
            "</p:txBody></p:sp></p:spTree></p:cSld><p:clrMapOvr><a:masterClrMapping/></p:clrMapOvr></p:sld>"
        ).encode("utf-8")
        members[f"ppt/slides/_rels/slide{i}.xml.rels"] = (
            '<?xml version="1.0" encoding="UTF-8" standalone="yes"?>'
            '<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">'
            '<Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideLayout" Target="../slideLayouts/slideLayout1.xml"/>'
            "</Relationships>"
        ).encode("utf-8")

    presentation = (
        '<?xml version="1.0" encoding="UTF-8" standalone="yes"?>'
        '<p:presentation xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" '
        'xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" '
        'xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main">'
        '<p:sldMasterIdLst><p:sldMasterId id="2147483648" r:id="rId1"/></p:sldMasterIdLst>'
        f'<p:sldIdLst>{"".join(sld_id_lst)}</p:sldIdLst>'
        '<p:sldSz cx="12192000" cy="6858000"/><p:notesSz cx="6858000" cy="9144000"/>'
        "</p:presentation>"
    ).encode("utf-8")

    # Minimal master/layout/theme stubs.
    master = (
        '<?xml version="1.0" encoding="UTF-8" standalone="yes"?>'
        '<p:sldMaster xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" '
        'xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" '
        'xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main">'
        '<p:cSld><p:bg><p:bgRef idx="1001"><a:schemeClr val="bg1"/></p:bgRef></p:bg>'
        '<p:spTree><p:nvGrpSpPr><p:cNvPr id="1" name=""/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr><p:grpSpPr/>'
        "</p:spTree></p:cSld>"
        '<p:clrMap bg1="lt1" tx1="dk1" bg2="lt2" tx2="dk2" accent1="accent1" accent2="accent2" '
        'accent3="accent3" accent4="accent4" accent5="accent5" accent6="accent6" hlink="hlink" folHlink="folHlink"/>'
        '<p:sldLayoutIdLst><p:sldLayoutId id="2147483649" r:id="rId1"/></p:sldLayoutIdLst>'
        "</p:sldMaster>"
    ).encode("utf-8")
    layout = (
        '<?xml version="1.0" encoding="UTF-8" standalone="yes"?>'
        '<p:sldLayout xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" '
        'xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" '
        'xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main" type="blank" preserve="1">'
        '<p:cSld name="Blank"><p:spTree><p:nvGrpSpPr><p:cNvPr id="1" name=""/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr>'
        "<p:grpSpPr/></p:spTree></p:cSld>"
        '<p:clrMapOvr><a:masterClrMapping/></p:clrMapOvr></p:sldLayout>'
    ).encode("utf-8")
    theme = (
        '<?xml version="1.0" encoding="UTF-8" standalone="yes"?>'
        '<a:theme xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" name="nitro">'
        "<a:themeElements><a:clrScheme name=\"nitro\">"
        '<a:dk1><a:sysClr val="windowText" lastClr="000000"/></a:dk1>'
        '<a:lt1><a:sysClr val="window" lastClr="FFFFFF"/></a:lt1>'
        '<a:dk2><a:srgbClr val="1F497D"/></a:dk2><a:lt2><a:srgbClr val="EEECE1"/></a:lt2>'
        '<a:accent1><a:srgbClr val="4F81BD"/></a:accent1><a:accent2><a:srgbClr val="C0504D"/></a:accent2>'
        '<a:accent3><a:srgbClr val="9BBB59"/></a:accent3><a:accent4><a:srgbClr val="8064A2"/></a:accent4>'
        '<a:accent5><a:srgbClr val="4BACC6"/></a:accent5><a:accent6><a:srgbClr val="F79646"/></a:accent6>'
        '<a:hlink><a:srgbClr val="0000FF"/></a:hlink><a:folHlink><a:srgbClr val="800080"/></a:folHlink>'
        "</a:clrScheme>"
        '<a:fontScheme name="nitro"><a:majorFont><a:latin typeface="Calibri"/><a:ea typeface=""/><a:cs typeface=""/></a:majorFont>'
        '<a:minorFont><a:latin typeface="Calibri"/><a:ea typeface=""/><a:cs typeface=""/></a:minorFont></a:fontScheme>'
        '<a:fmtScheme name="nitro"><a:fillStyleLst><a:solidFill><a:schemeClr val="phClr"/></a:solidFill>'
        '<a:solidFill><a:schemeClr val="phClr"/></a:solidFill><a:solidFill><a:schemeClr val="phClr"/></a:solidFill></a:fillStyleLst>'
        '<a:lnStyleLst><a:ln w="9525"><a:solidFill><a:schemeClr val="phClr"/></a:solidFill></a:ln>'
        '<a:ln w="9525"><a:solidFill><a:schemeClr val="phClr"/></a:solidFill></a:ln>'
        '<a:ln w="9525"><a:solidFill><a:schemeClr val="phClr"/></a:solidFill></a:ln></a:lnStyleLst>'
        '<a:effectStyleLst><a:effectStyle><a:effectLst/></a:effectStyle><a:effectStyle><a:effectLst/></a:effectStyle>'
        '<a:effectStyle><a:effectLst/></a:effectStyle></a:effectStyleLst>'
        '<a:bgFillStyleLst><a:solidFill><a:schemeClr val="phClr"/></a:solidFill>'
        '<a:solidFill><a:schemeClr val="phClr"/></a:solidFill><a:solidFill><a:schemeClr val="phClr"/></a:solidFill>'
        "</a:bgFillStyleLst></a:fmtScheme></a:themeElements></a:theme>"
    ).encode("utf-8")

    members.update(
        {
            "[Content_Types].xml": content_types(overrides),
            "_rels/.rels": rels_root(
                "ppt/presentation.xml",
                "http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument",
            ),
            "ppt/presentation.xml": presentation,
            "ppt/_rels/presentation.xml.rels": (
                '<?xml version="1.0" encoding="UTF-8" standalone="yes"?>'
                '<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">'
                + "".join(pres_rels)
                + "</Relationships>"
            ).encode("utf-8"),
            "ppt/slideMasters/slideMaster1.xml": master,
            "ppt/slideMasters/_rels/slideMaster1.xml.rels": (
                '<?xml version="1.0" encoding="UTF-8" standalone="yes"?>'
                '<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">'
                '<Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideLayout" Target="../slideLayouts/slideLayout1.xml"/>'
                '<Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/theme" Target="../theme/theme1.xml"/>'
                "</Relationships>"
            ).encode("utf-8"),
            "ppt/slideLayouts/slideLayout1.xml": layout,
            "ppt/slideLayouts/_rels/slideLayout1.xml.rels": (
                '<?xml version="1.0" encoding="UTF-8" standalone="yes"?>'
                '<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">'
                '<Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideMaster" Target="../slideMasters/slideMaster1.xml"/>'
                "</Relationships>"
            ).encode("utf-8"),
            "ppt/theme/theme1.xml": theme,
            "docProps/core.xml": core_props(f"pptx slides={n_slides}"),
            "docProps/app.xml": app_props("nitro-xml fixture generator", f"<Slides>{n_slides}</Slides>"),
        }
    )
    if pad:
        members["ppt/media/pad.bin"] = _pad_blob(f"pptx-{n_slides}", pad)
        overrides = list(overrides) + [("/ppt/media/pad.bin", "application/octet-stream")]
        members["[Content_Types].xml"] = content_types(
            overrides,
            defaults=[
                ("rels", "application/vnd.openxmlformats-package.relationships+xml"),
                ("xml", "application/xml"),
                ("bin", "application/octet-stream"),
            ],
        )
    write_ooxml(path, members)


# Spec recipes: id -> (relpath, builder kwargs / callable)
# Pads chosen so compressed packages land in the named tier after generation.
RECIPES: dict[str, dict] = {
    # xlsx — cells + features; pad for M/L/XL
    "gen-xlsx-cells-1k": {
        "format": "xlsx",
        "relpath": "xlsx/cells_1k.xlsx",
        "tier_hint": "S",
        "features": ["generated", "cells-1k", "shared-strings", "styles", "conditional-formats", "print", "painter:X3", "painter:X10"],
        "build": lambda p: gen_xlsx_cells(p, 1000, merges=False, shared_extra=0, pad=0),
    },
    "gen-xlsx-cells-1k-merges": {
        "format": "xlsx",
        "relpath": "xlsx/cells_1k_merges.xlsx",
        "tier_hint": "S",
        "features": ["generated", "cells-1k", "merges", "shared-strings", "painter:X3", "painter:X10"],
        "build": lambda p: gen_xlsx_cells(p, 1000, merges=True, shared_extra=50, pad=0),
    },
    "gen-xlsx-cells-1k-pad-s": {
        "format": "xlsx",
        "relpath": "xlsx/cells_1k_pad_s.xlsx",
        "tier_hint": "S",
        "features": ["generated", "cells-1k", "perf-tier"],
        "build": lambda p: gen_xlsx_cells(p, 1000, merges=False, shared_extra=0, pad=40_000),
    },
    "gen-xlsx-cells-100k": {
        "format": "xlsx",
        "relpath": "xlsx/cells_100k.xlsx",
        "tier_hint": "M",
        "features": ["generated", "cells-100k", "shared-strings", "styles", "conditional-formats", "perf-tier", "painter:X3"],
        "build": lambda p: gen_xlsx_cells(p, 100_000, merges=True, shared_extra=200, pad=0),
    },
    "gen-xlsx-cells-100k-b": {
        "format": "xlsx",
        "relpath": "xlsx/cells_100k_b.xlsx",
        "tier_hint": "M",
        "features": ["generated", "cells-100k", "perf-tier"],
        "build": lambda p: gen_xlsx_cells(p, 100_000, merges=False, shared_extra=500, pad=200_000),
    },
    "gen-xlsx-cells-100k-c": {
        "format": "xlsx",
        "relpath": "xlsx/cells_100k_c.xlsx",
        "tier_hint": "M",
        "features": ["generated", "cells-100k", "perf-tier"],
        "build": lambda p: gen_xlsx_cells(p, 120_000, merges=True, shared_extra=100, pad=350_000),
    },
    "gen-xlsx-cells-1m": {
        "format": "xlsx",
        "relpath": "xlsx/cells_1m.xlsx",
        "tier_hint": "L",
        "features": ["generated", "cells-1m", "shared-strings", "styles", "merges", "conditional-formats", "perf-tier", "painter:X3"],
        "build": lambda p: gen_xlsx_cells(p, 1_000_000, merges=True, shared_extra=1000, pad=0),
        "on_demand": True,
    },
    "gen-xlsx-cells-1m-pad-l": {
        "format": "xlsx",
        "relpath": "xlsx/cells_1m_pad_l.xlsx",
        "tier_hint": "L",
        "features": ["generated", "cells-1m", "perf-tier"],
        "build": lambda p: gen_xlsx_cells(p, 1_000_000, merges=False, shared_extra=0, pad=2_000_000),
        "on_demand": True,
    },
    "gen-xlsx-cells-1m-pad-l2": {
        "format": "xlsx",
        "relpath": "xlsx/cells_1m_pad_l2.xlsx",
        "tier_hint": "L",
        "features": ["generated", "cells-1m", "perf-tier"],
        "build": lambda p: gen_xlsx_cells(p, 1_000_000, merges=True, shared_extra=2000, pad=4_000_000),
        "on_demand": True,
    },
    "gen-xlsx-xl-a": {
        "format": "xlsx",
        "relpath": "xlsx/xl_a.xlsx",
        "tier_hint": "XL",
        "features": ["generated", "cells-1m", "perf-tier"],
        "build": lambda p: gen_xlsx_cells(p, 1_000_000, merges=True, shared_extra=5000, pad=12_000_000),
        "on_demand": True,
    },
    "gen-xlsx-xl-b": {
        "format": "xlsx",
        "relpath": "xlsx/xl_b.xlsx",
        "tier_hint": "XL",
        "features": ["generated", "cells-1m", "perf-tier"],
        "build": lambda p: gen_xlsx_cells(p, 1_000_000, merges=False, shared_extra=8000, pad=14_000_000),
        "on_demand": True,
    },
    "gen-xlsx-xl-c": {
        "format": "xlsx",
        "relpath": "xlsx/xl_c.xlsx",
        "tier_hint": "XL",
        "features": ["generated", "cells-1m", "perf-tier"],
        "build": lambda p: gen_xlsx_cells(p, 1_000_000, merges=True, shared_extra=10000, pad=16_000_000),
        "on_demand": True,
    },
    # docx
    "gen-docx-pages-10": {
        "format": "docx",
        "relpath": "docx/pages_10.docx",
        "tier_hint": "S",
        "features": ["generated", "pages-10", "tables", "images"],
        "build": lambda p: gen_docx_pages(p, 10, tables=True, images=True, pad=0),
    },
    "gen-docx-pages-10-b": {
        "format": "docx",
        "relpath": "docx/pages_10_b.docx",
        "tier_hint": "S",
        "features": ["generated", "pages-10"],
        "build": lambda p: gen_docx_pages(p, 10, tables=False, images=False, pad=30_000),
    },
    "gen-docx-pages-10-c": {
        "format": "docx",
        "relpath": "docx/pages_10_c.docx",
        "tier_hint": "S",
        "features": ["generated", "pages-10", "tables"],
        "build": lambda p: gen_docx_pages(p, 12, tables=True, images=False, pad=50_000),
    },
    "gen-docx-pages-100": {
        "format": "docx",
        "relpath": "docx/pages_100.docx",
        "tier_hint": "M",
        "features": ["generated", "pages-100", "tables", "images", "perf-tier"],
        "build": lambda p: gen_docx_pages(p, 100, tables=True, images=True, pad=400_000),
    },
    "gen-docx-pages-100-b": {
        "format": "docx",
        "relpath": "docx/pages_100_b.docx",
        "tier_hint": "M",
        "features": ["generated", "pages-100", "perf-tier"],
        "build": lambda p: gen_docx_pages(p, 100, tables=True, images=False, pad=600_000),
    },
    "gen-docx-pages-100-c": {
        "format": "docx",
        "relpath": "docx/pages_100_c.docx",
        "tier_hint": "M",
        "features": ["generated", "pages-100", "perf-tier"],
        "build": lambda p: gen_docx_pages(p, 120, tables=False, images=True, pad=700_000),
    },
    "gen-docx-pages-500": {
        "format": "docx",
        "relpath": "docx/pages_500.docx",
        "tier_hint": "L",
        "features": ["generated", "pages-500", "tables", "images", "perf-tier"],
        "build": lambda p: gen_docx_pages(p, 500, tables=True, images=True, pad=2_500_000),
        "on_demand": True,
    },
    "gen-docx-pages-500-b": {
        "format": "docx",
        "relpath": "docx/pages_500_b.docx",
        "tier_hint": "L",
        "features": ["generated", "pages-500", "perf-tier"],
        "build": lambda p: gen_docx_pages(p, 500, tables=True, images=False, pad=4_000_000),
        "on_demand": True,
    },
    "gen-docx-pages-500-c": {
        "format": "docx",
        "relpath": "docx/pages_500_c.docx",
        "tier_hint": "L",
        "features": ["generated", "pages-500", "perf-tier"],
        "build": lambda p: gen_docx_pages(p, 500, tables=False, images=True, pad=6_000_000),
        "on_demand": True,
    },
    "gen-docx-xl-a": {
        "format": "docx",
        "relpath": "docx/xl_a.docx",
        "tier_hint": "XL",
        "features": ["generated", "pages-500", "perf-tier"],
        "build": lambda p: gen_docx_pages(p, 500, tables=True, images=True, pad=12_000_000),
        "on_demand": True,
    },
    "gen-docx-xl-b": {
        "format": "docx",
        "relpath": "docx/xl_b.docx",
        "tier_hint": "XL",
        "features": ["generated", "pages-500", "perf-tier"],
        "build": lambda p: gen_docx_pages(p, 500, tables=True, images=False, pad=14_000_000),
        "on_demand": True,
    },
    "gen-docx-xl-c": {
        "format": "docx",
        "relpath": "docx/xl_c.docx",
        "tier_hint": "XL",
        "features": ["generated", "pages-500", "perf-tier"],
        "build": lambda p: gen_docx_pages(p, 500, tables=False, images=True, pad=16_000_000),
        "on_demand": True,
    },
    # pptx
    "gen-pptx-slides-10": {
        "format": "pptx",
        "relpath": "pptx/slides_10.pptx",
        "tier_hint": "S",
        "features": ["generated", "slides-10", "painter:P26", "painter:P27"],
        "build": lambda p: gen_pptx_slides(p, 10, pad=0),
    },
    "gen-pptx-slides-10-b": {
        "format": "pptx",
        "relpath": "pptx/slides_10_b.pptx",
        "tier_hint": "S",
        "features": ["generated", "slides-10"],
        "build": lambda p: gen_pptx_slides(p, 10, pad=40_000),
    },
    "gen-pptx-slides-10-c": {
        "format": "pptx",
        "relpath": "pptx/slides_10_c.pptx",
        "tier_hint": "S",
        "features": ["generated", "slides-10"],
        "build": lambda p: gen_pptx_slides(p, 15, pad=60_000),
    },
    "gen-pptx-slides-50": {
        "format": "pptx",
        "relpath": "pptx/slides_50.pptx",
        "tier_hint": "M",
        "features": ["generated", "slides-50", "perf-tier", "painter:P26", "painter:P27"],
        "build": lambda p: gen_pptx_slides(p, 50, pad=400_000),
    },
    "gen-pptx-slides-50-b": {
        "format": "pptx",
        "relpath": "pptx/slides_50_b.pptx",
        "tier_hint": "M",
        "features": ["generated", "slides-50", "perf-tier"],
        "build": lambda p: gen_pptx_slides(p, 50, pad=600_000),
    },
    "gen-pptx-slides-50-c": {
        "format": "pptx",
        "relpath": "pptx/slides_50_c.pptx",
        "tier_hint": "M",
        "features": ["generated", "slides-50", "perf-tier"],
        "build": lambda p: gen_pptx_slides(p, 60, pad=700_000),
    },
    "gen-pptx-slides-200": {
        "format": "pptx",
        "relpath": "pptx/slides_200.pptx",
        "tier_hint": "L",
        "features": ["generated", "slides-200", "perf-tier"],
        "build": lambda p: gen_pptx_slides(p, 200, pad=2_500_000),
        "on_demand": True,
    },
    "gen-pptx-slides-200-b": {
        "format": "pptx",
        "relpath": "pptx/slides_200_b.pptx",
        "tier_hint": "L",
        "features": ["generated", "slides-200", "perf-tier"],
        "build": lambda p: gen_pptx_slides(p, 200, pad=4_000_000),
        "on_demand": True,
    },
    "gen-pptx-slides-200-c": {
        "format": "pptx",
        "relpath": "pptx/slides_200_c.pptx",
        "tier_hint": "L",
        "features": ["generated", "slides-200", "perf-tier"],
        "build": lambda p: gen_pptx_slides(p, 220, pad=6_000_000),
        "on_demand": True,
    },
    "gen-pptx-xl-a": {
        "format": "pptx",
        "relpath": "pptx/xl_a.pptx",
        "tier_hint": "XL",
        "features": ["generated", "slides-200", "perf-tier"],
        "build": lambda p: gen_pptx_slides(p, 200, pad=12_000_000),
        "on_demand": True,
    },
    "gen-pptx-xl-b": {
        "format": "pptx",
        "relpath": "pptx/xl_b.pptx",
        "tier_hint": "XL",
        "features": ["generated", "slides-200", "perf-tier"],
        "build": lambda p: gen_pptx_slides(p, 200, pad=14_000_000),
        "on_demand": True,
    },
    "gen-pptx-xl-c": {
        "format": "pptx",
        "relpath": "pptx/xl_c.pptx",
        "tier_hint": "XL",
        "features": ["generated", "slides-200", "perf-tier"],
        "build": lambda p: gen_pptx_slides(p, 200, pad=16_000_000),
        "on_demand": True,
    },
}


def build_recipe(recipe_id: str, *, force: bool = False) -> Path:
    recipe = RECIPES[recipe_id]
    path = OUT / recipe["relpath"]
    if path.exists() and not force:
        return path
    recipe["build"](path)
    return path
