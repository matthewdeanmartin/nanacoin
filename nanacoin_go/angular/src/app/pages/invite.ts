import { Component } from '@angular/core';
import { FormsModule } from '@angular/forms';

import { IS_DEMO } from '../demo/demo';
import { toggleCaps } from './message-caps';

@Component({
  selector: 'app-invite',
  imports: [FormsModule],
  template: `
    <h1>Invite</h1>
    <p class="lede">Tell friends and family about NanaCoin.</p>
    <section class="panel panel--compact">
      <label>
        Post text
        <textarea [ngModel]="text" (ngModelChange)="text = allCaps ? $event.toLocaleUpperCase() : $event" maxlength="300" rows="4"></textarea>
      </label>
      <label class="checkbox">
        <input type="checkbox" [ngModel]="allCaps" (ngModelChange)="setAllCaps($event)" />
        <span>ALL CAPS</span>
      </label>
      <button class="btn btn--quiet" type="button" (click)="shareFacebook()">Share on Facebook</button>
      <p class="muted small">The draft is copied, then Facebook opens its posting dialog. Other platform intents remain on the roadmap.</p>
    </section>
  `,
})
export class InvitePage {
  protected text = IS_DEMO
    ? 'Hey, here is a cool project: NanaCoin, a tiny household economy.'
    : 'Hey, visit my house, connect to nanacoin.local, and join the economy!';
  protected allCaps = false;
  private beforeCaps = '';

  protected setAllCaps(on: boolean): void {
    const next = toggleCaps(this.text, this.allCaps, on, this.beforeCaps);
    this.text = next.text;
    this.beforeCaps = next.saved;
    this.allCaps = on;
  }

  protected shareFacebook(): void {
    const textarea = document.createElement('textarea');
    textarea.value = this.text.trim();
    textarea.style.position = 'fixed';
    textarea.style.opacity = '0';
    document.body.appendChild(textarea);
    textarea.select();
    document.execCommand('copy');
    textarea.remove();
    const params = new URLSearchParams({
      u: IS_DEMO ? 'https://matthewdeanmartin.github.io/nanacoin/' : 'http://nanacoin.local/',
      quote: this.text.trim(),
    });
    window.open(`https://www.facebook.com/sharer/sharer.php?${params}`, '_blank', 'noopener,noreferrer');
  }
}
