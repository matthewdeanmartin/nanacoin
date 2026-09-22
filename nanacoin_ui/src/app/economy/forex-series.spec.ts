import { Quote, Transaction } from '../api/models';
import { actualRates, forexSeries } from './forex-series';
const q=(id:string,status:Quote['status'],side:Quote['side'],rate:number):Quote=>({id,status,side,cents_per_coin:rate,coins:10000,cents:rate,maker:'account-1',maker_name:'Nana',created_at:10,updated_at:20,live:status==='OPEN',expires_at:0});
describe('exchange chart',()=>{
 it('does not mistake bids and cancelled asks for completed trades',()=>{
   const quotes=[q('quote-1','OPEN','BID',10),q('quote-2','CANCELLED','ASK',15),q('quote-3','FILLED','ASK',12)];
   const series=forexSeries(quotes,[],4,'USD_PER_NC');expect(series[0].points).toEqual([{at:10,value:.1}]);expect(series[2].points).toEqual([{at:20,value:.12}]);
   expect(forexSeries(quotes,[],4,'NC_PER_USD')[2].points[0].value).toBe(100/12);
 });
 it('recovers recycled quotes from both actual ledger legs without counting them twice',()=>{
   const leg=(usd:boolean,amount:number):Transaction=>({id:usd?'tx-4097':'tx-1',kind:'TRANSFER',reference:'quote-3',created_at:20,actor:'user-1',description:'Exchange',postings:[{account:`account-1${usd?'-usd':''}`,name:'Nana',amount:-amount},{account:`account-2${usd?'-usd':''}`,name:'Alice',amount}]});
   const tx=[leg(false,20000),leg(true,24)];expect(actualRates([],tx,4)).toEqual([{at:20,cents:12}]);expect(actualRates([q('quote-3','FILLED','ASK',12)],tx,4)).toHaveLength(1);
   expect(actualRates([],tx.slice(0,1),4)).toEqual([]);expect(actualRates([q('quote-3','FILLED','ASK',12)],[{...tx[0],reversed_by:'tx-9'},tx[1]],4)).toEqual([]);
 });
});
