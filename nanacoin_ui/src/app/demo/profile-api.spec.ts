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
