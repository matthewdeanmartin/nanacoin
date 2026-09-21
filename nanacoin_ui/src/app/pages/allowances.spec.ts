import { advanceDue, loadAllowances, saveAllowances } from './allowances';

describe('allowance schedules', () => {
  beforeEach(() => localStorage.clear());

  it('advances weekly and clamps monthly dates to the end of the month', () => {
    expect(advanceDue('2026-01-10', 'WEEKLY')).toBe('2026-01-17');
    expect(advanceDue('2026-01-31', 'MONTHLY')).toBe('2026-02-28');
  });

  it('round trips browser-owned schedules', () => {
    const allowance = { id: 'a1', ownerId: 'u1', recipientAccount: 'a2', recipientName: 'Sam', amount: 5, cadence: 'WEEKLY' as const, nextDue: '2026-01-10', memo: 'Allowance' };
    saveAllowances([allowance]);
    expect(loadAllowances()).toEqual([allowance]);
  });
});
