import { TestBed } from '@angular/core/testing';
import { provideHttpClient } from '@angular/common/http';
import { provideHttpClientTesting } from '@angular/common/http/testing';
import { provideRouter } from '@angular/router';
import { NanacoinService } from '../api/nanacoin.service';
import { Session } from '../api/session';
import { HistoryPage } from './history';

describe('My Account tabs', () => {
  afterEach(()=>TestBed.resetTestingModule());
  it('shows counts and only the selected panel, with arrow-key navigation', async () => {
    TestBed.configureTestingModule({providers:[provideHttpClient(),provideHttpClientTesting(),provideRouter([])]});
    const api=TestBed.inject(NanacoinService);
    TestBed.inject(Session).me.set({id:'user-2',account:'account-2',username:'dad',display_name:'Dad',role:'user',status:'ACTIVE',balance:0} as never);
    vi.spyOn(api,'accountHistory').mockResolvedValue({account:'account-2',balance:0,transactions:[]});
    vi.spyOn(api,'offers').mockResolvedValue({offers:[]});
    vi.spyOn(api,'quotes').mockResolvedValue({quotes:[]} as never);
    vi.spyOn(api,'loans').mockResolvedValue({loans:[]} as never);
    vi.spyOn(api,'lottos').mockResolvedValue({lottos:[]} as never);
    vi.spyOn(api,'fulfillments').mockResolvedValue({fulfillments:[{transaction:'tx-1',provider:'account-2',recipient:'account-3',provider_name:'Dad',recipient_name:'Mom',description:'Wash car',kind:'WORK',status:'TODO',updates:[]}]});
    const fixture=TestBed.createComponent(HistoryPage); fixture.detectChanges(); await fixture.whenStable(); fixture.detectChanges();
    const root=fixture.nativeElement as HTMLElement;
    const visible=()=>Array.from(root.querySelectorAll('[role="tabpanel"]')).filter(el=>!el.closest('[hidden]')).map(el=>el.id);
    expect(root.querySelector('#tab-todos')?.textContent).toContain('TODO (1)');
    expect(root.querySelector('#tab-loans')?.textContent).toContain('Loans & debts (0)');
    expect(visible()).toEqual(['panel-todos']);
    (root.querySelector('#tab-loans') as HTMLButtonElement).click();fixture.detectChanges();
    expect(visible()).toEqual(['panel-loans']);
    root.querySelector('#tab-loans')!.dispatchEvent(new KeyboardEvent('keydown',{key:'ArrowRight',bubbles:true}));fixture.detectChanges();
    expect(visible()).toEqual(['panel-lotto']);
    expect(root.querySelector('#tab-lotto')?.getAttribute('aria-selected')).toBe('true');
  });
});
