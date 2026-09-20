// ApiBase decides which NanaCoin the site talks to. It is the one piece of
// configuration a household member actually has to touch - the board's address
// comes from DHCP, so it moves - which makes it worth testing properly.

import { TestBed } from '@angular/core/testing';

import { ApiBase, normalise } from './api-base';

const STORAGE_KEY = 'nanacoin.apiBase';

describe('normalise', () => {
  it('accepts a bare host and fills in the scheme and path', () => {
    expect(normalise('192.168.1.158')).toBe('http://192.168.1.158/api/v1');
  });

  it('accepts a host with a scheme', () => {
    expect(normalise('http://nanacoin.local')).toBe('http://nanacoin.local/api/v1');
  });

  it('leaves a full API path alone', () => {
    expect(normalise('http://192.168.1.158/api/v1')).toBe('http://192.168.1.158/api/v1');
  });

  it('strips a trailing slash rather than producing a double one', () => {
    expect(normalise('http://192.168.1.158/')).toBe('http://192.168.1.158/api/v1');
  });

  it('keeps a port', () => {
    expect(normalise('192.168.1.158:8080')).toBe('http://192.168.1.158:8080/api/v1');
  });

  it('ignores surrounding whitespace, which pasted text often carries', () => {
    expect(normalise('  192.168.1.158  ')).toBe('http://192.168.1.158/api/v1');
  });

  it('returns empty for empty input, so callers can fall back', () => {
    expect(normalise('   ')).toBe('');
  });
});

describe('ApiBase', () => {
  let apiBase: ApiBase;

  beforeEach(() => {
    localStorage.removeItem(STORAGE_KEY);
    TestBed.configureTestingModule({ providers: [ApiBase] });
    apiBase = TestBed.inject(ApiBase);
  });

  afterEach(() => {
    localStorage.removeItem(STORAGE_KEY);
  });

  it('starts on this page origin', () => {
    expect(apiBase.current()).toBe('/api/v1');
    expect(apiBase.isRemote()).toBe(false);
    // Same-origin labels as this page's own host: it reads correctly inside
    // "could not reach NanaCoin at ...", which is where it is used.
    expect(apiBase.label()).toBe(location.host);
  });

  it('points at a typed address and reports it as remote', () => {
    apiBase.set('192.168.1.158');

    expect(apiBase.current()).toBe('http://192.168.1.158/api/v1');
    expect(apiBase.isRemote()).toBe(true);
    // The label is the host alone: the scheme and the /api/v1 suffix were
    // never typed, so echoing them back would only be noise.
    expect(apiBase.label()).toBe('192.168.1.158');
  });

  it('remembers the choice', () => {
    apiBase.set('192.168.1.158');
    expect(localStorage.getItem(STORAGE_KEY)).toBe('http://192.168.1.158/api/v1');
  });

  it('returns to this page origin and forgets the choice on reset', () => {
    apiBase.set('192.168.1.158');
    apiBase.reset();

    expect(apiBase.current()).toBe('/api/v1');
    expect(apiBase.isRemote()).toBe(false);
    expect(localStorage.getItem(STORAGE_KEY)).toBeNull();
  });

  it('falls back to this page origin when given blank input', () => {
    apiBase.set('192.168.1.158');
    apiBase.set('  ');

    expect(apiBase.current()).toBe('/api/v1');
  });

  it('reports the address it was given back, for the connect screen to confirm', () => {
    expect(apiBase.set('192.168.1.158')).toBe('http://192.168.1.158/api/v1');
  });
});

describe('the remembered address', () => {
  const originalSearch = location.search;

  function withQuery(search: string, fn: () => void) {
    // ApiBase reads location.search in its constructor, so the test drives it
    // through the real URL rather than a seam that would prove less.
    history.replaceState({}, '', search ? `?${search}` : location.pathname);
    try {
      fn();
    } finally {
      history.replaceState({}, '', originalSearch || location.pathname);
    }
  }

  /** A fresh service, so its constructor re-reads the URL and storage. */
  function freshBase(): ApiBase {
    TestBed.resetTestingModule();
    TestBed.configureTestingModule({ providers: [ApiBase] });
    return TestBed.inject(ApiBase);
  }

  beforeEach(() => {
    localStorage.removeItem(STORAGE_KEY);
    document.querySelector('meta[name="nanacoin-api"]')?.remove();
  });

  afterEach(() => {
    localStorage.removeItem(STORAGE_KEY);
    document.querySelector('meta[name="nanacoin-api"]')?.remove();
  });

  it('is taken from ?api= when present', () => {
    withQuery('api=192.168.1.158', () => {
      expect(freshBase().current()).toBe('http://192.168.1.158/api/v1');
    });
  });

  it('survives a reload, so the query string is only typed once', () => {
    withQuery('api=192.168.1.158', () => freshBase());
    withQuery('', () => {
      expect(freshBase().current()).toBe('http://192.168.1.158/api/v1');
    });
  });

  it('is cleared by an empty ?api=, returning to this page origin', () => {
    withQuery('api=192.168.1.158', () => freshBase());
    withQuery('api=', () => {
      expect(freshBase().current()).toBe('/api/v1');
    });
    withQuery('', () => {
      expect(freshBase().current()).toBe('/api/v1');
    });
  });

  it('falls back to the meta tag when nothing is remembered', () => {
    const meta = document.createElement('meta');
    meta.setAttribute('name', 'nanacoin-api');
    meta.setAttribute('content', 'http://board.example');
    document.head.append(meta);

    withQuery('', () => {
      expect(freshBase().current()).toBe('http://board.example/api/v1');
    });
  });

  it('prefers a remembered address over the meta tag', () => {
    const meta = document.createElement('meta');
    meta.setAttribute('name', 'nanacoin-api');
    meta.setAttribute('content', 'http://board.example');
    document.head.append(meta);
    localStorage.setItem(STORAGE_KEY, 'http://192.168.1.158/api/v1');

    withQuery('', () => {
      expect(freshBase().current()).toBe('http://192.168.1.158/api/v1');
    });
  });
});
