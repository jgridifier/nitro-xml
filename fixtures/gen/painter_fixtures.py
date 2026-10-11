#!/usr/bin/env python3
# SPDX-License-Identifier: MIT
"""Minimal generated fixtures that hit painter-inventory rows missing from real/upstream files."""

from __future__ import annotations

from pathlib import Path

from ooxml_zip import app_props, content_types, core_props, rels_root, write_ooxml
from tiers import OUT

# Rows PR #111 left "not exercised by fixture" (36 of 53).
PAINTER_ROWS_UNEXERCISED = [
    "X3",
    "X8",
    "X9",
    "X10",
    "X11",
    "X12",
    "D4",
    "D5",
    "D6",
    "D7",
    "D10",
    "D11",
    "P4",
    "P6",
    "P7",
    "P8",
    "P9",
    "P10",
    "P12",
    "P13",
    "P14",
    "P15",
    "P16",
    "P17",
    "P18",
    "P19",
    "P20",
    "P21",
    "P22",
    "P23",
    "P24",
    "P25",
    "P26",
    "P27",
    "S1",
    "S2",
]


def gen_docx_page_borders(path: Path) -> None:
    """D6 — section page borders."""
    document = (
        '<?xml version="1.0" encoding="UTF-8" standalone="yes"?>'
        '<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">'
        "<w:body><w:p><w:r><w:t>Page borders</w:t></w:r></w:p>"
        "<w:sectPr><w:pgSz w:w=\"12240\" w:h=\"15840\"/>"
        '<w:pgMar w:top="1440" w:right="1440" w:bottom="1440" w:left="1440"/>'
        '<w:pgBorders w:offsetFrom="page">'
        '<w:top w:val="single" w:sz="24" w:space="24" w:color="000000"/>'
        '<w:left w:val="single" w:sz="24" w:space="24" w:color="000000"/>'
        '<w:bottom w:val="single" w:sz="24" w:space="24" w:color="000000"/>'
        '<w:right w:val="single" w:sz="24" w:space="24" w:color="000000"/>'
        "</w:pgBorders></w:sectPr></w:body></w:document>"
    ).encode("utf-8")
    _write_simple_docx(path, "page-borders", document, ["painter:D6", "page-borders"])


def gen_docx_line_numbers(path: Path) -> None:
    """D7 — line number labels."""
    document = (
        '<?xml version="1.0" encoding="UTF-8" standalone="yes"?>'
        '<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">'
        "<w:body><w:p><w:r><w:t>Line one</w:t></w:r></w:p>"
        "<w:p><w:r><w:t>Line two</w:t></w:r></w:p>"
        "<w:sectPr><w:pgSz w:w=\"12240\" w:h=\"15840\"/>"
        '<w:pgMar w:top="1440" w:right="1440" w:bottom="1440" w:left="1440"/>'
        '<w:lnNumType w:countBy="1" w:restart="newPage"/>'
        "</w:sectPr></w:body></w:document>"
    ).encode("utf-8")
    _write_simple_docx(path, "line-numbers", document, ["painter:D7", "line-numbers"])


def gen_docx_equation(path: Path) -> None:
    """D10 — OMML equation (MathML paint path)."""
    document = (
        '<?xml version="1.0" encoding="UTF-8" standalone="yes"?>'
        '<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" '
        'xmlns:m="http://schemas.openxmlformats.org/officeDocument/2006/math">'
        "<w:body><w:p><m:oMathPara><m:oMath>"
        '<m:r><m:t>E=mc</m:t></m:r>'
        '<m:sSup><m:e><m:r><m:t>2</m:t></m:r></m:e><m:sup><m:r><m:t></m:t></m:r></m:sup></m:sSup>'
        "</m:oMath></m:oMathPara></w:p>"
        "<w:sectPr><w:pgSz w:w=\"12240\" w:h=\"15840\"/>"
        '<w:pgMar w:top="1440" w:right="1440" w:bottom="1440" w:left="1440"/>'
        "</w:sectPr></w:body></w:document>"
    ).encode("utf-8")
    _write_simple_docx(path, "equation", document, ["painter:D10", "equation"])


def _write_simple_docx(path: Path, title: str, document: bytes, _features: list[str]) -> None:
    styles = (
        '<?xml version="1.0" encoding="UTF-8" standalone="yes"?>'
        '<w:styles xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">'
        '<w:style w:type="paragraph" w:default="1" w:styleId="Normal"><w:name w:val="Normal"/></w:style>'
        "</w:styles>"
    ).encode("utf-8")
    write_ooxml(
        path,
        {
            "[Content_Types].xml": content_types(
                [
                    ("/word/document.xml", "application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"),
                    ("/word/styles.xml", "application/vnd.openxmlformats-officedocument.wordprocessingml.styles+xml"),
                    ("/docProps/core.xml", "application/vnd.openxmlformats-package.core-properties+xml"),
                    ("/docProps/app.xml", "application/vnd.openxmlformats-officedocument.extended-properties+xml"),
                ]
            ),
            "_rels/.rels": rels_root(
                "word/document.xml",
                "http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument",
            ),
            "word/document.xml": document,
            "word/_rels/document.xml.rels": (
                '<?xml version="1.0" encoding="UTF-8" standalone="yes"?>'
                '<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">'
                '<Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles" Target="styles.xml"/>'
                "</Relationships>"
            ).encode("utf-8"),
            "word/styles.xml": styles,
            "docProps/core.xml": core_props(title),
            "docProps/app.xml": app_props("nitro-xml fixture generator"),
        },
    )


def gen_pptx_equation(path: Path) -> None:
    """P20 — OMML inside a text shape."""
    slide = (
        '<?xml version="1.0" encoding="UTF-8" standalone="yes"?>'
        '<p:sld xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" '
        'xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" '
        'xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main" '
        'xmlns:m="http://schemas.openxmlformats.org/officeDocument/2006/math">'
        "<p:cSld><p:spTree><p:nvGrpSpPr><p:cNvPr id=\"1\" name=\"\"/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr><p:grpSpPr/>"
        '<p:sp><p:nvSpPr><p:cNvPr id="2" name="eq"/><p:cNvSpPr txBox="1"/><p:nvPr/></p:nvSpPr><p:spPr/>'
        "<p:txBody><a:bodyPr/><a:lstStyle/><a:p>"
        "<a:r><a:t>Eq </a:t></a:r>"
        "<m:oMath><m:r><m:t>x</m:t></m:r></m:oMath>"
        "</a:p></p:txBody></p:sp></p:spTree></p:cSld>"
        '<p:clrMapOvr><a:masterClrMapping/></p:clrMapOvr></p:sld>'
    ).encode("utf-8")
    _write_one_slide_pptx(path, "equation", slide)


def gen_pptx_activex(path: Path) -> None:
    """P16 — ActiveX control placeholder (static fallback picture path)."""
    slide = (
        '<?xml version="1.0" encoding="UTF-8" standalone="yes"?>'
        '<p:sld xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" '
        'xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" '
        'xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main">'
        "<p:cSld><p:spTree><p:nvGrpSpPr><p:cNvPr id=\"1\" name=\"\"/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr><p:grpSpPr/>"
        '<p:pic><p:nvPicPr><p:cNvPr id="2" name="ActiveX"/><p:cNvPicPr/><p:nvPr>'
        '<p:extLst><p:ext uri="{2DF8D04C-5BFA-101B-BDE5-00AA0044DE52}">'
        '<a:extLst xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"/>'
        "</p:ext></p:extLst>"
        '<p:controls xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main"/>'
        "</p:nvPr></p:nvPicPr>"
        '<p:blipFill><a:blip r:embed="rId2"/><a:stretch><a:fillRect/></a:stretch></p:blipFill>'
        '<p:spPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="1000000" cy="1000000"/></a:xfrm>'
        '<a:prstGeom prst="rect"><a:avLst/></a:prstGeom></p:spPr></p:pic>'
        "</p:spTree></p:cSld><p:clrMapOvr><a:masterClrMapping/></p:clrMapOvr>"
        '<p:controls><p:control id="1" name="CommandButton1" r:id="rId3">'
        '<p:extLst/></p:control></p:controls></p:sld>'
    ).encode("utf-8")
    from ooxml_zip import tiny_png

    extra = {
        "ppt/media/image1.png": tiny_png(),
        "ppt/activeX/activeX1.xml": (
            '<?xml version="1.0" encoding="UTF-8" standalone="yes"?>'
            '<ax:ocx ax:classid="{D7053240-CE69-11CD-A777-00DD01143C57}" '
            'xmlns:ax="http://schemas.microsoft.com/office/2006/activeX" '
            'xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"/>'
        ).encode("utf-8"),
    }
    slide_rels_extra = (
        '<Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image" Target="../media/image1.png"/>'
        '<Relationship Id="rId3" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/control" Target="../activeX/activeX1.xml"/>'
    )
    overrides_extra = [
        ("/ppt/media/image1.png", "image/png"),
        ("/ppt/activeX/activeX1.xml", "application/vnd.ms-office.activeX+xml"),
    ]
    _write_one_slide_pptx(path, "activex", slide, extra=extra, slide_rels_extra=slide_rels_extra, overrides_extra=overrides_extra)


def gen_pptx_model3d(path: Path) -> None:
    """P17 — 3D model poster placeholder (no GLB bytes; painter shows placeholder)."""
    slide = (
        '<?xml version="1.0" encoding="UTF-8" standalone="yes"?>'
        '<p:sld xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" '
        'xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" '
        'xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main" '
        'xmlns:asvg="http://schemas.microsoft.com/office/drawing/2016/SVG/main" '
        'xmlns:am3d="http://schemas.microsoft.com/office/drawing/2017/model3d">'
        "<p:cSld><p:spTree><p:nvGrpSpPr><p:cNvPr id=\"1\" name=\"\"/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr><p:grpSpPr/>"
        '<p:graphicFrame><p:nvGraphicFramePr><p:cNvPr id="2" name="3D Model"/>'
        "<p:cNvGraphicFramePr/><p:nvPr/></p:nvGraphicFramePr>"
        '<p:xfrm><a:off x="0" y="0"/><a:ext cx="2000000" cy="2000000"/></p:xfrm>'
        '<a:graphic><a:graphicData uri="http://schemas.microsoft.com/office/drawing/2017/model3d">'
        "<am3d:model3d><am3d:rasterImage r:embed=\"rId2\"/></am3d:model3d>"
        "</a:graphicData></a:graphic></p:graphicFrame>"
        "</p:spTree></p:cSld><p:clrMapOvr><a:masterClrMapping/></p:clrMapOvr></p:sld>"
    ).encode("utf-8")
    from ooxml_zip import tiny_png

    extra = {"ppt/media/poster.png": tiny_png(rgb=(200, 100, 50))}
    slide_rels_extra = (
        '<Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image" Target="../media/poster.png"/>'
    )
    overrides_extra = [("/ppt/media/poster.png", "image/png")]
    _write_one_slide_pptx(
        path, "model3d", slide, extra=extra, slide_rels_extra=slide_rels_extra, overrides_extra=overrides_extra
    )


def gen_pptx_zoom(path: Path) -> None:
    """P21 — slide zoom tile pointing at slide 2."""
    # Two-slide deck; slide 1 hosts a slideZoom.
    from tiers import gen_pptx_slides
    import zipfile

    tmp = path.with_suffix(".tmp.pptx")
    gen_pptx_slides(tmp, 2, pad=0)
    # Patch slide1 to include a p14:slideZoom graphic frame.
    with zipfile.ZipFile(tmp, "r") as zin:
        members = {n: zin.read(n) for n in zin.namelist()}
    zoom_slide = (
        '<?xml version="1.0" encoding="UTF-8" standalone="yes"?>'
        '<p:sld xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" '
        'xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" '
        'xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main" '
        'xmlns:p14="http://schemas.microsoft.com/office/powerpoint/2010/main">'
        "<p:cSld><p:spTree><p:nvGrpSpPr><p:cNvPr id=\"1\" name=\"\"/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr><p:grpSpPr/>"
        '<p:graphicFrame><p:nvGraphicFramePr><p:cNvPr id="2" name="Slide Zoom"/>'
        "<p:cNvGraphicFramePr/><p:nvPr><p:extLst>"
        '<p:ext uri="{FF2B5EF4-FFF2-40B4-BE49-F238E27FC236}">'
        '<p14:modId val="1"/>'
        "</p:ext></p:extLst></p:nvPr></p:nvGraphicFramePr>"
        '<p:xfrm><a:off x="0" y="0"/><a:ext cx="3000000" cy="2000000"/></p:xfrm>'
        '<a:graphic><a:graphicData uri="http://schemas.microsoft.com/office/powerpoint/2010/main">'
        '<p14:slideZoom cId="256"><p14:sldIdlist><p14:sldId id="256"/></p14:sldIdlist></p14:slideZoom>'
        "</a:graphicData></a:graphic></p:graphicFrame>"
        "</p:spTree></p:cSld><p:clrMapOvr><a:masterClrMapping/></p:clrMapOvr></p:sld>"
    ).encode("utf-8")
    members["ppt/slides/slide1.xml"] = zoom_slide
    # Rewrite with deterministic helper
    write_ooxml(path, members)
    tmp.unlink(missing_ok=True)


def gen_pptx_placeholder(path: Path) -> None:
    """P22 — empty inherited placeholder."""
    slide = (
        '<?xml version="1.0" encoding="UTF-8" standalone="yes"?>'
        '<p:sld xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" '
        'xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" '
        'xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main">'
        "<p:cSld><p:spTree><p:nvGrpSpPr><p:cNvPr id=\"1\" name=\"\"/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr><p:grpSpPr/>"
        '<p:sp><p:nvSpPr><p:cNvPr id="2" name="Title Placeholder"/>'
        '<p:cNvSpPr><a:spLocks noGrp="1"/></p:cNvSpPr>'
        '<p:nvPr><p:ph type="title"/></p:nvPr></p:nvSpPr><p:spPr/>'
        "<p:txBody><a:bodyPr/><a:lstStyle/><a:p/></p:txBody></p:sp>"
        "</p:spTree></p:cSld><p:clrMapOvr><a:masterClrMapping/></p:clrMapOvr></p:sld>"
    ).encode("utf-8")
    _write_one_slide_pptx(path, "placeholder", slide)


def gen_pptx_notes_print(path: Path) -> None:
    """P25 — notes slide for notes-pages / handout print path."""
    from tiers import gen_pptx_slides
    import zipfile

    tmp = path.with_suffix(".tmp.pptx")
    gen_pptx_slides(tmp, 1, pad=0)
    with zipfile.ZipFile(tmp, "r") as zin:
        members = {n: zin.read(n) for n in zin.namelist()}
    members["ppt/notesSlides/notesSlide1.xml"] = (
        '<?xml version="1.0" encoding="UTF-8" standalone="yes"?>'
        '<p:notes xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" '
        'xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" '
        'xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main">'
        "<p:cSld><p:spTree><p:nvGrpSpPr><p:cNvPr id=\"1\" name=\"\"/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr>"
        "<p:grpSpPr/><p:sp><p:nvSpPr><p:cNvPr id=\"2\" name=\"Notes\"/><p:cNvSpPr txBox=\"1\"/><p:nvPr/>"
        "</p:nvSpPr><p:spPr/><p:txBody><a:bodyPr/><a:lstStyle/><a:p><a:r><a:t>Speaker notes</a:t></a:r></a:p>"
        "</p:txBody></p:sp></p:spTree></p:cSld></p:notes>"
    ).encode("utf-8")
    members["ppt/notesSlides/_rels/notesSlide1.xml.rels"] = (
        '<?xml version="1.0" encoding="UTF-8" standalone="yes"?>'
        '<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">'
        '<Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slide" Target="../slides/slide1.xml"/>'
        '<Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/notesMaster" Target="../notesMasters/notesMaster1.xml"/>'
        "</Relationships>"
    ).encode("utf-8")
    members["ppt/slides/_rels/slide1.xml.rels"] = (
        '<?xml version="1.0" encoding="UTF-8" standalone="yes"?>'
        '<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">'
        '<Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideLayout" Target="../slideLayouts/slideLayout1.xml"/>'
        '<Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/notesSlide" Target="../notesSlides/notesSlide1.xml"/>'
        "</Relationships>"
    ).encode("utf-8")
    # Minimal notes master
    members["ppt/notesMasters/notesMaster1.xml"] = (
        '<?xml version="1.0" encoding="UTF-8" standalone="yes"?>'
        '<p:notesMaster xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" '
        'xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" '
        'xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main">'
        '<p:cSld><p:spTree><p:nvGrpSpPr><p:cNvPr id="1" name=""/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr><p:grpSpPr/>'
        "</p:spTree></p:cSld>"
        '<p:clrMap bg1="lt1" tx1="dk1" bg2="lt2" tx2="dk2" accent1="accent1" accent2="accent2" '
        'accent3="accent3" accent4="accent4" accent5="accent5" accent6="accent6" hlink="hlink" folHlink="folHlink"/>'
        "</p:notesMaster>"
    ).encode("utf-8")
    # Patch content types
    ct = members["[Content_Types].xml"].decode("utf-8")
    insert = (
        '<Override PartName="/ppt/notesSlides/notesSlide1.xml" '
        'ContentType="application/vnd.openxmlformats-officedocument.presentationml.notesSlide+xml"/>'
        '<Override PartName="/ppt/notesMasters/notesMaster1.xml" '
        'ContentType="application/vnd.openxmlformats-officedocument.presentationml.notesMaster+xml"/>'
    )
    members["[Content_Types].xml"] = ct.replace("</Types>", insert + "</Types>").encode("utf-8")
    write_ooxml(path, members)
    tmp.unlink(missing_ok=True)


def _write_one_slide_pptx(
    path: Path,
    title: str,
    slide: bytes,
    *,
    extra: dict[str, bytes] | None = None,
    slide_rels_extra: str = "",
    overrides_extra: list[tuple[str, str]] | None = None,
) -> None:
    from tiers import gen_pptx_slides
    import zipfile

    tmp = path.with_suffix(".tmp.pptx")
    gen_pptx_slides(tmp, 1, pad=0)
    with zipfile.ZipFile(tmp, "r") as zin:
        members = {n: zin.read(n) for n in zin.namelist()}
    members["ppt/slides/slide1.xml"] = slide
    members["ppt/slides/_rels/slide1.xml.rels"] = (
        '<?xml version="1.0" encoding="UTF-8" standalone="yes"?>'
        '<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">'
        '<Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideLayout" Target="../slideLayouts/slideLayout1.xml"/>'
        f"{slide_rels_extra}</Relationships>"
    ).encode("utf-8")
    members["docProps/core.xml"] = core_props(title)
    if extra:
        members.update(extra)
    if overrides_extra:
        ct = members["[Content_Types].xml"].decode("utf-8")
        insert = "".join(f'<Override PartName="{p}" ContentType="{c}"/>' for p, c in overrides_extra)
        members["[Content_Types].xml"] = ct.replace("</Types>", insert + "</Types>").encode("utf-8")
    write_ooxml(path, members)
    tmp.unlink(missing_ok=True)


PAINTER_RECIPES: dict[str, dict] = {
    "gen-docx-page-borders": {
        "format": "docx",
        "relpath": "docx/painter_page_borders.docx",
        "features": ["generated", "page-borders", "painter:D6"],
        "build": gen_docx_page_borders,
    },
    "gen-docx-line-numbers": {
        "format": "docx",
        "relpath": "docx/painter_line_numbers.docx",
        "features": ["generated", "line-numbers", "painter:D7"],
        "build": gen_docx_line_numbers,
    },
    "gen-docx-equation": {
        "format": "docx",
        "relpath": "docx/painter_equation.docx",
        "features": ["generated", "equation", "painter:D10"],
        "build": gen_docx_equation,
    },
    "gen-pptx-equation": {
        "format": "pptx",
        "relpath": "pptx/painter_equation.pptx",
        "features": ["generated", "equation", "painter:P20"],
        "build": gen_pptx_equation,
    },
    "gen-pptx-activex": {
        "format": "pptx",
        "relpath": "pptx/painter_activex.pptx",
        "features": ["generated", "activex", "painter:P16"],
        "build": gen_pptx_activex,
    },
    "gen-pptx-model3d": {
        "format": "pptx",
        "relpath": "pptx/painter_model3d.pptx",
        "features": ["generated", "model3d", "painter:P17", "painter:P13"],
        "build": gen_pptx_model3d,
    },
    "gen-pptx-zoom": {
        "format": "pptx",
        "relpath": "pptx/painter_zoom.pptx",
        "features": ["generated", "zoom", "painter:P21"],
        "build": gen_pptx_zoom,
    },
    "gen-pptx-placeholder": {
        "format": "pptx",
        "relpath": "pptx/painter_placeholder.pptx",
        "features": ["generated", "placeholder", "painter:P22"],
        "build": gen_pptx_placeholder,
    },
    "gen-pptx-notes-print": {
        "format": "pptx",
        "relpath": "pptx/painter_notes_print.pptx",
        "features": ["generated", "notes", "painter:P25", "painter:P26", "painter:P27"],
        "build": gen_pptx_notes_print,
    },
}


def build_painter_recipe(recipe_id: str, *, force: bool = False) -> Path:
    recipe = PAINTER_RECIPES[recipe_id]
    path = OUT / recipe["relpath"]
    if path.exists() and not force:
        return path
    recipe["build"](path)
    return path
