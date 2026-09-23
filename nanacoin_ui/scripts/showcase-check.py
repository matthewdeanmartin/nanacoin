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
                # Find the link's actual dropdown instead of maintaining a second
                # menu map here. Native details exposes its state through open.
                group = nav.locator('details').filter(has=page.get_by_role('link', name=label, exact=True, include_hidden=True))
                if group.count() and not group.evaluate('e => e.open'):
                    group.locator('summary').click()
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
            page.get_by_role('link', name='Vegetarian and vegan recipes').click()
            expect(page.get_by_role('heading', name='Vegan lemon bars (egg-free and dairy-free)')).to_be_visible()
            navigate('The Notebook')
            expect(page.locator('app-public-ledger .ledger-row').first).to_be_visible()
            page.keyboard.press('j')
            expect(page.locator('app-public-ledger .ledger-row').nth(0)).to_be_focused()
            page.keyboard.press('j')
            expect(page.locator('app-public-ledger .ledger-row').nth(1)).to_be_focused()
            page.keyboard.press('0')
            expect(page.locator('app-public-ledger .ledger-row').nth(0)).to_be_focused()
            page.get_by_label('Category').select_option('LABOR')
            expect(page.locator('app-public-ledger .ledger-row').first).to_contain_text('Organize the bookshelf')
            page.get_by_label('Category').select_option('ALL')
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
            navigate('Board Health')
            expect(page.get_by_role('navigation', name='Main navigation')).not_to_be_visible()
            expect(page.get_by_role('heading', name='Board health')).to_be_visible()
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
            for label in ['My Account', 'Forex', 'Offers', 'Send Money', 'Messages', 'Invitations', 'Loans', 'Lotto', 'Economy', 'Market']:
                navigate(label)
                expect(page.locator('main h1').first).to_be_visible()
            page.set_viewport_size({'width': 390, 'height': 844})
            for label in ['Send Money', 'Messages', 'Invitations', 'Loans', 'Lotto', 'Offers']:
                navigate(label)
                expect(page.locator('main h1').first).to_be_visible()
                expect(page.get_by_role('navigation', name='Main navigation')).not_to_be_visible()
                assert page.evaluate('document.documentElement.scrollWidth <= innerWidth + 1'), label
            page.set_viewport_size({'width': 1024, 'height': 768})
            navigate('Lotto')
            lotto = page.locator('app-lotto')
            expect(lotto.get_by_role('button', name='Buy tickets', exact=True)).to_have_count(3)
            expect(lotto.get_by_text('Interest paid to you:', exact=False)).to_have_count(3)
            expect(lotto.get_by_text('Ticket cost:', exact=False)).to_have_count(3)
            draw_titles = ['Winner takes the pool', 'A prize with interest', 'Save and win interest']
            for title in draw_titles:
                open_draw = lotto.locator('article').filter(has=page.get_by_role('heading', name=title, exact=True))
                open_draw.get_by_role('button', name='Buy tickets', exact=True).click()
                ticket_dialog = page.locator('app-dialog-host dialog[open]')
                ticket_dialog.get_by_label('Buy tickets', exact=True).fill('2')
                ticket_dialog.get_by_role('button', name='Confirm', exact=True).click()
                expect(ticket_dialog.get_by_role('heading', name='Confirm ticket purchase')).to_be_visible()
                ticket_dialog.get_by_role('button', name='Buy tickets', exact=True).click()
                expect(open_draw).to_contain_text('Your tickets: 2')
            expect(open_draw).to_contain_text('Principal due back: 2 NC')
            navigate('My Account')
            expect(page.locator('#my-lotto')).to_contain_text('Save and win interest')
            expect(page.locator('#my-lotto')).to_contain_text('NC net')
            page.locator('details.account-menu > summary').filter(has_text='Account').click()
            page.get_by_role('button', name='Sign in another account').click()
            page.get_by_role('button', name=re.compile(r'^Nana\b')).click()
            expect(page.locator('.topbar__who').get_by_text('Nana', exact=True)).to_be_visible()
            navigate('Household')
            page.set_viewport_size({'width': 390, 'height': 844})
            page.get_by_role('button', name='Resolve all lottos now', exact=True).click()
            expect(page.get_by_role('status').filter(has_text='Resolved 3 lottos.')).to_be_visible()
            page.get_by_role('button', name='Resolve all lottos now', exact=True).click()
            expect(page.get_by_role('status').filter(has_text='No pending lottos to resolve.')).to_be_visible()
            page.set_viewport_size({'width': 1024, 'height': 768})
            page.locator('details.account-menu > summary').filter(has_text='Account').click()
            page.get_by_role('button', name='Sign in another account').click()
            page.get_by_role('button', name=re.compile(r'^Dad\b')).click()
            expect(page.locator('.topbar__who').get_by_text('Dad', exact=True)).to_be_visible()
            navigate('Lotto')
            for title in draw_titles:
                result = lotto.locator('article').filter(has=page.get_by_role('heading', name=title, exact=True))
                expect(result).to_contain_text('settled')
                expect(result).to_contain_text('Winner: Dad')
                expect(result).to_contain_text('Ticket cost: 2 NC')
                expect(result).to_contain_text('Interest paid to you: ' + ('0 NC' if title == draw_titles[0] else '0.1 NC'))
                expect(result).to_contain_text('Principal returned: 2 NC' if title == draw_titles[2] else 'Prize paid: 2 NC')
            expect(lotto.get_by_role('button', name='Buy tickets', exact=True)).to_have_count(0)
            navigate('My Account')
            expect(page.locator('#my-lotto')).to_contain_text('No pending tickets.')
            expect(page.locator('#my-lotto')).to_contain_text('+0.1 NC net')
            navigate('Economy')
            indicators = page.locator('.economy-stats')
            expect(indicators.locator('app-economy-stat')).to_have_count(6)
            for label in ['employment', 'inflation', 'GDP', 'money supply', 'exchange rate', 'interest rate']:
                expect(indicators.get_by_role('button', name=label, exact=True)).to_be_visible()
            employment = indicators.get_by_role('button', name='employment', exact=True)
            employment_help = page.locator('#employment-help-description')
            expect(employment_help).not_to_be_visible()
            employment.hover()
            expect(employment_help).to_be_visible()
            expect(employment_help).to_contain_text('365 days')
            expect(employment_help).to_contain_text('available history may be shorter')
            page.locator('app-economy h1').hover()
            expect(employment_help).not_to_be_visible()
            employment.focus()
            expect(employment_help).to_be_visible()
            employment.press('Escape')
            expect(employment_help).not_to_be_visible()
            # Phone-width cards and tap toggling must work without label overflow.
            page.set_viewport_size({'width': 390, 'height': 844})
            interest = indicators.get_by_role('button', name='interest rate', exact=True)
            interest_help = page.locator('#interest-help-description')
            interest.click()
            expect(interest_help).to_be_visible()
            expect(interest_help).to_contain_text('weighted by outstanding loan principal')
            expect(interest_help).to_contain_text('normalized to 365 days')
            interest.click()
            expect(interest_help).not_to_be_visible()
            assert indicators.locator('.stat').evaluate_all('cards => cards.every(e => e.scrollWidth <= e.clientWidth + 1)')
            assert page.evaluate('document.documentElement.scrollWidth <= innerWidth + 1')
            page.set_viewport_size({'width': 1024, 'height': 768})
            assert indicators.locator('.stat').evaluate_all('cards => cards.every(e => e.scrollWidth <= e.clientWidth + 1)')
            expect(page.get_by_text('Exchange rates', exact=True)).to_be_visible()
            expect(page.get_by_role('listitem').filter(has_text='Completed trades')).to_be_visible()
            navigate('Send Money')
            page.get_by_role('tab', name='Setup Allowance').click()
            expect(page.get_by_role('heading', name='Setup Allowance', exact=True)).to_be_visible()
            allowance_to = page.get_by_label('To')
            allowance_to.select_option('account-user-4')
            expect(allowance_to).not_to_have_value('')
            page.get_by_label('Amount').fill('1')
            page.get_by_role('button', name='Save allowance').click()
            page.get_by_role('button', name='Check allowances').click()
            expect(page.get_by_role('status').filter(has_text='Sent 1 due allowance payment.')).to_be_visible()
            page.locator('details.account-menu > summary').filter(has_text='Account').click()
            page.get_by_role('button', name='Sign in another account').click()
            page.get_by_role('button', name=re.compile(r'^Sam\b')).click()
            expect(page.locator('.topbar__who').get_by_text('Sam', exact=True)).to_be_visible()
            navigate('The Notebook')
            allowance = page.locator('.ledger-row').filter(has_text=re.compile(r'Allowance \(\d{4}-\d{2}-\d{2}\)')).first
            allowance.get_by_role('button', name='Refund').click()
            refund_dialog = page.locator('app-dialog-host dialog[open]')
            expect(refund_dialog.get_by_role('heading', name='Refund this payment?')).to_be_visible()
            refund_dialog.get_by_role('button', name='Refund').click()
            expect(page.get_by_role('status').filter(has_text='Refunded.')).to_be_visible()
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
            page.locator('details.account-menu > summary').filter(has_text='Account').click()
            page.get_by_role('button', name='Sign in another account').click()
            page.get_by_role('button', name=re.compile(r'^Mom\b')).click()
            expect(page.locator('.topbar__who').get_by_text('Mom', exact=True)).to_be_visible()
            page.evaluate("token => location.hash = '#/redeem?token=' + encodeURIComponent(token)", token)
            expect(page).to_have_url(re.compile(r'#\/redeem$'))
            expect(page.get_by_label('Voucher code', exact=True)).to_have_value(token)
            page.get_by_role('button', name='Redeem once').click()
            expect(page.get_by_role('status').filter(has_text='Redeemed.')).to_be_visible()
            expect(page.get_by_role('heading', name='My voucher redemptions')).to_be_visible()
            expect(page.get_by_text(re.compile(r'Redeem nana-nickle NN-\d+'))).to_be_visible()
            page.get_by_label('Voucher code', exact=True).fill(token)
            page.get_by_role('button', name='Redeem once').click()
            expect(page.get_by_role('status').filter(has_text='already redeemed')).to_be_visible()
            assert not forbidden, f'Network escaped static demo: {forbidden}'
            assert not errors, errors
            browser.close()
    finally:
        server.shutdown()
        server.server_close()
    print('Static showcase passed: public routes, real browser health, notebook/font/mobile, economic indicators/help/mobile, all lotto purchases/draws/payouts, voucher issue/print/redeem/replay; no API or external requests.')

if __name__ == '__main__':
    main()
