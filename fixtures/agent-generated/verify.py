# SPDX-License-Identifier: MIT
"""Reopen saved packages with source Python libraries; assert the edited text."""
import sys
from pathlib import Path
from docx import Document
from openpyxl import load_workbook
from pptx import Presentation
path = Path(sys.argv[1])
marker = sys.argv[2]
if path.suffix == '.docx':
    text = '\n'.join(p.text for p in Document(path).paragraphs)
elif path.suffix == '.xlsx':
    book = load_workbook(path)
    text = '\n'.join(str(c.value) for sheet in book for row in sheet for c in row)
else:
    deck = Presentation(path)
    text = '\n'.join(s.text for slide in deck.slides for s in slide.shapes if s.has_text_frame)
assert marker in text, f'Edited text {marker!r} missing from {path.name}: {text[:300]}'
print(f'PASS source-library reopen {path.name}')
