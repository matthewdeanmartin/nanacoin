// Asking the person something, without window.prompt.
//
// The native dialogs worked, and they were the wrong thing in three ways:
// they block the whole tab, they cannot be styled so they look like a 1995
// browser chrome rather than this app, and they cannot show context - a
// reversal prompt could not display the amount it was about to undo.
//
// This is a service plus one host component, using the platform <dialog>
// element so focus trapping, Escape, and the top layer come from the browser
// rather than from a pile of our own event handlers.

import { Injectable, signal } from '@angular/core';

/** What a dialog is asking for. */
export type DialogKind = 'confirm' | 'text' | 'number' | 'offer';

export interface DialogRequest {
  kind: DialogKind;
  title: string;
  /** A sentence of context. Optional, but usually worth having. */
  message?: string;
  /** Extra lines rendered smaller - amounts, names, consequences. */
  detail?: string[];
  /** Label on the confirming button. "Confirm" when unset. */
  confirmLabel?: string;
  /** Marks the action as irreversible, which styles the button. */
  danger?: boolean;

  // For text and number:
  placeholder?: string;
  initial?: string;
  /** Text inputs only: refuse an empty answer. */
  required?: boolean;
  min?: number;
  max?: number;
}

/**
 * What came back. `null` means the person cancelled - which is different from
 * an empty string, and the distinction matters: cancelling a reversal must
 * not reverse anything.
 */
export type DialogResult = string | null;

/** What the offer dialog returns. */
export interface OfferAnswer {
  amount: number;
  message: string;
}

interface Pending extends DialogRequest {
  resolve: (value: DialogResult) => void;
}

@Injectable({ providedIn: 'root' })
export class Dialogs {
  /** The dialog currently open, if any. The host component renders it. */
  readonly current = signal<Pending | null>(null);

  /** Yes or no. Resolves to 'yes' or null. */
  confirm(req: Omit<DialogRequest, 'kind'>): Promise<DialogResult> {
    return this.open({ ...req, kind: 'confirm' });
  }

  /** A line of text. Resolves to the text, or null if cancelled. */
  prompt(req: Omit<DialogRequest, 'kind'>): Promise<DialogResult> {
    return this.open({ ...req, kind: 'text' });
  }

  /** A whole number. Resolves to the digits, or null if cancelled. */
  number(req: Omit<DialogRequest, 'kind'>): Promise<DialogResult> {
    return this.open({ ...req, kind: 'number' });
  }

  /**
   * An amount and an optional note, together.
   *
   * Making an offer used to be two stacked window.prompts - the amount, then
   * the message - which meant answering the first before seeing that a second
   * was coming. One dialog with two fields is the same information asked once.
   */
  offer(req: Omit<DialogRequest, 'kind'>): Promise<OfferAnswer | null> {
    return this.open({ ...req, kind: 'offer' }).then((v) =>
      v === null ? null : (JSON.parse(v) as OfferAnswer),
    );
  }

  private open(req: DialogRequest): Promise<DialogResult> {
    // One at a time. A second dialog while one is open would leave the first
    // unresolved forever, and nothing in this app needs to stack them.
    const existing = this.current();
    if (existing) existing.resolve(null);

    return new Promise<DialogResult>((resolve) => {
      this.current.set({ ...req, resolve });
    });
  }

  /** Called by the host when the person answers or dismisses. */
  settle(value: DialogResult): void {
    const pending = this.current();
    if (!pending) return;
    this.current.set(null);
    pending.resolve(value);
  }
}
