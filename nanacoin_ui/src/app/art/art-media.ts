import { IS_DEMO } from '../demo/demo';
import { digestSha256 } from '../api/sha256';

/**
 * Where the showcase's own sample art is published. The demo seeds editions
 * with these real HTTPS locators, then serves the same files from its own
 * origin so the static showcase never contacts another site.
 */
export const DEMO_ART_BASE = 'https://matthewdeanmartin.github.io/nanacoin/art/';

/**
 * The URL an <img> may load without asking, or null when the viewer must opt in.
 *
 * A picture on someone else's server tells that server who looked at it and
 * when, so off-site media waits for a click. Same-site media is already trusted.
 */
export function autoSource(locator: string, base = document.baseURI): string | null {
  if (IS_DEMO && locator.startsWith(DEMO_ART_BASE)) return new URL(`art/${locator.slice(DEMO_ART_BASE.length)}`, base).href;
  try {
    const url = new URL(locator);
    return url.protocol === 'https:' && url.origin === new URL(base).origin ? url.href : null;
  } catch { return null; }
}

/** The host a viewer is asked about before loading off-site media. */
export function mediaHost(locator: string): string {
  try { return new URL(locator).host; } catch { return 'another site'; }
}

export async function sha256Hex(bytes: ArrayBuffer | Uint8Array): Promise<string> {
  const digest = await digestSha256(bytes instanceof Uint8Array ? bytes : new Uint8Array(bytes));
  return Array.from(digest, (b) => b.toString(16).padStart(2, '0')).join('');
}

/**
 * Fetches media and hashes it. Many hosts refuse cross-site reads (CORS); that
 * is "could not check", not "does not match".
 */
export async function digestOf(url: string): Promise<string | null> {
  try {
    const response = await fetch(url, { credentials: 'omit', referrerPolicy: 'no-referrer' });
    return response.ok ? await sha256Hex(await response.arrayBuffer()) : null;
  } catch { return null; }
}

/**
 * A stand-in picture drawn from the edition's digest: distinct per edition,
 * made locally, and never a network request.
 */
export function digestPattern(sha256: string): { hue: number; cells: boolean[] } {
  const hex = /^[0-9a-f]{64}$/i.test(sha256) ? sha256.toLowerCase() : '0'.repeat(64);
  const bits = hex.slice(0, 15).split('').map((c) => parseInt(c, 16) % 2 === 1);
  // 5x5, mirrored left-right like a quilt square: columns 0-2 come from bits.
  const cells: boolean[] = [];
  for (let row = 0; row < 5; row++) for (let col = 0; col < 5; col++) cells.push(bits[row * 3 + (col < 3 ? col : 4 - col)]);
  return { hue: parseInt(hex.slice(16, 20), 16) % 360, cells };
}
