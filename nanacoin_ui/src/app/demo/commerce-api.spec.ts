import { HttpRequest, HttpResponse } from '@angular/common/http';
import { firstValueFrom } from 'rxjs';
import { demoBackend, demoLedger } from './demo-backend';

describe('standalone commerce API',()=>{
  it('routes authenticated commands and replays keyed contributions without double payment',async()=>{
    let token='';
    const send=async(path:string,body?:unknown,key?:string)=>{
      const req=new HttpRequest(body===undefined?'GET':'POST',`/api/v1${path}`,body??null);
      const event=await firstValueFrom(demoBackend(req.clone({setHeaders:{Authorization:`Bearer ${token}`,...(key?{'Idempotency-Key':key}:{})}}),()=>{throw new Error('Demo request escaped to the network');}));
      return (event as HttpResponse<any>).body;
    };
    const auth=await send('/auth/authorize',{username:'dad'});
    token=(await send('/auth/token',{code:auth.code})).access_token;
    const book=await send('/commerce');const request=book.requests[0];
    const dad=demoLedger.userByName('dad')!;
    const before=demoLedger.balanceOf(dad.account);
    const action={contribute:{request:request.id,amount:10_000,memo:'API gift'}};
    const key=`demo:e${demoLedger.moneyEpoch}:commerce-retry`;
    const receipt=await send('/commerce/commands',action,key);
    expect(await send('/commerce/commands',action,key)).toEqual(receipt);
    expect(demoLedger.balanceOf(dad.account)).toBe(before-10_000);
    expect((await send('/commerce')).requests[0].received).toBe(request.received+10_000);
    await expect(send('/commerce/commands',{contribute:{...action.contribute,amount:20_000}},key)).rejects.toMatchObject({status:409});
    const payment=demoLedger.ledger(1).transactions[0];
    await expect(send(`/transactions/${payment.id}/refund`,{amount:10_000,reason:'Not recipient'},`demo:e${demoLedger.moneyEpoch}:unauthorized-refund`)).rejects.toMatchObject({status:403});
    const ivyAuth=await send('/auth/authorize',{username:'ivy'});
    token=(await send('/auth/token',{code:ivyAuth.code})).access_token;
    const refundKey=`demo:e${demoLedger.moneyEpoch}:refund-retry`;
    const refund=await send(`/transactions/${payment.id}/refund`,{amount:10_000,reason:'Return gift'},refundKey);
    expect(await send(`/transactions/${payment.id}/refund`,{amount:10_000,reason:'Return gift'},refundKey)).toEqual(refund);
    expect(demoLedger.balanceOf(dad.account)).toBe(before);
    expect((await send('/commerce')).requests[0].received).toBe(request.received);
  });
});
