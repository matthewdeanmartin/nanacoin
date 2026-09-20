// What the client did, and why.
//
// # Why this exists
//
// A session spent chasing "Something went wrong." with nothing in the console
// and no failed request in the network panel. The cause turned out to be a
// 200 OK carrying HTML where JSON was expected - so nothing had failed, in
// HTTP's terms, and there was nothing to see. The app knew exactly what had
// happened at the moment it happened, and threw that away.
//
// So: every request, every response, every navigation, every state change
// worth naming, kept in a ring in memory and printed to the console. Not as a
// debug mode to be switched on after the fact - by then the interesting thing
// has already happened and cannot be reproduced.
//
// # Why a ring, and why it is small
//
// The whole point is to still be holding the entries when someone asks. A
// ring of 200 keeps roughly the last few minutes of an active session and
// cannot grow without bound on a tab left open all day. Entries are shallow -
// a level, a scope, a message and a small detail object - so that holding 200
// of them costs nothing worth measuring.
//
// # What is deliberately not recorded
//
// Passwords, PKCE verifiers and bearer tokens. The whole log is one "copy"
// button away from being pasted into a chat window, which is the point of it,
// and that is exactly why a token must never be in it. `redact` below is the
// single place that decides.

import { Injectable, signal } from '@angular/core';

export type LogLevel = 'debug' | 'info' | 'warn' | 'error';

export interface LogEntry {
  /** Monotonic, so entries can be ordered even within the same millisecond. */
  seq: number;
  at: number;
  level: LogLevel;
  /** Which part of the app spoke: 'http', 'auth', 'session', 'boot', … */
  scope: string;
  message: string;
  /** Structured extras. Already redacted; safe to render and to copy. */
  detail?: Record<string, unknown>;
}

/** How many entries to keep. A few minutes of an active session. */
const CAPACITY = 200;

/**
 * Keys whose values never appear in a log entry, whatever they are attached
 * to. Matched case-insensitively on the key name, so `Authorization`,
 * `code_verifier` and `access_token` are all caught wherever they appear.
 */
const SECRET_KEYS = [
  'password',
  'token',
  'access_token',
  'refresh_token',
  'authorization',
  'code_verifier',
  'code_challenge',
  'verifier',
  'secret',
  'pin',
];

@Injectable({ providedIn: 'root' })
export class Log {
  private readonly entries: LogEntry[] = [];
  private seq = 0;

  /**
   * Bumped on every write so views can react without the array itself being a
   * signal - pushing into a signal-wrapped array on every HTTP call would
   * churn change detection for something most sessions never look at.
   */
  readonly revision = signal(0);

  /**
   * Whether to also print to the browser console.
   *
   * On by default: the console is where someone looks first, and a log nobody
   * finds is no better than no log. `?quiet` turns it off for a session that
   * is being used to demonstrate something.
   */
  readonly toConsole = signal(!location.search.includes('quiet'));

  debug(scope: string, message: string, detail?: Record<string, unknown>): void {
    this.add('debug', scope, message, detail);
  }

  info(scope: string, message: string, detail?: Record<string, unknown>): void {
    this.add('info', scope, message, detail);
  }

  warn(scope: string, message: string, detail?: Record<string, unknown>): void {
    this.add('warn', scope, message, detail);
  }

  error(scope: string, message: string, detail?: Record<string, unknown>): void {
    this.add('error', scope, message, detail);
  }

  /** Everything held, oldest first. */
  all(): readonly LogEntry[] {
    return this.entries;
  }

  /** Everything held, newest first, which is how a reader wants it. */
  recent(limit = CAPACITY): LogEntry[] {
    const out = this.entries.slice(-limit);
    out.reverse();
    return out;
  }

  clear(): void {
    this.entries.length = 0;
    this.revision.update((n) => n + 1);
  }

  /**
   * The whole log as text, for pasting into a bug report.
   *
   * Plain lines rather than JSON: the commonest destination is a chat window,
   * where a wall of JSON is unreadable and a list of timestamped lines is not.
   */
  asText(): string {
    return this.entries
      .map((e) => {
        const when = new Date(e.at).toISOString().slice(11, 23);
        const detail = e.detail ? ` ${JSON.stringify(e.detail)}` : '';
        return `${when} ${e.level.padEnd(5)} ${e.scope.padEnd(8)} ${e.message}${detail}`;
      })
      .join('\n');
  }

  private add(
    level: LogLevel,
    scope: string,
    message: string,
    detail?: Record<string, unknown>,
  ): void {
    const entry: LogEntry = {
      seq: ++this.seq,
      at: Date.now(),
      level,
      scope,
      message,
      detail: detail ? (redact(detail) as Record<string, unknown>) : undefined,
    };

    this.entries.push(entry);
    if (this.entries.length > CAPACITY) this.entries.shift();
    this.revision.update((n) => n + 1);

    if (this.toConsole()) {
      const label = `%c${scope}%c ${message}`;
      const tag = 'color:#b8562f;font-weight:600';
      const rest = 'color:inherit;font-weight:normal';
      const args = entry.detail ? [label, tag, rest, entry.detail] : [label, tag, rest];

      // Levels map onto the console's own, so the browser's filter works and
      // an error still gets a stack trace attached by the console itself.
      if (level === 'error') console.error(...args);
      else if (level === 'warn') console.warn(...args);
      else if (level === 'debug') console.debug(...args);
      else console.info(...args);
    }
  }
}

/**
 * Strips secrets from anything about to be logged.
 *
 * Walks the value rather than checking a top-level key, because the things
 * worth hiding arrive nested - a request body inside a detail object, a
 * headers map inside that. Depth is bounded so a cyclic or pathological
 * structure cannot hang the logger, which would turn a diagnostic into an
 * outage.
 */
export function redact(value: unknown, depth = 0): unknown {
  if (depth > 4) return '[deep]';
  if (value === null || typeof value !== 'object') return value;

  if (Array.isArray(value)) {
    return value.slice(0, 20).map((v) => redact(v, depth + 1));
  }

  const out: Record<string, unknown> = {};
  for (const [key, v] of Object.entries(value as Record<string, unknown>)) {
    if (SECRET_KEYS.some((s) => key.toLowerCase().includes(s))) {
      // Recorded as present-but-hidden rather than dropped: "the request did
      // carry a token" is often the fact being checked.
      out[key] = v === undefined || v === null ? v : '[redacted]';
      continue;
    }
    out[key] = redact(v, depth + 1);
  }
  return out;
}
