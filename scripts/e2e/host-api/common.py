# SPDX-License-Identifier: MIT
import functools
import http.server
from pathlib import Path
import shutil
import threading
from contextlib import contextmanager

ROOT = Path(__file__).resolve().parents[3]
SITE = ROOT / '_site'
CHROME = '/home/box/.cache/ms-playwright/chromium_headless_shell-1254/chrome-headless-shell-linux64/chrome-headless-shell'
CSP = "default-src 'self'; script-src 'self' 'wasm-unsafe-eval'; worker-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data: blob:"

def assemble():
    for source, target in [(ROOT / 'demo', SITE / 'demo'), (ROOT / 'packages/nitro-xml/loader', SITE / 'demo/nitro-xml/loader'), (ROOT / 'packages/nitro-xml/pkg', SITE / 'demo/nitro-xml/pkg'), (ROOT / 'fixtures/agent-generated', SITE / 'fixtures/agent-generated')]:
        shutil.copytree(source, target, dirs_exist_ok=True)

class Handler(http.server.SimpleHTTPRequestHandler):
    def end_headers(self):
        self.send_header('Content-Security-Policy', CSP)
        super().end_headers()
    def log_message(self, *_): pass

@contextmanager
def server():
    assemble()
    with http.server.ThreadingHTTPServer(('127.0.0.1', 0), functools.partial(Handler, directory=str(SITE))) as srv:
        thread = threading.Thread(target=srv.serve_forever, daemon=True)
        thread.start()
        try: yield f'http://127.0.0.1:{srv.server_port}'
        finally:
            srv.shutdown()
            thread.join()

def ready(page):
    page.wait_for_function('() => window.ready || window.errors?.length', timeout=45000)
    assert page.evaluate('window.errors') == [], page.evaluate('window.errors')
    page.locator('#viewer .nxp').wait_for(state='attached')
    page.wait_for_timeout(500)
    box = page.locator('#viewer .v').bounding_box()
    assert box and box['width'] > 100 and box['height'] > 100, box
    assert page.locator('#viewer .dom').inner_text().strip(), 'No document content painted'
