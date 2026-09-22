import { Quote, Transaction } from '../api/models';
import { Series } from './series';
export type RateDirection = 'USD_PER_NC' | 'NC_PER_USD';
/** Completed quote records cover trades whose ledger legs have left the retained window. */
export function actualRates(quotes: Quote[], transactions: Transaction[], decimals: number): { at: number; cents: number }[] {
 const trades = new Map<string,{at:number;cents:number}>();
 for (const q of quotes) if (q.status === 'FILLED') trades.set(q.id,{at:q.updated_at,cents:q.cents_per_coin});
 const legs = new Map<string,{coins?:number;cash?:number;at:number;reversed:boolean}>();
 for (const t of transactions) {
   if (!t.reference?.startsWith('quote-') || t.kind !== 'TRANSFER') continue;
   const pair = legs.get(t.reference) ?? {at:t.created_at,reversed:false};
   pair.reversed ||= !!t.reversed_by;
   const positive = t.postings.reduce((n,p)=>n+Math.max(0,p.amount),0);
   if (t.postings.some(p=>p.account.endsWith('-usd'))) pair.cash=positive; else pair.coins=positive;
   legs.set(t.reference,pair);
 }
 for (const [id,p] of legs) {
   if (p.reversed) { trades.delete(id); continue; }
   if (p.cash && p.coins) trades.set(id,{at:p.at,cents:p.cash*10**decimals/p.coins});
 }
 return [...trades.values()].filter(p=>Number.isFinite(p.cents)&&p.cents>0).sort((a,b)=>a.at-b.at);
}
export function forexSeries(quotes: Quote[], transactions: Transaction[], decimals: number, direction: RateDirection): Series[] {
 const convert=(c:number)=>direction==='USD_PER_NC'?c/100:100/c;
 const posted=(side:'BID'|'ASK'):Series=>({name:side==='BID'?'Posted bids':'Posted asks',points:quotes.filter(q=>q.side===side && q.cents_per_coin>0).map(q=>({at:q.created_at,value:convert(q.cents_per_coin)})).sort((a,b)=>a.at-b.at)});
 return [posted('BID'),posted('ASK'),{name:'Completed trades',points:actualRates(quotes,transactions,decimals).map(p=>({at:p.at,value:convert(p.cents)}))}];
}
