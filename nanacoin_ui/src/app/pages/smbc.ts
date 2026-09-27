import { Component } from '@angular/core';
import { RouterLink } from '@angular/router';

const amazon = (title: string) => `https://www.amazon.com/s?i=stripbooks&k=${encodeURIComponent(`${title} Weinersmith`)}`;

/** Zach Weinersmith's books. Amazon links are title searches, so they never point at a stale listing. */
export const SMBC_BOOKS = [
  { title: 'Save Yourself, Mammal!', note: 'SMBC collection, 2011' },
  { title: 'The Most Dangerous Game', note: 'SMBC collection, 2011' },
  { title: 'Science: Ruining Everything Since 1543', note: 'SMBC collection, 2013' },
  { title: 'Religion: Ruining Everything Since 4004 BC', note: 'SMBC collection, 2015' },
  { title: 'Trial of the Clone', note: 'gamebook, 2012' },
  { title: 'Trial of the Clone 2: Wrath of the Pacifist', note: 'gamebook, 2013' },
  { title: 'Polystate: A Thought Experiment in Distributed Government', note: '2014' },
  { title: 'Augie and the Green Knight', note: "children's book, 2014" },
  { title: 'Science: Abridged Beyond the Point of Usefulness', note: '2017' },
  { title: "Soonish: Ten Emerging Technologies That'll Improve and/or Ruin Everything", note: 'with Kelly Weinersmith, 2017' },
  { title: 'Open Borders: The Science and Ethics of Immigration', note: 'with Bryan Caplan, 2019' },
  { title: 'Bea Wolf', note: 'with Boulet, 2023' },
  { title: 'A City on Mars', note: 'with Kelly Weinersmith, 2023' },
].map(b => ({ ...b, href: amazon(b.title) }));

@Component({
  selector: 'app-smbc', imports: [RouterLink],
  template: `<h1>Inspired by SMBC</h1>
    <section class="panel">
      <p>The beloved-Nana central bank, spiral notebook, cursive ledger, lemon squares and nana-nickel joke come from Zach Weinersmith’s
      <a href="https://www.smbc-comics.com/comic/nanacoin" target="_blank" rel="noopener noreferrer">SMBC: Nanacoin</a>.
      This is an independent homage, not an official SMBC product or endorsement. The comic is linked, not republished.</p>
      <p><a routerLink="/recipes">Have a lemon bar</a> · <a routerLink="/ledger">Visit the notebook</a></p>
    </section>
    <section class="panel"><h2>Support SMBC</h2>
      <ul>
        <li><a href="https://www.smbc-comics.com/" target="_blank" rel="noopener noreferrer">Read Saturday Morning Breakfast Cereal</a></li>
        <li><a href="https://smbc-store.myshopify.com/" target="_blank" rel="noopener noreferrer">SMBC merch store</a></li>
        <li><a href="https://www.patreon.com/ZachWeinersmith" target="_blank" rel="noopener noreferrer">SMBC on Patreon</a></li>
      </ul>
    </section>
    <section class="panel"><h2>Books on Amazon</h2>
      <ul class="smbc-books">
        @for (b of books; track b.title) {
          <li><a [href]="b.href" target="_blank" rel="noopener noreferrer">{{ b.title }}</a> <span class="muted">({{ b.note }})</span></li>
        }
      </ul>
    </section>
    <section class="panel"><h2>Other credits</h2>
      <p>The self-hosted Dancing Script typeface is by the Dancing Script Project Authors, under the SIL Open Font License. Its license is included with the static assets; no third-party font service is contacted.</p>
    </section>`,
  styles: `.smbc-books li{margin:.35rem 0}`,
})
export class SmbcPage {
  readonly books = SMBC_BOOKS;
}
