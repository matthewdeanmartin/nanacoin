import { signal } from '@angular/core';
import { TestBed } from '@angular/core/testing';
import { provideRouter } from '@angular/router';

import { Listing } from '../api/models';
import { NanacoinService } from '../api/nanacoin.service';
import { Session } from '../api/session';
import { MarketPage } from './market';

const listing = (id: string, over: Partial<Listing> = {}): Listing => ({
  id, seller: 'account-2', seller_name: 'Ivy', title: id, description: '', price: 10,
  status: 'OPEN', created_at: 0, updated_at: 0, ...over,
} as Listing);

describe('market sections', () => {
  afterEach(() => TestBed.resetTestingModule());

  it('splits the market into tabs instead of one long page', async () => {
    const session = {
      forSale: signal([
        listing('Cookies'), listing('Wash the car', { side: 'BUY' }),
        listing('Help with homework', { kind: 'good_deed', seller: 'account-1' }),
      ]),
      revision: signal(0),
      closed: signal([listing('Old bike', { status: 'CANCELLED' })]),
      me: signal({ account: 'account-3' }), balance: signal(100), household: signal([]), isNana: signal(false),
    };
    TestBed.configureTestingModule({ providers: [provideRouter([]),
      { provide: Session, useValue: session },
      { provide: NanacoinService, useValue: { offers: async () => ({offers:[]}), things: async () => ({ things: [] }), commerceBook: async () => ({ artworks: [] }) } }] });
    const fixture = TestBed.createComponent(MarketPage);
    fixture.detectChanges(); await fixture.whenStable(); fixture.detectChanges();
    const root = fixture.nativeElement as HTMLElement;
    const tabs = [...root.querySelectorAll('[role="tab"]')].map((t) => t.textContent!.trim());
    expect(tabs).toEqual(['Offers to Buy (1)', 'Offers to Sell (1)', 'Art (0)', 'Good Deeds (1)', 'Closed & Cancelled (1)']);
    const visible = () => root.querySelector('[role="tabpanel"]:not([hidden])')!;
    expect(visible().id).toBe('market-sell');
    expect(visible().textContent).toContain('Cookies');
    expect(visible().textContent).not.toContain('Wash the car');

    (root.querySelector('#market-tab-market-buy') as HTMLButtonElement).click(); fixture.detectChanges();
    expect(visible().textContent).toContain('Offer to do this');

    (root.querySelector('#market-tab-market-closed') as HTMLButtonElement).click(); fixture.detectChanges();
    expect(visible().textContent).toContain('Old bike');
    expect(root.querySelector('details.disclosure')).toBeNull();
  });

  it('shows a want-ad response awaiting the owners acceptance on the market card', async()=>{
    const wanted=listing('listing-1',{side:'BUY',seller:'account-3',title:'Playground trip'});
    const session={revision:signal(0),forSale:signal([wanted]),closed:signal([]),me:signal({account:'account-3'}),balance:signal(100),household:signal([]),isNana:signal(false)};
    TestBed.configureTestingModule({providers:[provideRouter([]),{provide:Session,useValue:session},{provide:NanacoinService,useValue:{things:async()=>({things:[]}),commerceBook:async()=>({artworks:[]}),offers:async()=>({offers:[{id:'offer-1',listing:'listing-1',status:'OPEN',offerer_name:'Ivy',amount:35,message:'After lunch'}]})}}]});
    const fixture=TestBed.createComponent(MarketPage); fixture.detectChanges(); await fixture.whenStable(); fixture.detectChanges();
    const root=fixture.nativeElement as HTMLElement;
    (root.querySelector('#market-tab-market-buy') as HTMLButtonElement).click(); fixture.detectChanges();
    const panel=root.querySelector('[role="tabpanel"]:not([hidden])')!;
    expect(panel.textContent).toContain('awaiting your acceptance'); expect(panel.textContent).toContain('Ivy offered'); expect(panel.querySelector('a')?.getAttribute('href')).toBe('/offers');
  });
});
