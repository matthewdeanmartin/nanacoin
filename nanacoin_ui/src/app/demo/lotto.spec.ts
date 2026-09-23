import { DemoLedger, SYSTEM_ISSUANCE } from './ledger';
import { LottoKind } from '../api/models';

describe('demo lotto accounting',()=>{
  function setup(kind: LottoKind) {
    const ledger=new DemoLedger();ledger.provision('nana','Nana','demo','Test');
    const nana=ledger.userByName('nana')!, alice=ledger.addUser('alice','Alice','demo','user'),bob=ledger.addUser('bob','Bob','demo','user');
    ledger.issue(nana,alice.account,100,'Start');ledger.issue(nana,bob.account,100,'Start');ledger.issue(nana,nana.account,1,'Interest reserve');
    const draw=ledger.lotto.create(nana,{title:'Test',kind,ticket_price:10,closes_at:100,rate_bps:kind==='SIMPLE'?0:1000},0);
    return {ledger,nana,alice,bob,draw};
  }
  for(const kind of ['SIMPLE','DELAYED','SAVINGS'] as const) it(`settles ${kind} once with conserved principal and correctly funded interest`,()=>{
    const {ledger,nana,alice,bob,draw}=setup(kind);
    ledger.lotto.buy(alice,draw.id,2,1);ledger.lotto.buy(bob,draw.id,1,1);
    expect(ledger.balanceOf(alice.account)).toBe(80);
    ledger.lotto.tick(100);
    if(kind!=='SIMPLE') {expect(ledger.lotto.book(alice)[0].status).toBe('WAITING');expect(ledger.balanceOf(alice.account)).toBe(80);}
    ledger.lotto.tick(draw.due_at);
    const result=ledger.lotto.book(alice)[0],interest=kind==='SIMPLE'?0:3;
    expect(result.status).toBe('SETTLED');expect(result.my_tickets).toBe(2);expect(result.interest).toBe(interest);
    expect(ledger.balanceOf('lotto-pool-1')).toBe(0);
    expect(ledger.balanceOf(alice.account)+ledger.balanceOf(bob.account)).toBe(200+interest);
    expect(ledger.balanceOf(nana.account)).toBe(kind==='SIMPLE'?1:0);
    expect(ledger.balanceOf(SYSTEM_ISSUANCE)).toBe(kind==='SIMPLE'?-201:-203);
    if(kind==='SAVINGS') {expect(ledger.balanceOf(alice.account)).toBe(100+(result.winner===alice.account?3:0));expect(ledger.balanceOf(bob.account)).toBe(100+(result.winner===bob.account?3:0));}
    const before=ledger.ledger(100).transactions.length;ledger.lotto.tick(draw.due_at+1);expect(ledger.ledger(100).transactions.length).toBe(before);
    const payout=ledger.ledger(100).transactions[0];expect(()=>ledger.reverse(nana,payout.id,'undo')).toThrow();
  });
  it('rejects invalid purchases without changing money or tickets',()=>{
    const {ledger,nana,alice,draw}=setup('SIMPLE');
    for(const count of [0,-1,1.5,11]) expect(()=>ledger.lotto.buy(alice,draw.id,count,1)).toThrow();
    expect(()=>ledger.lotto.buy(nana,draw.id,1,1)).toThrow();
    expect(()=>ledger.lotto.buy(alice,draw.id,1,100)).toThrow();
    expect(ledger.balanceOf(alice.account)).toBe(100);expect(ledger.lotto.book(alice)[0].tickets).toBe(0);
  });
  for(const kind of ['SIMPLE','DELAYED','SAVINGS'] as const) it(`allows only active Nana to force ${kind} and never pays twice`,()=>{
    const {ledger,nana,alice,draw}=setup(kind);
    ledger.lotto.buy(alice,draw.id,2,1);
    expect(()=>ledger.lotto.resolveNow(alice,2)).toThrow();
    expect(()=>ledger.lotto.resolveNow({...nana,status:'DISABLED'},2)).toThrow();
    if(kind==='DELAYED') ledger.lotto.tick(100); // Include already waiting draws.
    expect(ledger.lotto.resolveNow(nana,101)).toBe(1);
    expect(ledger.lotto.book(alice)[0]).toMatchObject({status:'SETTLED',winner:alice.account,due_at:101,interest:kind==='SIMPLE'?0:2});
    expect(ledger.balanceOf(alice.account)).toBe(kind==='SIMPLE'?100:102);
    const before=ledger.ledger(100).transactions.length;
    expect(ledger.lotto.resolveNow(nana,102)).toBe(0);
    ledger.lotto.tick(draw.due_at+1);
    expect(ledger.ledger(100).transactions.length).toBe(before);
    expect(()=>ledger.lotto.buy(alice,draw.id,1,102)).toThrow();
  });
});
