import { TestBed } from '@angular/core/testing';
import { computed, signal } from '@angular/core';
import { provideRouter } from '@angular/router';
import { NanacoinService } from '../api/nanacoin.service';
import { Session } from '../api/session';
import { EmailPage } from './email';
import { IS_DEMO } from '../demo/demo';

const SETTINGS = { provider: 'Brevo', address: 'alice@example.com', notifications: true, queued: 0, last_error: '' };

function page(me: { id: string } | null, emailAction = vi.fn().mockResolvedValue(true)) {
  const emailSettings = vi.fn().mockResolvedValue(SETTINGS);
  const user = signal(me);
  TestBed.configureTestingModule({ providers: [
    provideRouter([]),
    { provide: NanacoinService, useValue: { emailSettings, emailAction } },
    { provide: Session, useValue: { me: user, signedIn: computed(() => user() !== null), household: signal([]) } },
  ] });
  const fixture = TestBed.createComponent(EmailPage);
  fixture.detectChanges();
  return { component: fixture.componentInstance, emailAction, emailSettings };
}

// The demo build has no email page route or network calls; the page is inert there.
describe.skipIf(IS_DEMO)('email page', () => {
  afterEach(() => TestBed.resetTestingModule());

  it('requests recovery by username and clears the new password whatever the outcome', async () => {
    const failing = vi.fn().mockRejectedValue(new Error('401'));
    const { component, emailAction, emailSettings } = page(null, failing);
    component['username'] = 'alice';
    await component['recover']();
    expect(emailAction).toHaveBeenCalledWith('recover', { username: 'alice' });
    component['code'] = 'abc'; component['password'] = 'new-secret';
    await component['reset']();
    expect(emailAction).toHaveBeenLastCalledWith('reset', { code: 'abc', password: 'new-secret' });
    expect(component['password']).toBe('');
    expect(component['code']).toBe('abc');
    expect(component['error']()).toContain('Could not complete');
    expect(emailSettings).not.toHaveBeenCalled();
  });

  it('loads settings when signed in and sends member email to the numeric member id', async () => {
    const { component, emailAction, emailSettings } = page({ id: 'user-1' });
    await vi.waitFor(() => expect(component['settings']()).toEqual(SETTINGS));
    component['recipient'] = 'user-2'; component['subject'] = 'Dinner'; component['text'] = 'Six.';
    await component['notify']();
    expect(emailAction).toHaveBeenCalledWith('notify', { member: 2, subject: 'Dinner', text: 'Six.' });
    expect(component['subject']).toBe('');
    expect(component['text']).toBe('');
    expect(component['notice']()).toBe('Email queued.');
    expect(emailSettings).toHaveBeenCalledTimes(2);
  });

  it('never keeps the current password after asking for a verification code', async () => {
    const { component, emailAction } = page({ id: 'user-2' });
    component['address'] = 'alice@example.com'; component['currentPassword'] = '5678';
    await component['verifyAddress']();
    expect(emailAction).toHaveBeenCalledWith('address', { address: 'alice@example.com', password: '5678' });
    expect(component['currentPassword']).toBe('');
  });
});
