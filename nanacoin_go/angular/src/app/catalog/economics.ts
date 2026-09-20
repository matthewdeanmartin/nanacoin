import { EconomicKind, EconomicUnit } from '../api/models';
import { CatalogItem } from './catalog';

/**
 * Economic attributes for the fixed household catalog. They are intentionally
 * deterministic: picking the same standard entry always means the same kind
 * and unit, even though the board stores only the small set actually used.
 */
export function catalogEconomics(item: CatalogItem): {
  kind: EconomicKind;
  unit: EconomicUnit;
} {
  if ([1, 3, 4, 7, 8].includes(item.cat)) return { kind: 'LABOR', unit: 'TASK' };

  if (item.cat === 2) {
    // Prepared food is measured as a batch; kitchen work is labor. Pantry
    // objects and ingredients are goods.
    if (item.code === 202 || item.code === 203 || item.code >= 224) {
      return { kind: 'GOOD', unit: item.code === 202 || item.code === 203 ? 'BATCH' : 'EACH' };
    }
    return { kind: 'LABOR', unit: 'TASK' };
  }

  if (item.cat === 6 && ![621, 622, 623].includes(item.code)) {
    return { kind: 'GOOD', unit: 'EACH' };
  }

  if (item.cat === 10 && item.code <= 1005) return { kind: 'GIFT', unit: 'EACH' };
  return { kind: 'OTHER', unit: 'EACH' };
}

export function formatQuantity(milli = 0): string {
  if (!milli) return '1';
  const whole = Math.floor(milli / 1000);
  const fraction = String(milli % 1000).padStart(3, '0').replace(/0+$/, '');
  return fraction ? `${whole}.${fraction}` : String(whole);
}

export function validQuantity(value: string): boolean {
  if (!/^(?:0|[1-9]\d*)(?:\.\d{1,3})?$/.test(value)) return false;
  const [whole, fraction = ''] = value.split('.');
  const milli = Number(whole) * 1000 + Number(fraction.padEnd(3, '0'));
  return Number.isSafeInteger(milli) && milli >= 1 && milli <= 1_000_000_000;
}
