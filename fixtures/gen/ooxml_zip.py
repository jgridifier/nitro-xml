#!/usr/bin/env python3
# SPDX-License-Identifier: MIT
"""Deterministic OOXML zip writer (fixed timestamps, sorted members, stable CRC)."""

from __future__ import annotations

import hashlib
import io
import struct
import time
import zipfile
from pathlib import Path
from typing import Mapping
from zlib import crc32

# DOS epoch used by every generated package so two runs are byte-identical.
_ZIP_DATE = (1980, 1, 1, 0, 0, 0)


def sha256_bytes(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def sha256_file(path: Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as f:
        for chunk in iter(lambda: f.read(1 << 20), b""):
            h.update(chunk)
    return h.hexdigest()


def tiny_png(width: int = 8, height: int = 8, rgb: tuple[int, int, int] = (0, 128, 255)) -> bytes:
    """Minimal RGB PNG used as embedded media padding."""

    def chunk(tag: bytes, data: bytes) -> bytes:
        return struct.pack(">I", len(data)) + tag + data + struct.pack(">I", crc32(tag + data) & 0xFFFFFFFF)

    raw = b"".join(b"\x00" + bytes(rgb) * width for _ in range(height))
    return (
        b"\x89PNG\r\n\x1a\n"
        + chunk(b"IHDR", struct.pack(">IIBBBBB", width, height, 8, 2, 0, 0, 0))
        + chunk(b"IDAT", __import__("zlib").compress(raw, 9))
        + chunk(b"IEND", b"")
    )


def write_ooxml(path: Path, members: Mapping[str, bytes]) -> bytes:
    """Write an OOXML package and return its bytes. Members are stored sorted."""
    path.parent.mkdir(parents=True, exist_ok=True)
    buf = io.BytesIO()
    with zipfile.ZipFile(buf, "w", compression=zipfile.ZIP_DEFLATED, compresslevel=9) as zf:
        for name in sorted(members):
            info = zipfile.ZipInfo(filename=name, date_time=_ZIP_DATE)
            info.compress_type = zipfile.ZIP_DEFLATED
            info.create_system = 3  # Unix
            info.external_attr = 0o644 << 16
            zf.writestr(info, members[name])
    data = buf.getvalue()
    path.write_bytes(data)
    # Touch mtime to a fixed value so directory listings stay stable in tests.
    fixed = time.mktime(_ZIP_DATE + (0, 0, -1))
    Path(path).touch()
    import os

    os.utime(path, (fixed, fixed))
    return data


def content_types(overrides: list[tuple[str, str]], defaults: list[tuple[str, str]] | None = None) -> bytes:
    defaults = defaults or [
        ("rels", "application/vnd.openxmlformats-package.relationships+xml"),
        ("xml", "application/xml"),
    ]
    parts = ['<?xml version="1.0" encoding="UTF-8" standalone="yes"?>']
    parts.append('<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">')
    for ext, ctype in defaults:
        parts.append(f'<Default Extension="{ext}" ContentType="{ctype}"/>')
    for part, ctype in overrides:
        parts.append(f'<Override PartName="{part}" ContentType="{ctype}"/>')
    parts.append("</Types>")
    return "".join(parts).encode("utf-8")


def rels_root(target: str, rel_type: str) -> bytes:
    return (
        '<?xml version="1.0" encoding="UTF-8" standalone="yes"?>'
        '<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">'
        f'<Relationship Id="rId1" Type="{rel_type}" Target="{target}"/>'
        "</Relationships>"
    ).encode("utf-8")


def core_props(title: str, creator: str = "nitro-xml fixture generator") -> bytes:
    return (
        '<?xml version="1.0" encoding="UTF-8" standalone="yes"?>'
        '<cp:coreProperties xmlns:cp="http://schemas.openxmlformats.org/package/2006/metadata/core-properties" '
        'xmlns:dc="http://purl.org/dc/elements/1.1/" xmlns:dcterms="http://purl.org/dc/terms/" '
        'xmlns:dcmitype="http://purl.org/dc/dcmitype/" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance">'
        f"<dc:title>{title}</dc:title><dc:creator>{creator}</dc:creator>"
        "<dcterms:created xsi:type=\"dcterms:W3CDTF\">1980-01-01T00:00:00Z</dcterms:created>"
        "<dcterms:modified xsi:type=\"dcterms:W3CDTF\">1980-01-01T00:00:00Z</dcterms:modified>"
        "</cp:coreProperties>"
    ).encode("utf-8")


def app_props(app: str, extra: str = "") -> bytes:
    return (
        '<?xml version="1.0" encoding="UTF-8" standalone="yes"?>'
        '<Properties xmlns="http://schemas.openxmlformats.org/officeDocument/2006/extended-properties" '
        'xmlns:vt="http://schemas.openxmlformats.org/officeDocument/2006/docPropsVTypes">'
        f"<Application>{app}</Application>{extra}</Properties>"
    ).encode("utf-8")
