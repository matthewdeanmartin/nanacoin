import { Component, computed, inject, input, resource } from '@angular/core';
import { ActivatedRoute, RouterLink } from '@angular/router';
import { toSignal } from '@angular/core/rxjs-interop';
import { map } from 'rxjs';
import QRCode from 'qrcode';
import { ApiBase } from '../api/api-base';
import { Log } from '../api/log';
import { IS_DEMO } from '../demo/demo';

/** IDs stay in the hash route, and the URL retains the hosting subdirectory.
 * Share the bank address, never credentials or unrelated URL parameters. */
export function offerUrl(path: string, id: string | number, tab: string | undefined,
  baseUri: string, apiBase?: string): string {
  const url = new URL(baseUri);
  url.username = ''; url.password = '';
  url.search = '';
  url.hash = '';
  if (apiBase) {
    const api = new URL(apiBase, url);
    api.username = ''; api.password = ''; api.search = ''; api.hash = '';
    if (api.origin !== url.origin || api.pathname !== '/api/v1') url.searchParams.set('api', api.href);
  }
  const params = new URLSearchParams({ offer: String(id) });
  if (tab) params.set('tab', tab);
  url.hash = `${path}?${params}`;
  return url.href;
}

/** Calling this in a page's injection context makes shared links reactive to Back/Forward. */
export function offerTarget() {
  const route = inject(ActivatedRoute);
  return toSignal(route.queryParamMap.pipe(map(p => p.get('offer'))), {
    initialValue: route.snapshot.queryParamMap.get('offer'),
  });
}

@Component({
  selector: 'app-offer-selection', imports: [RouterLink],
  template: `@if (target()) {
    <p class="muted">Shared offer · <a routerLink="." [queryParams]="{offer: null}" queryParamsHandling="merge">Show all offers</a></p>
    @if (!loading() && !available()) { <p role="status">This offer is no longer available, or this account cannot view it.</p> }
  }`,
})
export class OfferSelection {
  protected readonly target = offerTarget();
  readonly available = input.required<boolean>();
  readonly loading = input(false);
}

@Component({
  selector: 'app-offer-qr',
  template: `<footer class="offer-qr">
    @if (qr.value(); as image) { <a [href]="url()" [attr.aria-label]="'Open offer: ' + label()"><img [src]="image" width="160" height="160" [alt]="'QR code for ' + label()" /></a> }
    <a [href]="url()">Open this offer on another device</a>
    @if (demo) { <small>Demo changes are saved in this browser tab.</small> }
  </footer>`,
  styles: `.offer-qr{display:flex;align-items:flex-start;flex-direction:column;gap:.4rem;border-top:1px solid var(--line);margin-top:1rem;padding-top:1rem;overflow-wrap:anywhere}img{display:block;width:160px;height:160px;max-width:100%;background:white}small{color:var(--muted)}`,
})
export class OfferQr {
  readonly path = input.required<string>();
  readonly offer = input.required<string | number>();
  readonly label = input.required<string>();
  readonly tab = input<string>();
  protected readonly demo = IS_DEMO;
  private readonly apiBase = inject(ApiBase);
  private readonly log = inject(Log);
  protected readonly url = computed(() => offerUrl(this.path(), this.offer(), this.tab(),
    document.baseURI, IS_DEMO ? undefined : this.apiBase.current()));
  protected readonly qr = resource({params: () => this.url(), loader: async ({params}) => {
    try { return await QRCode.toDataURL(params, {width: 160, margin: 4, errorCorrectionLevel: 'M'}); }
    catch (error) { this.log.warn('offers', 'Could not draw offer QR code', {error: String(error)}); return ''; }
  }});
}
