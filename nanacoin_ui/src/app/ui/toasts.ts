// Transient messages. One service so that any page can report an outcome
// without owning a banner.

import { Injectable, inject, signal } from '@angular/core';

import { Log } from '../api/log';
import { InputError } from '../api/money';
import { ApiError, BusyError } from '../api/nanacoin.service';

export interface Toast {
  id: number;
  text: string;
  kind: 'ok' | 'error';
}

@Injectable({ providedIn: 'root' })
export class Toasts {
  private readonly log = inject(Log);

  readonly items = signal<Toast[]>([]);
  private nextId = 1;

  ok(text: string) {
    this.push(text, 'ok');
  }

  error(text: string) {
    this.push(text, 'error');
  }

  /**
   * Reports a thrown value. An ApiError already carries a sentence written for
   * a person, so it is shown as-is; anything else gets a generic line, because
   * a stack trace is not useful to whoever is trying to pay their sibling.
   *
   * Whatever is shown, the real error is always logged. This method used to
   * discard anything that was not an ApiError, which is how a login that threw
   * before its first request produced "Something went wrong." and nothing
   * whatsoever to look at - the failure was known here and dropped on the
   * floor. A generic message to the user is fine; a generic message to the
   * log is not.
   */
  fromError(e: unknown) {
    if (e instanceof BusyError) {
      // Not a failure: the board refused on purpose and we ran out of
      // retries. Saying "busy" is both true and actionable.
      this.log.warn('ui', 'the board is busy', { retryAfterMs: e.retryAfterMs });
      this.push('NanaCoin is busy right now. Try again in a moment.', 'error');
      return;
    }
    if (e instanceof InputError) {
      // Something the person typed. The message already says what to fix and
      // quotes what they typed, so show it and log it rather than hiding it
      // behind the generic line below.
      this.log.warn('ui', 'showing an input problem to the user', { message: e.message });
      this.push(e.message, 'error');
      return;
    }
    if (e instanceof ApiError) {
      this.log.warn('ui', 'showing an error to the user', {
        status: e.status,
        code: e.code,
        message: e.message,
      });
      this.push(e.message, 'error');
      return;
    }

    this.log.error('ui', 'an unexpected error reached the UI', {
      error: String(e),
      name: e instanceof Error ? e.name : typeof e,
      // The stack is the whole point for this branch: something threw that was
      // not a recognised API failure, and this is the only record of where.
      stack: e instanceof Error ? firstFrames(e.stack) : undefined,
    });
    this.push('Something went wrong.', 'error');
  }

  dismiss(id: number) {
    this.items.update((list) => list.filter((t) => t.id !== id));
  }

  private push(text: string, kind: Toast['kind']) {
    const id = this.nextId++;
    this.items.update((list) => [...list, { id, text, kind }]);
    // Errors linger, because they usually need reading; confirmations do not.
    window.setTimeout(() => this.dismiss(id), kind === 'error' ? 8000 : 4000);
  }
}

/**
 * The top few stack frames on one line.
 *
 * Trimmed because the log is meant to be read and pasted: a full stack buries
 * everything around it, and the top frames are where the answer is.
 */
function firstFrames(stack: string | undefined): string | undefined {
  if (!stack) return undefined;
  return stack.split('\n').slice(0, 6).join(' | ');
}
