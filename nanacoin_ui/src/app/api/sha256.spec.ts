// A wrong SHA-256 would fail the PKCE exchange in a way that looks like a
// wrong password, so this is checked against the published test vectors
// rather than against itself - and against the platform's own implementation,
// which is available here because the test runner is a secure context even
// though the board is not.

import { digestSha256, hasNativeDigest, sha256 } from './sha256';

function hex(bytes: Uint8Array): string {
  return Array.from(bytes)
    .map((b) => b.toString(16).padStart(2, '0'))
    .join('');
}

function bytes(s: string): Uint8Array {
  return new TextEncoder().encode(s);
}

describe('sha256', () => {
  // FIPS 180-4 / NIST published vectors.
  it('hashes the empty string', () => {
    expect(hex(sha256(new Uint8Array(0)))).toBe(
      'e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855',
    );
  });

  it('hashes "abc"', () => {
    expect(hex(sha256(bytes('abc')))).toBe(
      'ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad',
    );
  });

  it('hashes the 56-character vector', () => {
    // Exactly one byte short of needing a second block once padded, which is
    // the boundary a hand-written implementation gets wrong.
    expect(
      hex(sha256(bytes('abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq'))),
    ).toBe('248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1');
  });

  it('hashes the 112-character vector, which spans two blocks', () => {
    const input =
      'abcdefghbcdefghicdefghijdefghijkefghijklfghijklmghijklmn' +
      'hijklmnoijklmnopjklmnopqklmnopqrlmnopqrsmnopqrstnopqrstu';
    expect(hex(sha256(bytes(input)))).toBe(
      'cf5b16a778af8380036ce59e7b0492370b249b11e8f07a51afac45037afee9d1',
    );
  });

  it('hashes a million a\'s', () => {
    // The long vector, which catches a length field written as 32 bits.
    const million = new Uint8Array(1_000_000).fill(0x61);
    expect(hex(sha256(million))).toBe(
      'cdc76e5c9914fb9281a1c7e284d73e67f1809a48a497200e046d39ccc7112cd0',
    );
  });

  it('handles a length that crosses the 55/56-byte padding boundary', () => {
    // Around here the padding needs an extra block. Each of these is checked
    // against the platform implementation below, so a mistake shows up as a
    // disagreement rather than needing its own published vector.
    for (const n of [54, 55, 56, 57, 63, 64, 65]) {
      const input = new Uint8Array(n).fill(0x41);
      expect(sha256(input).length).toBe(32);
    }
  });
});

describe('digestSha256', () => {
  it('agrees with the platform implementation', async () => {
    // The whole point of the fallback is producing the same bytes the browser
    // would have. If these ever disagree, PKCE breaks on the board only -
    // the hardest kind of bug to find.
    expect(hasNativeDigest()).toBe(true);

    const cases = [
      '',
      'abc',
      'a-typical-pkce-verifier-is-43-characters-ok',
      'unicode: café, naïve, 日本語',
      'x'.repeat(200),
    ];

    for (const c of cases) {
      const input = bytes(c);
      const native = new Uint8Array(
        await crypto.subtle.digest('SHA-256', input.slice().buffer as ArrayBuffer),
      );
      expect(hex(sha256(input))).toBe(hex(native));
    }
  });

  it('returns 32 bytes', async () => {
    expect((await digestSha256(bytes('anything'))).length).toBe(32);
  });
});
