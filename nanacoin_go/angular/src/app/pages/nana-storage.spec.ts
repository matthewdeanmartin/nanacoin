import { TestBed } from '@angular/core/testing';
import { NanacoinService } from '../api/nanacoin.service';
import { Session } from '../api/session';
import { Dialogs } from '../ui/dialog';
import { Toasts } from '../ui/toasts';
import { LiveSeeder } from '../demo/seed-live';
import { NanaPage } from './nana';

describe('Nana reset confirmation', () => {
  afterEach(() => TestBed.resetTestingModule());
  for (const answer of [null, 'reset', '']) {
    it(`does not reset for ${JSON.stringify(answer)}`, async () => {
      const resetEconomy = vi.fn();
      TestBed.configureTestingModule({ providers: [
        {provide: NanacoinService, useValue: { ledger: async () => ({transactions: []}), storage: async () => ({generation:1, sequence:2}), resetEconomy }},
        {provide: Session, useValue: { status: () => ({checkpoint_supported: true}) }},
        {provide: Dialogs, useValue: { prompt: async () => answer }},
        {provide: Toasts, useValue: { error: vi.fn(), fromError: vi.fn() }},
        {provide: LiveSeeder, useValue: { running: () => false }},
      ] });
      const page = TestBed.runInInjectionContext(() => new NanaPage());
      await (page as unknown as {resetEconomy(): Promise<void>}).resetEconomy();
      expect(resetEconomy).not.toHaveBeenCalled();
    });
  }
});
