// Everything that throws anywhere, recorded.
//
// Angular's default handler prints to the console and stops. That is fine
// when someone has devtools open before the failure, and useless afterwards -
// which is most of the time, and was exactly the situation that made a login
// failure look like it had left no trace at all.
//
// So every uncaught error goes into the same ring the rest of the app writes
// to, where the Browser log page can show it and the copy button can carry it
// into a bug report.

import { ErrorHandler, Injectable, inject } from '@angular/core';

import { Log } from './log';

@Injectable()
export class LoggingErrorHandler implements ErrorHandler {
  private readonly log = inject(Log);

  handleError(error: unknown): void {
    const err = error as { message?: string; name?: string; stack?: string } | null;

    this.log.error('uncaught', err?.message ?? String(error), {
      name: err?.name,
      stack: err?.stack?.split('\n').slice(0, 8).join(' | '),
    });

    // Still print it. The console is where a developer looks first, and
    // swallowing it here would trade one blind spot for another.
    console.error(error);
  }
}
