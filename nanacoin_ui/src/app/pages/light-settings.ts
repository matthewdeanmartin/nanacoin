import { Component, OnInit, inject, signal } from '@angular/core';
import { FormsModule } from '@angular/forms';
import { NanacoinService } from '../api/nanacoin.service';

@Component({
  selector: 'app-light-settings',
  imports: [FormsModule],
  template: `
    <section class="panel">
      <h2>Healthy light messages</h2>
      <p>The board rotates through these three phrases in cyan Morse code, with three green healthy blinks after each. Startup and fault indicators take priority.</p>
      @if (error()) { <p class="error" role="alert">{{ error() }}</p> }
      @if (loaded()) {
        <form (ngSubmit)="save()">
          @for (i of [0, 1, 2]; track i) {
            <label [for]="'phrase-' + i">Message {{ i + 1 }}</label>
            <input [id]="'phrase-' + i" [name]="'phrase-' + i" [(ngModel)]="phrases[i]" maxlength="80" required />
          }
          <p class="muted">Each message: 1–80 letters, numbers, spaces or Morse punctuation. Dots last 200 ms; dashes last 600 ms. Changes apply on the next maintenance pass, normally about ten seconds.</p>
          <button type="submit" [disabled]="busy()">Save messages</button>
          @if (saved()) { <p role="status">Messages saved.</p> }
        </form>
      }
    </section>`,
})
export class LightSettings implements OnInit {
  private readonly api = inject(NanacoinService);
  protected phrases: [string, string, string] = ['', '', ''];
  protected readonly loaded = signal(false);
  protected readonly busy = signal(false);
  protected readonly saved = signal(false);
  protected readonly error = signal('');
  async ngOnInit(): Promise<void> {
    try {
      this.phrases = (await this.api.lightSettings()).phrases;
      this.loaded.set(true);
    } catch { this.error.set('Could not load the light messages. Check your connection and admin sign-in.'); }
  }
  protected async save(): Promise<void> {
    if (this.busy()) return;
    this.busy.set(true);
    this.saved.set(false);
    this.error.set('');
    try {
      this.phrases = (await this.api.setLightSettings(this.phrases.map(p => p.trim()) as [string, string, string])).phrases;
      this.saved.set(true);
    } catch { this.error.set('Could not save. Use three messages of 1–80 ASCII letters, digits, spaces or Morse punctuation; check your connection and admin sign-in.'); }
    finally { this.busy.set(false); }
  }
}
