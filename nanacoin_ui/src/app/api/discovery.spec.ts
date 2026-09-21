import { afterEach, describe, expect, it, vi } from 'vitest';
import { candidates, Discovery, isNanacoinStatus, KNOWN_HOSTS } from './discovery';

const valid = { provisioned: true, household: 'Home', users: 2, ledger_balanced: true };
afterEach(() => { vi.unstubAllGlobals(); vi.useRealTimers(); });

describe('API discovery', () => {
  it('never downgrades discovery from an HTTPS page after TLS failure', async () => {
    vi.stubGlobal('location', { protocol: 'https:' });
    const fetcher = vi.fn().mockRejectedValue(new Error('TLS'));
    vi.stubGlobal('fetch', fetcher);
    expect(await new Discovery().find(['https://board/api/v1', 'http://board/api/v1'],
      new AbortController().signal, () => {})).toBeNull();
    expect(fetcher).toHaveBeenCalledTimes(1);
  });
  it('tries both protocols for every known host and saved/deployment addresses, once each', () => {
    const urls = candidates('http://192.168.1.158/api/v1', 'http://example.local');
    expect(urls[0]).toBe('https://192.168.1.158/api/v1');
    expect(new Set(urls).size).toBe(urls.length);
    for (const host of [...KNOWN_HOSTS, 'example.local']) {
      expect(urls).toContain(`https://${host}/api/v1`);
      expect(urls).toContain(`http://${host}/api/v1`);
    }
    expect(candidates('/api/v1')).not.toContain('http:///api/v1');
  });

  it('rejects generic JSON and SPA pages', () => {
    expect(isNanacoinStatus({ ok: true })).toBe(false);
    expect(isNanacoinStatus('<html></html>')).toBe(false);
    expect(isNanacoinStatus(valid)).toBe(true);
  });

  it('falls back from HTTPS to HTTP without credentials or bearer tokens', async () => {
    const fetcher = vi.fn().mockRejectedValueOnce(new Error('TLS'))
      .mockResolvedValueOnce({ ok: true, json: async () => valid });
    vi.stubGlobal('fetch', fetcher);
    const bases = ['https://board/api/v1', 'http://board/api/v1'];
    expect(await new Discovery().find(bases, new AbortController().signal, () => {})).toBe(bases[1]);
    expect(fetcher.mock.calls[1][1].credentials).toBe('omit');
    expect(fetcher.mock.calls[1][1].headers).toBeUndefined();
  });

  it('does not accept a non-NanaCoin response', async () => {
    vi.stubGlobal('fetch', vi.fn().mockResolvedValue({ ok: true, json: async () => ({ hello: 'world' }) }));
    expect(await new Discovery().find(['http://board'], new AbortController().signal, () => {})).toBeNull();
  });

  it('times out a stalled probe and continues', async () => {
    vi.useFakeTimers();
    const fetcher = vi.fn().mockImplementationOnce((_url, options) => new Promise((_resolve, reject) => {
      options.signal.addEventListener('abort', () => reject(new Error('aborted')));
    })).mockResolvedValueOnce({ ok: true, json: async () => valid });
    vi.stubGlobal('fetch', fetcher);
    const result = new Discovery().find(['https://board', 'http://board'], new AbortController().signal, () => {});
    await vi.advanceTimersByTimeAsync(3000);
    expect(await result).toBe('http://board');
    expect(vi.getTimerCount()).toBe(0);
  });

  it('stops on cancellation without trying another address', async () => {
    const controller = new AbortController();
    vi.stubGlobal('fetch', vi.fn().mockImplementation((_url, options) => new Promise((_resolve, reject) => {
      options.signal.addEventListener('abort', () => reject(new Error('aborted')));
    })));
    const result = new Discovery().find(['https://board', 'http://board'], controller.signal, () => {});
    controller.abort();
    expect(await result).toBeNull();
    expect(fetch).toHaveBeenCalledTimes(1);
  });
});
