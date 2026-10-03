import { HttpErrorResponse, HttpRequest, HttpResponse } from '@angular/common/http';
import { firstValueFrom } from 'rxjs';
import { demoBackend, demoLedger } from './demo-backend';

/** The demo answers bot members and scoped keys like the board. */
describe('demo bot members and API keys', () => {
  let token = '';
  let keys = 0;
  const send = async (path: string, body?: unknown, method?: string): Promise<{ status: number; body: any }> => {
    const verb = method ?? (body === undefined ? 'GET' : 'POST');
    const req = new HttpRequest(verb as 'GET', `/api/v1${path}`, body ?? null);
    const key: Record<string, string> = verb === 'GET' ? {} : { 'Idempotency-Key': `demo:e${demoLedger.moneyEpoch}:bot${++keys}` };
    try {
      const event = await firstValueFrom(demoBackend(req.clone({ setHeaders: { Authorization: `Bearer ${token}`, ...key } }), () => {
        throw new Error('Demo request escaped to the network');
      }));
      const res = event as HttpResponse<any>;
      return { status: res.status, body: res.body };
    } catch (e) {
      if (e instanceof HttpErrorResponse) return { status: e.status, body: e.error };
      throw e;
    }
  };
  const signIn = async (username: string) => {
    token = (await send('/auth/token', { code: (await send('/auth/authorize', { username })).body.code })).body.access_token;
  };

  it('lets Nana add a bot and make its key, never a person\'s', async () => {
    await signIn('nana');
    const bot = (await send('/users', { username: 'trader', display_name: 'Trader', password: 'x', grant: false, kind: 'bot' })).body;
    expect(bot.kind).toBe('bot');
    const made = (await send(`/users/${bot.id}/api-key`, {})).body;
    expect(made.api_key).toMatch(/^nc_/);
    expect((await send(`/users/${bot.id}/api-key`)).body.full.active).toBe(true);
    const users = (await send('/users')).body.users as { id: string; kind: string }[];
    expect(users.find((u) => u.id === bot.id)?.kind).toBe('bot');
    const person = users.find((u) => u.kind === 'human' && u.id !== bot.id)!;
    expect((await send(`/users/${person.id}/api-key`, {})).status).toBe(403);
  });

  it('keeps a member\'s full and read-only keys apart', async () => {
    await signIn('ivy');
    const read = (await send('/me/api-key', { password: 'demo', scope: 'read' })).body;
    expect(read.scope).toBe('read');
    let status = (await send('/me/api-key')).body;
    expect([status.full.active, status.read.active]).toEqual([false, true]);
    await send('/me/api-key', { password: 'demo' });
    status = (await send('/me/api-key?scope=read', undefined, 'DELETE')).body;
    expect([status.active, status.full.active, status.read.active]).toEqual([true, true, false]);
  });
});
