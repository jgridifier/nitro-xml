# SPDX-License-Identifier: MIT
"""Guard stable selectors across all formats/modes and chrome replacement."""
import json
import sys
sys.dont_write_bytecode = True
from pathlib import Path
from playwright.sync_api import sync_playwright
from common import CHROME, server, ready

CONTRACT = json.loads(Path(__file__).with_suffix('.json').read_text())

def check(browser, base, fmt, mode):
    # Catches: DOM replacement dropping documented selectors in full or embedded mode.
    page = browser.new_page()
    try:
        page.goto(f'{base}/demo/host-api/csp.html?f={fmt}&mode={mode}')
        ready(page)
        required = CONTRACT['common'] + CONTRACT[mode] + CONTRACT[fmt]
        if mode == 'embedded': required += CONTRACT['embeddedButtons'][fmt]
        if mode == 'full' and fmt == 'xlsx': required += ['sheet-tabs']
        for phase in ('open', 'chrome replacement'):
            for testid in required:
                page.locator(f'[data-testid="{testid}"]').first.wait_for(state='attached', timeout=5000)
            commands = page.locator('#viewer button[data-a="cmd"]')
            assert commands.count(), 'No ribbon commands to check'
            for command in commands.all():
                assert command.get_attribute('data-testid'), 'Ribbon command has no stable test id'
            if phase == 'open': page.evaluate('() => viewer.shellAction("zoom-in")')
        if mode == 'full':
            for tab, ids in CONTRACT['ribbon'][fmt].items():
                page.locator(f'.ox [data-testid="ribbon-tab-{tab.lower()}"]').click()
                for testid in ids:
                    page.locator(f'.ox .rbn [data-testid="{testid}"]').first.wait_for(state='attached')
            page.locator('.ox [data-testid="file-menu"]').click()
            page.locator('.ox [data-testid="file-save"]').wait_for(state='visible')
        print(f'PASS {fmt}/{mode}: {len(required)} documented ids + every rendered command; survives chrome replacement', flush=True)
    finally: page.close()

def main():
    failures = []
    with server() as base, sync_playwright() as p:
        browser = p.chromium.launch(executable_path=CHROME, args=['--no-sandbox', '--disable-dev-shm-usage'])
        try:
            for fmt in ('docx', 'xlsx', 'pptx'):
                for mode in ('full', 'embedded'):
                    try: check(browser, base, fmt, mode)
                    except Exception as e:
                        failures.append((fmt, mode))
                        print(f'FAIL {fmt}/{mode}: {e}', flush=True)
        finally: browser.close()
    print(f'failures: {len(failures)}', flush=True)
    return bool(failures)

if __name__ == '__main__': sys.exit(main())
