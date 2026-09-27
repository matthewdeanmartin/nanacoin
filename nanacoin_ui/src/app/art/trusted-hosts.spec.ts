import { TrustedArtHosts } from './trusted-hosts';

describe('trusted picture sites', () => {
  beforeEach(() => localStorage.removeItem('nanacoin:trusted-art-hosts'));
  it('remembers sites in this browser and forgets them on request', () => {
    const hosts = new TrustedArtHosts();
    hosts.trust('art.example'); hosts.trust('art.example'); hosts.trust('a.example');
    expect(hosts.hosts()).toEqual(['a.example', 'art.example']);
    expect(new TrustedArtHosts().has('art.example')).toBe(true);
    hosts.forget('art.example');
    expect(new TrustedArtHosts().hosts()).toEqual(['a.example']);
  });
  it('ignores damaged storage', () => {
    localStorage.setItem('nanacoin:trusted-art-hosts', '{"nope":1}');
    expect(new TrustedArtHosts().hosts()).toEqual([]);
    localStorage.setItem('nanacoin:trusted-art-hosts', '["ok.example","javascript:alert(1)"]');
    expect(new TrustedArtHosts().hosts()).toEqual(['ok.example']);
  });
});
