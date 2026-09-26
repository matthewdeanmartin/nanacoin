import { TestBed } from '@angular/core/testing';
import { provideHttpClient } from '@angular/common/http';
import { provideHttpClientTesting } from '@angular/common/http/testing';
import { provideRouter } from '@angular/router';
import { ApiBase } from '../api/api-base';
import { NanacoinService } from '../api/nanacoin.service';
import { Session } from '../api/session';
import { Status } from '../api/models';
import { Dialogs } from '../ui/dialog';
import { MarketMakerPage } from './market-maker';

afterEach(() => TestBed.resetTestingModule());
describe('market desk', () => {
  it('previews the dollar ladder and runs it step by step', async () => {
    TestBed.configureTestingModule({ providers: [provideHttpClient(), provideHttpClientTesting(), provideRouter([]), ApiBase] });
    const api = TestBed.inject(NanacoinService), session = TestBed.inject(Session);
    session.me.set({ id: 'user-1', account: 'account-1', username: 'nana', display_name: 'Nana', role: 'nana', status: 'ACTIVE', balance: 50_000_000, usd_cents: 100_000 } as never);
    session.status.set({ circulation: 10_000_000, sequence: 1 } as Status);
    session.refresh = () => Promise.resolve();
    api.quotes = () => Promise.resolve({ quotes: [] });
    api.loans = () => Promise.resolve({ loans: [] } as never);
    api.lottos = () => Promise.resolve({ lottos: [] } as never);
    const posted: number[] = [];
    api.postQuote = async (q) => { posted.push(q.cents_per_coin); return {} as never; };
    vi.spyOn(TestBed.inject(Dialogs), 'confirm').mockResolvedValue('');
    const fixture = TestBed.createComponent(MarketMakerPage);
    fixture.detectChanges(); await fixture.whenStable(); fixture.detectChanges();
    const root = fixture.nativeElement as HTMLElement;
    expect(root.textContent).toContain('$82.50');
    const go = Array.from(root.querySelectorAll('button')).find((b) => b.textContent?.includes('Do these 7 things'))!;
    go.click(); await fixture.whenStable(); await new Promise((r) => setTimeout(r)); fixture.detectChanges();
    expect(posted).toEqual([100, 75, 25, 1, 150, 200, 300]);
    expect(root.textContent).toContain('✓');
  });
});
