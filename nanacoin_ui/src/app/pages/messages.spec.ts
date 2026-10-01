import { TestBed } from '@angular/core/testing';
import { provideHttpClient } from '@angular/common/http';
import { provideHttpClientTesting } from '@angular/common/http/testing';
import { provideRouter } from '@angular/router';
import { MessagesPage } from './messages';
import { NanacoinService } from '../api/nanacoin.service';
import { Session } from '../api/session';
import { MailRow } from './mail-model';

describe('kitchen screen read updates', () => {
  afterEach(() => { TestBed.resetTestingModule(); localStorage.removeItem('nanacoin-mail-seen'); });
  it('dismisses incoming message copies, while sent mail and payments stay local', () => {
    TestBed.configureTestingModule({ providers: [provideHttpClient(), provideHttpClientTesting(), provideRouter([])] });
    const api = TestBed.inject(NanacoinService);
    const read = vi.spyOn(api, 'screenRead').mockResolvedValue({ queued: true });
    const session = TestBed.inject(Session);
    session.me.set({ id: 'user-2', account: 'account-2', display_name: 'Alice', role: 'user', status: 'ACTIVE', balance: 0 } as never);
    const fixture = TestBed.createComponent(MessagesPage);
    const page = fixture.componentInstance as unknown as { open(row: MailRow): void };
    const row: MailRow = { id: 'tx-5', revision: 'tx-5', at: 1700000000, sender: 'Nana', subject: 'Dinner', body: 'Dinner', kind: 'Message', sent: false, attention: false };
    page.open(row);
    page.open({ ...row, sent: true });
    page.open({ ...row, kind: 'Transaction' });
    expect(read).toHaveBeenCalledExactlyOnceWith('tx-5');
    expect(localStorage.getItem('nanacoin-mail-seen')).toContain('tx-5');
  });
});
