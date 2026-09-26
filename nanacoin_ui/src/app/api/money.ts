import { Injectable, Pipe, PipeTransform, inject, signal } from '@angular/core';

export const MAX_MONEY = 1_000_000_000_000_000;

/**
 * Something the person typed that cannot be used. The message is written for
 * them (kids included), so the toast shows it as-is instead of the generic
 * "Something went wrong." reserved for real faults.
 */
export class InputError extends Error {
  override readonly name = 'InputError';
}

/** Decimal text to exact minor units. No floating-point multiplication. */
export function parseMoney(text: string, decimals: number, locale = 'en-US'): number {
  if (!Number.isInteger(decimals) || decimals < 0 || decimals > 8) throw new Error('Invalid currency precision.');
  const point = new Intl.NumberFormat(locale).formatToParts(1.1).find(p => p.type === 'decimal')?.value ?? '.';
  let input = text.trim();
  for (let digit = 0; digit <= 9; digit++) {
    const native = new Intl.NumberFormat(locale, { useGrouping: false }).format(digit);
    if (native !== String(digit)) input = input.split(native).join(String(digit));
  }
  if (point !== '.') {
    if (input.includes('.')) throw new InputError(`Use ${point} to split whole coins from parts of a coin. You typed "${text}".`);
    input = input.split(point).join('.');
  }
  if (!/^(0|[1-9]\d*)(?:\.\d+)?$/.test(input)) throw new InputError(`Type the amount as a plain number, like 12 or 3${point}50. Leave out commas, spaces, and signs like $ or %. You typed "${text}".`);
  const [whole, fraction = ''] = input.split('.');
  if (fraction.length > decimals) throw new InputError(decimals ? `Use no more than ${decimals} numbers after the ${point === '.' ? 'dot' : point}. You typed "${text}".` : `Use a whole number, with nothing after the ${point === '.' ? 'dot' : point}. You typed "${text}".`);
  const minor = BigInt(whole) * 10n ** BigInt(decimals) + BigInt(fraction.padEnd(decimals, '0') || '0');
  if (minor > BigInt(MAX_MONEY)) throw new InputError('That number is too big. Try a smaller one.');
  return Number(minor);
}

export function moneyText(value: number | string | bigint, decimals: number, locale = 'en-US', grouping = true): string {
  if (typeof value === 'number' && !Number.isSafeInteger(value)) throw new Error('Money must be an exact integer.');
  const amount = BigInt(value), scale = 10n ** BigInt(decimals);
  const magnitude = amount < 0n ? -amount : amount;
  const whole = new Intl.NumberFormat(locale, { useGrouping: grouping, maximumFractionDigits: 0 }).format(magnitude / scale);
  const fraction = (magnitude % scale).toString().padStart(decimals, '0').replace(/0+$/, '');
  const nativeFraction = [...fraction].map(c => new Intl.NumberFormat(locale, { useGrouping: false }).format(Number(c))).join('');
  const parts = new Intl.NumberFormat(locale).formatToParts(-1.1);
  return `${amount < 0n ? parts.find(p => p.type === 'minusSign')?.value ?? '-' : ''}${whole}${fraction ? (parts.find(p => p.type === 'decimal')?.value ?? '.') + nativeFraction : ''}`;
}

@Injectable({ providedIn: 'root' })
export class Money {
  readonly decimals = signal(4);
  readonly epoch = signal(0);
  readonly changed = signal(false);
  readonly locale = typeof navigator === 'undefined' ? 'en-US' : navigator.language;
  private source = '';
  update(decimals: number, epoch: number, source: string): void {
    if (!Number.isInteger(decimals) || decimals < 0 || decimals > 8 || !Number.isSafeInteger(epoch) || epoch < 0) throw new Error('Invalid currency metadata.');
    if (this.source === source && epoch !== this.epoch()) this.changed.set(true);
    if (this.source !== source) this.changed.set(false);
    this.source = source; this.decimals.set(decimals); this.epoch.set(epoch);
  }
  parse(value: string | number): number {
    if (this.changed()) throw new InputError('NanaCoin money changed size. Reload the page, then try again.');
    return parseMoney(String(value), this.decimals(), this.locale);
  }
  format(value: number | string | bigint | null | undefined): string { return moneyText(value ?? 0, this.decimals(), this.locale); }
  input(value: number): string { return moneyText(value, this.decimals(), this.locale, false); }
  chart(value: number): number { return value / 10 ** this.decimals(); }
}

@Pipe({ name: 'nc', standalone: true, pure: false })
export class MoneyPipe implements PipeTransform {
  private readonly money = inject(Money);
  transform(value: number | string | bigint | null | undefined): string { return this.money.format(value); }
}
