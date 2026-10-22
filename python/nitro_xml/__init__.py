# SPDX-License-Identifier: MIT
"""Nitro XML: open, read, and save .xlsx / .docx / .pptx from Python."""

from nitro_xml._native import Document, open_bytes, open_path

__all__ = ["Document", "open_bytes", "open_path"]
