#!/usr/bin/env python3
# SPDX-License-Identifier: MIT
"""Real-input host API checks (#561/#562); any missing capability exits nonzero.

Assemble _site as described in the host-api brief, then run with pw-venv Python.
An optional positional port defaults to 7892. No synthetic input or model edits.
"""
import functools
import http.server
import json
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


def expect(actual, expected, message):
    assert actual == expected, f"{message}: expected {expected!r}, got {actual!r}"


def events(page):
    return page.evaluate("window.hostChanges")


def open_page(browser, port, fmt):
    page = browser.new_page(viewport={"width": 1400, "height": 900})
    # Install before the module creates/loads the element; never clear load events.
    page.add_init_script("""window.hostChanges = [];
        document.addEventListener('nitro-change', e => {
            if (e.target.id === 'viewer') window.hostChanges.push(e.detail);
        }, true);""")
    page.goto(f"http://127.0.0.1:{port}/demo/host-api/selection-dirty.html?f={fmt}")
    page.wait_for_function("document.querySelector('#log').textContent.includes('loaded')")
    page.wait_for_function("typeof document.querySelector('#viewer').getSelection === 'function'")
    page.wait_for_function("typeof document.querySelector('#viewer').document.whenEditReady === 'function'")
    page.evaluate("document.querySelector('#viewer').document.whenEditReady()")
    page.locator('#viewer .nxp').wait_for(state='attached')
    page.wait_for_timeout(500)
    box = page.locator('#viewer .v').bounding_box()
    assert box and box['width'] > 100 and box['height'] > 100, f"editor has no usable surface: {box}"
    expect(events(page), [], "events during load")
    expect(page.evaluate("document.querySelector('#viewer').dirty"), False, "dirty on load")
    return page


def click_text(page, fmt):
    selector = '#viewer [data-bid="p0"] span' if fmt == 'docx' else '#viewer .pptxv-para span'
    target = page.locator(selector).first
    target.click()
    return target


def assert_focus(page):
    expect(page.evaluate("""() => {
        const el = document.querySelector('#viewer');
        return document.activeElement === el && el.shadowRoot.activeElement?.matches('.nxp');
    }"""), True, "real click must focus editor input")


def cell_point(page, row, col):
    # Grid header and default cell dimensions in demo/samples/sample.xlsx.
    # Read the rect in one page turn: every painted frame rebuilds the grid DOM, so a
    # two-step locator lookup can land on a just-detached node mid-drag (flaky None).
    box = page.evaluate("""() => {
        const grid = document.querySelector('#viewer').shadowRoot.querySelector('.xg-root');
        const r = grid && grid.isConnected ? grid.getBoundingClientRect() : null;
        return r && r.width > 0 && r.height > 0 ? {x: r.x, y: r.y} : null;
    }""")
    assert box, 'spreadsheet grid missing'
    return box['x'] + 33 + col * 64 + 32, box['y'] + 20 + row * 20 + 10


def check_dirty(page, fmt):
    # Catches: unfocused keystrokes and cancelled cell edits passing as dirty-event proof.
    page.locator('#chat-area').click()
    if fmt == 'xlsx':
        page.mouse.click(*cell_point(page, 1, 1))
    else:
        click_text(page, fmt)
    assert_focus(page)
    page.wait_for_timeout(500)
    expect(events(page), [], 'selection/focus must not mark dirty')
    page.keyboard.type('x')
    if fmt == 'xlsx':
        page.keyboard.press('Enter')  # Commit; Escape cancels the edit.
    page.wait_for_function("document.querySelector('#viewer').dirty === true", timeout=3000)
    page.wait_for_timeout(500)
    expect(events(page), [{'dirty': True}], 'exactly one first-edit event')
    expect(page.evaluate("document.querySelector('#viewer').dirty"), True, 'dirty after edit')
    page.keyboard.type('y')
    if fmt == 'xlsx':
        page.keyboard.press('Enter')
    page.wait_for_timeout(700)
    expect(events(page), [{'dirty': True}], 'additional edits must not repeat dirty=true')
    page.locator('#btn-save').click()
    page.wait_for_function("document.querySelector('#viewer').dirty === false", timeout=3000)
    page.wait_for_timeout(500)
    expect(events(page), [{'dirty': True}, {'dirty': False}], 'save resets exactly once')
    page.locator('#btn-save').click()
    page.wait_for_timeout(500)
    expect(events(page), [{'dirty': True}, {'dirty': False}], 'clean save must not emit')
    page.locator('#viewer [data-a="zoom-out"]').click()
    page.locator('#viewer [data-a="zoom-in"]').click()
    page.locator('#viewer .v').hover()
    page.mouse.wheel(0, 100)
    if fmt == 'pptx':
        page.locator('#viewer [data-a="sheet"][data-g="1"]').click()
        page.locator('#viewer [data-a="sheet"][data-g="0"]').click()
    page.wait_for_timeout(700)
    expect(events(page), [{'dirty': True}, {'dirty': False}], 'zoom/scroll/navigation must not emit')
    print(f'  PASS {fmt}: focus; no load/selection event; first edit exactly once; dirty=true; save resets; clean save and zoom/scroll/navigation silent', flush=True)


def check_selection(page, fmt):
    # Catches: placeholder payloads passing without selecting text, cells or a shape.
    if fmt == 'xlsx':
        page.mouse.move(*cell_point(page, 1, 1))
        page.mouse.down()
        page.mouse.move(*cell_point(page, 2, 2), steps=12)
        page.mouse.up()
    else:
        # A real double-click selects the first word, if text selection is supported.
        selector = '#viewer [data-bid="p0"] span' if fmt == 'docx' else '#viewer .pptxv-para span'
        page.locator(selector).first.dblclick()
    page.wait_for_timeout(500)
    selection = page.evaluate("document.querySelector('#viewer').getSelection()")
    print(f'  {fmt} selection: {json.dumps(selection)}', flush=True)
    assert selection is not None, 'selection must not be null'
    if fmt == 'docx':
        expect(selection, {
            'format': 'docx', 'text': 'Quarterly', 'page': 0,
            'start': {'paragraph': 0, 'run': 0, 'offset': 0},
            'end': {'paragraph': 0, 'run': 0, 'offset': 9}, 'collapsed': False,
        }, 'DOCX selected word and paragraph offsets')
    elif fmt == 'xlsx':
        expect(selection, {
            'format': 'xlsx', 'sheet': 'Sheet1', 'sheetIndex': 0,
            'range': 'B2:C3', 'anchor': 'B2', 'values': [[4, 6], [6, 9]], 'truncated': False,
        }, 'XLSX multi-cell range and numeric values')
    else:
        expect(selection, {
            'format': 'pptx', 'slide': 0, 'shapeId': 'ppt/slides/slide1.xml-shape-0',
            'shapeName': 'TextBox 1', 'text': 'Nitro', 'textRange': {'start': 0, 'end': 5},
        }, 'PPTX selected shape and text range')
    expect(events(page), [], 'selection must not mark dirty')
    print(f'  PASS {fmt}: real selection payload', flush=True)


def main():
    failures = []
    handler = functools.partial(Handler, directory=str(SITE))
    # Bind synchronously: an occupied port is a hard failure, never an unrelated server.
    with http.server.ThreadingHTTPServer(('127.0.0.1', int(sys.argv[1]) if len(sys.argv) > 1 else 7892), handler) as server:
        thread = threading.Thread(target=server.serve_forever, daemon=True)
        thread.start()
        try:
            with sync_playwright() as p:
                browser = p.chromium.launch(**({} if not CHROME else {"executable_path": CHROME}), args=['--no-sandbox', '--disable-dev-shm-usage'])
                try:
                    for fmt in ('docx', 'xlsx', 'pptx'):
                        for name, check in [('dirty', check_dirty), ('selection', check_selection)]:
                            page = None
                            try:
                                page = open_page(browser, server.server_port, fmt)
                                check(page, fmt)
                            except Exception as error:
                                failure = f'{fmt} {name}: {error}'
                                failures.append(failure)
                                print(f'  FAIL {failure}', flush=True)
                            finally:
                                if page:
                                    page.close()
                finally:
                    browser.close()
        finally:
            server.shutdown()
            thread.join()
    print(f'\nfailures: {len(failures)}', flush=True)
    return 1 if failures else 0


if __name__ == '__main__':
    sys.exit(main())
