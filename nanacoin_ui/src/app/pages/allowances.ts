export type AllowanceCadence = 'WEEKLY' | 'MONTHLY';

export interface Allowance {
  id: string;
  ownerId: string;
  recipientAccount: string;
  recipientName: string;
  amount: number;
  cadence: AllowanceCadence;
  nextDue: string;
  memo: string;
  pendingKey?: string;
}

const KEY = 'nanacoin:allowances:v1';

export function loadAllowances(): Allowance[] {
  try {
    const parsed = JSON.parse(localStorage.getItem(KEY) ?? '[]') as unknown;
    return Array.isArray(parsed) ? parsed.filter(valid) : [];
  } catch { return []; }
}

export function saveAllowances(items: Allowance[]): void {
  localStorage.setItem(KEY, JSON.stringify(items));
}

export function advanceDue(date: string, cadence: AllowanceCadence): string {
  const [year, month, day] = date.split('-').map(Number);
  const value = new Date(Date.UTC(year, month - 1, day));
  if (cadence === 'WEEKLY') value.setUTCDate(value.getUTCDate() + 7);
  else {
    const targetMonth = value.getUTCMonth() + 1;
    value.setUTCDate(1);
    value.setUTCMonth(targetMonth);
    const last = new Date(Date.UTC(value.getUTCFullYear(), value.getUTCMonth() + 1, 0)).getUTCDate();
    value.setUTCDate(Math.min(day, last));
  }
  return value.toISOString().slice(0, 10);
}

export function today(): string {
  const now = new Date();
  return [now.getFullYear(), String(now.getMonth() + 1).padStart(2, '0'), String(now.getDate()).padStart(2, '0')].join('-');
}

function valid(value: unknown): value is Allowance {
  const item = value as Partial<Allowance>;
  return typeof item?.id === 'string' && typeof item.ownerId === 'string'
    && typeof item.recipientAccount === 'string' && typeof item.recipientName === 'string'
    && Number.isSafeInteger(item.amount) && Number(item.amount) > 0
    && (item.cadence === 'WEEKLY' || item.cadence === 'MONTHLY')
    && /^\d{4}-\d{2}-\d{2}$/.test(item.nextDue ?? '') && typeof item.memo === 'string';
}
