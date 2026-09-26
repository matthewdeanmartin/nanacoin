import { TestBed } from '@angular/core/testing';
import { provideHttpClient } from '@angular/common/http';
import { HttpTestingController, provideHttpClientTesting } from '@angular/common/http/testing';
import { ApiBase } from './api-base';
import { NanacoinService } from './nanacoin.service';

describe('immutable ledger HTTP presentation and paging', () => {
  let api: NanacoinService;
  let http: HttpTestingController;
  beforeEach(() => {
    TestBed.configureTestingModule({providers:[provideHttpClient(),provideHttpClientTesting()]});
    TestBed.inject(ApiBase).set('http://ledger-test.local');
    api = TestBed.inject(NanacoinService); http = TestBed.inject(HttpTestingController);
  });
  afterEach(() => { http.verify(); TestBed.resetTestingModule(); });
  const rows = (start: number, count: number, epoch = 1) => Array.from({length:count},(_,i) => ({ id:`tx-${start-i}`, postings:[{account:'account-2',name:'Alice',amount:10}], current_postings:[{account:'account-2',name:'Alice',amount:100}], current_money_epoch:epoch }));
  it('collects a 365-row report using bounded pages and preserves historical facts', async () => {
    const pending = api.ledger(365);
    for (let i=0;i<4;i++) {
      const remaining = 365-i*100;
      const request = await vi.waitFor(() => http.expectOne((r) => r.url.includes(`/transactions?limit=${Math.min(remaining,100)}`)));
      if(i>0) expect(request.request.url).toContain(`cursor=0:365:${365-(i*100)}:0`);
      request.flush({transactions:rows(365-i*100,Math.min(remaining,100)), circulation:2000, snapshot_upper:365, next_cursor:i<3?`0:365:${265-i*100}:0`:null, history_truncated:false});
    }
    const result = await pending;
    expect(result.transactions).toHaveLength(365);
    expect(result.transactions[0].postings[0].amount).toBe(100);
    expect(result.transactions[0].original_postings?.[0].amount).toBe(10);
    expect(result.next_cursor).toBeNull();
  });
  it('rejects a reform between pages rather than mixing units', async () => {
    const pending = api.ledger(101);
    const rejected = expect(pending).rejects.toThrow(/currency was reformed/);
    http.expectOne((r)=>r.url.endsWith('limit=100')).flush({transactions:rows(101,100),snapshot_upper:101,next_cursor:'0:101:1:0'});
    const next = await vi.waitFor(()=>http.expectOne((r)=>r.url.includes('cursor=')));
    next.flush({transactions:rows(1,1,2),snapshot_upper:101,next_cursor:null});
    await rejected;
  });
  it('surfaces an exact-conversion error from a single transaction endpoint', async () => {
    const pending = api.transaction('tx-1');
    const rejected = expect(pending).rejects.toThrow(/cannot be represented exactly/);
    http.expectOne((r)=>r.url.endsWith('/transactions/tx-1')).flush({...rows(1,1)[0],current_postings:null});
    await rejected;
  });
});
