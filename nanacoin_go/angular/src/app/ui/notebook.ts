import { Component, signal } from '@angular/core';
@Component({
 selector: 'app-notebook',
 template: `<div class="notebook-controls"><label><input type="checkbox" [checked]="paper()" (change)="paper.set(!paper())"> Spiral-bound notebook</label>
 <label><input type="checkbox" [checked]="cursive()" (change)="cursive.set(!cursive())"> Handwritten cursive</label></div>
 <div [class.notebook]="paper()" [class.cursive-ledger]="cursive()"><ng-content /></div>`,
})
export class Notebook { readonly paper = signal(false); readonly cursive = signal(false); }
