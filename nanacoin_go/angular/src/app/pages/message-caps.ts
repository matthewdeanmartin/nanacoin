export interface CapsDraft { text: string; saved: string; }

/** Toggle without destroying the draft that existed before ALL CAPS. */
export function toggleCaps(text: string, wasOn: boolean, turnOn: boolean, saved: string): CapsDraft {
  if (turnOn && !wasOn) return { text: text.toLocaleUpperCase(), saved: text };
  if (!turnOn && wasOn) return { text: saved, saved };
  return { text, saved };
}
