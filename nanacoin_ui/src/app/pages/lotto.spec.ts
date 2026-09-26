import { TestBed } from '@angular/core/testing';
import { provideRouter } from '@angular/router';
import { provideHttpClient } from '@angular/common/http';
import { provideHttpClientTesting } from '@angular/common/http/testing';
import { ApiBase } from '../api/api-base';
import { NanacoinService } from '../api/nanacoin.service';
import { Session } from '../api/session';
import { Lotto } from '../api/models';
import { Dialogs } from '../ui/dialog';
import { LottoDraft, LottoPage, checkLotto, rateHint } from './lotto';

const draw: Lotto = { id: 1, terms: { kind: 'SAVINGS', title: 'Family savings', ticket_price: 10000, closes_at: 1900000000, rate_bps: 100 }, house: 'account-1', pool: 40000, interest: 400, tickets: 4, my_tickets: 1, winner: null, winner_name: null, due_at: 1902592000, status: 'OPEN' };
async function render(role: 'nana' | 'user' = 'user', lotto: Lotto = draw) {
  TestBed.configureTestingModule({ providers: [provideHttpClient(), provideHttpClientTesting(), provideRouter([]), ApiBase] });
  const api = TestBed.inject(NanacoinService);
  api.lottos = () => Promise.resolve({ lottos: [lotto], decimals: 4, money_epoch: 0 });
  const session = TestBed.inject(Session);
  session.me.set({ id: 'user-2', account: role === 'nana' ? 'account-1' : 'account-2', username: 'alice', display_name: 'Alice', role, status: 'ACTIVE', balance: 100000 } as never);
  session.refresh = () => Promise.resolve();
  const fixture = TestBed.createComponent(LottoPage);
  fixture.componentRef.setInput('administration',role==='nana');
  fixture.detectChanges(); await fixture.whenStable(); fixture.detectChanges();
  return { api, fixture, element: fixture.nativeElement as HTMLElement };
}
afterEach(() => TestBed.resetTestingModule());
describe('lotto page', () => {
  it('keeps Nana creation controls in Household, with a link from the ordinary Lotto page',async()=>{
    const {fixture,element}=await render('nana');
    fixture.componentRef.setInput('administration',false);fixture.detectChanges();
    expect(element.querySelector('form')).toBeNull();
    expect(element.querySelector('a')?.getAttribute('href')).toContain('tab=lotto-admin');
  });
  it('explains the savings payout and shows the buyers ticket odds', async () => {
    const { element } = await render();
    expect(element.textContent).toContain('returns everyone');
    expect(element.textContent).toContain('Your tickets: 1');
    expect(element.textContent).toContain('25.00%');
    expect(element.textContent).toContain('Buy tickets');
    expect(element.textContent).not.toContain('Create a lotto');
  });
  it('lets Nana create draws but not enter her own pool', async () => {
    const { element } = await render('nana');
    expect(element.textContent).toContain('Create a lotto');
    expect(element.textContent).not.toContain('Buy tickets');
    expect(element.querySelectorAll('select[name="kind"] option')).toHaveLength(3);
  });
  it('shows completed winners without offering more tickets', async () => {
    const { element } = await render('user', { ...draw, status: 'SETTLED', winner: 'account-2', winner_name: 'Alice' });
    expect(element.textContent).toContain('Winner: Alice');
    expect(element.textContent).not.toContain('Buy tickets');
  });
  it('reuses the purchase key after an ambiguous failure', async () => {
    const { api, fixture } = await render();
    const dialogs = TestBed.inject(Dialogs);
    dialogs.prompt = () => Promise.resolve('2');
    dialogs.confirm = () => Promise.resolve('');
    const keys: string[] = [];
    api.buyTickets = async (_id, _count, key) => { keys.push(key); if (keys.length === 1) throw new Error('Connection lost'); return draw; };
    const page = fixture.componentInstance as unknown as { buy(lotto: Lotto): Promise<void> };
    await page.buy(draw); await page.buy(draw);
    expect(keys).toHaveLength(2); expect(keys[0]).toBe(keys[1]);
  });
});
describe('create-lotto checks', () => {
  const now = Date.UTC(2026, 8, 26);
  const good: LottoDraft = { title: 'Summer', kind: 'SAVINGS', price: '1', closes: '2026-10-01T12:00', rate: '10' };
  it('reads 10, 10% and 10 % as ten percent', () => {
    for (const rate of ['10', '10%', '10 %']) expect(checkLotto({ ...good, rate }, 4, 'en-US', now).terms?.rate_bps).toBe(1000);
  });
  it('names each bad box and quotes what was typed', () => {
    const { terms, problems } = checkLotto({ title: ' ', kind: 'DELAYED', price: '1,000', closes: '2020-01-01T00:00', rate: '150' }, 4, 'en-US', now);
    expect(terms).toBeUndefined();
    expect(problems.title).toContain('name');
    expect(problems.price).toContain('"1,000"');
    expect(problems.closes).toContain('already passed');
    expect(problems.rate).toContain('"150"');
  });
  it('asks for both the date and the time', () => {
    expect(checkLotto({ ...good, closes: '' }, 4, 'en-US', now).problems.closes).toContain('both the date and the time');
  });
  it('ignores the rate box for a simple lotto', () => {
    expect(checkLotto({ ...good, kind: 'SIMPLE', rate: 'lots' }, 4, 'en-US', now).terms?.rate_bps).toBe(0);
  });
  it('explains the rate and questions a rate that looks like a fraction', () => {
    expect(rateHint('10', 'en-US')).toContain('10 NC for every 100 NC');
    expect(rateHint('0.10', 'en-US')).toContain('Did you mean 10%');
  });
  it('shows problems on the page instead of only in the console', async () => {
    const { fixture, element } = await render('nana');
    const page = fixture.componentInstance as unknown as { create(): Promise<void>; rate: string; kind: string };
    page.kind = 'DELAYED'; page.rate = 'ten';
    await page.create(); fixture.detectChanges();
    expect(element.querySelector('.form-problems')?.textContent).toContain('boxes need fixing');
    expect(element.textContent).toContain('You typed "ten"');
    expect(element.querySelector('input[name="title"]')?.getAttribute('aria-invalid')).toBe('true');
  });
});
