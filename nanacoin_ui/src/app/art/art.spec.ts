import { autoSource, DEMO_ART_BASE, digestPattern, mediaHost, sha256Hex } from './art-media';
import { mintProblem } from '../pages/art';

describe('digital art', () => {
  const digest = 'ab'.repeat(32);
  it('checks an edition the way the board will', () => {
    expect(mintProblem('Moon', 'Display', 'https://example.org/moon.png', digest)).toBeNull();
    expect(mintProblem(' ', 'Display', 'https://example.org/moon.png', digest)).toContain('title');
    expect(mintProblem('Moon', 'Display', 'http://example.org/moon.png', digest)).toContain('https://');
    expect(mintProblem('Moon', 'Display', 'https://example.org/moon.png', 'abc')).toContain('64');
    expect(mintProblem('Moon', 'x'.repeat(97), 'https://example.org/moon.png', digest)).toContain('96');
  });
  it('loads only same-site pictures without asking', () => {
    expect(autoSource('https://board.example/art/a.png', 'https://board.example/app/')).toBe('https://board.example/art/a.png');
    expect(autoSource('https://tracker.example/a.png', 'https://board.example/app/')).toBeNull();
    expect(autoSource('not a url', 'https://board.example/')).toBeNull();
    expect(DEMO_ART_BASE.startsWith('https://')).toBe(true);
    expect(mediaHost('https://tracker.example/a.png')).toBe('tracker.example');
  });
  it('hashes bytes and draws a stable mirrored pattern', async () => {
    expect(await sha256Hex(new TextEncoder().encode('abc'))).toBe('ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad');
    const p = digestPattern(digest);
    expect(p).toEqual(digestPattern(digest));
    expect(p.cells).toHaveLength(25);
    for (let row = 0; row < 5; row++) expect(p.cells[row * 5]).toBe(p.cells[row * 5 + 4]);
  });
});
