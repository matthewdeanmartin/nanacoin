import { Component, computed, inject, input, signal } from '@angular/core';
import { Artwork } from '../api/models';
import { autoSource, digestOf, digestPattern, mediaHost } from './art-media';
import { TrustedArtHosts } from './trusted-hosts';

/**
 * One edition's picture. The board only knows the locator and digest, so the
 * image itself comes from wherever the creator put it: same-site media shows at
 * once, off-site media waits for the viewer, and the digest can be checked.
 */
@Component({
  selector: 'app-art-picture',
  template: `
    <figure class="art-picture" [class.art-picture--small]="small()">
      @if (source(); as src) {
        <img [src]="src" [alt]="art().title" loading="lazy" referrerpolicy="no-referrer" (error)="failed.set(true)" />
        @if (failed()) { <figcaption class="muted small">The picture could not be loaded from {{ host() }}.</figcaption> }
        @if (controls() && offSite()) {
          <label class="small art-trust"><input type="checkbox" [checked]="trusted.has(host())" (change)="trust($any($event.target).checked)" /> Always show pictures from {{ host() }}</label>
        }
      } @else {
        <svg viewBox="0 0 5 5" role="img" [attr.aria-label]="art().title + ' (pattern drawn from its digest)'" shape-rendering="crispEdges">
          <rect width="5" height="5" [attr.fill]="'hsl(' + pattern().hue + ' 45% 92%)'" />
          @for (on of pattern().cells; track $index) {
            @if (on) { <rect [attr.x]="$index % 5" [attr.y]="($index - $index % 5) / 5" width="1" height="1" [attr.fill]="'hsl(' + pattern().hue + ' 55% 42%)'" /> }
          }
        </svg>
        @if (controls()) {
          <button type="button" class="btn btn--quiet btn--small" (click)="shown.set(true)" title="Loading it lets that site see that you looked">Show picture from {{ host() }}</button>
        }
      }
      @if (controls() && source()) {
        @switch (check()) {
          @case ('idle') { <button type="button" class="btn btn--quiet btn--small" (click)="verify()">Check it matches the registered digest</button> }
          @case ('checking') { <span class="muted small" role="status">Checking…</span> }
          @case ('match') { <span class="small art-check art-check--ok" role="status">Matches the registered digest.</span> }
          @case ('mismatch') { <span class="small art-check art-check--bad" role="status">This is not the file that was registered.</span> }
          @case ('unknown') { <span class="muted small" role="status">{{ host() }} does not allow the check from here.</span> }
        }
      }
    </figure>`,
  styles: `
    .art-picture{margin:0;display:flex;flex-direction:column;gap:.35rem;align-items:flex-start}
    .art-picture img,.art-picture svg{width:100%;aspect-ratio:1;object-fit:contain;border-radius:var(--radius);border:1px solid var(--line);background:var(--surface)}
    .art-picture--small{width:2.5rem;flex:none}.art-picture--small img,.art-picture--small svg{border-radius:50%}
    .art-trust{display:flex;gap:.35rem;align-items:center}.art-check--ok{color:var(--credit)}.art-check--bad{color:var(--debit);font-weight:600}`,
})
export class ArtPicture {
  readonly art = input.required<Pick<Artwork, 'title' | 'sha256' | 'locator'>>();
  /** A profile badge: no controls, round. */
  readonly small = input(false);
  /** Show-picture and digest-check buttons; off where the picture is decoration. */
  readonly controls = input(true);
  protected readonly shown = signal(false);
  protected readonly failed = signal(false);
  protected readonly check = signal<'idle' | 'checking' | 'match' | 'mismatch' | 'unknown'>('idle');
  protected readonly host = computed(() => mediaHost(this.art().locator));
  protected readonly pattern = computed(() => digestPattern(this.art().sha256));
  protected readonly trusted = inject(TrustedArtHosts);
  /** Loading it would tell another site who looked; same-site media never asks. */
  protected readonly offSite = computed(() => autoSource(this.art().locator) === null);
  protected readonly source = computed(() => autoSource(this.art().locator)
    ?? (this.shown() || this.trusted.has(this.host()) ? this.art().locator : null));

  protected trust(always: boolean): void {
    if (always) this.trusted.trust(this.host()); else this.trusted.forget(this.host());
  }

  protected async verify(): Promise<void> {
    const src = this.source();
    if (!src) return;
    this.check.set('checking');
    const digest = await digestOf(src);
    this.check.set(digest === null ? 'unknown' : digest === this.art().sha256.toLowerCase() ? 'match' : 'mismatch');
  }
}
