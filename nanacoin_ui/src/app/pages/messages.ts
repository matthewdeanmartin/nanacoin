import { Component, DestroyRef, computed, inject, resource, signal } from '@angular/core';
import { DatePipe } from '@angular/common';
import { RouterLink } from '@angular/router';
import { NanacoinService } from '../api/nanacoin.service';
import { ApiBase } from '../api/api-base';
import { Session, reloadOnLedgerChange } from '../api/session';
import { MoneyPipe } from '../api/money';
import { mailRows, MailRow } from './mail-model';

@Component({
 selector: 'app-messages', imports: [DatePipe, RouterLink, MoneyPipe],
 template: `
 <div class="mail-heading"><h1>Messages</h1><a class="btn" routerLink="/send" [queryParams]="{amount: '0'}">Write a message</a></div>
 <p class="muted small">Messages, offers, loan requests, and transactions. Zero-value sends are messages; Mastodon is optional.</p>
 @if (!session.signedIn()) { <p>Sign in to open your mail.</p> }
 @else {
   <div class="mail-toolbar">
     <label class="mail-search">Search mail <input type="search" [value]="search()" (input)="search.set($any($event.target).value)" placeholder="Name or subject" /></label>
     <button class="btn btn--quiet" (click)="book.reload()">Refresh</button>
   </div>
   <nav class="mail-filters" aria-label="Mail folders">
     @for (folder of folders; track folder.id) { <button class="btn btn--quiet btn--small" [attr.aria-pressed]="filter() === folder.id" (click)="filter.set(folder.id); selected.set(null)">{{folder.label}}</button> }
   </nav>
   @if (book.isLoading()) { <p role="status">Loading mail…</p> }
   @if (book.error()) { <p role="alert">Could not load mail. Use Refresh to retry.</p> }
   @if (book.value()?.errors?.length) { <p role="alert">Could not load {{book.value()?.errors?.join(', ')}}. Other mail is shown; use Refresh to retry.</p> }
   <div class="mailbox" [class.mailbox--selected]="!!current()">
     <div class="mail-list" aria-label="Messages">
       @for (row of visible(); track row.id) {
         <button class="mail-row" [class.mail-row--unread]="unread(row)" [class.mail-row--active]="selected() === row.id" (click)="open(row)" [attr.aria-pressed]="selected() === row.id">
           <span class="mail-row__sender">{{row.sender}}</span><time>{{row.at * 1000 | date:'MMM d'}}</time>
           <span class="mail-row__subject">{{row.subject}}</span><span class="mail-row__kind">{{row.attention ? 'Action needed' : row.kind}}{{unread(row) ? ' · New' : ''}}</span>
         </button>
       } @empty { @if (!book.isLoading()) { <p class="muted">No mail in this view.</p> } }
     </div>
     <section class="mail-detail" aria-label="Selected message">
       @if (current(); as row) {
         <button class="btn btn--quiet btn--small mail-back" (click)="selected.set(null)">← Back to mail</button>
         <p class="muted small">{{row.kind}} · {{row.at * 1000 | date:'medium'}}</p>
         <h2>{{row.subject}}</h2><p><strong>{{row.sender}}</strong></p>
         @if (row.amount !== undefined) { <p>{{row.amount | nc}} NC</p> }
         <p class="mail-body">{{row.body}}</p>
         <div class="mail-actions">
           @if (row.route) { <a class="btn" [routerLink]="row.route" [queryParams]="row.route==='/history' ? {tab:row.kind==='Transaction' ? 'transactions' : 'todos'} : {}">{{row.kind === 'Offer' ? 'Review offer' : row.kind === 'Loan' ? 'Review loan' : 'View account'}}</a> }
           @if (row.replyTo) { <a class="btn btn--quiet" routerLink="/send" [queryParams]="{to:row.replyTo, amount:'0'}">{{row.sent ? 'Write again' : 'Reply'}}</a> }
         </div>
       } @else { <p class="muted">Select a row to read it.</p> }
     </section>
   </div>
   <p class="muted small">Shows the latest retained account activity. “New” is tracked in this browser.</p>
 }
 `,
 styleUrl: './messages.css',
})
export class MessagesPage {
 protected readonly session = inject(Session);
 private readonly api = inject(NanacoinService);
 private readonly base = inject(ApiBase);
 protected readonly search = signal(''); protected readonly selected = signal<string | null>(null);
 protected readonly filter = signal<string>('all');
 protected readonly folders = [{id:'all',label:'All mail'},{id:'attention',label:'Needs attention'},{id:'messages',label:'Messages'},{id:'sent',label:'Sent'}];
 private readonly seen = signal<Record<string,string[]>>({});
 private scope(): string { return `${this.base.current()}:${this.session.me()?.account ?? ''}`; }
 protected readonly book = resource({ params: () => this.session.me()?.account ? {account:this.session.me()!.account,source:this.base.current()} : undefined, loader: async ({params}) => {
   const [history,offers,loans,fulfillments] = await Promise.allSettled([this.api.accountHistory(params.account,100),this.api.offers(),this.api.loans(),this.api.fulfillments()]);
   return { rows: mailRows(params.account, history.status === 'fulfilled' ? history.value.transactions : [], offers.status === 'fulfilled' ? offers.value.offers : [], loans.status === 'fulfilled' ? loans.value.loans : [], fulfillments.status === 'fulfilled' ? fulfillments.value.fulfillments : []),
     errors: [history.status === 'rejected' ? 'transactions' : '', offers.status === 'rejected' ? 'offers' : '', loans.status === 'rejected' ? 'loans' : '', fulfillments.status === 'rejected' ? 'fulfillment activity' : ''].filter(Boolean) };
 }});
 /** Catch up when someone else changes the ledger while this page is open. */
 private readonly followLedger = reloadOnLedgerChange(this.book);
 protected readonly visible = computed(() => (this.book.value()?.rows ?? []).filter(row => {
   const folder = this.filter(), q = this.search().toLocaleLowerCase();
   return (folder === 'all' || (folder === 'attention' && (row.attention || (!row.sent && this.unread(row)))) || (folder === 'messages' && row.kind === 'Message') || (folder === 'sent' && row.sent))
     && `${row.sender} ${row.subject} ${row.body}`.toLocaleLowerCase().includes(q);
 }));
 protected readonly current = computed(() => this.book.value()?.rows.find(r => r.id === this.selected()));
 constructor() {
   try { const saved = JSON.parse(localStorage.getItem('nanacoin-mail-seen') ?? '{}'); if (saved && typeof saved === 'object' && !Array.isArray(saved)) this.seen.set(saved); } catch { /* local-only read markers */ }
   const timer = setInterval(() => this.book.reload(),15_000); inject(DestroyRef).onDestroy(() => clearInterval(timer));
 }
 protected unread(row: MailRow): boolean { const seen = this.seen()[this.scope()]; return !row.sent && !(Array.isArray(seen) && seen.includes(row.revision)); }
 protected open(row: MailRow): void {
   this.selected.set(row.id);
   const old = this.seen()[this.scope()];
   const updated = {...this.seen(),[this.scope()]: [...(Array.isArray(old) ? old : []).filter(id => id !== row.revision),row.revision].slice(-512)};
   this.seen.set(updated); try {localStorage.setItem('nanacoin-mail-seen',JSON.stringify(updated));} catch { /* still read this session */ }
 }
}
