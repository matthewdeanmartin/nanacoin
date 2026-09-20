import { Component, inject, resource } from '@angular/core';
import { DatePipe } from '@angular/common';
import { HttpClient } from '@angular/common/http';
import { firstValueFrom } from 'rxjs';
import { RouterLink } from '@angular/router';
import { IS_DEMO } from '../demo/demo';
import { Transaction } from '../api/models';
import { NanacoinService } from '../api/nanacoin.service';
import { Session } from '../api/session';
import { Notebook } from '../ui/notebook';

@Component({
 selector: 'app-public-ledger', imports: [Notebook, RouterLink, DatePipe],
 template: `<h1>Nana’s notebook</h1><p><a routerLink="/recipes">Vegetarian or vegan lemon bar?</a> The recipes are for everyone; eating is optional.</p>
 <p>{{ demo ? 'Public fictional ledger: the latest 100 entries in this browser tab. No household information is published.' : 'Real household ledgers remain Nana-only. This page does not make private transactions public.' }}</p>
 @if (!demo && !session.isNana()) { <p>Sign in as Nana to see this board’s ledger. The public showcase uses fictional data instead.</p> }
 @else if (data.isLoading()) { <p>Opening the notebook…</p> }
 @else if (data.error()) { <p role="alert">Could not open the ledger. <button class="btn" (click)="data.reload()">Retry</button></p> }
 @else { <app-notebook><div class="ledger">
 @for (t of data.value()?.transactions ?? []; track t.id) {
 <article class="ledger-row" data-keyboard-row tabindex="-1"><p>{{ t.created_at * 1000 | date:'medium' }} · {{ t.id }} · {{ t.kind }}</p>
 <p>{{ t.description }}</p>
 @for (p of t.postings; track $index) { <span class="posting">{{ p.name }}: {{ p.amount >= 0 ? '+' : '' }}{{ p.amount }} NC </span> }
 </article> } @empty { <p>No transactions yet.</p> }
 </div></app-notebook> }`,
})
export class PublicLedger {
 readonly demo = IS_DEMO;
 readonly session = inject(Session);
 private readonly http = inject(HttpClient);
 private readonly api = inject(NanacoinService);
 readonly data = resource({
   params: () => this.session.isNana(),
   loader: ({ params }) => IS_DEMO
     ? firstValueFrom(this.http.get<{ transactions: Transaction[] }>('/api/v1/public/ledger'))
     : params ? this.api.ledger(100) : Promise.resolve({ transactions: [] }),
 });
}
