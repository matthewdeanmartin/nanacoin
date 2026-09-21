import {
  ApplicationConfig,
  ErrorHandler,
  provideBrowserGlobalErrorListeners,
  provideZonelessChangeDetection,
} from '@angular/core';
import { provideHttpClient, withFetch, withInterceptors } from '@angular/common/http';
import { provideRouter, withHashLocation } from '@angular/router';

import { LoggingErrorHandler } from './api/error-handler';
import { IS_DEMO } from './demo/demo';
import { demoBackend } from './demo/demo-backend';
import { routes } from './app.routes';
import { journalGenerationInterceptor } from './api/nanacoin.service';

export const appConfig: ApplicationConfig = {
  providers: [
    provideBrowserGlobalErrorListeners(),
    // Every uncaught error into the same ring as everything else, so the
    // Browser log page shows it and the copy button carries it away. The
    // default handler only reaches the console, which is empty by the time
    // anyone thinks to look.
    { provide: ErrorHandler, useClass: LoggingErrorHandler },
    // Signals throughout, so zone.js has nothing to do.
    provideZonelessChangeDetection(),
    // The demo answers its own requests from an in-memory ledger, so every
    // line of the real client runs unchanged and only the wire is different.
    // IS_DEMO is a compile-time constant, so an ordinary build tree-shakes
    // the whole demo away rather than shipping a mock it never calls.
    provideHttpClient(
      withFetch(),
      withInterceptors([journalGenerationInterceptor]),
      ...(IS_DEMO ? [withInterceptors([demoBackend])] : []),
    ),
    // Hash routing: this is a static site that may end up on a plain file
    // host with no rewrite rules, where /market would 404 on refresh.
    provideRouter(routes, withHashLocation()),
    // ApiBase is a plain root-provided service: the address it holds must be
    // changeable at runtime, which a bootstrap-time factory value could not be.
  ],
};
