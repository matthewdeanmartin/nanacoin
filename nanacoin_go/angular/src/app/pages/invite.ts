import { Component, inject } from '@angular/core';
import { FormsModule } from '@angular/forms';

import { IS_DEMO } from '../demo/demo';
import { toggleCaps } from './message-caps';
import { Mastodon } from '../api/mastodon';

@Component({
  selector: 'app-invite',
  imports: [FormsModule],
  template: `
    <h1>Invite</h1>
    <p class="lede">Tell friends and family about NanaCoin.</p>
    <section class="panel panel--compact">
      <label>
        Post text
        <textarea
          [ngModel]="text"
          (ngModelChange)="text = allCaps ? $event.toLocaleUpperCase() : $event"
          maxlength="300"
          rows="4"
        ></textarea>
      </label>
      <label class="checkbox">
        <input type="checkbox" [ngModel]="allCaps" (ngModelChange)="setAllCaps($event)" />
        <span>ALL CAPS</span>
      </label>
      <button class="btn btn--quiet" type="button" (click)="shareFacebook()">
        Share on Facebook
      </button>
      <button class="btn btn--quiet" type="button" (click)="shareMastodon()">
        Share on Mastodon
      </button>
      <button class="btn btn--quiet" type="button" (click)="shareBluesky()">
        Share on Bluesky
      </button>
      <p class="muted small">
        Each button opens that platform's posting screen with this draft. Facebook also copies the
        draft because its share dialog may omit custom text.
      </p>
    </section>
  `,
})
export class InvitePage {
  private readonly mastodon = inject(Mastodon);
  protected text = IS_DEMO
    ? 'Hey, here is a cool project: NanaCoin, a tiny household economy.\n' +
      '\n' +
      'Demo at https://matthewdeanmartin.github.io/nanacoin/\n' +
      '\n' +
      'Runs on an $10 tiny computer.'
    : 'Hey, visit my house, connect to nanacoin.local, and join the economy!\n' +
      '\n' +
      'Live board at https://nanacoin.local\n' +
      '\n' +
      'or visit the demo at https://matthewdeanmartin.github.io/nanacoin/';
  protected allCaps = false;
  private beforeCaps = '';

  protected setAllCaps(on: boolean): void {
    const next = toggleCaps(this.text, this.allCaps, on, this.beforeCaps);
    this.text = next.text;
    this.beforeCaps = next.saved;
    this.allCaps = on;
  }

  protected shareFacebook(): void {
    this.copyDraft();
    const params = new URLSearchParams({
      u: IS_DEMO ? 'https://matthewdeanmartin.github.io/nanacoin/' : 'http://nanacoin.local/',
      quote: this.text.trim(),
    });
    window.open(
      `https://www.facebook.com/sharer/sharer.php?${params}`,
      '_blank',
      'noopener,noreferrer',
    );
  }

  protected shareMastodon(): void {
    window.open(this.mastodon.shareUrl(this.text.trim()), '_blank', 'noopener,noreferrer');
  }

  protected shareBluesky(): void {
    const params = new URLSearchParams({ text: this.text.trim() });
    window.open(`https://bsky.app/intent/compose?${params}`, '_blank', 'noopener,noreferrer');
  }

  private copyDraft(): void {
    const textarea = document.createElement('textarea');
    textarea.value = this.text.trim();
    textarea.style.position = 'fixed';
    textarea.style.opacity = '0';
    document.body.appendChild(textarea);
    textarea.select();
    document.execCommand('copy');
    textarea.remove();
  }
}
