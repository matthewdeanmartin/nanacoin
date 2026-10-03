import { TestBed } from '@angular/core/testing';
import { Session } from '../api/session';
import { IS_DEMO } from '../demo/demo';
import { Tour } from '../ui/tour';
import { LoginForm } from './login-form';

describe.skipIf(!IS_DEMO)('demo welcome page', () => {
  it('orders the hero, Nana login, essay, tour and account choices, and starts the tour after login', async () => {
    const login=vi.fn().mockResolvedValue(undefined),start=vi.fn().mockResolvedValue(undefined);
    TestBed.configureTestingModule({providers:[
      {provide:Session,useValue:{login,signedIn:()=>true}},
      {provide:Tour,useValue:{start}},
    ]});
    const fixture=TestBed.createComponent(LoginForm);fixture.detectChanges();
    const root=fixture.nativeElement as HTMLElement;
    const content=[...root.querySelectorAll('.panel > *')].map(e=>e.textContent?.trim() ?? '');
    expect(root.querySelector('img')?.getAttribute('src')).toBe('nanacoin.png');
    const essay=content.findIndex(t=>t.startsWith('This is a full implementation'));
    expect(content[essay-1]).toBe('Log in as Nana');
    expect(content[essay+1]).toBe('Take a tour as Nana');
    expect(root.querySelectorAll('.whoami__pick')).toHaveLength(5);
    const tour=[...root.querySelectorAll('button')].find(b=>b.textContent?.trim()==='Take a tour as Nana')!;
    tour.click();await fixture.whenStable();
    expect(login).toHaveBeenCalledExactlyOnceWith('nana','demo');
    expect(start).toHaveBeenCalledOnce();
  });
});
