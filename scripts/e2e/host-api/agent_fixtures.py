# SPDX-License-Identifier: MIT
"""Real click/type/save/reopen for every generated Office fixture. Failures exit 1."""
import os
import subprocess
import sys
sys.dont_write_bytecode = True
import tempfile
from pathlib import Path
from playwright.sync_api import sync_playwright
from common import ROOT, CHROME, server, ready

MARKER = 'HOSTEDIT'

def check(browser, base, source):
    # Catches: generated files painting successfully but losing edits on save/reopen.
    fmt = source.suffix[1:]
    page = browser.new_page()
    try:
        page.goto(f'{base}/demo/host-api/csp.html?f={fmt}&file=/fixtures/agent-generated/{source.name}')
        ready(page)
        if fmt == 'xlsx':
            box = page.locator('#viewer .xg-root').bounding_box()
            assert box, 'No grid'
            page.mouse.click(box['x'] + 65, box['y'] + 30)
        else:
            selector = '[data-bid="p0"] span' if fmt == 'docx' else '.pptxv-para span'
            page.locator('#viewer ' + selector).first.click()
        assert page.evaluate("document.activeElement === viewer && viewer.shadowRoot.activeElement?.matches('.nxp')"), 'Editor did not receive focus'
        page.keyboard.type(MARKER)
        if fmt == 'xlsx': page.keyboard.press('Enter')
        page.wait_for_function('() => viewer.dirty === true')
        saved = page.evaluate('async () => Array.from(await viewer.save())')
        with tempfile.TemporaryDirectory() as tmp:
            out = Path(tmp) / source.name
            out.write_bytes(bytes(saved))
            subprocess.run(['/workspace/host-api-fixtures-venv/bin/python', str(ROOT / 'fixtures/agent-generated/verify.py'), str(out), MARKER], check=True)
            if source.stem == 'exceljs':
                env = dict(os.environ, NODE_PATH='/workspace/host-api-fixtures-npm/node_modules')
                subprocess.run(['node', str(ROOT / 'fixtures/agent-generated/verify.cjs'), str(out), MARKER], env=env, check=True)
        page.evaluate('async bytes => { await viewer.loadBuffer(new Uint8Array(bytes), "saved.' + fmt + '"); await viewer.document.whenEditReady(); }', saved)
        page.wait_for_function("() => viewer.shadowRoot.querySelector('.dom').textContent.includes('HOSTEDIT')")
        assert page.evaluate('window.errors') == [], page.evaluate('window.errors')
        print(f'PASS {source.name}: paint, real edit, save, source-library reopen, browser reopen persisted', flush=True)
    finally: page.close()

def main():
    failures = []
    with server() as base, sync_playwright() as p:
        browser = p.chromium.launch(executable_path=CHROME, args=['--no-sandbox', '--disable-dev-shm-usage'])
        try:
            for source in sorted((ROOT / 'fixtures/agent-generated').iterdir()):
                if source.suffix not in ('.docx', '.xlsx', '.pptx'): continue
                try: check(browser, base, source)
                except Exception as e:
                    failures.append(source.name)
                    print(f'FAIL {source.name}: {e}', flush=True)
        finally: browser.close()
    print(f'failures: {len(failures)}', flush=True)
    return bool(failures)

if __name__ == '__main__': sys.exit(main())
