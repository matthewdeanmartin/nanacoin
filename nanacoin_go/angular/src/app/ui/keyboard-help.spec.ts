import { TestBed } from '@angular/core/testing';
import { Router } from '@angular/router';
import { KeyboardHelp, isEditing } from './keyboard-help';

describe('Mawkingbird-style navigation shortcuts', () => {
 afterEach(() => { TestBed.resetTestingModule(); localStorage.removeItem('nanacoin.keyboardShortcuts'); vi.restoreAllMocks(); });
 function setup() {
   const navigateByUrl = vi.fn().mockResolvedValue(true);
   TestBed.configureTestingModule({ providers: [{ provide: Router, useValue: { navigateByUrl } }] });
   const fixture = TestBed.createComponent(KeyboardHelp); fixture.detectChanges();
   return { fixture, navigateByUrl };
 }
 function key(key: string, extra: KeyboardEventInit = {}, target: HTMLElement = document.body) {
   const event = new KeyboardEvent('keydown', { key, bubbles: true, cancelable: true, ...extra });
   target.dispatchEvent(event); return event;
 }
 it('uses g then h/e/l without mapping money actions', () => {
   const { navigateByUrl } = setup();
   key('g'); key('h'); expect(navigateByUrl).toHaveBeenLastCalledWith('/market');
   key('g'); key('l'); expect(navigateByUrl).toHaveBeenLastCalledWith('/ledger');
   key('g'); key('e'); expect(navigateByUrl).toHaveBeenLastCalledWith('/market');
   for (const value of ['f', 'b', 'r', 'Enter']) expect(key(value).defaultPrevented).toBe(false);
   expect(navigateByUrl).toHaveBeenCalledTimes(3);
 });
 it('times out sequences and ignores browser modifiers, composition and repeats', () => {
   const { navigateByUrl } = setup();
   const now = vi.spyOn(performance, 'now').mockReturnValue(10);
   key('g'); now.mockReturnValue(2000); key('h');
   key('g', { ctrlKey: true }); key('l');
   key('g', { isComposing: true }); key('l');
   key('g', { repeat: true }); key('l');
   expect(navigateByUrl).not.toHaveBeenCalled();
 });
 it('does not intercept typing or confirmation dialogs', () => {
   const { fixture, navigateByUrl } = setup();
   const input = document.createElement('input'); fixture.nativeElement.append(input);
   key('g'); key('h', {}, input);
   const dialog = fixture.nativeElement.querySelector('dialog'); dialog.setAttribute('open', '');
   key('g'); key('l');
   expect(navigateByUrl).not.toHaveBeenCalled();
   const editable = document.createElement('div'); editable.contentEditable = 'true'; editable.setAttribute('contenteditable', 'true');
   expect(isEditing(editable)).toBe(true);
 });
 it('lets people disable shortcuts without disabling normal controls', () => {
   const { fixture, navigateByUrl } = setup();
   fixture.componentInstance.toggle();
   key('g'); key('l'); expect(navigateByUrl).not.toHaveBeenCalled();
   expect(localStorage.getItem('nanacoin.keyboardShortcuts')).toBe('off');
   expect(key('Tab').defaultPrevented).toBe(false);
 });
});
