import { DemoLedger } from './ledger';
import { seed } from './seed';
import { centralBankFlows } from '../economy/central-bank';

function household() {
  const l=new DemoLedger();l.provision('nana','Nana','demo','Test');
  const nana=l.userByName('nana')!,buyer=l.addUser('buyer','Buyer','demo','user'),artist=l.addUser('artist','Artist','demo','user');
  l.issue(nana,buyer.account,100_000,'Opening balance');
  return {l,nana,buyer,artist};
}
describe('demo commerce',()=>{
  it('keeps partial refunds linked and bounded, and nets gift request progress',()=>{
    const {l,nana,buyer,artist}=household();
    const request=l.commerce.command(artist,{create_request:{title:'Paint fund',description:'Thank you',target:100_000,deadline:null}}).sequence;
    l.commerce.command(buyer,{contribute:{request,amount:60_000,memo:'Paints'}});
    const original=l.ledger(1).transactions[0];
    expect(()=>l.refund(buyer,original.id,20_000,'No permission')).toThrow();
    l.refund(artist,original.id,20_000,'One fewer tube');
    const partial=l.ledger(10).transactions.find(t=>t.id===original.id)!;
    expect(partial.postings).toEqual(original.postings);expect(partial.reversed_by).toBeUndefined();expect(partial.refunded).toBe(20_000);
    expect(l.commerce.book().requests[0].received).toBe(40_000);
    const snapshot=l.ledger(100);
    expect(()=>l.refund(nana,original.id,40_001,'Too much')).toThrow();expect(l.ledger(100)).toEqual(snapshot);
    l.refund(artist,original.id,40_000,'Return remainder');
    expect(l.commerce.book().requests[0].received).toBe(0);
    expect(l.ledger(10).transactions.find(t=>t.id===original.id)?.reversed_by).toBeTruthy();
    expect(l.balanceOf(buyer.account)).toBe(100_000);expect(l.balanced()).toBe(true);
  });
  it('rejects stale art buys without payment and changes ownership atomically',()=>{
    const {l,nana,buyer,artist}=household();
    const art=l.commerce.command(artist,{mint_art:{title:'Moon',license:'Profile display',sha256:'a'.repeat(64),locator:'https://example.org/moon.svg'}}).sequence;
    const listing=l.commerce.command(artist,{list_art:{art,price:60_000}}).sequence;
    const purchase={art,expected_owner:3,expected_revision:listing,expected_price:60_000};
    expect(()=>l.commerce.command(buyer,{buy_art:{...purchase,expected_price:50_000}})).toThrow();
    expect(l.balanceOf(buyer.account)).toBe(100_000);
    l.commerce.command(buyer,{buy_art:purchase});
    const payment=l.ledger(1).transactions[0];expect(payment.fulfillment).toBeUndefined();expect(payment.art).toBe(art);
    expect(l.commerce.book().artworks[0]).toMatchObject({owner:2,price:null,equipped:false});
    expect(()=>l.commerce.command(buyer,{buy_art:purchase})).toThrow();
    expect(()=>l.reverse(nana,payment.id,'Cash only')).toThrow();expect(()=>l.refund(artist,payment.id,60_000,'Cash only')).toThrow();
    l.commerce.command(buyer,{equip_art:{art,equipped:true}});
    l.commerce.command(buyer,{gift_art:{art,to:1}});
    expect(l.commerce.book().artworks[0]).toMatchObject({owner:1,creator:3,equipped:false});expect(l.balanced()).toBe(true);
  });
  it('preserves historical amounts across reform and refunds in original units',()=>{
    const {l,nana,buyer,artist}=household();
    const t=l.transfer(buyer,artist.account,20_000,'Paints');
    l.issueUSD(nana,nana.account,10_000,'Dollar reserve');
    const before=structuredClone(t);
    l.reform(nana,{decimals:5,power:0,expected_epoch:0,expected_sequence:l.revision,preview:false});
    expect(t).toEqual(before);expect(l.balanceOf(artist.account)).toBe(200_000);
    l.refund(artist,t.id,5_000,'Partial');
    expect(l.balanceOf(artist.account)).toBe(150_000);expect(l.view(nana,nana).usd_cents).toBe(10_000);
    expect(l.ledger(10).transactions.find(r=>r.id===t.id)).toMatchObject({refunded:5_000,original_postings:before.postings});
  });
});
describe('showcase coverage',()=>{
  it('pages back to reserves and reconciles Nana’s book from seeded transactions',()=>{
    const l=new DemoLedger();seed(l);const nana=l.userByName('nana')!;
    let page=l.ledgerPage(100,null);const transactions=[...page.transactions];
    expect(page.next_cursor).toMatch(/^\d+:\d+:\d+:\d+$/);
    while(page.next_cursor){page=l.ledgerPage(100,page.next_cursor);transactions.push(...page.transactions);}
    expect(new Set(transactions.map(t=>t.id)).size).toBe(transactions.length);
    expect(transactions).toEqual(l.ledger(1000).transactions);
    const flows=centralBankFlows(transactions,nana.account);
    expect(flows.issuedToNana).toBe(10_000_000);expect(flows.usdRecorded).toBe(30_000);
    expect(flows.interestPaid).toBe(4_000);expect(flows.lent).toBe(100_000);
    expect(flows.interestReceived).toBeGreaterThan(0);expect(flows.repaid+flows.interestReceived).toBe(20_000);
    const recent=l.ledger(100).transactions;
    expect(recent.some(t=>t.gift_request && t.economic_kind==='GIFT')).toBe(true);
    expect(recent.some(t=>t.art && t.economic_kind==='GOOD')).toBe(true);
    expect(recent.some(t=>t.kind==='REVERSAL' && t.description.startsWith('Partial refund:'))).toBe(true);
    expect(l.commerce.book().artworks.some(a=>a.owner===1&&a.equipped)).toBe(true);
    expect(l.commerce.book().artworks.some(a=>a.price!==null)).toBe(true);
    expect(l.balanced()).toBe(true);
  });
});
