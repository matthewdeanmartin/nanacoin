import { toggleCaps } from './message-caps';

describe('ALL CAPS drafts', () => {
  it('restores the exact pre-checkbox draft when unchecked', () => {
    const on = toggleCaps('Pay me Saturday?', false, true, '');
    expect(on).toEqual({ text: 'PAY ME SATURDAY?', saved: 'Pay me Saturday?' });
    // Even if the all-caps version was edited, off means the original draft.
    expect(toggleCaps('PAY ME SUNDAY!', true, false, on.saved).text).toBe('Pay me Saturday?');
  });
});
