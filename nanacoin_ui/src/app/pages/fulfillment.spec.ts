import { provideRouter } from '@angular/router';
import { TestBed } from '@angular/core/testing';
import { provideHttpClient } from '@angular/common/http';
import { provideHttpClientTesting } from '@angular/common/http/testing';
import { Fulfillment } from '../api/models';
import { Session } from '../api/session';
import { NanacoinService } from '../api/nanacoin.service';
import { Dialogs } from '../ui/dialog';
import { AccountTodos, CHANGED_MIND, FulfillmentControl } from './fulfillment';
import { mailRows } from './mail-model';
const work: Fulfillment = {
  transaction: 'tx-1',
  provider: 'account-2',
  recipient: 'account-3',
  provider_name: 'Dad',
  recipient_name: 'Mom',
  description: 'Wash car',
  kind: 'WORK',
  status: 'TODO',
  updates: [
    { id: 'event-1', at: 1, actor: 'account-3', actor_name: 'Mom', status: 'TODO', reason: '' },
  ],
};
function setup(account: string) {
  TestBed.configureTestingModule({ providers: [provideHttpClient(), provideHttpClientTesting(), provideRouter([])] });
  const session = TestBed.inject(Session);
  session.me.set({
    id: 'user-2',
    account,
    username: 'dad',
    display_name: 'Dad',
    role: 'user',
    status: 'ACTIVE',
  } as never);
  return TestBed.inject(NanacoinService);
}
afterEach(() => TestBed.resetTestingModule());
describe('fulfillment controls and activity', () => {
  it('lets the provider claim completion', async () => {
    const api = setup('account-2');
    const update = vi.spyOn(api, 'setFulfillment').mockResolvedValue({ ...work, status: 'DONE' });
    const fixture = TestBed.createComponent(FulfillmentControl);
    fixture.componentRef.setInput('item', work);
    fixture.detectChanges();
    const root = fixture.nativeElement as HTMLElement;
    expect(root.textContent).toContain('Mark work done');
    expect(root.textContent).not.toContain('Work not done');
    root.querySelector('button')!.click();
    await fixture.whenStable();
    expect(update).toHaveBeenCalledWith('tx-1', 'COMPLETE', '', expect.any(String));
  });
  it('lets the recipient record an outstanding delivery as done', async () => {
    const api=setup('account-3'); const update=vi.spyOn(api,'setFulfillment').mockResolvedValue({...work,status:'DONE'});
    const fixture=TestBed.createComponent(FulfillmentControl); fixture.componentRef.setInput('item',work); fixture.detectChanges();
    const button=Array.from((fixture.nativeElement as HTMLElement).querySelectorAll('button')).find(b=>b.textContent?.includes('Record as done'))!;
    button.click(); await fixture.whenStable();
    expect(update).toHaveBeenCalledWith('tx-1','COMPLETE','',expect.any(String));
  });
  it('lets the recipient dispute and then withdraw to done', async () => {
    const api = setup('account-3');
    const update = vi
      .spyOn(api, 'setFulfillment')
      .mockResolvedValue({ ...work, status: 'DISPUTED' });
    vi.spyOn(TestBed.inject(Dialogs), 'prompt').mockResolvedValue('Car still dirty');
    const fixture = TestBed.createComponent(FulfillmentControl);
    fixture.componentRef.setInput('item', { ...work, status: 'DONE' });
    fixture.detectChanges();
    const root = fixture.nativeElement as HTMLElement;
    expect(root.textContent).not.toContain('Mark work done');
    root.querySelector('button')!.click();
    await fixture.whenStable();
    expect(update).toHaveBeenCalledWith('tx-1', 'DISPUTE', 'Car still dirty', expect.any(String));
    fixture.componentRef.setInput('item', { ...work, status: 'DISPUTED' });
    fixture.detectChanges();
    root.querySelector('button')!.click();
    await fixture.whenStable();
    expect(update).toHaveBeenLastCalledWith('tx-1', 'WITHDRAW_DISPUTE', '', expect.any(String));
  });
  it('shows unfinished work independently of account transaction history', async () => {
    const api = setup('account-2');
    vi.spyOn(api, 'fulfillments').mockResolvedValue({ fulfillments: [work] });
    const fixture = TestBed.createComponent(AccountTodos);
    fixture.detectChanges();
    await fixture.whenStable();
    fixture.detectChanges();
    expect((fixture.nativeElement as HTMLElement).textContent).toContain('Wash car');
    expect((fixture.nativeElement as HTMLElement).textContent).toContain('Mark work done');
  });
  it('keeps separate event identities even when updates share a timestamp', () => {
    const f: Fulfillment = {
      ...work,
      status: 'DONE',
      updates: [
        ...work.updates,
        { id: 'event-2', at: 1, actor: 'account-2', actor_name: 'Dad', status: 'DONE', reason: '' },
        {
          id: 'event-3',
          at: 1,
          actor: 'account-3',
          actor_name: 'Mom',
          status: 'DISPUTED',
          reason: 'Dirty',
        },
        { id: 'event-4', at: 1, actor: 'account-3', actor_name: 'Mom', status: 'DONE', reason: '' },
      ],
    };
    const rows = mailRows('account-2', [], [], [], [f]);
    expect(rows).toHaveLength(4);
    expect(new Set(rows.map((r) => r.revision)).size).toBe(4);
    expect(rows.some((r) => r.body.includes('Dirty'))).toBe(true);
    expect(rows.every((r) => !r.attention)).toBe(true);
    expect(mailRows('account-9', [], [], [], [f])).toEqual([]);
  });
});
describe('undoing a purchase', () => {
  const button = (root: HTMLElement, text: string) =>
    Array.from(root.querySelectorAll('button')).find((b) => b.textContent?.includes(text));
  it('lets the buyer say they changed their mind', async () => {
    const api = setup('account-3');
    const update = vi.spyOn(api, 'setFulfillment').mockResolvedValue({ ...work, status: 'DISPUTED' });
    vi.spyOn(TestBed.inject(Dialogs), 'confirm').mockResolvedValue('');
    const fixture = TestBed.createComponent(FulfillmentControl);
    fixture.componentRef.setInput('item', work);
    fixture.detectChanges();
    button(fixture.nativeElement, 'I changed my mind')!.click();
    await fixture.whenStable();
    expect(update).toHaveBeenCalledWith('tx-1', 'DISPUTE', CHANGED_MIND, expect.any(String));
  });
  it('lets the seller give the money back and shows why it was asked for', async () => {
    const api = setup('account-2');
    const reverse = vi.spyOn(api, 'reverse').mockResolvedValue({} as never);
    vi.spyOn(TestBed.inject(Dialogs), 'prompt').mockResolvedValue('Bought by mistake');
    const asked = {
      ...work,
      status: 'DISPUTED' as const,
      updates: [...work.updates, { id: 'event-2', at: 2, actor: 'account-3', actor_name: 'Mom', status: 'DISPUTED' as const, reason: CHANGED_MIND }],
    };
    const fixture = TestBed.createComponent(FulfillmentControl);
    fixture.componentRef.setInput('item', asked);
    fixture.detectChanges();
    const root = fixture.nativeElement as HTMLElement;
    expect(root.textContent).toContain('Buyer changed their mind');
    expect(root.textContent).toContain(`Mom says: “${CHANGED_MIND}”`);
    button(root, 'Give the money back')!.click();
    await fixture.whenStable();
    expect(reverse).toHaveBeenCalledWith('tx-1', 'Bought by mistake', expect.any(String));
  });
  it('points both sides at Nana', () => {
    setup('account-3');
    TestBed.inject(Session).household.set([{ id: 'user-1', account: 'account-1', role: 'nana', status: 'ACTIVE' } as never]);
    const fixture = TestBed.createComponent(FulfillmentControl);
    fixture.componentRef.setInput('item', work);
    fixture.detectChanges();
    const link = (fixture.nativeElement as HTMLElement).querySelector('a');
    expect(link?.textContent).toContain('Send Nana a message');
    expect(link?.getAttribute('href')).toContain('to=account-1');
  });
});
