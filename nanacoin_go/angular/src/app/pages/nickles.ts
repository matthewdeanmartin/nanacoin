import { Component, inject, signal } from '@angular/core';
import { FormsModule } from '@angular/forms';
import { IS_DEMO } from '../demo/demo';
import { Session } from '../api/session';
import { NanacoinService } from '../api/nanacoin.service';
import QRCode from 'qrcode';

@Component({
 selector: 'app-nickles', imports: [FormsModule],
 template: `<h1>Nana-nickles</h1>
 <p>Bearer vouchers: whoever presents the secret first can redeem it. Showing or printing a second copy does not create more money.</p>
 @if (!demo) { <p>This is a browser-demo prototype, not implemented by this live server. Do not use it for real money.</p> }
 @else {
 <p class="warning">Play money only. Valid only in this tab’s fictional economy—not on another device or tab. Reloading destroys these vouchers. Never paste real secrets here.</p>
 <section class="panel"><h2>Package coins for someone else</h2>
 <p>Any member can move existing coins into the voucher reserve. Only Nana can issue new money. A lost secret leaves its coins reserved until this demo is reset.</p>
 <label>Whole NanaCoins <input type="number" min="1" step="1" [(ngModel)]="amount"></label>
 @if (session.isNana()) { <label><input type="checkbox" [(ngModel)]="fresh"> Nana: issue new money instead of using my balance</label> }
 <button class="btn" [disabled]="busy() || !session.signedIn()" (click)="create()">Create voucher</button>
 </section>
 @if (voucher(); as v) {
 <section class="panel printable-voucher"><h2>DEMO nana-nickle · {{ v.amount }} NC</h2><p>Serial {{ v.serial }} · One redemption only · Same tab only</p>
 <p>This code is the money. Anyone who copies it can redeem it first.</p><code class="voucher-secret">{{ v.token }}</code>
 @if (qrCode()) {
   <figure class="voucher-qr"><img [src]="qrCode()" width="256" height="256" alt="QR code containing this Nana-nickle voucher code"><figcaption>Scan to copy the voucher code into a QR reader.</figcaption></figure>
 }
 <p>DEMO ONLY — no cash value — destroyed by reload. Redeem in NanaCoin → Nana-nickles in the issuing browser tab.</p></section>
 <button class="btn" (click)="print()">Print this voucher</button> <button class="btn" (click)="hide()">Hide secret</button>
 }
 <section class="panel"><h2>Redeem into your account</h2>
 <label>Voucher code <input type="password" autocomplete="off" spellcheck="false" [(ngModel)]="token"></label>
 <button class="btn" [disabled]="busy() || !session.signedIn()" (click)="redeem()">Redeem once</button></section>
 <p role="status">{{ message() }}</p>
 }`,
})
export class NicklesPage {
 readonly demo = IS_DEMO; readonly session = inject(Session);
 private readonly api = inject(NanacoinService);
 readonly voucher = signal<{ token: string; amount: number; serial: string } | null>(null);
 readonly qrCode = signal('');
 readonly busy = signal(false); readonly message = signal('');
 amount = 5; fresh = false; token = '';
 async create(): Promise<void> {
   if (!IS_DEMO || this.busy()) return;
   if (!Number.isSafeInteger(this.amount) || this.amount <= 0) { this.message.set('Enter a positive whole number.'); return; }
   this.busy.set(true);
   try {
     const voucher = await this.api.createNickle(this.amount, this.session.isNana() && this.fresh);
     this.voucher.set(voucher);
     await this.session.refresh();
     try {
       this.qrCode.set(await QRCode.toDataURL(voucher.token, { errorCorrectionLevel: 'M', margin: 2, width: 256 }));
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
   this.busy.set(true);
   const secret = this.token.trim(); this.token = '';
   try { await this.api.redeemNickle(secret); this.voucher.set(null); this.qrCode.set(''); await this.session.refresh(); this.message.set('Redeemed. These coins are now in your account.'); }
   catch (e) { this.message.set(e instanceof Error ? e.message : 'Could not redeem voucher.'); }
   finally { this.busy.set(false); }
 }
 print(): void { if (this.voucher()) window.print(); }
 hide(): void { this.voucher.set(null); this.qrCode.set(''); }
}
