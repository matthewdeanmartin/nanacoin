import { Money, MoneyPipe } from '../api/money';
import { inject as moneyInject } from '@angular/core';
// The marketplace: what is for sale, and the form for offering something.

import { Component, computed, inject, resource, signal } from '@angular/core';
import { FormsModule } from '@angular/forms';
import { NgTemplateOutlet } from '@angular/common';
import { takeUntilDestroyed } from '@angular/core/rxjs-interop';
import { ActivatedRoute, Router, RouterLink } from '@angular/router';

import { EconomicKind, EconomicUnit, Listing, ListingSide, Thing } from '../api/models';
import { ApiError, NanacoinService, newIdempotencyKey } from '../api/nanacoin.service';
import { Session, reloadOnLedgerChange } from '../api/session';
import { Dialogs } from '../ui/dialog';
import { Toasts } from '../ui/toasts';
import { CATEGORIES, Category, ITEMS } from '../catalog/catalog';
import { catalogEconomics, formatQuantity, validQuantity } from '../catalog/economics';
import { isGoodDeed } from '../nana/good-deeds';
import { ArtPicture } from '../art/art-picture';
import { nameOf } from '../people/people';
import { SectionTabs } from '../ui/section-tabs';

const MARKET_TABS = [
  { id: 'market-buy', label: 'Offers to Buy' },
  { id: 'market-sell', label: 'Offers to Sell' },
  { id: 'market-art', label: 'Art' },
  { id: 'market-deeds', label: 'Good Deeds' },
  { id: 'market-closed', label: 'Closed & Cancelled' },
] as const;

@Component({
  selector: 'app-market',
  imports: [MoneyPipe, FormsModule, RouterLink, ArtPicture, SectionTabs, NgTemplateOutlet],
  templateUrl: './market.html',
})
export class MarketPage {
  protected readonly money = moneyInject(Money);
  private readonly api = inject(NanacoinService);
  private readonly toasts = inject(Toasts);
  private readonly dialogs = inject(Dialogs);
  private readonly router = inject(Router);
  protected readonly session = inject(Session);
  protected readonly responses = resource({params:()=>this.session.me()?.account, loader:()=>this.api.offers()});
  private readonly followOffers = reloadOnLedgerChange(this.responses);
  protected pendingResponses(id: string) { return (this.responses.value()?.offers ?? []).filter(o=>o.listing === id && o.status === 'OPEN'); }
  protected readonly listingPage = this.router.url.split('?')[0] === '/list';

  protected title = '';
  protected description = '';
  protected price: string | number | null = null;
  protected economicKind: EconomicKind | '' = '';
  protected quantity = '1';
  protected unit: EconomicUnit = 'EACH';
  protected standard = false;
  protected selectedThing = '';
  /** Cash belongs in Forex, never in the goods/services market. */
  protected readonly catalog = ITEMS.filter((item) => !item.currency);
  protected readonly categories = CATEGORIES;
  protected catalogChoice = '';
  protected readonly units: readonly EconomicUnit[] = [
    'EACH', 'BATCH', 'TASK', 'MINUTE', 'HOUR', 'GRAM', 'KILOGRAM',
    'MILLILITER', 'LITER', 'LOAD', 'OTHER',
  ];
  protected readonly thingData = resource({
    loader: async () => {
      try { return (await this.api.things()).things; }
      catch (e) {
        if (e instanceof ApiError && e.status === 404) return [];
        throw e;
      }
    },
  });

  /** Which way round a new listing is. Selling is the familiar default. */
  protected side: ListingSide = 'SELL';

  protected readonly posting = signal(false);

  /** The listing currently being bought, so only its own button shows a spinner. */
  protected readonly buying = signal<string | null>(null);

  /** The listing an offer is being made against. */
  protected readonly offering = signal<string | null>(null);

  constructor() {
    inject(ActivatedRoute).queryParamMap.pipe(takeUntilDestroyed()).subscribe((params) => {
      const tab = params.get('tab');
      this.tab.set(MARKET_TABS.some((t) => t.id === tab) ? tab! : 'market-sell');
    });
    const repeated = history.state?.['repeat'] as Listing | undefined;
    if (this.listingPage && repeated?.id) this.prefill(repeated);
  }

  /** Nana's standing good deeds, shown apart from ordinary buying and selling. */
  protected readonly goodDeeds = computed(() => this.session.forSale().filter(isGoodDeed));
  private readonly market = computed(() => this.session.forSale().filter((l) => !isGoodDeed(l)));
  /** Want-ads: someone has the coins and wants a thing or a job done. */
  protected readonly wantAds = computed(() => this.market().filter((l) => this.wanted(l)));
  protected readonly forSale = computed(() => this.market().filter((l) => !this.wanted(l)));

  /** The open section; kept in ?tab= so reload and Back return to it. */
  protected readonly tab = signal('market-sell');
  protected readonly tabs = computed(() => {
    const counts: Record<string, number | null> = {
      'market-buy': this.wantAds().length,
      'market-sell': this.forSale().length,
      'market-art': this.commerce.hasValue() ? this.artForSale().length : null,
      'market-deeds': this.goodDeeds().length,
      'market-closed': this.session.closed().length,
    };
    return MARKET_TABS.map((t) => ({ id: t.id, label: `${t.label} (${counts[t.id] ?? '…'})` }));
  });
  protected select(tab: string): void {
    this.tab.set(tab);
    void this.router.navigate([], { queryParams: { tab }, replaceUrl: true });
  }
  /** Art editions on sale; buying happens in the gallery, where ownership changes hands with the coins. */
  private readonly commerce = resource({
    params: () => (this.listingPage ? undefined : this.session.me()?.account),
    loader: () => this.api.commerceBook(),
  });
  protected readonly artForSale = computed(() => (this.commerce.value()?.artworks ?? []).filter((a) => a.price !== null));
  protected artist(member: number): string { return nameOf(this.session.household(), member); }

  /**
   * Claims a good deed: an offer at Nana's reward, with a note saying what was
   * done. It goes through the ordinary offer process; the coins are new money
   * issued only when Nana accepts.
   */
  protected async claim(l: Listing): Promise<void> {
    if (this.offering()) return;
    try {
      const open = (await this.api.offers()).offers.some((o) => o.listing === l.id && o.offerer === this.session.me()?.account && o.status === 'OPEN');
      if (open) { this.toasts.error('Nana has not answered your last claim for this deed yet.'); return; }
    } catch (e) { this.toasts.fromError(e); return; }
    const note = await this.dialogs.prompt({
      title: `I did this: "${l.title}"`,
      message: `Tell Nana what you did. If she accepts, she issues ${this.money.format(l.price)} new NC to you.`,
      placeholder: 'Helped Ivy with fractions on Tuesday',
      confirmLabel: 'Send to Nana',
      required: true,
    });
    if (note === null) return;
    this.offering.set(l.id);
    try {
      await this.api.makeOffer(l.id, l.price, note, newIdempotencyKey());
      this.toasts.ok('Sent to Nana. The reward arrives when she accepts.');
    } catch (e) { this.toasts.fromError(e); }
    finally { this.offering.set(null); }
  }

  protected mine(l: Listing): boolean {
    return l.seller === this.session.me()?.account;
  }

  /**
   * Whether this is a want-ad rather than something for sale.
   *
   * Absent side means SELL, so listings from a server that predates two-way
   * listings read as what they are instead of all becoming want-ads.
   */
  protected wanted(l: Listing): boolean {
    return l.side === 'BUY';
  }

  /**
   * Proposes a price, or proposes doing the thing in a want-ad.
   *
   * The amount is asked for rather than assumed even on a want-ad, where the
   * poster named a figure: someone may be willing to do it for less, and the
   * whole point of an offer is that it is negotiable.
   */
  protected async offer(l: Listing): Promise<void> {
    if (this.offering()) return;

    try {
      const existing = (await this.api.offers()).offers.find((candidate) =>
        candidate.listing === l.id
        && candidate.offerer === this.session.me()?.account
        && candidate.status === 'OPEN');
      if (existing) {
        this.toasts.error('You already have an open offer on this listing. Withdraw it before making another.');
        return;
      }
    } catch (e) {
      if (!(e instanceof ApiError && e.status === 404)) {
        this.toasts.fromError(e);
        return;
      }
    }

    const answer = await this.dialogs.offer({
      title: this.wanted(l) ? `Offer to do "${l.title}"` : `Offer on "${l.title}"`,
      message: this.wanted(l)
        ? `${l.seller_name} is offering ${l.price} for this. Name your price - they still have to accept.`
        : `${l.seller_name} is asking ${l.price}. Offer what you like - they still have to accept.`,
      initial: this.money.input(l.price),
      confirmLabel: 'Send offer',
    });
    if (answer === null) return;
    const { amount, message } = answer;

    this.offering.set(l.id);
    try {
      await this.api.makeOffer(l.id, amount, message, newIdempotencyKey());
      this.toasts.ok('Offer sent. It is not a deal until they accept.');
    } catch (e) {
      // The board may not have offers yet, which is expected while the UI
      // runs ahead of the firmware - say so rather than reporting a raw 404.
      if (e instanceof ApiError && e.status === 404) {
        this.toasts.error('This NanaCoin does not support offers yet.');
      } else if (e instanceof ApiError && e.status === 409 && e.code === 'conflict') {
        this.toasts.error('You already have an open offer on this listing. Withdraw it before making another.');
      } else {
        this.toasts.fromError(e);
      }
    } finally {
      this.offering.set(null);
    }
  }

  protected affordable(l: Listing): boolean {
    return this.session.balance() >= l.price;
  }

  /** For a currency listing: '5.00 USD'. */
  protected cashAmount(l: Listing): string {
    return `${((l.minor_units ?? 0) / 100).toFixed(2)} ${l.currency}`;
  }

  protected async post(): Promise<void> {
    let price: number;
    try { price = this.money.parse(this.price ?? ''); } catch (e) { this.toasts.fromError(e); return; }
    if (price < 0) {
      this.toasts.error("Can't do negative prices.");
      return;
    }
    if (!Number.isInteger(price) || price <= 0) {
      // Amounts are integer minor units after parsing.
      this.toasts.error('Enter a positive amount in NC.');
      return;
    }
    if (!this.economicKind) {
      this.toasts.error('Choose what kind of exchange this is.');
      return;
    }
    if (!validQuantity(this.quantity)) {
      this.toasts.error('Quantity must be a positive decimal with at most three places.');
      return;
    }
    if (this.side === 'SELL' && this.economicKind === 'LABOR' && this.session.isNana()) {
      this.toasts.error('Nana is the household organization and cannot sell labor.');
      return;
    }
    this.posting.set(true);
    try {
      await this.api.createListing({
        title: this.title.trim(),
        description: this.description.trim(),
        price,
        // Only sent when it is a want-ad, so an older server - which has
        // never heard of side - keeps receiving exactly what it used to.
        ...(this.side === 'BUY' ? { side: this.side } : {}),
        economic_kind: this.economicKind,
        ...(this.selectedThing ? { thing: this.selectedThing } : {}),
        quantity: this.quantity,
        unit: this.unit,
        standard: this.standard,
      });
      this.title = '';
      this.description = '';
      this.price = null;
      this.economicKind = '';
      this.quantity = '1';
      this.unit = 'EACH';
      this.standard = false;
      this.selectedThing = '';
      this.catalogChoice = '';
      this.toasts.ok(this.side === 'BUY' ? 'Want-ad posted.' : 'Listed.');
      await this.session.refresh();
      if (this.listingPage) await this.router.navigate(['/market']);
    } catch (e) {
      this.toasts.fromError(e);
    } finally {
      this.posting.set(false);
    }
  }

  protected titleChanged(value: string): void {
    this.title = value;
    const known = this.things().find((t) => t.name.localeCompare(value, undefined, { sensitivity: 'accent' }) === 0);
    if (known) {
      this.selectedThing = known.id;
      this.economicKind = known.economic_kind;
      this.unit = known.unit;
      this.standard = known.standard;
      return;
    }
    const item = this.catalog.find((i) => i.name.localeCompare(value, undefined, { sensitivity: 'accent' }) === 0);
    this.selectedThing = '';
    if (item) {
      const attributes = catalogEconomics(item);
      this.economicKind = attributes.kind;
      this.unit = attributes.unit;
      this.standard = true;
    } else {
      this.standard = false;
    }
  }

  protected chooseCatalog(code: string): void {
    this.catalogChoice = code;
    const item = this.catalog.find((candidate) => candidate.code === Number(code));
    if (item) this.titleChanged(item.name);
  }

  protected itemsIn(category: Category) {
    return this.catalog.filter((item) => item.cat === category.id);
  }

  protected things(): Thing[] {
    return this.thingData.value() ?? [];
  }

  protected attributesLocked(): boolean {
    return Boolean(this.selectedThing) || this.catalog.some(
      (i) => i.name.localeCompare(this.title, undefined, { sensitivity: 'accent' }) === 0,
    );
  }

  protected quantityLabel(l: Listing): string {
    return `${formatQuantity(l.quantity_milli)} ${(l.unit ?? 'EACH').toLocaleLowerCase()}`;
  }

  protected offerAgain(l: Listing): void {
    void this.router.navigate(['/list'], { state: { repeat: l } });
  }

  private prefill(l: Listing): void {
    this.title = l.title;
    this.description = l.description;
    this.price = this.money.input(l.price);
    this.side = l.side ?? 'SELL';
    this.economicKind = l.economic_kind ?? 'OTHER';
    this.quantity = formatQuantity(l.quantity_milli);
    this.unit = l.unit ?? 'EACH';
    this.standard = l.standard ?? false;
    this.selectedThing = l.thing ?? '';
    this.catalogChoice = '';
  }

  /**
   * Buying is one request. The client never transfers and then marks the
   * listing sold - those could partially succeed - and the server does not
   * offer that shape anyway.
   *
   * The idempotency key is made here, before the attempt, so that a retry
   * after a dropped connection is recognisably the same purchase.
   */
  protected async buy(l: Listing): Promise<void> {
    if (this.buying()) return;
    // One tap used to pay. A child who tapped "Buy" on a car wash they could
    // not use had no chance to stop, so every purchase is confirmed first.
    const price = this.money.format(l.price);
    const answer = await this.dialogs.confirm({
      title: `Buy “${l.title}”?`,
      message: `You pay ${l.seller_name} ${price} NC right now.`,
      detail: [
        'Only buy it if you really want it.',
        `Changed your mind later? Open My account, then TODO, and press “I changed my mind”. ${l.seller_name} or Nana can give the money back.`,
      ],
      confirmLabel: `Buy for ${price} NC`,
    });
    if (answer === null || this.buying()) return;
    this.buying.set(l.id);
    const key = newIdempotencyKey();
    try {
      const res = await this.api.purchase(l.id, key);
      this.toasts.ok(`Bought ${res.listing.title} for ${this.money.format(res.listing.price)} coins. Find it in My account under TODO.`);
      await this.session.refresh();
    } catch (e) {
      this.toasts.fromError(e);
    } finally {
      this.buying.set(null);
    }
  }

  protected async cancel(l: Listing): Promise<void> {
    try {
      await this.api.cancelListing(l.id);
      this.toasts.ok('Listing cancelled.');
      await this.session.refresh();
    } catch (e) {
      this.toasts.fromError(e);
    }
  }
}
