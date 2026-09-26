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
            channel = os.environ.get('SHOWCASE_BROWSER', 'msedge' if os.name == 'nt' else 'chromium')
            browser = p.chromium.launch(headless=True, **({} if channel == 'chromium' else {'channel': channel}))
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
            def viewport(width, height):
                page.set_viewport_size({'width': width, 'height': height})
                # Resize acknowledgement can precede the responsive layout update.
                # Wait for the actual viewport and menu breakpoint, not a timer.
                page.wait_for_function('''([width, height]) => {
                    const toggle = document.querySelector('.menu-toggle');
                    return innerWidth === width && innerHeight === height && toggle
                        && (getComputedStyle(toggle).display !== 'none') === (width <= 980);
                }''', arg=[width, height])

            def open_navigation():
                # Use a DOM locator: the mobile nav is intentionally absent from
                # the accessibility tree while closed.
                nav = page.locator('#site-navigation')
                toggle = page.locator('.menu-toggle')
                if page.viewport_size['width'] <= 980:
                    expect(toggle).to_be_visible()
                    if toggle.get_attribute('aria-expanded') != 'true':
                        toggle.click()
                    expect(toggle).to_have_attribute('aria-expanded', 'true')
                else:
                    expect(toggle).not_to_be_visible()
                expect(nav).to_be_visible()
                return nav

            def navigation_closed():
                # Escape/click dispatch finishes before Angular necessarily paints
                # the closed state. A later resize must not sample the old state.
                expect(page.locator('.menu-toggle')).to_have_attribute('aria-expanded', 'false')
                expect(page.locator('#site-navigation details[open]')).to_have_count(0)
                if page.viewport_size['width'] <= 980:
                    expect(page.locator('#site-navigation')).not_to_be_visible()

            def navigate(label):
                nav = open_navigation()
                # Find the link's actual dropdown instead of maintaining a second
                # menu map here. Native details exposes its state through open.
                group = nav.locator('details').filter(has=page.locator('a').filter(has_text=re.compile('^' + re.escape(label) + '$')))
                if group.count() and not group.evaluate('e => e.open'):
                    group.locator('summary').click()
                link = nav.get_by_role('link', name=label, exact=True)
                link.click()
                expect(page.locator('#site-navigation a').filter(has_text=re.compile('^' + re.escape(label) + '$'))).to_have_attribute('aria-current', 'page')
                navigation_closed()
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
            expect(page.locator('.demobar')).to_contain_text('Demo of Nanacoin! A non-crypto household currency!')
            expect(page.locator('.demobar').get_by_role('link',name='SMBC Specification (2021 ed)')).to_have_attribute('href','https://www.smbc-comics.com/comic/nanacoin')
            page.get_by_role('link',name='Get your own!',exact=True).click()
            expect(page.get_by_role('heading',name='Docs',exact=True)).to_be_visible()
            for topic in ['My Account','Mail','Market','Loans & lotto','Nana','Reports','Hardware','Start']:
                page.get_by_role('tab',name=topic,exact=True).click()
                expect(page.locator('app-docs [role="tabpanel"]')).to_have_count(1)
                expect(page.locator('app-docs [role="tabpanel"] h2')).to_be_visible()
            viewport(320, 844)
            page.get_by_role('tab',name='Hardware',exact=True).click()
            expect(page.locator('app-docs')).to_contain_text('ESP32-S3-N16R8')
            assert page.evaluate('document.documentElement.scrollWidth <= innerWidth + 1')
            viewport(1024, 768)
            navigate('The Notebook')
            page.get_by_role('link', name='Vegetarian and vegan recipes').click()
            expect(page.get_by_role('heading', name='Vegan lemon bars (egg-free and dairy-free)')).to_be_visible()
            navigate('The Notebook')
            expect(page.locator('app-public-ledger .ledger-row').first).to_be_visible()
            for example in ['Partial refund: one damaged sketchbook', 'Watercolor fund: a gift from Dad', 'Digital art: Moonlit garden', 'Loan interest']:
                expect(page.locator('app-public-ledger .ledger-row').filter(has_text=example).first).to_be_visible()
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
            viewport(390, 844)
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
            viewport(1024, 768)
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
            viewport(390, 844)
            for label in ['Send Money', 'Messages', 'Invitations', 'Loans', 'Lotto', 'Offers']:
                navigate(label)
                expect(page.locator('main h1').first).to_be_visible()
                expect(page.get_by_role('navigation', name='Main navigation')).not_to_be_visible()
                assert page.evaluate('document.documentElement.scrollWidth <= innerWidth + 1'), (label, page.locator('main *').evaluate_all('els => els.filter(e => e.getBoundingClientRect().right > innerWidth + 1).map(e => ({tag:e.tagName,cls:e.className,text:e.textContent.slice(0,120),width:e.getBoundingClientRect().width}))'))
            viewport(1024, 768)
            navigate('Loans')
            expect(page.get_by_text('Tools for the garden', exact=True)).to_be_visible()
            page.locator('app-loans h1').click()
            for width in [320, 390, 768, 1024]:
                viewport(width, 844)
                expect(page.get_by_label("Draw once when the borrower's balance reaches exactly zero")).to_be_visible()
                assert page.evaluate('document.documentElement.scrollWidth <= innerWidth + 1'), ('Loans', width)
                if width == 320:
                    page.screenshot(path=str(screenshots / 'loans-mobile.png'), full_page=True)
            viewport(1024, 768)
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
            page.get_by_role('tab', name=re.compile(r'^Lotto')).click()
            expect(page.locator('#panel-lotto')).to_contain_text('Save and win interest')
            expect(page.locator('#panel-lotto')).to_contain_text('NC net')
            page.locator('details.account-menu > summary').filter(has_text='Account').click()
            page.get_by_role('button', name='Sign in another account').click()
            page.get_by_role('button', name=re.compile(r'^Nana\b')).click()
            expect(page.locator('.topbar__who').get_by_text('Nana', exact=True)).to_be_visible()
            expect(page.locator('app-login-form')).not_to_be_visible()
            expect(page.locator('app-history')).to_be_visible()
            # Exercise both directions across the breakpoint, including reopening
            # the mobile menu immediately after Escape and a desktop visit.
            for width in [320, 390, 768, 1024, 1280, 768, 320, 1024]:
                viewport(width, 844)
                nav = open_navigation()
                group=nav.locator('details:has(a[href="#/nana"])')
                expect(group).to_have_count(1)
                if not group.evaluate('e=>e.open'): group.locator('summary').click()
                expect(group.get_by_role('link',name='My Account',exact=True)).to_be_visible()
                expect(group.get_by_role('link',name='Household',exact=True)).to_be_visible()
                assert page.evaluate('document.documentElement.scrollWidth <= innerWidth + 1'), ('Nana menu',width)
                if width>=1024:
                    assert nav.evaluate('e=>e.getBoundingClientRect().bottom-e.getBoundingClientRect().top<70'), ('Wrapped Nana menu',width)
                    assert page.locator('.brand').evaluate('e=>e.getBoundingClientRect().left>=0'), ('Clipped brand',width)
                page.keyboard.press('Escape')
                navigation_closed()
            viewport(1024, 768)
            navigate('My Account')
            expect(page.get_by_role('tab',name=re.compile('^Transactions'))).to_have_attribute('aria-selected','true')
            expect(page.get_by_role('button',name='Reverse',exact=True)).to_have_count(0)
            for label in ['TODO','Loans & debts','Lotto','My offers','My forex bids','Transactions']:
                page.get_by_role('tab',name=re.compile('^'+re.escape(label))).click()
                expect(page.get_by_role('tabpanel')).to_have_count(1)
            navigate('Nana as Central Bank')
            bank = page.locator('app-central-bank')
            expect(bank.locator('#cb-reserve')).to_contain_text('$100.00')
            bank.locator('.chart-controls select').select_option('all')
            expect(bank.get_by_role('row').filter(has_text='New coins issued to Nana')).to_contain_text('1,000 NC')
            expect(bank.get_by_role('row').filter(has_text='Real dollars recorded into the household')).to_contain_text('$300.00')
            expect(bank.get_by_role('row').filter(has_text='Loans Nana made / repaid to her')).to_contain_text('10 /')
            page.screenshot(path=str(screenshots / 'central-bank.png'), full_page=True)
            navigate('Household')
            expect(page.get_by_role('tab',name='Ledger',exact=True)).to_have_attribute('aria-selected','true')
            expect(page.locator('#full-ledger')).to_be_visible()
            expect(page.locator('#members')).not_to_be_visible()
            page.screenshot(path=str(screenshots / 'household-tabs.png'))
            for label in ['Members','Money','Demo data','Ledger']:
                page.get_by_role('tab',name=label,exact=True).click()
                expect(page.get_by_role('tabpanel')).to_have_count(1)
            page.get_by_role('tab',name='Money',exact=True).click()
            expect(page.get_by_role('button',name='Issue',exact=True)).to_be_visible()
            expect(page.get_by_role('button',name='Record dollars held',exact=True)).to_be_visible()
            page.get_by_role('tab',name='Lotto',exact=True).click()
            expect(page.get_by_role('heading',name='Create a lotto',exact=True)).to_be_visible()
            viewport(390, 844)
            page.get_by_role('button', name='Resolve all lottos now', exact=True).click()
            expect(page.get_by_role('status').filter(has_text='Resolved 3 lottos.')).to_be_visible()
            page.get_by_role('button', name='Resolve all lottos now', exact=True).click()
            expect(page.get_by_role('status').filter(has_text='No pending lottos to resolve.')).to_be_visible()
            viewport(1024, 768)
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
            page.get_by_role('tab', name=re.compile(r'^Lotto')).click()
            expect(page.locator('#panel-lotto')).to_contain_text('No pending tickets.')
            expect(page.locator('#panel-lotto')).to_contain_text('+0.1 NC net')
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
            viewport(390, 844)
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
            viewport(1024, 768)
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
    print('Static showcase passed: notebook commerce/refunds, Loans at 320/390/768/1024px, Nana reserves and book, public routes, browser health, notebook/font/mobile, economic indicators/help, lotto purchases/draws/payouts, voucher issue/print/redeem/replay; no API or external requests.')

if __name__ == '__main__':
    main()
