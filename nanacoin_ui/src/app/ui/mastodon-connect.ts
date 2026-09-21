import { Component, ElementRef, inject, signal, viewChild } from '@angular/core';
import { FormsModule } from '@angular/forms';

import { Mastodon } from '../api/mastodon';
import { Toasts } from './toasts';

@Component({
  selector: 'app-mastodon-connect',
  imports: [FormsModule],
  template: `
    <button class="btn btn--quiet btn--small" type="button" (click)="open()">
      {{ mastodon.connected() ? 'Mastodon' : 'Connect Mastodon' }}
    </button>
    <dialog #box class="mastodon-dialog" (click)="backdrop($event)">
      <div class="mastodon-dialog__body">
        <h2>Mastodon</h2>
        @if (mastodon.connected()) {
          <p>Connected as <strong>{{ mastodon.account() }}</strong>.</p>
          <p class="muted small">The access token stays in this browser.</p>
          <div class="mastodon-dialog__actions">
            <button class="btn btn--quiet" type="button" (click)="disconnect()">Disconnect</button>
            <button class="btn" type="button" (click)="close()">Done</button>
          </div>
        } @else {
          <p class="muted small">Choose your Mastodon server. NanaCoin stores credentials only in this browser.</p>
          <form (ngSubmit)="connect()">
            <label>
              Your Mastodon server
              <input name="server" [(ngModel)]="server" placeholder="mastodon.social" required />
            </label>
            <div class="mastodon-dialog__actions">
              <button class="btn btn--quiet" type="button" (click)="close()">Cancel</button>
              <button class="btn" type="submit" [disabled]="connecting()">
                {{ connecting() ? 'Connecting…' : 'Connect' }}
              </button>
            </div>
          </form>
        }
      </div>
    </dialog>
  `,
  styles: [`
    .mastodon-dialog { border: 0; padding: 0; border-radius: var(--radius); color: var(--ink); background: transparent; }
    .mastodon-dialog::backdrop { background: rgb(0 0 0 / 45%); }
    .mastodon-dialog__body { width: min(28rem, calc(100vw - 2rem)); padding: 1.5rem; background: var(--surface); border: 1px solid var(--line); border-radius: var(--radius); }
    .mastodon-dialog__body h2 { margin-top: 0; }
    .mastodon-dialog__actions { display: flex; justify-content: flex-end; gap: .5rem; margin-top: 1rem; }
  `],
})
export class MastodonConnect {
  protected readonly mastodon = inject(Mastodon);
  private readonly toasts = inject(Toasts);
  private readonly box = viewChild.required<ElementRef<HTMLDialogElement>>('box');
  protected server = 'mastodon.social';
  protected readonly connecting = signal(false);

  protected open(): void { this.box().nativeElement.showModal(); }
  protected close(): void { this.box().nativeElement.close(); }
  protected backdrop(event: MouseEvent): void {
    if (event.target === this.box().nativeElement) this.close();
  }
  protected async connect(): Promise<void> {
    if (this.connecting()) return;
    this.connecting.set(true);
    try { await this.mastodon.connect(this.server); }
    catch (error) {
      this.connecting.set(false);
      this.toasts.error(error instanceof Error ? error.message : 'Could not connect Mastodon.');
    }
  }
  protected disconnect(): void {
    this.mastodon.disconnect();
    this.toasts.ok('Mastodon disconnected from this browser.');
  }
}
