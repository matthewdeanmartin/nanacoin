import { HttpRequest, HttpResponse } from '@angular/common/http';
import { firstValueFrom } from 'rxjs';
import { demoBackend, demoLedger } from './demo-backend';

describe('demo profile reads', () => {
  it('lets a member read another member\'s money movements but not their messages', async () => {
    let token = '';
    const send = async (path: string, body?: unknown) => {
      const req = new HttpRequest(body === undefined ? 'GET' : 'POST', `/api/v1${path}`, body ?? null);
      const event = await firstValueFrom(demoBackend(req.clone({ setHeaders: { Authorization: `Bearer ${token}` } }), () => { throw new Error('Demo request escaped to the network'); }));
      return (event as HttpResponse<any>).body;
    };
    const signIn = async (username: string) => { token = (await send('/auth/token', { code: (await send('/auth/authorize', { username })).code })).access_token; };
    await signIn('sam');
    const mom = demoLedger.userByName('mom')!, ivy = demoLedger.userByName('ivy')!;
    demoLedger.transfer(mom, ivy.account, 0, 'Secret birthday plan');
    const history = await send(`/accounts/${ivy.account}/transactions?limit=100`);
    expect(history.transactions.length).toBeGreaterThan(0);
    expect(JSON.stringify(history)).not.toContain('Secret birthday plan');
    expect((await send('/users')).users.find((u: { id: string }) => u.id === ivy.id).balance).toBe(demoLedger.balanceOf(ivy.account));
    const offers = (await send(`/offers?member=${ivy.id}`)).offers as { offerer: string; status: string; listing_owner?: string; message: string }[];
    expect(offers.length).toBeGreaterThan(0);
    for (const o of offers) {
      expect([o.offerer, o.status]).toEqual([ivy.account, 'OPEN']);
      if (o.listing_owner !== demoLedger.userByName('sam')!.account) expect(o.message).toBe('');
    }
    await signIn('ivy');
    expect(JSON.stringify(await send(`/accounts/${ivy.account}/transactions?limit=100`))).toContain('Secret birthday plan');
  });
});

describe('demo profile loans and lotto', () => {
  it('answers member queries like the board', async () => {
    let token = '';
    const send = async (path: string, body?: unknown, method?: string) => {
      const req = new HttpRequest((method ?? (body === undefined ? 'GET' : 'POST')) as 'GET', `/api/v1${path}`, body ?? null);
      const key: Record<string, string> = method === 'PATCH' ? { 'Idempotency-Key': `demo:e${demoLedger.moneyEpoch}:bio` } : {};
      const event = await firstValueFrom(demoBackend(req.clone({ setHeaders: { Authorization: `Bearer ${token}`, ...key } }), () => { throw new Error('escaped'); }));
      return (event as HttpResponse<any>).body;
    };
    token = (await send('/auth/token', { code: (await send('/auth/authorize', { username: 'ivy' })).code })).access_token;
    const sam = demoLedger.userByName('sam')!;
    const loans = (await send(`/loans?member=${sam.id}`)).loans as { status: string; lender: string; borrower: string; memo: string }[];
    for (const l of loans) {
      expect(['ACTIVE', 'PAID']).toContain(l.status);
      expect([l.lender, l.borrower]).toContain(sam.account);
    }
    const draws = (await send(`/lottos?member=${sam.id}`)).lottos as { my_tickets: number }[];
    expect(draws.length).toBeGreaterThan(0);
    const ivy = demoLedger.userByName('ivy')!;
    expect((await send(`/users/${ivy.id}`, { bio: 'Painter' }, 'PATCH')).bio).toBe('Painter');
  });
});
