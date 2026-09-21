import { Money, MoneyPipe } from '../api/money';
import { inject as moneyInject } from '@angular/core';
import { Component, computed, inject, resource, signal } from '@angular/core';
import { FormsModule } from '@angular/forms';
import { ActivatedRoute, Router } from '@angular/router';
import { IS_DEMO } from '../demo/demo';
import { Session } from '../api/session';
import { NanacoinService } from '../api/nanacoin.service';
import QRCode from 'qrcode';
import { Transaction } from '../api/models';
import { loadSavedNickles, saveNickle, SavedNickle } from './nickle-history';

/**
 * A scannable URL whose secret stays in the hash-router portion, so the web
 * server, access logs and referrer headers never receive it. The redeem page
 * removes the token from browser history as soon as Angular has read it.
 */
export function redemptionUrl(token: string, pageUrl = location.href): string {
 const url = new URL(pageUrl);
 url.hash = `/redeem?token=${encodeURIComponent(token)}`;
 return url.href;
}

@Component({
 selector: 'app-nickles', imports: [MoneyPipe, FormsModule],
 template: `<h1>Nana-nickles</h1>
 <p>Bearer vouchers: whoever presents the secret first can redeem it. Showing or printing a second copy does not create more money.</p>
 @if (!demo) { <p>This is a browser-demo prototype, not implemented by this live server. Do not use it for real money.</p> }
 @else {
 <section class="panel"><h2>Package coins for someone else</h2>
 <p>Any member can move existing coins into the voucher reserve. Only Nana can issue new money. A lost secret leaves its coins reserved until this demo is reset.</p>
 <label>NanaCoins <input type="text" inputmode="decimal" [(ngModel)]="amount"></label>
 @if (session.isNana()) { <label><input type="checkbox" [(ngModel)]="fresh"> Nana: issue new money instead of using my balance</label> }
 <button class="btn voucher-create" title="Move coins into a new single-use bearer voucher" [disabled]="busy() || !session.signedIn()" (click)="create()">Create voucher</button>
 </section>
 @if (voucher(); as v) {
 <section class="panel printable-voucher"><h2>DEMO nana-nickle · {{ v.amount | nc }} NC</h2><p>Serial {{ v.serial }} · One redemption only · Same tab only</p>
 <p>This code is the money. Anyone who copies it can redeem it first.</p><code class="voucher-secret">{{ v.token }}</code>
 @if (qrCode()) {
   <figure class="voucher-qr"><img [src]="qrCode()" width="256" height="256" alt="QR code linking to the Nana-nickle redemption page"><figcaption>Scan to open NanaCoin’s redemption page with this voucher ready.</figcaption></figure>
 }
 <p>DEMO ONLY — no cash value — destroyed by reload. Redeem in NanaCoin → Nana-nickles in the issuing browser tab.</p></section>
 <div class="voucher-actions"><button class="btn" title="Open the browser print dialog for this voucher" (click)="print()">Print this voucher</button> <button class="btn" title="Remove the voucher secret from the screen" (click)="hide()">Hide secret</button></div>
 }
 <section class="panel"><h2>Redeem into your account</h2>
 <label>Voucher code <input type="password" autocomplete="off" spellcheck="false" [(ngModel)]="token"></label>
 <button class="btn" title="Claim this voucher into the active account; it cannot be redeemed twice" [disabled]="busy() || !session.signedIn()" (click)="redeem()">Redeem once</button>
 <p class="form-message" role="status">{{ message() }}</p></section>

 <section class="panel"><h2>{{ session.isNana() ? 'All vouchers saved in this browser' : 'My issued vouchers' }}</h2>
 @if (visibleIssued().length === 0) { <p class="muted">No saved vouchers yet.</p> }
 @else { <div class="account-summary-list">
   @for (item of visibleIssued(); track item.serial) {
     <div class="voucher-history-row">
       <span><strong>{{ item.serial }}</strong> · {{ item.amount | nc }} NC · {{ item.issuerName }}</span>
       <span>
         <span class="tag" [class.tag--warn]="!redeemed(item)">{{ redeemed(item) ? 'redeemed' : 'outstanding' }}</span>
         @if (item.issuerId === session.me()?.id && !redeemed(item)) { <button class="btn btn--quiet btn--small" type="button" (click)="showSaved(item)">Show</button> }
       </span>
     </div>
   }
 </div> }
 </section>

 <section class="panel"><h2>{{ session.isNana() ? 'All voucher redemptions' : 'My voucher redemptions' }}</h2>
 @for (txn of visibleRedemptions(); track txn.id) {
   <p class="voucher-history-row"><span>{{ txn.description }} · {{ redemptionAmount(txn) | nc }} NC</span><span>{{ when(txn.created_at) }}</span></p>
 } @empty { <p class="muted">No redemptions yet.</p> }
 </section>
 }`,
})
export class NicklesPage {
  protected readonly money = moneyInject(Money);
 readonly demo = IS_DEMO; readonly session = inject(Session);
 private readonly api = inject(NanacoinService);
 private readonly route = inject(ActivatedRoute);
 private readonly router = inject(Router);
 readonly voucher = signal<{ token: string; amount: number; serial: string } | null>(null);
 readonly qrCode = signal('');
 readonly busy = signal(false); readonly message = signal('');
 readonly issued = signal<SavedNickle[]>(loadSavedNickles());
 readonly ledger = resource({
   params: () => this.session.me()?.id,
   loader: ({ params }) => params ? this.api.ledger(365) : Promise.resolve({ transactions: [], circulation: 0 }),
 });
 readonly visibleIssued = computed(() => this.session.isNana()
   ? this.issued()
   : this.issued().filter((item) => item.issuerId === this.session.me()?.id));
 readonly visibleRedemptions = computed(() => (this.ledger.value()?.transactions ?? []).filter((txn) =>
   txn.reference?.startsWith('nickle:')
   && txn.description.startsWith('Redeem nana-nickle')
   && (this.session.isNana() || txn.actor === this.session.me()?.id)));
 amount: string | number = '5'; fresh = false; token = '';
 constructor() {
   const scannedToken = this.route.snapshot.queryParamMap.get('token')?.trim();
   if (scannedToken) {
     this.token = scannedToken;
     this.message.set('Voucher loaded from the QR code. Sign in, then redeem it once.');
     // replaceUrl prevents the bearer secret lingering in the browser's Back
     // history after the scanner has handed the URL to NanaCoin.
     void this.router.navigate(['/redeem'], { replaceUrl: true });
   }
 }
 async create(): Promise<void> {
   if (!IS_DEMO || this.busy()) return;
   let amount: number;
   try { amount = this.money.parse(this.amount); if (amount <= 0) throw new Error('Enter a positive amount.'); }
   catch (e) { this.message.set(e instanceof Error ? e.message : String(e)); return; }
   this.busy.set(true);
   try {
     const voucher = await this.api.createNickle(amount, this.session.isNana() && this.fresh);
     this.voucher.set(voucher);
     const me = this.session.me()!;
     this.issued.set(saveNickle({
       ...voucher, issuerId: me.id, issuerAccount: me.account,
       issuerName: me.display_name, createdAt: Math.floor(Date.now() / 1000),
     }));
     await this.session.refresh();
     this.ledger.reload();
     try {
       this.qrCode.set(await QRCode.toDataURL(redemptionUrl(voucher.token), { errorCorrectionLevel: 'M', margin: 2, width: 256 }));
       this.message.set('Voucher and QR code created. Save the secret before leaving this page.');
     } catch {
       this.qrCode.set('');
       this.message.set('Voucher created. The QR code could not be drawn, so save the text code.');
     }
   }
   catch (e) { this.message.set(e instanceof Error ? e.message : 'Could not create voucher.'); }
   finally { this.busy.set(false); }
 }
 async redeem(): Promise<void> {
   if (!IS_DEMO || this.busy()) return;
   const secret = this.token.trim();
   const mine = this.issued().find((item) => item.token === secret && item.issuerId === this.session.me()?.id);
   if (mine) { this.message.set(`You issued ${mine.serial}, so you cannot redeem it yourself.`); return; }
   this.busy.set(true);
   try { await this.api.redeemNickle(secret); this.token = ''; await this.session.refresh(); this.ledger.reload(); this.message.set('Redeemed. These coins are now in your account.'); }
   catch (e) { this.message.set(e instanceof Error ? e.message : 'Could not redeem voucher.'); }
   finally { this.busy.set(false); }
 }
 print(): void { if (this.voucher()) window.print(); }
 hide(): void { this.voucher.set(null); this.qrCode.set(''); }
 async showSaved(item: SavedNickle): Promise<void> {
   this.voucher.set({ token: item.token, amount: item.amount, serial: item.serial });
   try { this.qrCode.set(await QRCode.toDataURL(redemptionUrl(item.token), { errorCorrectionLevel: 'M', margin: 2, width: 256 })); }
   catch { this.qrCode.set(''); }
   requestAnimationFrame(() => document.querySelector('.printable-voucher')?.scrollIntoView({ behavior: 'smooth' }));
 }
 redeemed(item: SavedNickle): boolean { return (this.ledger.value()?.transactions ?? []).some((txn) => txn.reference === `nickle:${item.serial}` && txn.description.startsWith('Redeem nana-nickle')); }
 redemptionAmount(txn: Transaction): number { return txn.postings.reduce((sum, posting) => sum + Math.max(0, posting.amount), 0); }
 when(seconds: number): string { return new Date(seconds * 1000).toLocaleString(); }
}
