import { describe, expect, it } from 'vitest';
import { bankHost, DEFAULT_BANK_HOST } from './bank-host';

describe('bank host', () => {
  it('links each bank back to itself', () => {
    expect(bankHost('nanacoin.local')).toBe('nanacoin.local');
    expect(bankHost('nanacoin-s2.local')).toBe('nanacoin-s2.local');
    expect(bankHost('NanaCoin-S2.local')).toBe('nanacoin-s2.local');
  });

  it('falls back to the first bank for development and unrelated hosts', () => {
    for (const host of ['localhost', '127.0.0.1', '192.168.1.158', 'example.com', 'nanacoin.local.evil.com', '']) {
      expect(bankHost(host)).toBe(DEFAULT_BANK_HOST);
    }
  });
});
