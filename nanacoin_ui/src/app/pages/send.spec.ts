import { TestBed } from '@angular/core/testing';
import { provideHttpClient } from '@angular/common/http';
import { provideHttpClientTesting } from '@angular/common/http/testing';
import { provideRouter } from '@angular/router';
import { signal } from '@angular/core';
import { SendPage } from './send';
import { NanacoinService } from '../api/nanacoin.service';
import { Session } from '../api/session';
import { Mastodon } from '../api/mastodon';
import { Transaction } from '../api/models';

describe('bank messages',()=>{
 afterEach(()=>TestBed.resetTestingModule());
 async function setup() {
   const mastodon={connected:signal(false),sendDirect:vi.fn()};
   TestBed.configureTestingModule({providers:[provideHttpClient(),provideHttpClientTesting(),provideRouter([]),{provide:Mastodon,useValue:mastodon}]});
   const api=TestBed.inject(NanacoinService),session=TestBed.inject(Session);
   session.me.set({id:'user-2',account:'account-2',display_name:'Alice',role:'user',status:'ACTIVE',balance:10000} as never);
   session.household.set([{id:'user-3',account:'account-3',display_name:'Bob',role:'user',status:'ACTIVE'} as never]);
   session.refresh=()=>Promise.resolve();
   api.accountHistory=()=>Promise.resolve({account:'account-2',balance:10000,transactions:[]});
   const send=vi.spyOn(api,'transfer').mockResolvedValue({id:'tx-5',kind:'MESSAGE'} as Transaction);
   const fixture=TestBed.createComponent(SendPage);fixture.detectChanges();await fixture.whenStable();
   const page=fixture.componentInstance as unknown as {to:string;amount:string|null;memo:string;send():Promise<void>};page.to='account-3';
   return {page,send,mastodon};
 }
 for(const amount of ['0',null,'']) it(`saves ${String(amount)} as a zero-value transaction without Mastodon`,async()=>{
   const {page,send,mastodon}=await setup();page.amount=amount;page.memo='Can you return the ladder?';await page.send();
   expect(send).toHaveBeenCalledWith('account-3',0,'Can you return the ladder?',expect.any(String),undefined);expect(mastodon.sendDirect).not.toHaveBeenCalled();
 });
 it('rejects blank messages and reuses the same key after an uncertain send',async()=>{
   const {page,send}=await setup();page.amount='0';page.memo=' ';await page.send();expect(send).not.toHaveBeenCalled();
   page.memo='A note';send.mockRejectedValueOnce(new Error('Connection lost'));await page.send();await page.send();
   expect(send).toHaveBeenCalledTimes(2);expect(send.mock.calls[0][3]).toBe(send.mock.calls[1][3]);
 });
});
