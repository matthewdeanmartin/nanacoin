import { TestBed } from '@angular/core/testing';
import { signal } from '@angular/core';
import { NavigationEnd, Router } from '@angular/router';
import { Subject } from 'rxjs';
import { Session } from '../api/session';
import { Tour, tourSteps } from './tour';

describe('account-aware tour', () => {
  it('includes management only for Nana and vouchers only in the demo', () => {
    const member = tourSteps(false, false).map((s) => s.path);
    expect(member).toContain('/wealth');
    expect(member).toContain('/messages');
    expect(member.some((p) => p.startsWith('/nana'))).toBe(false);
    expect(member).not.toContain('/nickles');
    expect(member).not.toContain('/diagnostics');
    const demo = tourSteps(true, true).map((s) => s.path);
    expect(demo).toContain('/nana?tab=members');
    expect(demo).toContain('/nickles');
    expect(demo).toContain('/diagnostics');
    expect(tourSteps(false, false, true, true).map((s) => s.path)).toEqual(
      expect.arrayContaining(['/diagnostics', '/logs']),
    );
  });
  it('ends the tour when the active account changes', async () => {
    const me = signal({ id: 'nana' });
    const events = new Subject<NavigationEnd>();
    TestBed.configureTestingModule({
      providers: [
        { provide: Router, useValue: { events, navigateByUrl: vi.fn().mockResolvedValue(true) } },
        {
          provide: Session,
          useValue: {
            me,
            signedIn: signal(true),
            isNana: signal(true),
            diagAvailable: signal(false),
            logsAvailable: signal(false),
          },
        },
      ],
    });
    const tour = TestBed.inject(Tour);
    TestBed.tick();
    await tour.start();
    expect(tour.index()).toBe(0);
    await tour.go(1);
    expect(tour.index()).toBe(1);
    events.next(new NavigationEnd(1,'/wealth','/wealth'));
    expect(tour.step()?.path).toBe('/wealth');
    me.set({ id: 'child' });
    TestBed.tick();
    expect(tour.index()).toBeNull();
  });
});
