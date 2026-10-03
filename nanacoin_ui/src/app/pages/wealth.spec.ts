import { TestBed } from '@angular/core/testing';
import { signal } from '@angular/core';
import { WealthPage } from './wealth';
import { NanacoinService } from '../api/nanacoin.service';
import { Session } from '../api/session';
import { ApiBase } from '../api/api-base';
import { Money } from '../api/money';
import { User } from '../api/models';

describe('My wealth report', () => {
  const user: User = {
    id: 'user-1',
    account: 'account-1',
    username: 'sam',
    display_name: 'Sam',
    role: 'user',
    status: 'ACTIVE',
    balance: 10000,
    usd_cents: 500,
    created_at: 0,
  };
  afterEach(() => {
    TestBed.resetTestingModule();
    localStorage.clear();
  });
  async function setup() {
    const me = signal(user);
    const history = vi
      .fn()
      .mockResolvedValue({
        balance: 10000,
        account: 'account-1',
        transactions: [],
        next_cursor: null,
      });
    TestBed.configureTestingModule({
      providers: [
        { provide: ApiBase, useValue: { current: signal('http://household/api/v1') } },
        { provide: Session, useValue: { me, signedIn: signal(true), revision: signal(0) } },
        {
          provide: NanacoinService,
          useValue: {
            accountHistory: history,
            me: vi.fn(async () => me()),
            loans: vi.fn().mockResolvedValue({ loans: [] }),
            quotes: vi.fn().mockResolvedValue({ quotes: [] }),
            lottos: vi.fn().mockResolvedValue({ lottos: [] }),
          },
        },
      ],
    });
    TestBed.inject(Money).decimals.set(2);
    const fixture = TestBed.createComponent(WealthPage);
    fixture.detectChanges();
    await fixture.whenStable();
    fixture.detectChanges();
    return { fixture, me, history };
  }
  it('renders current coin and USD balances, allows zero valuation, and scopes the preference to the user', async () => {
    const { fixture, me } = await setup(),
      page = fixture.componentInstance;
    expect(fixture.nativeElement.textContent).toContain('Balance sheet');
    expect(page.cashEstimate()).toBe('$5.00 + unpriced NC');
    expect(page.savingsRate()).toBe('—');
    page.setRate(0.1);
    page.usd.set(true);
    fixture.detectChanges();
    expect(page.cashEstimate()).toBe('$15.00');
    expect(page.totalLabel(page.netCoins(), page.position().dollars)).toBe('$15.00');
    page.setRate(0);
    expect(page.cashEstimate()).toBe('$5.00');
    expect(page.positionChart()[0].points.at(-1)?.value).toBe(5);
    me.set({ ...user, id: 'user-2', account: 'account-2' });
    await fixture.whenStable();
    expect(page.customRate()).toBeNull();
    me.set(user);
    await fixture.whenStable();
    expect(page.customRate()).toBe(0);
    TestBed.inject(Money).epoch.set(1);
    await fixture.whenStable();
    expect(page.customRate()).toBeNull();
  });
  it('pages through retained history rather than treating a server page cap as complete history', async () => {
    const { fixture, history } = await setup();
    history
      .mockResolvedValueOnce({
        balance: 10000,
        account: 'account-1',
        transactions: [],
        next_cursor: 'page-2',
      })
      .mockResolvedValueOnce({
        balance: 10000,
        account: 'account-1',
        transactions: [],
        next_cursor: null,
      });
    fixture.componentInstance.data.reload();
    await fixture.whenStable();
    expect(history).toHaveBeenLastCalledWith('account-1', 100, 'page-2');
  });
});
