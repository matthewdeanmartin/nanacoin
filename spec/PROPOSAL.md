# NanaCoin: future directions

Status: product and architecture proposal. This is a direction-setting document,
not a claim that the features below are implemented.

## What NanaCoin should become

NanaCoin should be a small, playful household economy: part piggy bank, part
market, part bulletin board, and part economics experiment. Its main users may
be eight years old. It should be understandable without knowing banking words,
fun before it is comprehensive, auditable by everyone in the household, operable by Nana, and small
enough to run reliably on the board already on the refrigerator.

The goal is not to imitate a commercial bank or social network feature for
feature. The useful test is: does this help a child make a deal, earn or spend
coins, say thanks, laugh, or discover how an economic rule behaves?

This is a trusted home-network appliance. There is no path for a stranger on the
hostile Internet to send messages directly to the board. The realistic problems
are mistakes, confusing controls, curious siblings, copied links, a lost device,
and an optional outbound integration revealing more than intended. Designs should
fit that threat model rather than importing enterprise machinery for its own sake.

## Product principles

1. **The public book is the source of truth.** Amounts, parties, timestamps,
   reversals, and economic classifications remain auditable. Private-description
   flags may redact human text but never change balances or hide money movement.
2. **Append, do not rewrite.** Corrections, cancellations, classification fixes,
   and schedule changes leave an audit trail.
3. **Safe defaults beat configuration.** A household should work after setup;
   advanced policy belongs under Nana's controls.
4. **The server decides.** Authorization, settlement, schedules, fairness, and
   privacy enforcement cannot depend on an Angular button being hidden.
5. **Bound every feature.** The ESP32 implementation needs fixed or explicitly
   capped records, text, jobs, retries, subscriptions, and external-delivery queues.
6. **Keep home things at home.** Optional outbound notifications are opt-in and
   say as little as possible; the board does not accept inbound Internet messages.
7. **Prefer understandable economics.** Use whole minor units, named categories,
   explicit quantities, and visible rules rather than clever derived behavior.
8. **Make it delightful.** Friendly names, stickers, celebrations, recipes, and
   small surprises belong here. Serious accounting rules can still wear a smile.

## Recommended sequence

### Phase 1: make the current system effortless

#### Accessibility and inclusive interaction

- Audit against WCAG 2.2 AA: keyboard-only flows, visible focus, landmarks,
  heading order, labels, error association, contrast, zoom to 400%, reduced
  motion, touch target size, and screen-reader announcements.
- Test VoiceOver/Safari, NVDA/Firefox, and TalkBack/Chrome rather than relying
  only on automated checks.
- Give charts accessible tables and summaries of the important change, not just
  SVG labels. Preserve the existing plain-text notebook mode.
- Never encode debit/credit, status, or urgency with color alone.
- Make destructive confirmations describe the object, amount, and consequence;
  restore focus to the invoking control afterward.
- Write for an eight-year-old by default. Put the precise accounting term in
  secondary help where it teaches something useful, rather than requiring a
  separate “kids mode.”

#### Ease of use

- Turn My Account into the default dashboard: balance, actionable events,
  pending offers, bids, and the next scheduled obligations.
- Use one consistent, child-readable vocabulary: “person,” “coins,” “offer,”
  “for sale,” “money move,” and “fix.” Introduce terms such as transaction,
  ledger, bid, wage, and inflation beside plain words so the app teaches rather
  than quizzes.
- Add receipts after every write with a human summary and a stable transaction
  link. A dropped connection should lead to “checking what happened,” not an
  invitation to submit the payment again.
- Add contextual empty states: “No offers” should link to the Market and explain
  how to make one.
- Provide filters and search for the public notebook: person, transaction kind,
  category, date, amount, unit, and free text where descriptions are visible.
- Add household onboarding tasks for Nana: add members, choose currency display,
  issue an opening float, post the first listing, and test a transfer.
- Celebrate meaningful firsts—a first earned coin, sale, accepted offer, or fully
  repaid loan—with local animation or a printable badge. Respect reduced-motion
  settings and never make celebration obscure the receipt.

#### Events and notifications inside NanaCoin

Create a bounded domain event inbox per member. Events include:

- payment sent, payment received, and payment corrected;
- offer received, accepted, declined, withdrawn, reversed, or settled;
- listing sold, cancelled, expired, or matched;
- exchange bid/ask filled or expired;
- scheduled payment due, completed, skipped, or failed;
- account/security changes made by Nana.

Each event needs a stable kind, actor, related object ID, timestamp, read state,
and short server-generated summary. The UI can decorate kinds with friendly icons
or stickers. Keep event text separate from diagnostic logs.
Clients can then show badges and Recent Events without reconstructing meaning from
several endpoints.

### Phase 2: make transactions more expressive

#### Transaction taxonomy

Add an economic purpose field distinct from the accounting transaction kind.
Suggested top-level purposes:

- wages and chore pay;
- goods;
- services;
- gifts and allowances;
- reimbursements;
- rent or household contribution;
- loan principal;
- interest;
- tax or fee;
- foreign exchange;
- message/note;
- correction;
- other.

The accounting kind says *how balances moved*; the purpose says *why*. Keep both.
Allow a correction event to fix a classification without rewriting the original
transaction. With this data the Economy page can report earned income, gifts,
goods/services spending, employment participation, and wage trends without
guessing from memo text.

“Employment rate” in a household of children is a game statistic, not an official
labor measure. A useful first version is “people who earned chore/work pay this
week” divided by active people Nana chose to include. Call it **People earning**
in the main UI, put the economic term and exact definition in the explanation,
and never rank or shame children for the result.

#### Quantities, units, and price indexes

Listings and settlements should optionally carry a quantity and standardized
unit in addition to a coin price:

- `2 batch` of cookies;
- `12 each` eggs;
- `30 minute` tutoring;
- `1 load` laundry;
- `500 gram` bread;
- `1 room-hour` of a shared space.

Represent quantity as a scaled integer or rational pair, never a float. Start
with a small built-in unit catalog (`each`, `batch`, `minute`, `hour`, `gram`,
`kilogram`, `milliliter`, `liter`, `load`) plus bounded household-defined units.
Do not pretend unlike goods are comparable merely because both say “batch.” An
item definition should pair a stable household item ID with its unit and quality
notes.

Settled listings produce price observations: coins per normalized unit. From
those observations NanaCoin can show:

- price history for “one batch of the same cookies”;
- median wage per hour for classified work;
- a small household basket index selected by Nana;
- nominal versus basket-adjusted balances;
- data sufficiency warnings when there are too few trades.

Call this a price index, not “the inflation rate,” until the basket, substitutions,
quality changes, and time window are explicit.

#### Zero-coin messages

Zero-coin activity can be useful: thank-you notes, fulfillment updates, receipts,
and discussion attached to an offer. It should not masquerade as money movement.

Recommended design: add an append-only `NOTE` event with author, recipients or
related object, visibility, content, and timestamp, but no ledger postings and no
effect on transaction counts, GDP, balances, or circulation. Give notes their own
bounded retention and per-member rate limit so free messages cannot evict financial
history. If zero-value `TRANSFER` is accepted for API compatibility, normalize it
to `NOTE` and render it clearly as “no coins moved.”

Make notes fun: a small built-in sticker set, reactions such as “Thanks!”, and
links to the payment, chore, or offer they discuss. A zero-coin note should feel
like attaching a card, not like filing an accounting form.

#### Safe rich messages

Do not accept arbitrary HTML. It creates script injection, tracking pixels,
deceptive forms, CSS spoofing, and a sanitizer that must be maintained forever.

Use a small CommonMark-like source format and render only:

- paragraphs and line breaks;
- emphasis and strong text;
- short lists;
- inline code if it proves useful;
- hyperlinks with visible destination and `http`/`https` schemes only.

Strip raw HTML, remote images, embeds, forms, styles, event attributes, `data:`
URLs, and automatic remote fetches. This is mostly protection from pasted junk,
tracking links, broken layouts, and sibling pranks—not an assumption that attackers
can post from the Internet. Add `rel="noopener noreferrer"`; show the destination
before leaving NanaCoin. Store the source plus a format version, and render on the
client from a shared, tested parser. Plain text must always remain a complete fallback.

### Phase 3: schedules, matching, and household policy

#### Durable schedules

Build one bounded server-side scheduler used by:

- recurring allowances and wages;
- rent or household contributions;
- loan installments and interest;
- subscriptions;
- listing/offer/quote expiry;
- idle-money taxes if a household deliberately enables them;
- watches and standing market orders.

A schedule records owner, command template, next due time, recurrence, end rule,
failure policy, and last execution receipt. Execution must be idempotent and use
server wall time. Clock unavailable, insufficient funds, or storage failure should
produce a visible event and a deterministic retry/skip state—never silent drift.

#### Cookie alarms instead of browser “snipers”

A browser loop waiting to click first is unreliable and stops when the tab closes.
Call the child-facing feature a **cookie alarm** or **wish**, with “watch” as the
precise term in help. Implement it as a server-side watch or standing order:

- “Notify me when cookies are listed at 20 coins or less.”
- “Automatically offer up to 15 coins for the next listing matching tag X.”
- “Buy the first matching listing, but no more than once per week.”

Cap watches per person and require a maximum price, quantity, expiry, and total
commitment. Where several children want the same cookies, show a simple fairness
rule: take turns, draw a random winner, or run a short secret-bid auction. Nana
chooses the household rule. Do not silently let Wi-Fi speed decide the winner.

#### More transaction families

- **Requests/invoices:** ask a named member for an amount and purpose; the other
  member accepts, declines, or partially pays.
- **Split payments:** one purchase divided among several members, committed
  atomically or not at all.
- **Escrow:** reserve coins until delivery is accepted or Nana resolves a dispute.
- **Loans:** principal, lender, borrower, schedule, remaining principal, and
  append-only repayments.
- **Interest:** explicit periodic transactions, never invisible balance mutation.
- **Budgets/envelopes:** labels or subaccounts that do not create money.
- **Treasury/taxes:** a household-owned account with visible rules and spending.
- **Auctions:** timed bids with a clear close rule and no client-clock authority.
- **Donations/pools:** multi-member contributions toward a visible goal.

Every new family needs a statement of authorization, conservation of value,
idempotency, reversal semantics, persistence cost, and what appears in the public
book before it earns an endpoint.

### Phase 4: Mastodon as the household's private notification channel

#### Mastodon integration

Each NanaCoin member can choose **Connect Mastodon**, sign in to their own account,
and authorize NanaCoin to send private messages. The connection belongs to that
member, not to the whole household. A member can disconnect it at any time.

The two first uses are deliberately small:

1. NanaCoin DMs you when something happens: “You got 12 coins from Nana,” “Sam
   made an offer on your cookies,” or “Your cookie alarm found something.”
2. From a listing or offer, you can choose **DM to…** and select another connected
   person in the household. The DM contains a short explanation and a link back
   to that exact NanaCoin object.

Mastodon is only a private delivery channel, never an authority over money. A
failed DM must never fail, reverse, or retry a payment. NanaCoin does not read DMs,
replies, mentions, or reactions, and it never accepts payment instructions from
Mastodon. Posting a public status is outside the first version.

Use a bounded transactional outbox written after the NanaCoin action commits.
Recommended behavior:

- opt in per member and choose which event kinds produce DMs;
- send direct-message visibility only;
- let the member choose whether DMs include amount, counterpart, and description;
- offer quiet hours and a daily digest so an eight-year-old does not receive a
  pile of notices;
- show connected household Mastodon recipients by NanaCoin display name, without
  making children type federated addresses for every offer;
- include a NanaCoin link, but never a bearer secret, login token, or voucher code;
- use bounded retries and show “Mastodon could not deliver this” in Recent Events;
- provide a **Send me a test DM** button that moves no money;
- revoke/disconnect without changing NanaCoin history.

The board can act as a small outbound OAuth client and store one bounded connection
record per member in NVS; physical extraction of the board is outside this project's
threat model. If Mastodon TLS/OAuth code proves too large or fragile on the ESP32,
the same **Connect Mastodon** UI can target a tiny trusted home-network bridge that
holds the tokens. That is an implementation fallback, not a different user feature.
There is deliberately no inbound Internet webhook to the board.

#### Standard event and integration formats

Define versioned schemas for transaction, offer, quote, schedule, and notification
events. Use stable IDs and explicit minor units. Provide JSON export plus a simple
CSV view; defer heavyweight accounting standards until there is a concrete
interoperability need.

Useful standards to adopt selectively:

- ISO 4217 codes and minor-unit metadata for national currencies;
- BCP 47 language tags;
- IANA time-zone identifiers where the server can support them, otherwise an
  explicit fixed offset with visible daylight-saving limitations;
- UCUM-inspired unit codes for physical quantity, using only a small supported
  subset;
- RFC 3339 timestamps at external boundaries;
- accessible-name and ARIA practices from WAI-ARIA, without replacing native
  HTML controls.

Avoid adopting a standard merely because it exists. Each code table consumes
storage, UI, migration, and testing budget.

### Phase 5: currency and internationalization

#### Household national-currency configuration

Replace the hard-coded dollar presentation with a household currency profile:

- ISO code (`USD`, `CAD`, `EUR`, `GBP`, and so on);
- display symbol and whether it precedes or follows the amount;
- minor-unit digits;
- locale-independent stored integer amount;
- cash accounting enabled/disabled;
- optional Nana-managed opening balances;
- exchange enabled/disabled and household policy text.

The ledger should store currency codes and integer minor units, never formatted
strings. Formatting belongs to the client. Changing display configuration must
not reinterpret old amounts. Supporting multiple national currencies concurrently
is a later feature and should use one account per member per currency rather than
an ambiguous amount field.

#### Internationalization, deliberately delayed but not sabotaged

Full i18n is expensive: extracting strings is the easy part; plural rules, dates,
currency, layout expansion, right-to-left behavior, validation, screenshots,
documentation, and translation review are the real cost. Defer translation until
the vocabulary and workflows settle.

Prepare now at low cost:

- keep user-facing strings out of server error codes; return stable codes plus
  parameters and let the client phrase common errors;
- never concatenate sentence fragments around variable names or counts;
- use `Intl` for dates, numbers, and configured currency;
- allow layout expansion and avoid text baked into images;
- store Unicode safely with byte limits explained in character-oriented UI;
- attach stable semantic IDs to server-generated event kinds;
- reserve BCP 47 language and text-direction fields in exports, not necessarily
  in the constrained live state.

When translation begins, ship one complete second locale first. It will expose
architectural mistakes more reliably than building a large empty translation
framework now.

## Platform work that enables the roadmap

- Keep the Rust implementation authoritative and the TinyGo firmware frozen until
  its upstream runtime constraints change.
- Add a versioned capability document so the Angular client can show supported
  features without learning by 404.
- Budget RAM/flash before accepting a feature: maximum live records, serialized
  size, response size, text arenas, checkpoint migration, and replay time.
- Add schema migrations and downgrade rules before the first persisted format
  that cannot be interpreted by older firmware.
- Preserve idempotency receipts durably enough for scheduled and externally
  triggered actions.
- Add backup/export and tested restore before loans, schedules, or integrations
  make the board the only copy of long-lived obligations.
- Add a read-only simulator that can replay a household export and preview a rule
  change—tax, allowance, wage, or exchange policy—without touching live balances.
- Measure board health and failure behavior under every new background worker.
  Schedules and notification delivery must sleep cheaply and remain bounded.

## What not to build yet

- Arbitrary HTML, JavaScript, images, or remote embeds in messages.
- Browser-based high-frequency polling or latency contests.
- A general plugin runtime on the ESP32.
- Invisible interest, taxes, fees, or balance recalculation.
- Floating-point money or quantity.
- Algorithmic credit scoring, reputation scores, streak pressure, or leaderboards
  that turn siblings' different ages and abilities into a judgment.
- Multiple exchange mechanisms before the existing offer and forex flows are
  understandable and well used.
- A full translation platform before the product vocabulary stabilizes.

## Suggested next three deliverables

1. **Domain event inbox and capability endpoint.** This unlocks a truthful Recent
   Events page, badges, future notifications, and graceful mixed-version clients.
2. **Purpose, quantity, and unit metadata.** Add them first to listings and settled
   transactions, then build wage/goods/service and unit-price reports from real
   data.
3. **Bounded scheduler with recurring allowance as its first job.** Prove clock,
   persistence, idempotency, failure reporting, and cancellation before adding
   loans, standing orders, taxes, or external delivery.

Those three create the vocabulary and infrastructure for nearly every later idea
without committing NanaCoin to unsafe rich content, unreliable browser automation,
or an unbounded background system.
