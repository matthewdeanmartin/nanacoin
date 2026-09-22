import { Offer, Transaction, Lotto } from '../api/models';
import { offerGroup, offerPayment, offerSentence } from './offer-language';
import { mailRows } from './mail-model';
import { lottoOutcome } from './account-commitments';
const offer: Offer={id:'offer-1',listing:'listing-1',listing_title:'Cookies',listing_owner:'account-2',listing_owner_name:'Alice',listing_side:'SELL',offerer:'account-3',offerer_name:'Bob',amount:100,message:'Saturday?',status:'OPEN',created_at:1,updated_at:2};
describe('bank activity presentation',()=>{
 it('distinguishes all four offer directions and the person who pays',()=>{
   expect(offerGroup(offer,'account-2')).toBe('received-buy');expect(offerGroup(offer,'account-3')).toBe('sent-buy');
   const sell={...offer,listing_side:'BUY' as const};expect(offerGroup(sell,'account-2')).toBe('received-sell');expect(offerGroup(sell,'account-3')).toBe('sent-sell');
   expect(offerSentence(offer)).toBe('Bob made an offer to buy from Alice.');expect(offerSentence(sell)).toBe('Bob made an offer to sell to Alice.');
   expect(offerPayment(offer)).toBe('Bob pays Alice');expect(offerPayment(sell)).toBe('Alice pays Bob');expect(offerGroup(offer,'account-1')).toBeNull();
 });
 it('recognizes zero-posting sender order and combines offers without leaking other peoples requests',()=>{
   const tx:Transaction={id:'tx-2',kind:'MESSAGE',created_at:3,actor:'user-3',description:'Hello',postings:[{account:'account-3',name:'Bob',amount:0},{account:'account-2',name:'Alice',amount:0}]};
   const rows=mailRows('account-2',[tx],[offer,{...offer,id:'offer-9',listing_owner:'account-8',offerer:'account-7'}],[]);
   expect(rows).toHaveLength(2);expect(rows[0]).toMatchObject({sender:'From Bob',kind:'Message',sent:false,replyTo:'account-3',amount:undefined});expect(rows[1].attention).toBe(true);
   expect(mailRows('account-3',[tx],[],[])[0]).toMatchObject({sender:'To Alice',sent:true});
 });
 it('shows lotto net gains, losses, and no-loss principal returns correctly',()=>{
   const lotto={terms:{kind:'DELAYED',ticket_price:100},my_tickets:2,pool:1000,interest:50,status:'SETTLED',winner:'account-2'} as Lotto;
   expect(lottoOutcome(lotto,'account-2')).toEqual({label:'Won',net:850});
   expect(lottoOutcome(lotto,'account-3')).toEqual({label:'No win',net:-200});
   expect(lottoOutcome({...lotto,terms:{...lotto.terms,kind:'SAVINGS'}},'account-3')).toEqual({label:'Principal returned · no interest win',net:0});
   expect(lottoOutcome({...lotto,status:'PAYING'},'account-2').label).toBe('Payment in progress');
 });
});
