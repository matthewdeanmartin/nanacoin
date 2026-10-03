import { TestBed } from '@angular/core/testing';
import { Log } from '../api/log';
import { ProblemLog } from './problem-log';

describe('inline warning diagnostics', () => {
  afterEach(() => {
    TestBed.resetTestingModule();
    document.querySelector('#problem-test')?.remove();
    vi.restoreAllMocks();
  });
  it('logs visible validation errors once per change, excluding inputs, hidden panels and already-logged toasts', () => {
    vi.spyOn(console, 'warn').mockImplementation(() => {});
    const container = document.createElement('div');
    container.id = 'problem-test';
    container.innerHTML =
      '<p role="alert">Failure</p><p class="field-problem">Fix this amount</p><div hidden><p class="warning">Hidden warning</p></div><app-toast-list><p role="alert">Toast</p></app-toast-list><input value="secret">';
    document.body.append(container);
    const service = TestBed.inject(ProblemLog),
      log = TestBed.inject(Log);
    service.record();
    expect(log.all().map((e) => e.message)).toEqual(['Failure', 'Fix this amount']);
    container.querySelector('p')!.textContent = 'New failure';
    service.record();
    expect(log.recent()[0].message).toBe('New failure');
    expect(log.asText()).not.toContain('secret');
  });
});
