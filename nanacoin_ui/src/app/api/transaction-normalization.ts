/** Adapt immutable ledger facts to the current-unit amounts used by existing screens.
 * Unchanged response branches retain identity; the wire response is never mutated.
 */
export function normalizeTransactions<T>(value: T): T {
  function visit(input: unknown): unknown {
    if (input === null || typeof input !== 'object') return input;
    if (Array.isArray(input)) {
      let changed = false;
      const rows = input.map((row) => { const next = visit(row); changed ||= next !== row; return next; });
      return changed ? rows : input;
    }
    const source = input as Record<string, unknown>;
    let result = source;
    for (const [key, child] of Object.entries(source)) {
      const next = visit(child);
      if (next !== child) {
        if (result === source) result = { ...source };
        result[key] = next;
      }
    }
    if (typeof source['id'] === 'string' && Array.isArray(source['postings']) && Object.hasOwn(source, 'current_postings')) {
      if (source['current_postings'] === null) {
        throw new Error(`Historical transaction ${source['id']} cannot be represented exactly in the current currency units.`);
      }
      if (!Array.isArray(source['current_postings'])) throw new Error('Invalid current transaction postings.');
      if (result === source) result = { ...source };
      result['original_postings'] = source['original_postings'] ?? source['postings'];
      result['postings'] = source['current_postings'];
    }
    return result;
  }
  return visit(value) as T;
}
