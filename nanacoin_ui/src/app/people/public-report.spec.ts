import { Transaction, User } from '../api/models';
import { population, publicReport } from './public-report';

const tx = (id: number, kind: Transaction['kind'], postings: [string, string, number][], economic_kind?: Transaction['economic_kind']): Transaction => ({
  id: `tx-${id}`, kind, created_at: 1_000 + id, actor: 'user-1', description: `t${id}`, economic_kind,
  postings: postings.map(([account, name, amount]) => ({ account, name, amount })),
});

describe('public report', () => {
  const ivy = 'account-5';
  const history = [
    tx(1, 'ISSUE', [['account:system-issuance', 'Issuance', -25], [ivy, 'Ivy', 25]]),
    tx(2, 'TRANSFER', [['account-3', 'Mom', -13], [ivy, 'Ivy', 13]], 'LABOR'),
    tx(3, 'TRANSFER', [[ivy, 'Ivy', -6], ['account-4', 'Sam', 6]], 'GOOD'),
    tx(4, 'TRANSFER', [['account-3', 'Mom', -3], [ivy, 'Ivy', 3]], 'GIFT'),
    tx(5, 'MESSAGE', [['account-3', 'Mom', 0], [ivy, 'Ivy', 0]]),
    tx(6, 'TRANSFER', [[ivy + '-usd', 'Ivy', -100], ['account-3-usd', 'Mom', 100]]),
  ];
  it('totals money in and out by kind and ignores messages and dollar postings', () => {
    const r = publicReport(ivy, history);
    expect(r.count).toBe(4);
    expect([r.received, r.paid]).toEqual([41, 6]);
    expect(r.rows).toEqual([
      { label: 'Work', received: 13, paid: 0 },
      { label: 'Goods', received: 0, paid: 6 },
      { label: 'Gifts', received: 3, paid: 0 },
      { label: 'From Nana (new money)', received: 25, paid: 0 },
    ]);
    expect([r.first, r.last]).toEqual([1_001, 1_004]);
  });
  it('ranks trading partners and leaves out system accounts', () => {
    expect(publicReport(ivy, history).partners.map((p) => [p.name, p.count])).toEqual([['Mom', 2], ['Sam', 1]]);
  });
  it('is empty for a quiet account', () => {
    expect(publicReport('account-9', history)).toMatchObject({ count: 0, first: null, rows: [], partners: [] });
  });
});

describe('population', () => {
  const user = (id: number, patch: Partial<User>): User => ({
    id: `user-${id}`, username: `u${id}`, display_name: `U${id}`, role: 'user', status: 'ACTIVE', account: `account-${id}`, created_at: id, ...patch,
  });
  it('counts members and summarizes active balances', () => {
    const p = population([user(1, { role: 'nana', balance: 100 }), user(2, { balance: 10 }), user(3, { balance: 30 }), user(4, { status: 'DISABLED', balance: 999 })]);
    expect(p).toMatchObject({ members: 4, active: 3, disabled: 1, nanas: 1, since: 1, total: 140, median: 30 });
    expect(p.newest?.id).toBe('user-4');
    expect(population([user(1, { balance: 10 }), user(2, { balance: 21 })]).median).toBe(15);
    expect(population([]).median).toBeNull();
  });
});
