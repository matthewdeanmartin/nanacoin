export interface SavedNickle {
  token: string;
  serial: string;
  amount: number;
  issuerId: string;
  issuerAccount: string;
  issuerName: string;
  createdAt: number;
}

const KEY = 'nanacoin:nickles:issued:v1';

export function loadSavedNickles(): SavedNickle[] {
  try {
    const value = JSON.parse(localStorage.getItem(KEY) ?? '[]') as unknown;
    if (!Array.isArray(value)) return [];
    return value.filter((item): item is SavedNickle => {
      const candidate = item as Partial<SavedNickle>;
      return typeof candidate.token === 'string'
        && typeof candidate.serial === 'string'
        && typeof candidate.amount === 'number'
        && typeof candidate.issuerId === 'string'
        && typeof candidate.issuerAccount === 'string'
        && typeof candidate.issuerName === 'string'
        && typeof candidate.createdAt === 'number';
    });
  } catch {
    return [];
  }
}

export function saveNickle(nickle: SavedNickle): SavedNickle[] {
  const saved = [nickle, ...loadSavedNickles().filter((item) => item.serial !== nickle.serial)];
  localStorage.setItem(KEY, JSON.stringify(saved));
  return saved;
}
