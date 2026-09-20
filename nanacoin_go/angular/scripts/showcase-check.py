"""Exercise the actual static Pages build. No board or outside requests allowed.

Build with the Pages base href, then:
uv run --with playwright python scripts/showcase-check.py
On Windows this uses installed Edge; elsewhere install Playwright Chromium.
"""
from functools import partial
from http.server import SimpleHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
import os
import re
import threading
from urllib.parse import urlsplit
from playwright.sync_api import sync_playwright, expect

ROOT = Path(__file__).resolve().parents[1]
PREFIX = '/microcontroller/nanacoin/'

class Static(SimpleHTTPRequestHandler):
    def do_GET(self):
        if not self.path.startswith(PREFIX):
            self.send_error(404)
            return
        self.path = '/' + self.path[len(PREFIX):]
        super().do_GET()

    def log_message(self, *_):
        pass

def main():
    server = ThreadingHTTPServer(('127.0.0.1', 0), partial(Static, directory=str(ROOT / 'dist/nanacoin-web/browser')))
    threading.Thread(target=server.serve_forever, daemon=True).start()
    base = f'http://127.0.0.1:{server.server_port}{PREFIX}'
    forbidden = []
    errors = []
    try:
        with sync_playwright() as p:
            browser = p.chromium.launch(headless=True, **({'channel': 'msedge'} if os.name == 'nt' else {}))
            page = browser.new_page(viewport={'width': 1024, 'height': 768})
            page.on('pageerror', lambda e: errors.append(str(e)))
            def network(route):
                url = urlsplit(route.request.url)
                if url.netloc != urlsplit(base).netloc or '/api/' in url.path:
                    forbidden.append(route.request.url)
                    route.abort()
                else:
                    route.continue_()
            page.route('**/*', network)
            def navigate(label):
                link = page.get_by_role('navigation', name='Main navigation').get_by_role('link', name=label, exact=True)
                if not link.is_visible():
                    page.get_by_role('button', name='Open navigation menu').click()
                link.click()
                expect(page.locator('#site-navigation a').filter(has_text=re.compile('^' + re.escape(label) + '$'))).to_have_attribute('aria-current', 'page')
            page.goto(base + '#/about')
            expect(page.get_by_role('heading', name='A small bank. A very large notebook.')).to_be_visible()
            page.keyboard.press('?')
            expect(page.get_by_role('dialog', name='Keyboard shortcuts')).to_be_visible()
            page.keyboard.press('Escape')
            expect(page.get_by_role('dialog', name='Keyboard shortcuts')).not_to_be_visible()
            expect(page.get_by_role('link', name='SMBC: Nanacoin')).to_have_attribute('href', 'https://www.smbc-comics.com/comic/nanacoin')
            assert page.get_by_role('navigation', name='Main navigation').evaluate('e => getComputedStyle(e).flexDirection') == 'row'
            navigate('Lemon bars')
            expect(page.get_by_role('heading', name='Vegan lemon bars (egg-free and dairy-free)')).to_be_visible()
            navigate('The notebook')
            expect(page.locator('app-public-ledger .ledger-row').first).to_be_visible()
            page.keyboard.press('j')
            expect(page.locator('app-public-ledger .ledger-row').nth(0)).to_be_focused()
            page.keyboard.press('j')
            expect(page.locator('app-public-ledger .ledger-row').nth(1)).to_be_focused()
            page.keyboard.press('0')
            expect(page.locator('app-public-ledger .ledger-row').nth(0)).to_be_focused()
            page.get_by_label('Spiral-bound notebook', exact=True).check()
            page.get_by_label('Handwritten cursive', exact=True).check()
            page.evaluate('document.fonts.load(\'20px "Nana Hand"\')')
            assert page.evaluate('document.fonts.check(\'20px "Nana Hand"\')')
            screenshots = ROOT / '__screenshots__'
            screenshots.mkdir(exist_ok=True)
            page.screenshot(path=str(screenshots / 'notebook.png'))
            page.set_viewport_size({'width': 390, 'height': 844})
            page.screenshot(path=str(screenshots / 'notebook-mobile.png'))
            assert page.evaluate('document.documentElement.scrollWidth <= innerWidth + 1')
            expect(page.get_by_role('navigation', name='Main navigation')).not_to_be_visible()
            page.get_by_role('button', name='Open navigation menu').click()
            expect(page.get_by_role('navigation', name='Main navigation')).to_be_visible()
            assert page.get_by_role('navigation', name='Main navigation').evaluate('e => getComputedStyle(e).flexDirection') == 'column'
            page.screenshot(path=str(screenshots / 'menu-mobile.png'))
            page.keyboard.press('Escape')
            expect(page.get_by_role('button', name='Open navigation menu')).to_be_focused()
            expect(page.get_by_role('navigation', name='Main navigation')).not_to_be_visible()
            navigate('Browser health')
            expect(page.get_by_role('navigation', name='Main navigation')).not_to_be_visible()
            expect(page.get_by_role('heading', name='Browser health')).to_be_visible()
            page.get_by_role('button', name='Refresh readings').click()
            page.get_by_role('link', name='NanaCoin app', exact=True).click()
            page.get_by_role('button', name='Dad an ordinary member').click()
            expect(page.locator('app-market')).to_be_visible()
            page.keyboard.press('n')
            expect(page.locator('app-market input[name="title"]')).to_be_focused()
            page.keyboard.type('gh?')
            expect(page.locator('app-market input[name="title"]')).to_have_value('gh?')
            expect(page.get_by_role('dialog', name='Keyboard shortcuts')).not_to_be_visible()
            page.locator('app-market input[name="title"]').fill('')
            page.locator('app-market h2').first.click()
            for label in ['History', 'Exchange', 'Offers', 'Economy', 'Market']:
                navigate(label)
                expect(page.locator('main h2').first).to_be_visible()
            navigate('Nana-nickles')
            page.get_by_role('button', name='Create voucher').click()
            expect(page.locator('.voucher-secret')).to_be_visible()
            token = page.locator('.voucher-secret').inner_text()
            page.emulate_media(media='print')
            expect(page.locator('.printable-voucher')).to_be_visible()
            assert page.get_by_role('button', name='Create voucher', include_hidden=True).evaluate('e => getComputedStyle(e).visibility') == 'hidden'
            page.emulate_media(media='screen')
            page.get_by_label('Voucher code', exact=True).fill(token)
            page.get_by_role('button', name='Redeem once').click()
            expect(page.get_by_role('status').filter(has_text='Redeemed.')).to_be_visible()
            page.get_by_label('Voucher code', exact=True).fill(token)
            page.get_by_role('button', name='Redeem once').click()
            expect(page.get_by_role('status').filter(has_text='already redeemed')).to_be_visible()
            assert not forbidden, f'Network escaped static demo: {forbidden}'
            assert not errors, errors
            browser.close()
    finally:
        server.shutdown()
        server.server_close()
    print('Static showcase passed: public routes, real browser health, notebook/font/mobile, voucher issue/print/redeem/replay; no API or external requests.')

if __name__ == '__main__':
    main()
