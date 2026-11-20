# SPDX-License-Identifier: MIT
"""Run with /workspace/host-api-fixtures-venv/bin/python; see requirements.txt."""
from pathlib import Path
from docx import Document
from docx.oxml import OxmlElement
from docx.oxml.ns import qn
from docx.opc.constants import RELATIONSHIP_TYPE as RT
from openpyxl import Workbook
from openpyxl.styles import Font, PatternFill
from pptx import Presentation
from pptx.util import Inches
from PIL import Image, ImageDraw

OUT = Path(__file__).resolve().parent
image = Image.new('RGB', (120, 60), '#245ba8')
ImageDraw.Draw(image).rectangle((10, 10, 110, 50), fill='#f3bd42')
image.save(OUT / 'sample.png')
d = Document()
d.add_heading('Agent fixture', 0)
d.add_paragraph('Editable source text.')
link = OxmlElement('w:hyperlink')
link.set(qn('r:id'), d.part.relate_to('https://example.com/host-api', RT.HYPERLINK, is_external=True))
run = OxmlElement('w:r')
text = OxmlElement('w:t')
text.text = 'Host link'
run.append(text)
link.append(run)
d.add_paragraph()._p.append(link)
d.add_heading('Generated content', 1)
for text in ['First bullet', 'Second bullet']:
    d.add_paragraph(text, style='List Bullet')
t = d.add_table(rows=2, cols=2)
for row, values in zip(t.rows, [('Name', 'Value'), ('Alpha', '42')]):
    for cell, value in zip(row.cells, values): cell.text = value
d.add_picture(str(OUT / 'sample.png'))
d.save(OUT / 'python-docx.docx')
w = Workbook()
s = w.active
s.title = 'Summary'
s.append(['Agent fixture', 'Value'])
s.append(['Alpha', 42])
s.append(['Total', '=SUM(B2:B2)'])
s['A4'] = 'Host link'
s['A4'].hyperlink = 'https://example.com/host-api'
s['A1'].font = Font(bold=True, color='245BA8')
s['B2'].fill = PatternFill('solid', fgColor='F3BD42')
w.create_sheet('Details').append(['Second sheet', 7])
w.save(OUT / 'openpyxl.xlsx')
p = Presentation()
slide = p.slides.add_slide(p.slide_layouts[1])
slide.shapes.title.text = 'Agent fixture'
tf = slide.placeholders[1].text_frame
tf.text = 'First bullet'
tf.add_paragraph().text = 'Second bullet'
run = tf.add_paragraph().add_run()
run.text = 'Host link'
run.hyperlink.address = 'https://example.com/host-api'
slide.shapes.add_picture(str(OUT / 'sample.png'), Inches(7), Inches(1), width=Inches(2))
table = slide.shapes.add_table(2, 2, Inches(1), Inches(5), Inches(5), Inches(1)).table
for r, values in enumerate([['Name', 'Value'], ['Alpha', '42']]):
    for c, value in enumerate(values): table.cell(r,c).text = value
p.slides.add_slide(p.slide_layouts[6])
p.save(OUT / 'python-pptx.pptx')
print('Generated python-docx.docx, openpyxl.xlsx, python-pptx.pptx')
