import { Component, DestroyRef, computed, inject, resource } from '@angular/core';
import { DatePipe } from '@angular/common';
import { RouterLink } from '@angular/router';
import { MoneyPipe } from '../api/money';
import { NanacoinService } from '../api/nanacoin.service';
import { Session } from '../api/session';
import { Loan, Lotto } from '../api/models';

export function lottoOutcome(l: Lotto, account: string): { label: string; net: number } {
 const cost = l.my_tickets*l.terms.ticket_price;
 if (l.status !== 'SETTLED') return {label: l.status === 'PAYING' ? 'Payment in progress' : 'Upcoming draw', net: 0};
 const won = l.winner === account;
 const net = (l.terms.kind === 'SAVINGS' ? cost : won ? l.pool : 0) + (won ? l.interest : 0) - cost;
 return {label: won ? 'Won' : l.terms.kind === 'SAVINGS' ? 'Principal returned · no interest win' : 'No win', net};
}
@Component({
 selector:'app-account-commitments', imports:[MoneyPipe,DatePipe,RouterLink],
 template:`
 <section id="my-loans" class="account-section">
   <div class="section-heading"><h2>Loans & debts</h2><a routerLink="/loans">Manage loans</a></div>
   @if (loans.isLoading()) { <p role="status">Loading loans…</p> }
   @if (loans.error()) { <p role="alert">Could not load loans. <button class="btn btn--quiet" (click)="loans.reload()">Retry</button></p> }
   @for (group of loanGroups(); track group.title) {
     <h3>{{group.title}}</h3><p class="muted small">{{group.description}}</p>
     <p>Outstanding principal: <strong>{{group.principal | nc}} NC</strong> · Accrued interest: {{group.interest | nc}} NC</p>
     <div class="account-summary-list">
       @for (loan of group.items; track loan.id) {
         <a routerLink="/loans"><span>{{loan.lender_name}} → {{loan.borrower_name}} · {{loan.status.toLowerCase()}}<br><small>{{loan.memo}}</small></span>
           <span>{{loan.principal | nc}} NC principal · {{loan.interest | nc}} NC interest
           @if (loan.overdue) { <br><strong>{{loan.overdue | nc}} NC overdue</strong> }
           @if (loan.next_due_at) { <br><small>Next due {{loan.next_due_at*1000 | date:'mediumDate'}}</small> }</span></a>
       } @empty { <p class="muted small">None yet.</p> }
     </div>
   }
 </section>
 <section id="my-lotto" class="account-section">
   <div class="section-heading"><h2>Lotto</h2><a routerLink="/lotto">View lottos</a></div>
   @if (lottos.isLoading()) { <p role="status">Loading lotto…</p> }
   @if (lottos.error()) { <p role="alert">Could not load lotto. <button class="btn btn--quiet" (click)="lottos.reload()">Retry</button></p> }
   <h3>Upcoming draws</h3><div class="account-summary-list">
     @for (l of upcoming(); track l.id) {
       <a routerLink="/lotto"><span>{{l.terms.title}} · {{l.my_tickets}} tickets<br><small>{{l.due_at*1000 | date:'medium'}}</small></span><span>{{l.my_tickets*l.terms.ticket_price | nc}} NC entered<br><small>{{outcome(l).label}}</small></span></a>
     } @empty { <p class="muted small">No pending tickets.</p> }
   </div>
   <h3>Recent results</h3><div class="account-summary-list">
     @for (l of results(); track l.id) {
       <a routerLink="/lotto"><span>{{l.terms.title}}<br><small>{{outcome(l).label}}</small></span><span>{{outcome(l).net > 0 ? '+' : ''}}{{outcome(l).net | nc}} NC net<br><small>{{l.due_at*1000 | date:'mediumDate'}}</small></span></a>
     } @empty { <p class="muted small">No completed draws yet.</p> }
   </div>
 </section>
 `,
 styles:[`.account-summary-list a { gap:.75rem; flex-wrap:wrap; } .account-summary-list span { min-width:0; overflow-wrap:anywhere; }`],
})
export class AccountCommitments {
 private readonly api=inject(NanacoinService); private readonly session=inject(Session);
 protected readonly loans=resource({params:()=>this.session.me()?.account,loader:()=>this.api.loans()});
 protected readonly lottos=resource({params:()=>this.session.me()?.account,loader:()=>this.api.lottos()});
 protected readonly loanGroups=computed(()=> {
   const account=this.session.me()?.account, all=this.loans.value()?.loans ?? [];
   return [{title:'Loans · money lent',description:'Money others owe this account.',items:all.filter(l=>l.lender===account)},
     {title:'Debts · money borrowed',description:'Money this account owes to others.',items:all.filter(l=>l.borrower===account)}]
     .map(g=>({...g,principal:g.items.reduce((n,l)=>n+BigInt(l.principal),0n).toString(),interest:g.items.reduce((n,l)=>n+BigInt(l.interest),0n).toString(),items:g.items.slice().sort((a,b)=>b.updated_at-a.updated_at).slice(0,6)}));
 });
 protected readonly upcoming=computed(()=>(this.lottos.value()?.lottos ?? []).filter(l=>l.my_tickets>0 && l.status!=='SETTLED').sort((a,b)=>a.due_at-b.due_at));
 protected readonly results=computed(()=>(this.lottos.value()?.lottos ?? []).filter(l=>l.my_tickets>0 && l.status==='SETTLED').sort((a,b)=>b.due_at-a.due_at).slice(0,6));
 protected outcome(l: Lotto) { return lottoOutcome(l,this.session.me()?.account ?? ''); }
 constructor() { const timer=setInterval(()=>{this.loans.reload();this.lottos.reload();},15_000);inject(DestroyRef).onDestroy(()=>clearInterval(timer)); }
}
