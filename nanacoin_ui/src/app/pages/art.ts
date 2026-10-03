import { Component, computed, inject, resource, signal } from '@angular/core';
import { FormsModule } from '@angular/forms';
import { RouterLink } from '@angular/router';
import { Artwork, CommerceAction } from '../api/models';
import { Money, MoneyPipe } from '../api/money';
import { NanacoinService, newIdempotencyKey } from '../api/nanacoin.service';
import { Session, reloadOnLedgerChange } from '../api/session';
import { Toasts } from '../ui/toasts';
import { Dialogs } from '../ui/dialog';
import { ArtPicture } from '../art/art-picture';
import { TrustedArtHosts } from '../art/trusted-hosts';
import { digestOf, sha256Hex } from '../art/art-media';
import { MinicloudArt, StoredCopyright } from '../art/minicloud-art';
import { memberNumber, nameOf, userIdOf } from '../people/people';

const bytes = (text: string) => new TextEncoder().encode(text).length;
export const DEFAULT_LICENSE = 'Personal profile display; artist retains copyright';

/** What the board will accept for a new edition, checked before sending. */
export function mintProblem(title: string, license: string, locator: string, sha256: string): string | null {
  if (!title.trim() || bytes(title) > 80) return 'Give the art a title of at most 80 bytes.';
  if (!license.trim() || bytes(license) > 96) return 'Say what owners may do with it, in at most 96 bytes.';
  if (!locator.startsWith('https://') || bytes(locator) > 192) return 'The picture needs an https:// address of at most 192 bytes.';
  if (!/^[0-9a-f]{64}$/i.test(sha256)) return 'The SHA-256 digest is 64 hexadecimal characters. Compute it from the picture.';
  return null;
}

/**
 * Digital art: household-issued editions with an owner, a price and a picture
 * that lives elsewhere. Not an NFT: there is no blockchain, and Nana's board is
 * the one record of who owns which edition.
 */
@Component({
  selector: 'app-art', imports: [FormsModule, MoneyPipe, RouterLink, ArtPicture],
  template: `
    <h1>Digital Art</h1>
    <p class="muted">Make an edition of your art, sell it, give it away, or show it on your profile. The household notebook records who owns each edition; the picture can live in minicloud or at an external address. No crypto, no NFT, no blockchain. Owning an edition does not transfer copyright; the license says what the owner may do.</p>
    @if (!session.signedIn()) { <p>Sign in to see the household's art.</p> }
    @else {
      <div class="tabs" role="tablist" aria-label="Art views">
        @for (v of views; track v.id) {
          <button class="tab" type="button" role="tab" [class.tab--active]="view() === v.id" [attr.aria-selected]="view() === v.id" (click)="view.set(v.id)">{{ v.label }}@if (v.id !== 'make') { ({{ filtered(v.id).length }}) }</button>
        }
      </div>
      @if (book.isLoading() && !book.hasValue()) { <p role="status">Loading art…</p> }
      @if (book.error()) { <p role="alert">Could not load art. <button class="btn btn--quiet btn--small" (click)="book.reload()">Retry</button></p> }

      @if (view() === 'make') {
        <form class="panel art-make" (ngSubmit)="mint()">
          <h2>Make an edition</h2>
          <p class="muted small">Upload a picture to minicloud and register its copyright, choose an existing registered file, or use an external picture address. Each edition is one of a kind on this board, even if you register the same picture twice.</p>
          <label>Title <input name="title" [(ngModel)]="title" required maxlength="80" placeholder="Moonlit garden" /></label>
          <label>What may the owner do with it? <input name="license" [(ngModel)]="license" required maxlength="96" /></label>
          <section class="panel">
            <h3>Minicloud files and copyrights</h3>
            <label>Image file <input name="cloudFile" type="file" accept="image/*" (change)="chooseCloudFile($event)" /></label>
            <button type="button" class="btn btn--quiet btn--small" [disabled]="busy() || !cloudFile" (click)="uploadToCloud()">Store file and register copyright</button>
            <label>Search registered files <input name="cloudSearch" [(ngModel)]="cloudSearch" /></label>
            <button type="button" class="btn btn--quiet btn--small" [disabled]="busy()" (click)="loadCloudFiles()">Browse my registered images</button>
            @for (c of cloudFiles(); track c.bucket + '/' + c.key) {
              <button type="button" class="btn btn--quiet btn--small" (click)="useCloudFile(c)">{{ c.title }} · {{ c.key }}</button>
            }
            <p class="muted small">Uses your bank login. Copyright stays with the registered owner when an edition is sold or given away.</p>
          </section>
          <label>Picture address <input name="locator" type="url" [(ngModel)]="locator" required maxlength="192" placeholder="https://example.org/my-art.png" /></label>
          <label>SHA-256 digest <input name="sha256" [(ngModel)]="sha256" required pattern="[0-9a-fA-F]{64}" spellcheck="false" autocomplete="off" placeholder="Computed from the picture" /></label>
          <div class="art-digest">
            <button type="button" class="btn btn--quiet btn--small" [disabled]="digesting()" (click)="digestFromUrl()">Compute from the address</button>
            <label class="btn btn--quiet btn--small art-file">Compute from a file on this device<input type="file" accept="image/*" (change)="digestFromFile($event)" /></label>
          </div>
          @if (preview(); as p) { <img class="art-preview" [src]="p" alt="Preview of the chosen file" /> }
          <p class="muted small">The digest is a fingerprint of the exact file. Anyone can later check that the picture at the address is the one you registered. Minicloud stores uploaded files; external addresses remain references.</p>
          <button class="btn" type="submit" [disabled]="busy()">{{ busy() ? 'Registering…' : 'Register edition' }}</button>
        </form>
      } @else {
        <div class="cards art-gallery">
          @for (a of filtered(view()); track a.id) {
            <article data-keyboard-row tabindex="-1" class="card" [attr.aria-label]="a.title">
              <app-art-picture [art]="a" />
              <h3>{{ a.title }}</h3>
              <p class="card__meta">Edition #{{ a.id }} · by <a [routerLink]="['/people', userIdOf(a.creator)]">{{ name(a.creator) }}</a></p>
              <p class="card__meta">Owned by <a [routerLink]="['/people', userIdOf(a.owner)]">{{ isMine(a) ? 'you' : name(a.owner) }}</a>@if (a.equipped) { · on {{ isMine(a) ? 'your' : name(a.owner) + '’s' }} profile }</p>
              <p class="card__desc">{{ a.license }}</p>
              @if (a.price !== null) { <p class="card__status">For sale: {{ a.price | nc }} NC</p> }
              @if (isMine(a)) {
                <div class="art-actions">
                  <button type="button" class="btn btn--quiet btn--small" [disabled]="busy()" (click)="equip(a, !a.equipped)">{{ a.equipped ? 'Remove from my profile' : 'Show on my profile' }}</button>
                  @if (a.price !== null) {
                    <button type="button" class="btn btn--quiet btn--small" [disabled]="busy()" (click)="list(a, null)">Take off sale</button>
                  } @else {
                    <form class="art-inline" (ngSubmit)="sell(a)">
                      <label>Price <input [name]="'price-' + a.id" type="text" inputmode="decimal" [(ngModel)]="prices[a.id]" placeholder="0" /></label>
                      <button class="btn btn--small" type="submit" [disabled]="busy()">Put on sale</button>
                    </form>
                  }
                  <form class="art-inline" (ngSubmit)="give(a)">
                    <label>Give to
                      <select [name]="'to-' + a.id" [(ngModel)]="recipients[a.id]">
                        <option [ngValue]="undefined" disabled>Choose someone</option>
                        @for (u of session.recipients(); track u.id) { <option [ngValue]="number(u)">{{ u.display_name }}</option> }
                      </select>
                    </label>
                    <button class="btn btn--quiet btn--small" type="submit" [disabled]="busy()">Give</button>
                  </form>
                </div>
              } @else if (a.price !== null) {
                <button type="button" class="btn" [disabled]="busy()" (click)="buy(a)">Buy for {{ a.price | nc }} NC</button>
              }
            </article>
          } @empty {
            @if (book.hasValue()) { <p class="muted">{{ view() === 'mine' ? 'You do not own any art yet.' : view() === 'sale' ? 'Nothing is for sale right now.' : 'Nobody has made any art yet.' }}</p> }
          }
        </div>
        @if (trusted.hosts().length) {
          <section class="panel art-trusted" aria-labelledby="trusted-heading">
            <h2 id="trusted-heading">Sites you show pictures from</h2>
            <p class="muted small">Remembered in this browser only. Pictures from other sites wait until you ask.</p>
            <ul>
              @for (h of trusted.hosts(); track h) { <li>{{ h }} <button type="button" class="btn btn--quiet btn--small" (click)="trusted.forget(h)">Stop showing</button></li> }
            </ul>
          </section>
        }
      }
    }`,
  styles: `.art-actions,.art-inline{display:flex;flex-direction:column;gap:.4rem;margin-top:auto}.art-inline label{font-size:.875rem}.art-digest{display:flex;flex-wrap:wrap;gap:.5rem}.art-file{position:relative;overflow:hidden}.art-file input{position:absolute;inset:0;opacity:0;cursor:pointer}.art-preview{max-width:10rem;border:1px solid var(--line);border-radius:var(--radius)}`,
})
export class ArtPage {
  protected readonly session = inject(Session);
  private readonly api = inject(NanacoinService);
  private readonly cloud = inject(MinicloudArt);
  protected cloudFile: File | null = null;
  protected cloudSearch = '';
  protected readonly cloudFiles = signal<StoredCopyright[]>([]);
  private cloudURL = '';

  private readonly money = inject(Money);
  private readonly toasts = inject(Toasts);
  private readonly dialogs = inject(Dialogs);
  protected readonly trusted = inject(TrustedArtHosts);
  protected readonly userIdOf = userIdOf;
  protected readonly number = memberNumber;

  protected readonly views = [
    { id: 'all', label: 'Gallery' }, { id: 'sale', label: 'For sale' }, { id: 'mine', label: 'Mine' }, { id: 'make', label: 'Make art' },
  ] as const;
  protected readonly view = signal<'all' | 'sale' | 'mine' | 'make'>('all');
  protected readonly busy = signal(false);
  protected readonly digesting = signal(false);
  protected readonly preview = signal<string | null>(null);
  protected title = ''; protected license = DEFAULT_LICENSE; protected locator = ''; protected sha256 = '';
  protected prices: Record<number, string> = {};
  protected recipients: Record<number, number | undefined> = {};
  /** A retried sale or purchase reuses its key, so a dropped reply cannot pay twice. */
  private readonly pendingKeys = new Map<string, string>();

  protected readonly book = resource({ params: () => this.session.me()?.account, loader: () => this.api.commerceBook() });
  private readonly followLedger = reloadOnLedgerChange(this.book);
  private readonly me = computed(() => memberNumber(this.session.me()));
  private readonly artworks = computed(() => [...(this.book.value()?.artworks ?? [])].sort((a, b) => b.id - a.id));

  protected filtered(view: string): Artwork[] {
    return this.artworks().filter((a) => view === 'mine' ? this.isMine(a) : view === 'sale' ? a.price !== null : view === 'all');
  }
  protected isMine(a: Artwork): boolean { return a.owner === this.me(); }
  protected name(member: number): string { return nameOf(this.session.household(), member); }

  protected chooseCloudFile(event: Event): void {
    this.cloudFile = (event.target as HTMLInputElement).files?.[0] ?? null;
    if (this.cloudFile && !this.title) this.title = this.cloudFile.name.replace(/\.[^.]+$/, '').slice(0, 80);
  }
  protected async uploadToCloud(): Promise<void> {
    if (!this.cloudFile || this.busy()) return;
    if (!this.title.trim() || !this.license.trim()) { this.toasts.error('Enter a title and license first.'); return; }
    this.busy.set(true);
    try {
      const stored = await this.cloud.upload(this.cloudFile, this.title.trim(), this.license.trim());
      this.cloudURL = stored.url; this.useCloudFile(stored.record);
      this.toasts.ok('File stored and copyright registered. You can now register the edition.');
    } catch (e) { this.toasts.fromError(e); } finally { this.busy.set(false); }
  }
  protected async loadCloudFiles(): Promise<void> {
    if (this.busy()) return; this.busy.set(true);
    try { const found = await this.cloud.list(this.cloudSearch); this.cloudURL = found.url; this.cloudFiles.set(found.records); }
    catch (e) { this.toasts.fromError(e); } finally { this.busy.set(false); }
  }
  protected useCloudFile(c: StoredCopyright): void {
    this.title = c.title; this.license = c.license; this.sha256 = c.hash;
    this.locator = this.cloud.fileURL(this.cloudURL, c);
  }

  protected async digestFromUrl(): Promise<void> {
    if (!this.locator.startsWith('https://')) { this.toasts.error('Enter the picture’s https:// address first.'); return; }
    this.digesting.set(true);
    const digest = await digestOf(this.locator);
    this.digesting.set(false);
    if (digest) { this.sha256 = digest; this.toasts.ok('Digest computed from the address.'); }
    else this.toasts.error('That site does not let this page read the picture. Choose the same file from this device instead.');
  }

  protected async digestFromFile(event: Event): Promise<void> {
    const file = (event.target as HTMLInputElement).files?.[0];
    if (!file) return;
    this.sha256 = await sha256Hex(await file.arrayBuffer());
    const old = this.preview(); if (old) URL.revokeObjectURL(old);
    this.preview.set(URL.createObjectURL(file));
    if (!this.title) this.title = file.name.replace(/\.[^.]+$/, '').slice(0, 80);
    this.toasts.ok('Digest computed. Upload this exact file to the picture address.');
  }

  protected async mint(): Promise<void> {
    const title = this.title.trim(), license = this.license.trim(), locator = this.locator.trim(), sha256 = this.sha256.trim().toLowerCase();
    const problem = mintProblem(title, license, locator, sha256);
    if (problem) { this.toasts.error(problem); return; }
    await this.run(['mint', title, license, locator, sha256], { mint_art: { title, license, sha256, locator } }, 'Edition registered.', () => {
      this.title = ''; this.license = DEFAULT_LICENSE; this.locator = ''; this.sha256 = ''; this.view.set('mine');
      const old = this.preview(); if (old) URL.revokeObjectURL(old); this.preview.set(null);
    });
  }

  protected async sell(a: Artwork): Promise<void> {
    let price: number;
    try { price = this.money.parse(this.prices[a.id] ?? ''); } catch (e) { this.toasts.fromError(e); return; }
    if (price <= 0) { this.toasts.error('Enter a price above zero.'); return; }
    await this.list(a, price);
  }

  protected async list(a: Artwork, price: number | null): Promise<void> {
    await this.run(['list', a.id, a.revision, price], { list_art: { art: a.id, price } }, price === null ? 'Taken off sale.' : 'Put on sale.', () => { delete this.prices[a.id]; });
  }

  protected async buy(a: Artwork): Promise<void> {
    if (a.price === null) return;
    if (await this.dialogs.confirm({ title: 'Buy this edition?', message: 'The coins and the edition change hands together.', detail: [`${a.title} (edition #${a.id})`, `${this.money.format(a.price)} NC to ${this.name(a.owner)}`], confirmLabel: 'Buy' }) === null) return;
    await this.run(['buy', a.id, a.revision], { buy_art: { art: a.id, expected_owner: a.owner, expected_revision: a.revision, expected_price: a.price } }, 'It’s yours.', () => {});
  }

  protected async give(a: Artwork): Promise<void> {
    const to = this.recipients[a.id];
    if (to === undefined) { this.toasts.error('Choose who receives it.'); return; }
    if (await this.dialogs.confirm({ title: 'Give this edition away?', message: 'It leaves your collection, and any sale listing is cancelled.', detail: [`${a.title} (edition #${a.id})`, `to ${this.name(to)}`], confirmLabel: 'Give' }) === null) return;
    await this.run(['gift', a.id, a.revision, to], { gift_art: { art: a.id, to } }, 'Given.', () => { delete this.recipients[a.id]; });
  }

  protected async equip(a: Artwork, equipped: boolean): Promise<void> {
    await this.run(['equip', a.id, a.revision, equipped], { equip_art: { art: a.id, equipped } }, equipped ? 'Now on your profile.' : 'Removed from your profile.', () => {});
  }

  private async run(identity: unknown[], action: CommerceAction, done: string, reset: () => void): Promise<void> {
    if (this.busy()) return;
    const id = JSON.stringify(identity);
    this.busy.set(true);
    const key = this.pendingKeys.get(id) ?? newIdempotencyKey(); this.pendingKeys.set(id, key);
    try {
      await this.api.commerce(action, key);
      this.pendingKeys.delete(id);
      reset(); this.toasts.ok(done); this.book.reload();
      await this.session.refresh();
    } catch (e) { this.toasts.fromError(e); this.book.reload(); }
    finally { this.busy.set(false); }
  }
}
