#!/usr/bin/env python3
# SPDX-License-Identifier: MIT
"""Typed text must be VISIBLE in the rendered page after editing a multi-page docx (#593).

Catches: refresh_view calling open_first_view on edited bytes and then flush_pending_body
reloading from the original (unedited) handle.bytes, which wiped the edit from the view.

Assemble _site as described in the host-api brief, then run with pw-venv Python.
An optional positional port defaults to 7893. No synthetic input or model edits.
"""
import functools
import http.server
from pathlib import Path
import os
import sys
import threading
from playwright.sync_api import sync_playwright

SITE = Path(__file__).resolve().parents[3] / "_site"


def _chrome_path():
    env = os.environ.get("HOST_API_CHROME")
    if env:
        return env
    default = "/home/box/.cache/ms-playwright/chromium_headless_shell-1254/chrome-headless-shell-linux64/chrome-headless-shell"
    return default if os.path.exists(default) else None


CHROME = _chrome_path()


class Handler(http.server.SimpleHTTPRequestHandler):
    def log_message(self, *_):
        pass


def open_page(browser, port):
    page = browser.new_page(viewport={"width": 1400, "height": 900})
    page.goto(f"http://127.0.0.1:{port}/demo/host-api/docx-repaint.html")
    page.wait_for_function("document.querySelector('#log').textContent.includes('loaded')")
    page.wait_for_function("typeof document.querySelector('#viewer').document.whenEditReady === 'function'")
    page.evaluate("document.querySelector('#viewer').document.whenEditReady()")
    page.locator('#viewer .nxp').wait_for(state='attached')
    page.wait_for_timeout(600)
    box = page.locator('#viewer .v').bounding_box()
    assert box and box['width'] > 100 and box['height'] > 100, f"editor has no usable surface: {box}"
    return page


def check_repaint(page):
    # Catches: typed text visible in rendered DOM after editing a multi-page docx;
    # the bug caused flush_pending_body to overwrite the view with pre-edit bytes.

    # Click on the first paragraph span on page 1.
    target = page.locator('#viewer [data-bid="p0"] span').first
    target.click()
    page.wait_for_timeout(300)

    # Verify focus reached the editor input.
    focused = page.evaluate("""() => {
        const el = document.querySelector('#viewer');
        return document.activeElement === el && el.shadowRoot.activeElement?.matches('.nxp');
    }""")
    assert focused, "real click must focus editor input"

    # Type a distinctive sentinel character.
    SENTINEL = "Z"
    page.keyboard.type(SENTINEL)

    # Wait for dirty (proves the keystroke reached the model).
    page.wait_for_function(
        "document.querySelector('#viewer').dirty === true",
        timeout=4000,
    )

    # Give the repaint pipeline time to settle (measure→layout→paint rounds).
    page.wait_for_timeout(1500)

    # The sentinel MUST appear in a rendered span inside the shadow DOM.
    # Checking textContent rather than innerText catches invisible-but-present glyphs.
    visible = page.evaluate(f"""() => {{
        const root = document.querySelector('#viewer').shadowRoot;
        const spans = root ? Array.from(root.querySelectorAll('span')) : [];
        return spans.some(s => s.textContent.includes('{SENTINEL}'));
    }}""")
    assert visible, (
        f"typed character '{SENTINEL}' not visible in rendered spans after edit "
        "— repaint did not propagate the edit (issue #593)"
    )
    print(
        f"  PASS docx repaint: sentinel '{SENTINEL}' visible in rendered span after typing",
        flush=True,
    )


def main():
    failures = []
    handler = functools.partial(Handler, directory=str(SITE))
    port = int(sys.argv[1]) if len(sys.argv) > 1 else 7894
    with http.server.ThreadingHTTPServer(('127.0.0.1', port), handler) as server:
        thread = threading.Thread(target=server.serve_forever, daemon=True)
        thread.start()
        try:
            with sync_playwright() as p:
                browser = p.chromium.launch(
                    **({} if not CHROME else {"executable_path": CHROME}),
                    args=['--no-sandbox', '--disable-dev-shm-usage'],
                )
                try:
                    page = None
                    try:
                        page = open_page(browser, server.server_port)
                        check_repaint(page)
                    except Exception as error:
                        failure = f"docx repaint: {error}"
                        failures.append(failure)
                        print(f"  FAIL {failure}", flush=True)
                    finally:
                        if page:
                            page.close()
                finally:
                    browser.close()
        finally:
            server.shutdown()
            thread.join()
    print(f"\nfailures: {len(failures)}", flush=True)
    return 1 if failures else 0


if __name__ == '__main__':
    sys.exit(main())
