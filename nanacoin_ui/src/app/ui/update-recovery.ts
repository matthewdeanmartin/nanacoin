import { Component, DestroyRef, Injectable, InjectionToken, inject, signal } from '@angular/core';
import { Log } from '../api/log';

export const RELOAD_PAGE = new InjectionToken<() => void>('Reload NanaCoin', {
  providedIn: 'root', factory: () => () => window.location.reload(),
});
export const UPDATE_RECOVERY_KEY = 'nanacoin.update-recovery';

/** Browser and Angular errors wrap rejected imports in several different ways. */
export function isChunkLoadError(error: unknown, depth = 0): boolean {
  if (depth > 6 || error == null) return false;
  if (typeof error === 'object') {
    const value = error as Record<string, unknown>;
    return isChunkLoadError(`${value['name'] ?? ''}: ${value['message'] ?? ''}`, depth + 1)
      || ['reason', 'rejection', 'error'].some(key => isChunkLoadError(value[key], depth + 1));
  }
  if (typeof error !== 'string') return false;
  const text = error.toLowerCase();
  return ['error loading dynamically imported module', 'failed to fetch dynamically imported module',
    'importing a module script failed', 'chunkloaderror', 'loading chunk', 'failed to load module script']
    .some(fragment => text.includes(fragment)) || (text.includes('module') && text.includes('mime type'));
}

/** The Mawkingbird recovery pattern: one reload, then a manual retry if it fails again. */
@Injectable({providedIn: 'root'})
export class UpdateRecovery {
  private readonly log = inject(Log);
  private readonly reload = inject(RELOAD_PAGE);
  readonly updating = signal(false);
  readonly failed = signal(false);
  private started = false;
  private target?: string;
  private reloadTimer?: ReturnType<typeof setTimeout>;
  constructor() {
    // Keep the last failure available in Browser Log after the automatic reload.
    try {
      const previous = JSON.parse(sessionStorage.getItem(UPDATE_RECOVERY_KEY) ?? 'null');
      if (previous) this.log.warn('update', 'A previous page load required recovery', previous);
    } catch { /* Invalid or unavailable storage is handled conservatively in recover. */ }
    const stableTimer = setTimeout(() => {
      if (!this.started) {
        try { sessionStorage.removeItem(UPDATE_RECOVERY_KEY); } catch { /* No guard to clear. */ }
      }
    }, 30_000);
    inject(DestroyRef).onDestroy(() => {
      clearTimeout(stableTimer);
      clearTimeout(this.reloadTimer);
    });
  }
  recover(error: unknown, target?: string): boolean {
    if (!isChunkLoadError(error)) return false;
    if (target) this.target = target;
    if (this.started) return true;
    this.started = true;
    this.log.error('update', 'Could not load a page module', {error: String(error), target});
    const now = Date.now();
    try {
      const previous = JSON.parse(sessionStorage.getItem(UPDATE_RECOVERY_KEY) ?? 'null');
      if (!navigator.onLine || (typeof previous?.attemptedAt === 'number' && now - previous.attemptedAt < 60_000)) {
        this.failed.set(true);
        return true;
      }
      // Persist before navigating. Without storage, stop instead of risking a loop.
      sessionStorage.setItem(UPDATE_RECOVERY_KEY, JSON.stringify({attemptedAt: now, error: String(error), target}));
    } catch {
      this.log.warn('update', 'Automatic recovery paused because its reload guard could not be saved');
      this.failed.set(true);
      return true;
    }
    this.updating.set(true);
    this.reloadTimer = setTimeout(() => this.reloadTarget(), 400);
    return true;
  }
  retry(): void {
    // A manual retry is also guarded, so a second failure cannot auto-reload again.
    try { sessionStorage.setItem(UPDATE_RECOVERY_KEY, JSON.stringify({attemptedAt: Date.now(), target: this.target})); }
    catch { /* A deliberate manual reload remains available without storage. */ }
    this.failed.set(false);
    this.updating.set(true);
    this.reloadTarget();
  }
  private reloadTarget(): void {
    if (this.target?.startsWith('/')) {
      const url = new URL(location.href);
      url.hash = this.target;
      history.replaceState(history.state, '', url.href);
    }
    // Reload revalidates the HTML and retains the host, base path, API query and session.
    this.reload();
  }
}

@Component({
  selector: 'app-update-overlay',
  template: `@if (recovery.failed()) {
    <section class="update-overlay" role="alert" aria-labelledby="update-title">
      <div class="panel"><h1 id="update-title">Could not load this page</h1>
      <p>A deployment or connection problem may be responsible. Check your connection and try again in a moment.</p>
      <button class="btn" type="button" (click)="recovery.retry()">Try again</button></div>
    </section>
  } @else if (recovery.updating()) {
    <section class="update-overlay" role="status" aria-live="polite">
      <div class="panel"><span class="request-spinner" aria-hidden="true"></span>
      <h1>Updating NanaCoin…</h1><p>Reloading the app to load the latest pages.</p></div>
    </section>
  }`,
  styles: `.update-overlay{position:fixed;inset:0;z-index:1100;background:var(--bg);color:var(--ink);display:grid;place-items:center;padding:1rem;overflow:auto}.panel{width:min(100%,32rem);box-sizing:border-box}h1{font-size:1.5rem}`,
})
export class UpdateOverlay {
  protected readonly recovery = inject(UpdateRecovery);
}
