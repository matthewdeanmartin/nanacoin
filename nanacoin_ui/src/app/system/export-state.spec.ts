import { ServerState, exportFileName, exportHtml } from './export-state';

const f = { coins: (n: number) => (n / 10_000).toFixed(2), date: (s: number) => new Date(s * 1000).toISOString() };
function state(extra: Partial<ServerState> = {}): ServerState {
  return {
    exportedAt: 1_790_000_000, source: 'https://nanacoin.local',
    status: { household: 'Martin House', currency: 'NanaCoin', decimals: 4, circulation: 2_000_000 } as never,
    config: { household_name: 'Martin House', initial_grant: 1_000_000, currency: 'NanaCoin', offer_settles_after: 172_800 },
    transport: { https_only: true, supported: true },
    users: [
      { id: 'user-1', account: 'account-1', username: 'nana', display_name: 'Nana', role: 'nana', status: 'ACTIVE', balance: 1_500_000, usd_cents: 8250, created_at: 0 },
      { id: 'user-2', account: 'account-2', username: 'kid', display_name: 'Kid <script>', role: 'user', status: 'ACTIVE', balance: 500_000, created_at: 0 },
    ],
    listings: [{ id: 'l1', seller: 'account-1', seller_name: 'Nana', title: 'Wash the car', description: '', price: 350_000, status: 'ACTIVE', created_at: 0, updated_at: 0 }],
    things: [], loans: [], lottos: [], quotes: [], offers: [], fulfillments: [],
    allowances: [{ id: 'a', ownerId: 'user-1', recipientAccount: 'account-2', recipientName: 'Kid', amount: 50_000, cadence: 'WEEKLY', nextDue: '2026-10-01', memo: 'Allowance' }],
    missing: [],
    ...extra,
  };
}

describe('server state export', () => {
  it('is a standalone page with what Nana must re-enter', () => {
    const html = exportHtml(state(), f);
    expect(html.startsWith('<!doctype html>')).toBe(true);
    expect(html).toContain('Martin House: server state');
    expect(html).toContain('<td>150.00</td>');
    expect(html).toContain('$82.50');
    expect(html).toContain('Wash the car');
    expect(html).toContain('HTTPS required for everyone</td><td>Yes');
    expect(html).toContain('Passwords are not exported');
    expect(html).toContain('weekly');
  });

  it('escapes names and keeps the embedded data inert', () => {
    const html = exportHtml(state(), f);
    expect(html).toContain('Kid &lt;script&gt;');
    const embedded = html.split('<script type="application/json" id="nanacoin-export">')[1].split('</script>')[0];
    expect(embedded).not.toContain('<');
    expect(JSON.parse(embedded).users[1].display_name).toBe('Kid <script>');
  });

  it('says what could not be read', () => {
    expect(exportHtml(state({ missing: ['loans'] }), f)).toContain('Could not read: loans');
  });

  it('names the file after the household and date', () => {
    expect(exportFileName('Martin House', 1_790_000_000)).toBe('martin-house-export-2026-09-21.html');
  });
});
