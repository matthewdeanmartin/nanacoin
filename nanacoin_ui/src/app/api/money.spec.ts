import { moneyText, parseMoney, MAX_MONEY } from './money';

describe('exact decimal money', () => {
  it('round trips four-place fractions and large amounts without floating arithmetic', () => {
    for (const amount of [1, 101, 10001, MAX_MONEY - 1, MAX_MONEY]) {
      expect(parseMoney(moneyText(amount, 4, 'en-US', false), 4)).toBe(amount);
    }
    expect(parseMoney('0.0001', 4)).toBe(1);
  });
  it('supports decimal commas and native digits', () => {
    expect(parseMoney('12,3456', 4, 'de-DE')).toBe(123456);
    expect(parseMoney('١٢٫٣٤٥٦', 4, 'ar-EG')).toBe(123456);
    expect(moneyText(123456, 4, 'de-DE')).toBe('12,3456');
  });
  it('refuses silently rounded, negative, grouped, and oversized input', () => {
    for (const input of ['0.00001', '-1', '1e4', '1,000', '100000000000.0001']) {
      expect(() => parseMoney(input, 4)).toThrow();
    }
    expect(() => moneyText(1.1, 4)).toThrow();
  });
});
