import { offerUrl } from './offer-qr';

describe('shared offer URLs', () => {
  it('retains the Pages base path and chooses the buying section', () => {
    const url = new URL(offerUrl('/market', 'listing with & signs', 'market-buy',
      'https://example.com/nanacoin/?token=private#/old'));
    expect(url.pathname).toBe('/nanacoin/');
    expect(url.search).toBe('');
    const [path, query] = url.hash.slice(1).split('?');
    expect(path).toBe('/market');
    expect(new URLSearchParams(query).get('offer')).toBe('listing with & signs');
    expect(new URLSearchParams(query).get('tab')).toBe('market-buy');
  });
  it('uses the board origin when the board serves its own client', () => {
    expect(offerUrl('/loans', 123, undefined, 'http://nanacoin.local/', '/api/v1'))
      .toBe('http://nanacoin.local/#/loans?offer=123');
  });
  it('points a separately hosted client at the same board without sharing credentials', () => {
    const url = new URL(offerUrl('/forex', 'quote-1', undefined,
      'https://example.com/nanacoin/?oauth=secret', 'http://user:password@192.168.1.10/api/v1?token=secret#secret'));
    expect(url.searchParams.get('api')).toBe('http://192.168.1.10/api/v1');
    expect(url.href).not.toContain('secret');
    expect(url.href).not.toContain('password');
    expect(url.hash).toBe('#/forex?offer=quote-1');
  });
});
