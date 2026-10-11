#!/usr/bin/env python3
# SPDX-License-Identifier: MIT
"""Generate minimal allowed-variant OPC zips and negative fixtures for the #13 gate."""

from __future__ import annotations

import struct
import sys
import zlib
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
ALLOWED = ROOT / "fixtures" / "opc" / "allowed"
NEGATIVE = ROOT / "fixtures" / "opc" / "negative"

CT_NS = "http://schemas.openxmlformats.org/package/2006/content-types"
REL_NS = "http://schemas.openxmlformats.org/package/2006/relationships"
OFFICE_DOC = (
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument"
)

VARIANTS = {
    "minimal.docx": (
        "word/document.xml",
        "application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml",
        "<w:document xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\"/>",
    ),
    "minimal.docm": (
        "word/document.xml",
        "application/vnd.ms-word.document.macroEnabled.main+xml",
        "<w:document xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\"/>",
    ),
    "minimal.dotx": (
        "word/document.xml",
        "application/vnd.openxmlformats-officedocument.wordprocessingml.template.main+xml",
        "<w:document xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\"/>",
    ),
    "minimal.dotm": (
        "word/document.xml",
        "application/vnd.ms-word.template.macroEnabledTemplate.main+xml",
        "<w:document xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\"/>",
    ),
    "minimal.xlsx": (
        "xl/workbook.xml",
        "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml",
        "<workbook xmlns=\"http://schemas.openxmlformats.org/spreadsheetml/2006/main\"/>",
    ),
    "minimal.xlsm": (
        "xl/workbook.xml",
        "application/vnd.ms-excel.sheet.macroEnabled.main+xml",
        "<workbook xmlns=\"http://schemas.openxmlformats.org/spreadsheetml/2006/main\"/>",
    ),
    "minimal.xltx": (
        "xl/workbook.xml",
        "application/vnd.openxmlformats-officedocument.spreadsheetml.template.main+xml",
        "<workbook xmlns=\"http://schemas.openxmlformats.org/spreadsheetml/2006/main\"/>",
    ),
    "minimal.xltm": (
        "xl/workbook.xml",
        "application/vnd.ms-excel.template.macroEnabled.main+xml",
        "<workbook xmlns=\"http://schemas.openxmlformats.org/spreadsheetml/2006/main\"/>",
    ),
    "minimal.pptx": (
        "ppt/presentation.xml",
        "application/vnd.openxmlformats-officedocument.presentationml.presentation.main+xml",
        "<p:presentation xmlns:p=\"http://schemas.openxmlformats.org/presentationml/2006/main\"/>",
    ),
    "minimal.pptm": (
        "ppt/presentation.xml",
        "application/vnd.ms-powerpoint.presentation.macroEnabled.main+xml",
        "<p:presentation xmlns:p=\"http://schemas.openxmlformats.org/presentationml/2006/main\"/>",
    ),
    "minimal.potx": (
        "ppt/presentation.xml",
        "application/vnd.openxmlformats-officedocument.presentationml.template.main+xml",
        "<p:presentation xmlns:p=\"http://schemas.openxmlformats.org/presentationml/2006/main\"/>",
    ),
    "minimal.potm": (
        "ppt/presentation.xml",
        "application/vnd.ms-powerpoint.template.macroEnabled.main+xml",
        "<p:presentation xmlns:p=\"http://schemas.openxmlformats.org/presentationml/2006/main\"/>",
    ),
}


def crc32(data: bytes) -> int:
    return zlib.crc32(data) & 0xFFFFFFFF


def store_zip(parts: list[tuple[str, bytes]]) -> bytes:
    out = bytearray()
    central = bytearray()
    for name, data in parts:
        name_b = name.encode("utf-8")
        offset = len(out)
        c = crc32(data)
        size = len(data)
        out += struct.pack("<IHHHHHIIIHH", 0x04034B50, 20, 0, 0, 0, 0, c, size, size, len(name_b), 0)
        out += name_b
        out += data
        central += struct.pack("<IHHHH", 0x02014B50, 20, 20, 0, 0)
        central += struct.pack("<HH", 0, 0)  # time, date
        central += struct.pack("<III", c, size, size)
        central += struct.pack("<HHHHH", len(name_b), 0, 0, 0, 0)
        central += struct.pack("<II", 0, offset)  # external attr, local header offset
        central += name_b
    cd_offset = len(out)
    out += central
    out += struct.pack("<IHHHHIIH", 0x06054B50, 0, 0, len(parts), len(parts), len(central), cd_offset, 0)
    return bytes(out)


def build_package(main_part: str, content_type: str, main_xml: str) -> bytes:
    types = (
        f'<?xml version="1.0" encoding="UTF-8" standalone="yes"?>'
        f'<Types xmlns="{CT_NS}">'
        f'<Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>'
        f'<Default Extension="xml" ContentType="application/xml"/>'
        f'<Override PartName="/{main_part}" ContentType="{content_type}"/>'
        f"</Types>"
    )
    root_rels = (
        f'<?xml version="1.0" encoding="UTF-8" standalone="yes"?>'
        f'<Relationships xmlns="{REL_NS}">'
        f'<Relationship Id="rId1" Type="{OFFICE_DOC}" Target="{main_part}"/>'
        f"</Relationships>"
    )
    parts = [
        ("[Content_Types].xml", types.encode()),
        ("_rels/.rels", root_rels.encode()),
        (main_part, main_xml.encode()),
    ]
    if "macroEnabled" in content_type or "macroEnabledTemplate" in content_type:
        # Opaque VBA part: preserved, never executed.
        vba_dir = main_part.split("/", 1)[0]
        vba_part = f"{vba_dir}/vbaProject.bin"
        parts.append((vba_part, b"VBAPROJECT\0"))
        parts[0] = (
            "[Content_Types].xml",
            (
                types[:-8]
                + '<Default Extension="bin" ContentType="application/vnd.ms-office.vbaProject"/>'
                + "</Types>"
            ).encode(),
        )
    return store_zip(parts)


def write_negatives() -> None:
    NEGATIVE.mkdir(parents=True, exist_ok=True)
    # CFB magic → ole / xls
    cfb = bytearray(64)
    cfb[:8] = bytes([0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1])
    cfb += "Workbook".encode("utf-16le")
    (NEGATIVE / "legacy.xls").write_bytes(cfb)
    # Encrypted package markers
    enc = bytearray(64)
    enc[:8] = bytes([0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1])
    enc += "EncryptionInfo".encode("utf-16le")
    enc += "EncryptedPackage".encode("utf-16le")
    (NEGATIVE / "encrypted.bin").write_bytes(enc)
    (NEGATIVE / "plain.csv").write_text("a,b,c\n1,2,3\n", encoding="utf-8")
    # Zip with xlsb-like main part content type
    xlsb = store_zip(
        [
            (
                "[Content_Types].xml",
                (
                    f'<?xml version="1.0"?><Types xmlns="{CT_NS}">'
                    f'<Override PartName="/xl/workbook.bin" '
                    f'ContentType="application/vnd.ms-excel.sheet.binary.macroEnabled.main"/>'
                    f"</Types>"
                ).encode(),
            ),
            (
                "_rels/.rels",
                (
                    f'<?xml version="1.0"?><Relationships xmlns="{REL_NS}">'
                    f'<Relationship Id="rId1" Type="{OFFICE_DOC}" Target="xl/workbook.bin"/>'
                    f"</Relationships>"
                ).encode(),
            ),
            ("xl/workbook.bin", b"\x00\x01"),
        ]
    )
    (NEGATIVE / "workbook.xlsb.zip").write_bytes(xlsb)


def main() -> int:
    ALLOWED.mkdir(parents=True, exist_ok=True)
    for name, (main, ct, xml) in VARIANTS.items():
        (ALLOWED / name).write_bytes(build_package(main, ct, xml))
    write_negatives()
    print(f"wrote {len(VARIANTS)} allowed + negatives under fixtures/opc/")
    return 0


if __name__ == "__main__":
    sys.exit(main())
