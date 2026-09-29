import { type Meta, type StoryObj } from "@storybook/angular";

const meta: Meta = { title: "Content" };
export default meta;

export const MarketCards: StoryObj = {
  name: "Market cards",
  render: () => ({
    template: `
      <div class="cards">
        <article class="card">
          <h3>Mow the lawn</h3>
          <p class="card__desc">Front and back, bagged clippings.</p>
          <p class="card__meta">15 NC · Timmy</p>
          <button type="button" class="btn">Hire</button>
        </article>
        <article class="card">
          <h3>Banana bread</h3>
          <p class="card__desc">One loaf, still warm.</p>
          <p class="card__note">Nana says: walnuts on request.</p>
          <p class="card__meta">8 NC · Nana</p>
          <button type="button" class="btn">Buy</button>
        </article>
        <article class="card card--sold">
          <h3>Old bike</h3>
          <p class="card__desc">Needs a new chain.</p>
          <p class="card__status">Sold</p>
        </article>
      </div>
    `,
  }),
};

export const Tags: StoryObj = {
  render: () => ({
    template: `
      <div class="ds-row">
        <span class="tag">transfer</span>
        <span class="tag tag--warn">pending</span>
        <span class="tag tag--reversed">reversed</span>
        <span class="tag tag--off">disabled</span>
      </div>
    `,
  }),
};

export const Transactions: StoryObj = {
  render: () => ({
    template: `
      <div class="history">
        <div class="txn txn--in" tabindex="-1">
          <div class="txn__main">
            <span class="txn__desc">Allowance</span>
            <span class="txn__who">from Nana</span>
            <span class="txn__when">Saturday, 9:00</span>
          </div>
          <div class="txn__side"><span class="txn__amount">+5 NC</span></div>
        </div>
        <div class="txn txn--out" tabindex="-1">
          <div class="txn__main">
            <span class="txn__desc">Banana bread</span>
            <span class="txn__who">to Nana</span>
            <span class="txn__when">Sunday, 15:12</span>
          </div>
          <div class="txn__side"><span class="txn__amount">−8 NC</span></div>
          <div class="txn__actions">
            <span class="tag tag--reversed">reversed</span>
          </div>
        </div>
      </div>
    `,
  }),
};

export const LedgerRows: StoryObj = {
  name: "Ledger rows",
  render: () => ({
    template: `
      <div class="ledger">
        <article class="ledger-row">
          <div class="ledger-row__head">
            <span class="ledger-row__kind">Transfer</span>
            <span class="ledger-row__desc">Mowing the lawn</span>
            <span class="ledger-row__when">Sep 28, 14:02</span>
          </div>
          <div class="ledger-row__postings">
            <span class="posting posting--debit">Nana −15</span>
            <span class="posting posting--credit">Timmy +15</span>
          </div>
        </article>
        <article class="ledger-row">
          <div class="ledger-row__head">
            <span class="ledger-row__kind">Mint</span>
            <span class="ledger-row__desc">Good deed: shovelled the walk</span>
            <span class="ledger-row__when">Sep 27, 08:30</span>
          </div>
          <div class="ledger-row__postings">
            <span class="posting posting--credit">Timmy +3</span>
          </div>
          <span class="tag">good deed</span>
        </article>
      </div>
    `,
  }),
};

export const Members: StoryObj = {
  render: () => ({
    template: `
      <div class="members">
        <div class="member"><span class="member__name">Nana</span><span class="member__balance">412 NC</span></div>
        <div class="member"><span class="member__name">Timmy</span><span class="member__balance">37 NC</span></div>
        <div class="member"><span class="member__name">Aunt Jo</span><span class="member__balance">0 NC</span></div>
      </div>
    `,
  }),
};

export const Stats: StoryObj = {
  render: () => ({
    template: `
      <div class="stats">
        <div class="stat"><span>12</span><span class="stat__label">members</span></div>
        <div class="stat"><span>9</span><span class="stat__label">active</span></div>
        <div class="stat"><span>3</span><span class="stat__label">disabled</span></div>
      </div>
    `,
  }),
};
