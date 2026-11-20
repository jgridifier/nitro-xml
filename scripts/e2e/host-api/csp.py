# SPDX-License-Identifier: MIT
"""Strict CSP, request allowlist, and real hyperlink-click checks. No soft passes."""
import sys
sys.dont_write_bytecode = True
from urllib.parse import urlsplit
from playwright.sync_api import sync_playwright
from common import CHROME, server, ready

def check(browser, base, fmt):
    # Catches: CSP violations, remote font traffic, or document links navigating the host.
    context = browser.new_context(service_workers='block')
    unexpected, console_errors = [], []
    page = context.new_page()
    popups = []
    context.on("page", lambda popup: popups.append(popup.url))
    def route(request_route):
        url = request_route.request.url
        path = urlsplit(url).path
        allowed = url.startswith(base + '/') and (path.startswith('/demo/nitro-xml/loader/') or path.startswith('/demo/nitro-xml/pkg/') or path in ['/demo/host-api/csp.html', '/demo/host-api/csp.js', '/demo/host-api/csp.css', '/demo/samples/notes.docx', '/demo/samples/sample.xlsx', '/demo/samples/deck.pptx', '/fixtures/agent-generated/python-docx.docx', '/fixtures/agent-generated/openpyxl.xlsx', '/fixtures/agent-generated/python-pptx.pptx'])
        if allowed: request_route.continue_()
        else:
            unexpected.append(url)
            request_route.abort()
    context.route('**/*', route)
    page.on('console', lambda msg: console_errors.append(msg.text) if msg.type == 'error' else None)
    page.on('pageerror', lambda error: console_errors.append(str(error)))
    try:
        page.goto(f'{base}/demo/host-api/csp.html?f={fmt}')
        ready(page)
        page.wait_for_timeout(1000)
        assert not unexpected, unexpected
        assert not console_errors, console_errors
        assert page.evaluate('window.violations') == [], page.evaluate('window.violations')
        print(f'PASS {fmt}: first paint, CSP violations=0, unexpected requests=0', flush=True)
        fixture = {'docx':'python-docx.docx', 'xlsx':'openpyxl.xlsx', 'pptx':'python-pptx.pptx'}[fmt]
        page.goto(f'{base}/demo/host-api/csp.html?f={fmt}&file=/fixtures/agent-generated/{fixture}')
        ready(page)
        link = page.locator('#viewer [data-run]').filter(has_text='Host').first if fmt == 'docx' else page.locator('#viewer a[href], #viewer [data-pptx-hyperlink], #viewer .xg-link').first
        assert link.count(), 'No rendered document hyperlink available for real click proof'
        before = page.url
        link.click()
        page.wait_for_function('() => window.links.length === 1', timeout=3000)
        detail = page.evaluate('window.links[0]')
        assert detail == {'href': 'https://example.com/host-api', 'text': 'Host link'}, detail
        assert page.evaluate('window.linkFlags') == [{'bubbles': True, 'composed': True}], 'Event must bubble to document and be composed'
        assert page.url == before and len(context.pages) == 1 and not popups, 'Hyperlink navigated/opened a page'
        assert not unexpected and not console_errors and page.evaluate('window.violations') == [], 'Link caused CSP/network failure'
        print(f'PASS {fmt}: real hyperlink click emits href/text only; no navigation', flush=True)
    finally: context.close()

def main():
    failures = []
    with server() as base, sync_playwright() as p:
        browser = p.chromium.launch(executable_path=CHROME, args=['--no-sandbox', '--disable-dev-shm-usage'])
        try:
            for fmt in ('docx', 'xlsx', 'pptx'):
                try: check(browser, base, fmt)
                except Exception as e:
                    failures.append(fmt)
                    print(f'FAIL {fmt}: {e}', flush=True)
        finally: browser.close()
    print(f'failures: {len(failures)}', flush=True)
    return bool(failures)

if __name__ == '__main__': sys.exit(main())
