import { Component, computed, inject, resource, signal } from '@angular/core';
import { DatePipe } from '@angular/common';
import { FormsModule } from '@angular/forms';
import { HttpClient } from '@angular/common/http';
import { firstValueFrom } from 'rxjs';
import { RouterLink } from '@angular/router';
import { IS_DEMO } from '../demo/demo';
import { Transaction } from '../api/models';
import { NanacoinService, newIdempotencyKey } from '../api/nanacoin.service';
import { Notebook } from '../ui/notebook';
import { Session } from '../api/session';
import { Dialogs } from '../ui/dialog';
import { Toasts } from '../ui/toasts';

@Component({
 selector: 'app-public-ledger', imports: [Notebook, RouterLink, DatePipe, FormsModule],
 template: `<h1>The Notebook</h1><p class="notebook-treat"><a routerLink="/recipes">Vegetarian and vegan recipes</a> are available to everyone; eating is optional.</p>
 <p>{{ demo ? 'Public fictional household ledger: everyone can audit the latest 100 entries in this browser tab. Private descriptions are on the roadmap.' : 'The household ledger is public: anyone can audit every transaction. Private descriptions are on the roadmap.' }}</p>
 <div class="ledger-filters" aria-label="Notebook filters">
   <label>Category <select [ngModel]="category()" (ngModelChange)="category.set($event)">
     <option value="ALL">All transactions</option><option value="LABOR">Labor</option>
     <option value="GOOD">Goods</option><option value="GIFT">Gifts</option>
     <option value="OTHER">Other</option><option value="MONEY">Issuance and retirement</option>
     <option value="FOREX">Forex</option><option value="VOUCHER">Nana-nickles</option>
     <option value="REVERSAL">Corrections</option><option value="UNCLASSIFIED">Unclassified transfers</option>
   </select></label>
   <label>Find <input type="search" [ngModel]="query()" (ngModelChange)="query.set($event)" placeholder="Description, person, or transaction ID"></label>
 </div>
 @if (data.isLoading()) { <p>Opening the notebook…</p> }
 @else if (data.error()) { <p role="alert">Could not open the ledger. <button class="btn" title="Try loading the notebook again" (click)="data.reload()">Retry</button></p> }
 @else { <app-notebook><div class="ledger">
 @for (t of filtered(); track t.id) {
 <article class="ledger-row" data-keyboard-row tabindex="-1"><p>{{ t.created_at * 1000 | date:'medium' }} · {{ t.id }} · {{ t.kind }}</p>
 <p>{{ t.description }}</p>
 @for (p of t.postings; track $index) { <span class="posting">{{ p.name }}: {{ p.amount >= 0 ? '+' : '' }}{{ p.amount }} NC </span> }
 @if (refundable(t)) { <button class="btn btn--quiet btn--small ledger-refund" type="button" [disabled]="refunding() === t.id" (click)="refund(t)">{{ refunding() === t.id ? 'Refunding…' : 'Refund' }}</button> }
 </article> } @empty { <p>No transactions yet.</p> }
 </div></app-notebook> }`,
})
export class PublicLedger {
 readonly demo = IS_DEMO;
 private readonly http = inject(HttpClient);
 private readonly api = inject(NanacoinService);
 private readonly session = inject(Session);
 private readonly dialogs = inject(Dialogs);
 private readonly toasts = inject(Toasts);
 readonly refunding = signal<string | null>(null);
 readonly data = resource({
   params: () => true,
   loader: () => IS_DEMO
     ? firstValueFrom(this.http.get<{ transactions: Transaction[] }>('/api/v1/public/ledger'))
     : this.api.ledger(100),
 });
 readonly category = signal('ALL');
 readonly query = signal('');
 readonly filtered = computed(() => {
   const category = this.category();
   const query = this.query().trim().toLocaleLowerCase();
   return (this.data.value()?.transactions ?? []).filter((txn) => {
     const matchesCategory = category === 'ALL'
       || txn.economic_kind === category
       || (category === 'MONEY' && (txn.kind === 'ISSUE' || txn.kind === 'RETIRE'))
       || (category === 'FOREX' && txn.reference?.startsWith('quote-'))
       || (category === 'VOUCHER' && txn.reference?.startsWith('nickle:'))
       || (category === 'REVERSAL' && txn.kind === 'REVERSAL')
       || (category === 'UNCLASSIFIED' && !txn.economic_kind && txn.kind === 'TRANSFER' && !txn.reference);
     if (!matchesCategory) return false;
     if (!query) return true;
     const haystack = [txn.id, txn.description, txn.kind, txn.economic_kind ?? '', ...txn.postings.map((p) => p.name)].join(' ').toLocaleLowerCase();
     return haystack.includes(query);
   });
 });

 refundable(txn: Transaction): boolean {
   const account = this.session.me()?.account;
   if (!account || txn.reversed_by || txn.kind === 'REVERSAL' || txn.kind === 'ISSUE' || txn.kind === 'RETIRE') return false;
   if (txn.reference?.startsWith('quote-') || txn.reference?.startsWith('nickle:')) return false;
   return txn.postings.some((posting) => posting.account === account && posting.amount > 0);
 }

 async refund(txn: Transaction): Promise<void> {
   if (!this.refundable(txn) || this.refunding()) return;
   const payer = txn.postings.find((posting) => posting.amount < 0);
   const amount = txn.postings.reduce((sum, posting) => sum + Math.max(0, posting.amount), 0);
   const answer = await this.dialogs.confirm({
     title: 'Refund this payment?',
     message: `Return ${amount} ${amount === 1 ? 'coin' : 'coins'} to ${payer?.name || 'the payer'}?`,
     detail: txn.economic_kind
       ? [`This also removes the original ${txn.economic_kind.toLocaleLowerCase()} transaction from the economy statistics.`]
       : ['This appends a linked refund; the original remains visible in the notebook.'],
     confirmLabel: 'Refund',
   });
   if (answer === null) return;
   this.refunding.set(txn.id);
   try {
     await this.api.reverse(txn.id, `Refund: ${txn.description}`.slice(0, 96), newIdempotencyKey());
     await this.session.refresh();
     this.data.reload();
     this.toasts.ok('Refunded. The original economic impact was undone.');
   } catch (e) { this.toasts.fromError(e); }
   finally { this.refunding.set(null); }
 }
}
