# SPDX-License-Identifier: MIT
# Catches: host navigation/reload/mode changes remounting editors or taking chat focus.
"""Run after assembling _site. Test variants derive only from MIT demo/samples files."""
import functools
import io
import http.server
import os
import pathlib
import re
import threading
import zipfile
from playwright.sync_api import sync_playwright

ROOT = pathlib.Path(__file__).resolve().parents[3]
OUT = ROOT / '_site/demo/host-api'
OUT.mkdir(parents=True, exist_ok=True)


def variants(fmt):
    source = {'docx': 'notes.docx', 'xlsx': 'sample.xlsx', 'pptx': 'deck.pptx'}[fmt]
    with zipfile.ZipFile(ROOT / 'demo/samples' / source) as z:
        parts = {n: z.read(n) for n in z.namelist()}
    if fmt == 'docx':
        xml = parts['word/document.xml'].decode()
        extra = ''.join('<w:p><w:r><w:br w:type="page"/></w:r></w:p><w:p><w:r><w:t>HOST ORIGINAL page %d</w:t></w:r></w:p>' % n for n in [2, 3])
        parts['word/document.xml'] = xml.replace('<w:sectPr', extra + '<w:sectPr', 1).encode()
    elif fmt == 'xlsx':
        parts['xl/workbook.xml'] = parts['xl/workbook.xml'].replace(b'</sheets>', b'<sheet name="Budget" sheetId="2" r:id="hostSheet"/></sheets>')
        parts['xl/_rels/workbook.xml.rels'] = parts['xl/_rels/workbook.xml.rels'].replace(b'</Relationships>', b'<Relationship Id="hostSheet" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet2.xml"/></Relationships>')
        parts['[Content_Types].xml'] = parts['[Content_Types].xml'].replace(b'</Types>', b'<Override PartName="/xl/worksheets/sheet2.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/></Types>')
        parts['xl/worksheets/sheet2.xml'] = parts['xl/worksheets/sheet1.xml']
        parts['xl/sharedStrings.xml'] = parts['xl/sharedStrings.xml'].replace(b'row-', b'HOST ORIGINAL row-')
    else:
        for n in parts:
            if re.match(r'ppt/slides/slide\d+\.xml$', n):
                parts[n] = parts[n].replace(b'<a:t>', b'<a:t>HOST ORIGINAL ', 1)
    for version in ['original', 'changed']:
        with zipfile.ZipFile(OUT / f'{version}.{fmt}', 'w', zipfile.ZIP_DEFLATED) as z:
            for n, data in parts.items():
                z.writestr(n, data.replace(b'HOST ORIGINAL', b'HOST RELOADED') if version == 'changed' else data)


def chrome_path():
    # Same rule as the other host-api drivers: HOST_API_CHROME, else the box-local headless
    # shell when present, else Playwright's bundled Chromium (CI / ubuntu-latest).
    env = os.environ.get('HOST_API_CHROME')
    if env:
        return env
    default = '/home/box/.cache/ms-playwright/chromium_headless_shell-1254/chrome-headless-shell-linux64/chrome-headless-shell'
    return default if os.path.exists(default) else None


class Handler(http.server.SimpleHTTPRequestHandler):
    def log_message(self, *_):
        pass


for fmt in ['docx', 'xlsx', 'pptx']:
    variants(fmt)
server = http.server.ThreadingHTTPServer(('127.0.0.1', 0), functools.partial(Handler, directory=str(ROOT / '_site')))
threading.Thread(target=server.serve_forever, daemon=True).start()
try:
    with sync_playwright() as p:
        chrome = chrome_path()
        browser = p.chromium.launch(**({'executable_path': chrome} if chrome else {}), args=['--no-sandbox', '--disable-dev-shm-usage'])
        for fmt in ['docx', 'xlsx', 'pptx']:
            page = browser.new_page()
            page.set_default_timeout(20000)
            errors = []
            page.on('pageerror', lambda e: errors.append(str(e)))
            page.goto(f'http://127.0.0.1:{server.server_port}/demo/host-api/position-reload-readonly.html?f={fmt}')
            print(fmt, 'waiting for initial open', flush=True)
            page.evaluate('Promise.race([ready,new Promise((_,reject)=>setTimeout(()=>reject(new Error("initial open timeout")),30000))])')
            print(fmt, 'initial open ready', flush=True)
            page.wait_for_function('el.document?._position')
            print(fmt, 'initial paint ready', flush=True)
            assert page.evaluate('document.activeElement === chat'), (fmt, 'open stole focus')
            key = {'docx': 'page', 'xlsx': 'sheet', 'pptx': 'slide'}[fmt]
            target = 2 if fmt == 'pptx' else 1
            position = {key: target, 'scrollTop': 40 if fmt != 'pptx' else 0, 'scrollLeft': 0, 'zoom': 100}
            page.evaluate('''async ({fmt,position}) => {
              window.before=el; window.shadow=el.shadowRoot; window.changes=0;
              el.addEventListener('nitro-change',()=>changes++);
              window.original=new Uint8Array(await (await fetch('original.'+fmt)).arrayBuffer());
              window.changed=new Uint8Array(await (await fetch('changed.'+fmt)).arrayBuffer());
              await el.loadBuffer(original.slice(), 'original.'+fmt, {position});
            }''', {'fmt':fmt, 'position':position})
            page.wait_for_function('(key)=>el.getPosition()[key]!==undefined', arg=key)
            actual = page.evaluate('el.getPosition()')
            print(fmt, 'open position', actual, flush=True)
            page.wait_for_timeout(1000)
            assert actual[key] == target, (fmt, 'open-at-position', actual)
            page.evaluate('async ({fmt,position})=>{await el.loadFile(new File([original], "original."+fmt), undefined, {position});}', {'fmt':fmt,'position':position})
            assert page.evaluate('key=>el.getPosition()[key]', key) == target
            page.evaluate('async ({fmt,position})=>{await el.loadUrl("original."+fmt, {position});}', {'fmt':fmt,'position':position})
            assert page.evaluate('key=>el.getPosition()[key]', key) == target
            page.evaluate('async pos=>{await el.setPosition(pos)}', position)
            actual = page.evaluate('el.getPosition()')
            assert actual[key] == target and actual['scrollTop'] == position['scrollTop'], (fmt, actual)
            if fmt == 'xlsx':
                page.evaluate('el.setPosition({sheet:"Budget"})')
                assert page.evaluate('el.getPosition().sheetName') == 'Budget'
            page.evaluate('async()=>{window.savedPosition=el.getPosition(); window.worker=el.document._worker; await el.reload(changed.slice())}')
            assert page.evaluate('el === before && el.shadowRoot === shadow && el.document._worker === worker')
            assert page.evaluate('JSON.stringify(el.getPosition()) === JSON.stringify(savedPosition)'), (fmt, 'reload position')
            page.wait_for_function('el.shadowRoot.querySelector(".dom").textContent.includes("HOST RELOADED")')
            assert page.evaluate('document.activeElement === chat && changes === 0'), (fmt, 'reload focus/events')
            page.evaluate('''async()=>{await new Promise(resolve=>{el.addEventListener('nitro-mode',resolve,{once:true});el.readonly=false})}''')
            assert page.evaluate('!el.readonly && el===before && el.shadowRoot===shadow && document.activeElement===chat')
            assert page.evaluate('JSON.stringify(el.getPosition()) === JSON.stringify(savedPosition)'), (fmt, 'mode position')
            # Editing intentionally focuses the document; APIs below must leave focus in chat.
            page.evaluate('window.textBefore=el.shadowRoot.querySelector(".dom").textContent')
            if fmt == 'docx':
                page.locator('nitro-docx .dve-print-line span').first.click()
            elif fmt == 'xlsx':
                page.evaluate('el.shellAction("select", "A1")')
                page.locator('nitro-xlsx .v').click(position={'x':90,'y':45})
            else:
                page.locator('nitro-pptx [data-element-id]').first.click()
            page.keyboard.type('HOSTEDIT')
            if fmt == 'xlsx':
                page.keyboard.press('Enter')
            page.wait_for_function('el.shadowRoot.querySelector(".dom").textContent !== textBefore')
            print(fmt, 'typing changed content', flush=True)
            page.wait_for_function('el.dirty === true')
            page.locator('#chat').focus()
            page.evaluate('''async()=>{window.changeCount=changes;el._dT.touch();await el.reload(changed.slice())}''')
            page.wait_for_timeout(500)
            assert page.evaluate('el.dirty === false && changes === changeCount && !el.readonly && document.activeElement===chat')
            page.evaluate('''async()=>{await new Promise(resolve=>{el.addEventListener('nitro-mode',resolve,{once:true});el.setAttribute('readonly','true')})}''')
            assert page.evaluate('el.readonly && el===before && el.shadowRoot===shadow && document.activeElement===chat')
            assert page.evaluate('el.shadowRoot.querySelector(".nxp") === null')
            page.evaluate('window.readonlyText=el.shadowRoot.querySelector(".dom").textContent')
            page.locator(f'nitro-{fmt} .v').focus()
            page.keyboard.type('READONLY')
            page.locator('#chat').focus()
            page.wait_for_timeout(150)
            assert page.evaluate('el.shadowRoot.querySelector(".dom").textContent === readonlyText && document.activeElement===chat')
            page.evaluate('async key=>{await el.setPosition({[key]:999999, scrollTop:1e9});await el.setPosition({[key]:-99,scrollTop:-10})}', key)
            assert page.evaluate('key=>el.getPosition()[key]', key) == 0
            assert page.evaluate('el.getPosition().scrollTop') == 0
            assert page.evaluate('document.activeElement === chat')
            # Catches: a live readonly round trip that reparses or remounts and drops unsaved edits (#565).
            # Proof is the saved package, so it does not depend on how each format repaints.
            page.evaluate('''async()=>{await new Promise(resolve=>{el.addEventListener('nitro-mode',resolve,{once:true});el.readonly=false})}''')
            if fmt == 'docx':
                page.locator('nitro-docx .dve-print-line span').first.click()
            elif fmt == 'xlsx':
                page.evaluate('el.shellAction("select", "A1")')
                page.locator('nitro-xlsx .v').click(position={'x':90,'y':45})
            else:
                page.locator('nitro-pptx [data-element-id]').first.click()
            page.keyboard.type('KEEPME')
            if fmt == 'xlsx':
                page.keyboard.press('Enter')
            page.wait_for_function('el.dirty === true')
            page.evaluate('window.keepChanges=changes')
            page.evaluate('''async()=>{await new Promise(resolve=>{el.addEventListener('nitro-mode',resolve,{once:true});el.readonly=true})}''')
            assert page.evaluate('el.readonly && el===before && el.shadowRoot===shadow'), (fmt, 'readonly toggle remounted')
            page.evaluate('''async()=>{await new Promise(resolve=>{el.addEventListener('nitro-mode',resolve,{once:true});el.readonly=false})}''')
            assert page.evaluate('!el.readonly && el===before && el.shadowRoot===shadow'), (fmt, 'writable toggle remounted')
            assert page.evaluate('el.dirty === true && changes === keepChanges'), (fmt, 'readonly round trip lost dirty or emitted nitro-change')
            saved = bytes(page.evaluate('async()=>Array.from(await el.save())'))
            with zipfile.ZipFile(io.BytesIO(saved)) as z:
                assert any(b'KEEPME' in z.read(n) for n in z.namelist()), (fmt, 'readonly round trip dropped unsaved edits')
            print(fmt, 'live readonly round trip kept unsaved edits', flush=True)
            assert not errors, (fmt, errors)
            print(f'PASS {fmt}: open position; round-trip; reload content/identity/position; readonly typing blocked; live readonly keeps edits; chat focus; bounds', flush=True)
            page.close()
        browser.close()
finally:
    server.shutdown()
