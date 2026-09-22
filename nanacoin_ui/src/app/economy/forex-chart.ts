import { Component, computed, input, signal } from '@angular/core';
import { Quote, Transaction } from '../api/models';
import { LineChart } from './line-chart';
import { actualRates, forexSeries, RateDirection } from './forex-series';
@Component({
 selector:'app-forex-chart',imports:[LineChart],
 template:`
 <section class="chart-controls">
   <label>Exchange-rate display <select [value]="direction()" (change)="direction.set($any($event.target).value)"><option value="USD_PER_NC">Dollars per NC</option><option value="NC_PER_USD">NC per dollar</option></select></label>
   <p class="rates"><span>Best live bid: <strong>{{label(bestBid())}}</strong></span><span>Best live ask: <strong>{{label(bestAsk())}}</strong></span><span>Last completed trade: <strong>{{label(lastTrade())}}</strong></span></p>
 </section>
 <app-line-chart title="Exchange rates" [series]="series()" [zeroBased]="false" [subtitle]="direction() === 'USD_PER_NC' ? 'Dollars per NC · retained posted bids, asks, and completed trades.' : 'NC per dollar · retained posted bids, asks, and completed trades.'" />
 <p class="muted small">Quoted rates are offers, not completed exchanges. Historical quote points show when rates were posted; the live bid and ask above show what is available now.</p>
 `,
 styles:[`.rates {display:flex;flex-wrap:wrap;gap:.5rem 1.25rem;font-size:.85rem;} :host {display:block;min-width:0;}`],
})
export class ForexChart {
 readonly quotes=input<Quote[]>([]);readonly transactions=input<Transaction[]>([]);readonly decimals=input(4);
 protected readonly direction=signal<RateDirection>('USD_PER_NC');
 protected readonly series=computed(()=>forexSeries(this.quotes(),this.transactions(),this.decimals(),this.direction()));
 protected readonly bestBid=computed(()=>{const q=this.quotes().filter(q=>q.live&&q.side==='BID');return q.length?Math.max(...q.map(q=>q.cents_per_coin)):null;});
 protected readonly bestAsk=computed(()=>{const q=this.quotes().filter(q=>q.live&&q.side==='ASK');return q.length?Math.min(...q.map(q=>q.cents_per_coin)):null;});
 protected readonly lastTrade=computed(()=>actualRates(this.quotes(),this.transactions(),this.decimals()).at(-1)?.cents ?? null);
 protected label(cents:number|null):string {if(cents===null)return 'None yet'; const value=this.direction()==='USD_PER_NC'?cents/100:100/cents;return `${value.toLocaleString(undefined,{maximumFractionDigits:6})} ${this.direction()==='USD_PER_NC'?'USD/NC':'NC/USD'}`;}
}
