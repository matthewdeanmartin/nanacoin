import { Component, Injectable, inject, signal } from '@angular/core';
@Injectable({providedIn:'root'})
export class NotebookPreferences {
  readonly paper = signal(this.read('nanacoin-notebook-paper'));
  readonly cursive = signal(this.read('nanacoin-notebook-cursive'));
  private read(key: string): boolean {
    try { return localStorage.getItem(key) !== 'false'; } catch { return true; }
  }
  set(kind: 'paper' | 'cursive', value: boolean) {
    this[kind].set(value);
    try { localStorage.setItem(`nanacoin-notebook-${kind}`, String(value)); } catch { /* Still applies for this session. */ }
  }
}
@Component({
 selector:'app-notebook',
 template:`<div class="notebook-controls"><label><input type="checkbox" [checked]="prefs.paper()" (change)="prefs.set('paper', $any($event.target).checked)"> Spiral-bound notebook</label>
 <label><input type="checkbox" [checked]="prefs.cursive()" (change)="prefs.set('cursive', $any($event.target).checked)"> Handwritten cursive</label></div>
 <div [class.notebook]="prefs.paper()" [class.cursive-ledger]="prefs.cursive()"><ng-content /></div>`,
})
export class Notebook { readonly prefs = inject(NotebookPreferences); }
