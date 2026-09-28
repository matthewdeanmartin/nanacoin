/**
 * The household bank that served this page, for links back to itself.
 *
 * A household can run two banks (nanacoin.local on the S3 and
 * nanacoin-s2.local on the S2). The same app is embedded in both, so a
 * trust or invite link must name the board the family is actually using,
 * never a fixed hostname that belongs to the other bank. Development
 * servers and other hosts fall back to the first bank.
 */
export const DEFAULT_BANK_HOST = 'nanacoin.local';

export function bankHost(hostname: string = globalThis.location?.hostname ?? ''): string {
  return /^nanacoin(-[a-z0-9]+)?\.local$/i.test(hostname) ? hostname.toLowerCase() : DEFAULT_BANK_HOST;
}
