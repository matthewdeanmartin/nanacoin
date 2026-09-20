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
# Must match the base href the site was built with, so this harness serves it
# the way Pages will. Taken from the environment the workflow already sets,
# rather than repeated here where it would drift.
PREFIX = os.environ.get('BASE_HREF', '/nanacoin/')
if not (PREFIX.startswith('/') and PREFIX.endswith('/')):
    raise SystemExit(f'BASE_HREF must start and end with "/": {PREFIX!r}')

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
                nav = page.get_by_role('navigation', name='Main navigation')
                if not nav.is_visible():
                    page.get_by_role('button', name='Open navigation menu').click()
                    expect(nav).to_be_visible()
                group = None
                if label == 'Economy' or (label == 'The Notebook' and nav.get_by_text('Accounting', exact=True).count()):
                    group = 'Accounting'
                elif label in ('Market', 'Offers', 'Exchange', 'Nana-nickles'):
                    group = 'Buy/Sell'
                elif label in ('Browser Log', 'Browser Health', 'Board Health', 'Server Log'):
                    group = 'System Info'
                if group:
                    summary = nav.get_by_text(group, exact=True)
                    if summary.get_attribute('aria-expanded') != 'true':
                        summary.click()
                link = nav.get_by_role('link', name=label, exact=True)
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
            about_width = page.locator('#main-content').bounding_box()['width']
            assert 840 <= about_width <= 850, about_width
            assert page.locator('.energy-chart').first.evaluate('e => e.scrollWidth <= e.clientWidth')
            navigate('The Notebook')
            page.get_by_role('link', name='Have a lemon bar').click()
            expect(page.get_by_role('heading', name='Vegan lemon bars (egg-free and dairy-free)')).to_be_visible()
            navigate('The Notebook')
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
            navigate('Browser Health')
            expect(page.get_by_role('navigation', name='Main navigation')).not_to_be_visible()
            expect(page.get_by_role('heading', name='Browser health')).to_be_visible()
            page.get_by_role('button', name='Refresh readings').click()
            page.set_viewport_size({'width': 1024, 'height': 768})
            page.get_by_role('link', name='NanaCoin app', exact=True).click()
            page.get_by_role('button', name='Dad an ordinary member').click()
            expect(page.locator('app-market')).to_be_visible()
            account_left = page.get_by_role('link', name='My Account', exact=True).bounding_box()['x']
            content_left = page.locator('#main-content').bounding_box()['x']
            assert abs(account_left - content_left) <= 1, (account_left, content_left)
            page.keyboard.press('n')
            expect(page.locator('app-market input[name="title"]')).to_be_focused()
            page.keyboard.type('gh?')
            expect(page.locator('app-market input[name="title"]')).to_have_value('gh?')
            expect(page.get_by_role('dialog', name='Keyboard shortcuts')).not_to_be_visible()
            page.locator('app-market input[name="title"]').fill('')
            page.locator('app-market h1').first.click()
            for label in ['My Account', 'Exchange', 'Offers', 'Economy', 'Market']:
                navigate(label)
                expect(page.locator('main h1').first).to_be_visible()
            page.evaluate("location.hash = '#/nickles'")
            expect(page.get_by_role('heading', name='Nana-nickles')).to_be_visible()
            page.get_by_role('button', name='Create voucher').click()
            expect(page.locator('.voucher-secret')).to_be_visible()
            expect(page.get_by_role('img', name='QR code linking to the Nana-nickle redemption page')).to_be_visible()
            token = page.locator('.voucher-secret').inner_text()
            page.emulate_media(media='print')
            expect(page.locator('.printable-voucher')).to_be_visible()
            assert page.get_by_role('button', name='Create voucher', include_hidden=True).evaluate('e => getComputedStyle(e).visibility') == 'hidden'
            page.emulate_media(media='screen')
            page.evaluate("token => location.hash = '#/redeem?token=' + encodeURIComponent(token)", token)
            expect(page).to_have_url(re.compile(r'#\/redeem$'))
            expect(page.get_by_label('Voucher code', exact=True)).to_have_value(token)
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
