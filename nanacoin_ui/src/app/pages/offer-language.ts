import { Offer } from '../api/models';
export type OfferGroup = 'received-buy' | 'received-sell' | 'sent-buy' | 'sent-sell';
export function offerGroup(o: Offer, account: string): OfferGroup | null {
  if (o.offerer !== account && o.listing_owner !== account) return null;
  const action = o.listing_side === 'BUY' ? 'sell' : 'buy';
  return `${o.offerer === account ? 'sent' : 'received'}-${action}`;
}
export function offerSentence(o: Offer): string {
  const owner = o.listing_owner_name || o.listing_owner || 'the listing owner';
  return o.listing_side === 'BUY'
    ? `${o.offerer_name} made an offer to sell to ${owner}.`
    : `${o.offerer_name} made an offer to buy from ${owner}.`;
}
export function offerPayment(o: Offer): string {
  const owner = o.listing_owner_name || o.listing_owner || 'the listing owner';
  return o.listing_side === 'BUY' ? `${owner} pays ${o.offerer_name}` : `${o.offerer_name} pays ${owner}`;
}
