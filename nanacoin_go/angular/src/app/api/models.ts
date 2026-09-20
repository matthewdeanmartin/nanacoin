// Wire types for the NanaCoin API.
//
// These mirror the view types the Go server serialises in
// internal/api/views.go. They are hand-written rather than generated: the API
// is small, stable and versioned under /api/v1, and a generator would be more
// machinery than the whole client.

export type AccountId = string;
export type UserId = string;
export type TransactionId = string;
export type ListingId = string;

export type Role = 'nana' | 'user';
export type UserStatus = 'ACTIVE' | 'DISABLED';
export type ListingStatus = 'ACTIVE' | 'SOLD' | 'CANCELLED';

export type TransactionKind =
  | 'ISSUE'
  | 'RETIRE'
  | 'TRANSFER'
  | 'PURCHASE'
  | 'REVERSAL';

export interface User {
  id: UserId;
  username: string;
  display_name: string;
  role: Role;
  status: UserStatus;
  account: AccountId;
  created_at: number;
  /** Present only where the caller is entitled to see it. */
  balance?: number;

  /**
   * The dollar balance, in cents. $5.00 is 500.
   *
   * Cents rather than dollars for the same reason the ledger has no floats:
   * 5.0 invites arithmetic that rounds, and a household that loses a cent per
   * trade to rounding has no way to find out where it went.
   */
  usd_cents?: number;
}

export interface Posting {
  account: AccountId;
  name: string;
  /** Signed. Negative is money leaving the account. */
  amount: number;
}

export interface Transaction {
  id: TransactionId;
  kind: TransactionKind;
  created_at: number;
  actor: UserId;
  description: string;
  reference?: string;
  /** Set on a REVERSAL: the transaction being undone. */
  reverses?: TransactionId;
  /** Set on a transaction that has been reversed. */
  reversed_by?: TransactionId;
  postings: Posting[];
}

export interface Listing {
  id: ListingId;
  seller: AccountId;
  seller_name: string;
  title: string;
  description: string;
  price: number;
  status: ListingStatus;
  created_at: number;
  updated_at: number;
  buyer?: AccountId;
  buyer_name?: string;
  sold_tx?: TransactionId;
  /** '' or 'item' or 'service' or 'currency'. */
  kind?: string;
  /** For a currency listing: the code, e.g. 'USD'. */
  currency?: string;
  /** For a currency listing: minor units, e.g. 500 for $5.00. */
  minor_units?: number;

  /**
   * Which way round the listing is.
   *
   * 'SELL' is the original kind: someone offers a thing and wants coins for
   * it. 'BUY' is the reverse - "100 NanaCoin for peanut butter cookies" -
   * where the poster has the money and wants the thing.
   *
   * Absent means SELL, so a server that predates two-way listings reads
   * correctly rather than showing every listing as a want-ad.
   */
  side?: ListingSide;
}

export type ListingSide = 'SELL' | 'BUY';

/**
 * A proposal against a listing, which is not a deal until it is accepted.
 *
 * Both directions use it. On a SELL listing an offer is a bid below the
 * asking price; on a BUY listing it is someone saying "I'll do that for your
 * 100". Either way the money only moves when the listing's owner accepts,
 * which is the step the marketplace was missing: before this, the only
 * transaction available was buying at the asking price, with no way to
 * propose anything.
 */
export interface Offer {
  id: OfferId;
  listing: ListingId;
  listing_title: string;
  /** Who made the offer. */
  offerer: AccountId;
  offerer_name: string;
  /** What they are proposing, in NanaCoin. */
  amount: number;
  /** Optional note - "I can do it Saturday". */
  message: string;
  status: OfferStatus;
  created_at: number;
  updated_at: number;
  /** Set once accepted: the transaction that moved the money. */
  settled_tx?: TransactionId;
}

export type OfferId = string;

/**
 * OPEN until the listing's owner decides. ACCEPTED means the money moved and
 * the listing closed. DECLINED and WITHDRAWN are the two ways it ends without
 * a deal - by the owner and by the offerer respectively.
 */
export type OfferStatus = 'OPEN' | 'ACCEPTED' | 'DECLINED' | 'WITHDRAWN';

export interface Status {
  provisioned: boolean;
  household: string;
  currency: string;
  users: number;
  transactions: number;
  active_listings: number;
  circulation: number;
  journal_used: number;
  journal_capacity: number;
  journal_generation?: number;
  checkpoint_supported?: boolean;
  /** False means the ledger does not add up and nobody should trust it. */
  ledger_balanced: boolean;

  /**
   * Whether this server records events and serves /logs at all.
   *
   * A build made without the event ring - which is how the board reclaims
   * the ~4.6 KB it costs - does not register the route, so a Logs tab would
   * link to a 404. Absent on an older server, which predates the flag and
   * always had logs; treat undefined as true.
   */
  logs_enabled?: boolean;

  /** Whether /diag exists. Same reasoning as logs_enabled. */
  diag_enabled?: boolean;
}

export interface LogEvent {
  seq: number;
  /** Unix seconds, or 0 on a board with no clock - use seq for ordering. */
  at: number;
  level: 'info' | 'warn' | 'error';
  /** An HTTP status, or a decision name like 'cors-refuse'. */
  kind: string;
  detail: string;
}

export interface LogPage {
  events: LogEvent[];
  /** Everything ever recorded; higher than events.length once the ring wraps. */
  total: number;
  /** The host's own health - heap figures on the board, absent on a desktop. */
  health?: string;
}

export interface Config {
  household_name: string;
  initial_grant: number;
  currency: string;
}

export interface AccountHistory {
  account: AccountId;
  balance: number;
  transactions: Transaction[];
}

export interface LedgerPage {
  transactions: Transaction[];
  circulation: number;
}

export interface TokenResponse {
  access_token: string;
  token_type: string;
  expires_in: number;
  user: User;
}

export interface PurchaseResult {
  listing: Listing;
  transaction: Transaction;
}

/** The server's error shape: a stable code plus a human sentence. */
export interface ApiErrorBody {
  error: string;
  message: string;
}

/** What accepting an offer returns: the closed offer and the money it moved. */
export interface OfferResult {
  offer: Offer;
  transaction: Transaction;
  listing: Listing;
}

export type QuoteId = string;
export type QuoteSide = 'BID' | 'ASK';
export type QuoteStatus = 'OPEN' | 'FILLED' | 'CANCELLED' | 'EXPIRED';

/**
 * A standing offer to exchange NanaCoin for dollars at a stated rate.
 *
 * Bid means the maker is buying coins and paying dollars; ask means they are
 * selling coins for dollars. The names are the market's, and they are worth
 * keeping: "buy" and "sell" invite the question "buying which one?".
 */
export interface Quote {
  id: QuoteId;
  maker: AccountId;
  maker_name: string;
  side: QuoteSide;

  /** Whole cents one coin is worth. 25 means a coin trades for a quarter. */
  cents_per_coin: number;
  coins: number;

  /** The dollar side: coins x cents_per_coin, computed by the server so the
   *  client never does money arithmetic of its own. */
  cents: number;

  status: QuoteStatus;
  created_at: number;
  updated_at: number;
  expires_at?: number;

  /** Whether it can be taken right now, judged against the server's clock. */
  live: boolean;

  taker?: AccountId;
  taker_name?: string;
  coin_tx?: TransactionId;
  cash_tx?: TransactionId;
}

/** What taking a quote returns: the filled quote and both legs of the money. */
export interface TradeResult {
  quote: Quote;
  coin_transaction: Transaction;
  cash_transaction: Transaction;
}
