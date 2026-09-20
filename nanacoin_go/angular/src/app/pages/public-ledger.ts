import { Component, inject, resource } from '@angular/core';
import { DatePipe } from '@angular/common';
import { HttpClient } from '@angular/common/http';
import { firstValueFrom } from 'rxjs';
import { RouterLink } from '@angular/router';
import { IS_DEMO } from '../demo/demo';
import { Transaction } from '../api/models';
import { NanacoinService } from '../api/nanacoin.service';
import { Notebook } from '../ui/notebook';

@Component({
 selector: 'app-public-ledger', imports: [Notebook, RouterLink, DatePipe],
 template: `<h1>The Notebook</h1><p class="notebook-treat"><a routerLink="/recipes">Vegetarian and vegan recipes</a> are available to everyone; eating is optional.</p>
 <p>{{ demo ? 'Public fictional household ledger: everyone can audit the latest 100 entries in this browser tab. Private descriptions are on the roadmap.' : 'The household ledger is public: anyone can audit every transaction. Private descriptions are on the roadmap.' }}</p>
 @if (data.isLoading()) { <p>Opening the notebook…</p> }
 @else if (data.error()) { <p role="alert">Could not open the ledger. <button class="btn" title="Try loading the notebook again" (click)="data.reload()">Retry</button></p> }
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
 private readonly http = inject(HttpClient);
 private readonly api = inject(NanacoinService);
 readonly data = resource({
   params: () => true,
   loader: () => IS_DEMO
     ? firstValueFrom(this.http.get<{ transactions: Transaction[] }>('/api/v1/public/ledger'))
     : this.api.ledger(100),
 });
}
