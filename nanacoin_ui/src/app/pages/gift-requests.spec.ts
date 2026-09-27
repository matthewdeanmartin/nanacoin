import { GiftRequest } from '../api/models';
import { deadlineFromDate, requestState } from './gift-requests';

const request = (patch: Partial<GiftRequest> = {}): GiftRequest => ({
  id: 1, owner: 2, title: 'Paints', description: '', target: null, deadline: null, received: 0, closed: false, created_at: 0, ...patch,
});

describe('gift requests', () => {
  it('treats closed and past-deadline requests as no longer open', () => {
    expect(requestState(request(), 100)).toBe('open');
    expect(requestState(request({ deadline: 101 }), 100)).toBe('open');
    expect(requestState(request({ deadline: 100 }), 100)).toBe('ended');
    expect(requestState(request({ deadline: 50, closed: true }), 100)).toBe('closed');
  });
  it('turns a chosen day into the last second of that local day', () => {
    expect(deadlineFromDate('')).toBeNull();
    const end = new Date(deadlineFromDate('2026-10-02')! * 1000);
    expect([end.getFullYear(), end.getMonth(), end.getDate(), end.getHours(), end.getMinutes()]).toEqual([2026, 9, 2, 23, 59]);
  });
});
