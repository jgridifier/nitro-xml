# SPDX-License-Identifier: MIT
"""Original regression samples for host APIs #568, #569, #572."""
from pathlib import Path
from docx import Document
from pptx import Presentation
from pptx.oxml.xmlchemy import OxmlElement
from pptx.util import Inches
from copy import deepcopy
import hashlib
import json

root = Path(__file__).resolve().parents[2]
doc = Document()
for page in range(3):
    if page:
        doc.add_page_break()
    doc.add_paragraph(f'Host page {page + 1}')
    doc.add_paragraph('Repeated host phrase')
    doc.add_paragraph(f'Unique destination {page + 1}')
doc.save(root / 'demo/samples/host-text.docx')
prs = Presentation()
slide = prs.slides.add_slide(prs.slide_layouts[1])
for idx, shape in enumerate(slide.placeholders):
    shape.text = '' if idx == 0 else 'Inherited body'
    shape.left, shape.top = Inches(1), Inches(1 + idx * 2)
    shape.width, shape.height = Inches(8), Inches(1.5)
    p = shape.text_frame.paragraphs[0]._p
    end = OxmlElement('a:endParaRPr')
    end.set('sz', '3200' if idx == 0 else '2400')
    end.set('b', '1')
    end.set('i', '1')
    latin = OxmlElement('a:latin')
    latin.set('typeface', 'Arial')
    if idx == 1:
        end.append(latin)
    fill = OxmlElement('a:solidFill')
    color = OxmlElement('a:srgbClr')
    color.set('val', '154A82')
    fill.append(color)
    if idx == 1:
        end.append(fill)
    p.append(end)
    if idx == 1:
        run_props = deepcopy(end)
        run_props.tag = run_props.tag.replace("endParaRPr", "rPr")
        p.find("{http://schemas.openxmlformats.org/drawingml/2006/main}r").insert(0, run_props)
# Title font comes from the master; its color comes from the layout. The
# slide contributes only size/bold/italic through endParaRPr.
master_rpr = prs.slide_master._element.xpath('./p:txStyles/p:titleStyle/a:lvl1pPr/a:defRPr')[0]
master_latin = master_rpr.find('{http://schemas.openxmlformats.org/drawingml/2006/main}latin')
master_latin.set('typeface', 'Arial')
layout_title = prs.slide_layouts[1].placeholders[0]
layout_list = layout_title.text_frame._txBody.find('{http://schemas.openxmlformats.org/drawingml/2006/main}lstStyle')
layout_ppr = OxmlElement('a:lvl1pPr')
layout_list.append(layout_ppr)
layout_rpr = OxmlElement('a:defRPr')
layout_fill = OxmlElement('a:solidFill')
layout_color = OxmlElement('a:srgbClr')
layout_color.set('val', '154A82')
layout_fill.append(layout_color)
layout_rpr.append(layout_fill)
layout_ppr.append(layout_rpr)
path = root / 'fixtures/generated/pptx/host-inherited-style.pptx'
prs.save(path)
(root / 'demo/samples/host-style.pptx').write_bytes(path.read_bytes())
print(path, hashlib.sha256(path.read_bytes()).hexdigest())

manifest_path = root / 'fixtures/manifest.json'
manifest = json.loads(manifest_path.read_text())
for entry in manifest['fixtures']:
    if entry['id'] in ('host-inherited-style', 'demo-host-style', 'demo-host-text'):
        data = (root / entry['path']).read_bytes()
        entry.update(sha256=hashlib.sha256(data).hexdigest(), bytes=len(data))
manifest_path.write_text(json.dumps(manifest, indent=2) + '\n')
