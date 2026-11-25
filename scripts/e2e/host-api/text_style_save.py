#!/usr/bin/env python3
# SPDX-License-Identifier: MIT
"""Fails closed on missing input, save loss, style loss or navigation failure."""
from pathlib import Path
from http.server import SimpleHTTPRequestHandler, ThreadingHTTPServer
from threading import Thread
import io
import os
import signal
import zipfile
import xml.etree.ElementTree as ET
from playwright.sync_api import sync_playwright

ROOT = Path(__file__).resolve().parents[3]
class Handler(SimpleHTTPRequestHandler):
    def __init__(self, *a, **kw):
        super().__init__(*a, directory=str(ROOT / '_site'), **kw)
    def log_message(self, *a):
        pass
server = ThreadingHTTPServer(('127.0.0.1', 0), Handler)
Thread(target=server.serve_forever, daemon=True).start()
CHROME = os.environ.get('HOST_API_CHROME') or '/home/box/.cache/ms-playwright/chromium_headless_shell-1254/chrome-headless-shell-linux64/chrome-headless-shell'
def timed_out(*_):
    raise TimeoutError('Host API browser checks exceeded 180 seconds')
signal.signal(signal.SIGALRM, timed_out)
signal.alarm(180)
try:
    with sync_playwright() as pw:
        browser = pw.chromium.launch(executable_path=CHROME if Path(CHROME).is_file() else None, args=['--no-sandbox'])
        for fmt in ('xlsx', 'docx', 'pptx'):
            print(f'CHECK {fmt}', flush=True)
            page = browser.new_page(viewport={'width':1280,'height':900})
            errors = []
            page.on('pageerror', lambda e: errors.append(str(e)))
            page.goto(f'http://127.0.0.1:{server.server_port}/demo/host-api/text-style-save.html?f={fmt}')
            page.wait_for_function("document.querySelector('#status').textContent === 'Ready'", timeout=60000)
            page.wait_for_function('viewer._nxpTa && viewer._dT')
            if fmt == 'docx':
                page.locator('#chat').focus()
                page.evaluate('changes.length = 0')
                position = page.evaluate("viewer.scrollToText('Unique destination 3')")
                assert position['page'] == 2, position
                assert page.evaluate("document.activeElement.id === 'chat'"), 'find stole chat focus'
                target = page.locator('#viewer .dve-print-line').filter(has_text='Unique destination 3')
                assert target.evaluate('n => {const r=n.getBoundingClientRect(), v=n.getRootNode().querySelector(".dom").getBoundingClientRect(); return r.top>=v.top && r.bottom<=v.bottom;}'), 'match outside scroll viewport'
                second = page.evaluate("viewer.scrollToText('Repeated host phrase', {occurrence:1, select:true})")
                assert second['page'] == 1, second
                assert page.evaluate('(viewer.shadowRoot.getSelection?.() || window.getSelection()).toString()') == 'Repeated host phrase'
                assert not page.evaluate("viewer.document.shellAction('ui').then(r=>r.ui.find.open)"), 'Find UI opened'
                assert page.evaluate("viewer.scrollToText('no such host text')") is None
                assert page.evaluate("viewer.scrollToText('UNIQUE DESTINATION 3', {matchCase:true})") is None
                assert page.evaluate("viewer.scrollToText('UNIQUE DESTINATION 3')")['page'] == 2
                assert page.evaluate("viewer.scrollToText('Repeated host phrase', {occurrence:3})") is None
                assert page.evaluate("document.activeElement.id === 'chat'"), 'selection stole chat focus'
                assert not page.evaluate('changes'), 'find emitted nitro-change'
                print('PASS docx: page three, second occurrence, missing text, focus, no change', flush=True)
            if fmt == 'xlsx':
                page.evaluate("viewer.shellAction('select','A1')")
            elif fmt == 'pptx':
                page.locator('#viewer .pptxv-para').filter(has_text='Inherited body').click()
            else:
                page.locator('#viewer .dve-print-line').first.click()
            page.evaluate('viewer._nxpTa.focus()')
            assert page.evaluate('viewer.shadowRoot.activeElement === viewer._nxpTa'), f'{fmt}: no input focus'
            if fmt == 'pptx':
                style_of = "text => { const sp = [...viewer.shadowRoot.querySelectorAll('.pptxv-para span')].find(n => n.textContent.includes(text)); if (!sp) return null; const s = getComputedStyle(sp); return [s.fontFamily,s.fontSize,s.color,s.fontWeight,s.fontStyle]; }"
                body_style = page.evaluate(style_of, "Inherited")
                assert body_style is not None, 'Inherited body span not found'
                assert 'Arial' in body_style[0], body_style
                assert body_style[1:] == ['32px', 'rgb(21, 74, 130)', '700', 'italic'], body_style
            page.keyboard.type('HostPending')
            if fmt == 'pptx':
                page.wait_for_function("expected => [...viewer.shadowRoot.querySelectorAll('.pptxv-para span')].some(n=>{const s=getComputedStyle(n); return n.textContent.includes('HostPending') && JSON.stringify([s.fontFamily,s.fontSize,s.color,s.fontWeight,s.fontStyle]) === JSON.stringify(expected)})", arg=body_style)
                live_style = page.evaluate(style_of, "HostPending")
                assert live_style == body_style, ('live typing style changed', live_style, body_style)
            page.evaluate('saveReopen()')
            data = bytes(page.evaluate('saved'))
            with zipfile.ZipFile(io.BytesIO(data)) as package:
                xml = ''.join(''.join(ET.fromstring(package.read(n)).itertext()) for n in package.namelist() if n.endswith('.xml'))
            assert 'HostPending' in xml, f'{fmt}: pending input missing from saved XML'
            assert not page.evaluate('savedDirty || viewer.dirty'), f'{fmt}: dirty after save'
            print(f'PASS {fmt}: pending typing saved, reopened, dirty false', flush=True)
            if fmt == 'pptx':
                typed_style = page.evaluate(style_of, "HostPending")
                assert typed_style == body_style, 'existing run style changed'
                page.locator('#viewer [data-element-id="ppt/slides/slide1.xml-shape-0"]').click()
                assert page.evaluate('viewer.shadowRoot.activeElement === viewer._nxpTa'), 'empty title lacks focus'
                page.keyboard.type('TitlePending')
                page.keyboard.press('Enter')
                page.keyboard.type('NextParagraph')
                page.evaluate('saveReopen()')
                with zipfile.ZipFile(io.BytesIO(bytes(page.evaluate('saved')))) as package:
                    slide = ET.fromstring(package.read('ppt/slides/slide1.xml'))
                ns = {'p':'http://schemas.openxmlformats.org/presentationml/2006/main', 'a':'http://schemas.openxmlformats.org/drawingml/2006/main'}
                shapes = slide.findall('.//p:sp', ns)
                title_runs = shapes[0].findall('.//a:rPr', ns)
                assert title_runs, 'empty title was not saved'
                assert all(r.find('a:latin', ns) is None and r.find('a:solidFill', ns) is None for r in title_runs), 'saved XML hardcodes inherited font/color'
                body_text = ''.join(n.text or '' for n in shapes[1].findall('.//a:t', ns))
                assert body_text == 'Inherited bodyHostPending', body_text
                title_style = page.evaluate(style_of, "TitlePending")
                assert title_style is not None, 'TitlePending not found'
                assert 'Arial' in title_style[0], title_style
                assert title_style[1:] == ['42.6667px', 'rgb(21, 74, 130)', '700', 'italic'], title_style
                page.wait_for_function("[...viewer.shadowRoot.querySelectorAll('.pptxv-para span')].some(n=>n.textContent.includes('NextParagraph') && getComputedStyle(n).fontSize === '42.6667px')")
                next_style = page.evaluate(style_of, "NextParagraph")
                assert next_style == title_style, (title_style, next_style)
                print('PASS pptx: empty title, existing run, new paragraph font/size/color/bold/italic', flush=True)
            # Catches: an active native composition being discarded or doubled by save().
            if fmt == 'xlsx':
                page.evaluate("viewer.shellAction('select','B2')")
            elif fmt == 'docx':
                page.locator('#viewer .dve-print-line').first.click()
            else:
                page.locator('#viewer .pptxv-para span').first.click()
            page.evaluate('viewer._nxpTa.focus()')
            assert page.evaluate('viewer.shadowRoot.activeElement === viewer._nxpTa'), page.evaluate('({message:viewer.shadowRoot.querySelector(".msg").textContent, connected:viewer._nxpTa.isConnected, active:viewer.shadowRoot.activeElement?.outerHTML, input:viewer._nxpTa.outerHTML, rect:viewer._nxpTa.getBoundingClientRect().toJSON()})')
            cdp = page.context.new_cdp_session(page)
            cdp.send('Input.imeSetComposition', {'text':'漢字', 'selectionStart':2, 'selectionEnd':2})
            page.evaluate('saveReopen()')
            with zipfile.ZipFile(io.BytesIO(bytes(page.evaluate('saved')))) as package:
                text = ''.join(''.join(ET.fromstring(package.read(n)).itertext()) for n in package.namelist() if n.endswith('.xml'))
            assert text.count('漢字') == 1, f'{fmt}: active composition lost or duplicated'
            assert not page.evaluate('savedDirty || viewer.dirty'), f'{fmt}: IME save remains dirty'
            assert not errors, errors
            print(f'PASS {fmt}: CDP composition saved and reopened, dirty false', flush=True)
            page.close()
        browser.close()
finally:
    signal.alarm(0)
    server.shutdown()
