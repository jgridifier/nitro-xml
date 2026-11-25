#!/usr/bin/env python3
# SPDX-License-Identifier: MIT
"""Typed text must be VISIBLE in the rendered page after editing a multi-page docx (#593).

Catches: refresh_view calling open_first_view on edited bytes and then flush_pending_body
reloading from the original (unedited) handle.bytes, which wiped the edit from the view.

Fixture: paged-overflow.docx — 50+ paragraphs overflow page 1 onto page 2.
(paged.docx uses an explicit w:br page break that the renderer does not yet honour as a
page boundary; see issue #627. The overflow fixture is used here instead.)

Checks (all three must pass):
  1. page_count >= 2 BEFORE any typing (fixture really paginates).
  2. Sentinel typed on page 1 appears in page 1's rendered spans.
  3. Sentinel typed on page 2 appears in page 2's rendered spans (and page 1
     sentinel is still present — proves the repaint did not wipe either page).

Assemble _site as described in the host-api brief, then run with pw-venv Python.
An optional positional port defaults to 7894. No synthetic input or model edits.
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

    # --- 1. Assert page count >= 2 BEFORE typing (fixture must really paginate). ---
    page.wait_for_function(
        """() => {
            const root = document.querySelector('#viewer').shadowRoot;
            const c = root && root.querySelector('[data-page-count]');
            return c && parseInt(c.getAttribute('data-page-count'), 10) >= 2;
        }""",
        timeout=10000,
    )
    page_count = page.evaluate("""() => {
        const root = document.querySelector('#viewer').shadowRoot;
        const c = root.querySelector('[data-page-count]');
        return c ? parseInt(c.getAttribute('data-page-count'), 10) : 0;
    }""")
    assert page_count >= 2, (
        f"fixture must render >= 2 pages before any typing, got {page_count}. "
        "If using the explicit-page-break fixture (paged.docx), that break does not "
        "currently render; use paged-overflow.docx instead (see issue #627)."
    )
    print(f"  PASS page count before typing: {page_count} pages", flush=True)

    # --- 2. Type on page 1, assert sentinel visible in page 1 spans. ---
    SENT_P1 = "ZPAGE1"
    # Click the first editable block in page 1.
    p1_target = page.locator('#viewer .dve-print-page[data-page-index="0"] [data-bid]').first
    p1_target.click()
    page.wait_for_timeout(300)

    # Verify focus reached the editor input.
    focused = page.evaluate("""() => {
        const el = document.querySelector('#viewer');
        return document.activeElement === el && el.shadowRoot.activeElement?.matches('.nxp');
    }""")
    assert focused, "click on page 1 must focus editor input"

    page.keyboard.type(SENT_P1)
    page.wait_for_function(
        "document.querySelector('#viewer').dirty === true",
        timeout=4000,
    )
    # Give the repaint pipeline time to settle (measure→layout→paint rounds).
    page.wait_for_timeout(1500)

    p1_visible = page.evaluate(f"""() => {{
        const root = document.querySelector('#viewer').shadowRoot;
        const pg = root && root.querySelector('.dve-print-page[data-page-index="0"]');
        const spans = pg ? Array.from(pg.querySelectorAll('span')) : [];
        return spans.some(s => s.textContent.includes('{SENT_P1}'));
    }}""")
    assert p1_visible, (
        f"typed '{SENT_P1}' on page 1 not visible in page 1 rendered spans after edit "
        "— repaint did not propagate the edit (issue #593)"
    )
    print(f"  PASS page 1 repaint: '{SENT_P1}' visible in page 1 spans", flush=True)

    # --- 3. Type on page 2, assert sentinel visible in page 2 spans. ---
    SENT_P2 = "ZPAGE2"
    # Click the first editable block in page 2 (all pages are in the DOM simultaneously).
    p2_target = page.locator('#viewer .dve-print-page[data-page-index="1"] [data-bid]').first
    p2_target.click()
    page.wait_for_timeout(300)

    page.keyboard.type(SENT_P2)
    # dirty stays true; wait for repaint to settle.
    page.wait_for_timeout(2000)

    p2_visible = page.evaluate(f"""() => {{
        const root = document.querySelector('#viewer').shadowRoot;
        const pg = root && root.querySelector('.dve-print-page[data-page-index="1"]');
        const spans = pg ? Array.from(pg.querySelectorAll('span')) : [];
        return spans.some(s => s.textContent.includes('{SENT_P2}'));
    }}""")
    assert p2_visible, (
        f"typed '{SENT_P2}' on page 2 not visible in page 2 rendered spans after edit "
        "— repaint did not propagate the edit (issue #593)"
    )
    print(f"  PASS page 2 repaint: '{SENT_P2}' visible in page 2 spans", flush=True)

    # Also verify page 1 sentinel survived the page-2 edit (proves no cross-page wipe).
    p1_still = page.evaluate(f"""() => {{
        const root = document.querySelector('#viewer').shadowRoot;
        const pg = root && root.querySelector('.dve-print-page[data-page-index="0"]');
        const spans = pg ? Array.from(pg.querySelectorAll('span')) : [];
        return spans.some(s => s.textContent.includes('{SENT_P1}'));
    }}""")
    assert p1_still, (
        f"'{SENT_P1}' on page 1 disappeared after editing page 2 "
        "— cross-page repaint wipe (issue #593)"
    )
    print(f"  PASS cross-page check: '{SENT_P1}' still present on page 1 after page-2 edit", flush=True)


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
