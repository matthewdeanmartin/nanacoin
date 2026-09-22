import { Loan, Offer, Transaction } from '../api/models';
import { offerSentence, offerPayment } from './offer-language';
export interface MailRow { id: string; revision: string; at: number; sender: string; subject: string; body: string; kind: 'Message' | 'Offer' | 'Loan' | 'Transaction'; sent: boolean; attention: boolean; amount?: number; route?: string; replyTo?: string }
export function mailRows(account: string, transactions: Transaction[], offers: Offer[], loans: Loan[]): MailRow[] {
  if (!account) return [];
  const rows: MailRow[] = transactions.filter(t => t.postings.some(p => p.account === account)).map(t => {
    const message = t.kind === 'MESSAGE';
    const from = message ? t.postings[0] : t.postings.find(p => p.amount < 0);
    const to = message ? t.postings[1] : t.postings.find(p => p.amount > 0);
    const sent = from?.account === account;
    const other = sent ? to : from;
    return { id: t.id, revision: t.id, at: t.created_at, sender: `${sent ? 'To' : 'From'} ${other?.name || 'System'}`,
      subject: t.description || t.kind.toLowerCase(), body: t.description || t.kind.toLowerCase(), kind: message ? 'Message' : 'Transaction', sent, attention: false,
      amount: message ? undefined : t.postings.filter(p => p.account === account).reduce((n,p) => n+p.amount,0),
      route: message ? undefined : '/history', replyTo: other?.account?.startsWith('account-') ? other.account : undefined };
  });
  for (const o of offers) {
    if (o.offerer !== account && o.listing_owner !== account) continue;
    const sent = o.offerer === account;
    rows.push({ id: o.id, revision: `${o.id}:${o.status}:${o.updated_at}`, at: o.updated_at,
      sender: sent ? `To ${o.listing_owner_name || o.listing_owner}` : `From ${o.offerer_name}`, subject: `${o.listing_title} · ${o.status.toLowerCase().replaceAll('_',' ')}`,
      body: `${offerSentence(o)} ${offerPayment(o)} if accepted.${o.message ? '\n'+o.message : ''}`,
      kind: 'Offer', sent, attention: !sent && o.status === 'OPEN', amount: o.amount, route: '/offers', replyTo: sent ? o.listing_owner : o.offerer });
  }
  for (const l of loans) {
    if (l.lender !== account && l.borrower !== account) continue;
    const sent = l.lender === account;
    rows.push({ id: `loan-${l.id}`, revision: `loan-${l.id}:${l.status}:${l.updated_at}`, at: l.updated_at,
      sender: sent ? `To ${l.borrower_name}` : `From ${l.lender_name}`, subject: `Loan · ${l.status.toLowerCase()}`,
      body: `${l.lender_name} lends to ${l.borrower_name}.${l.memo ? '\n'+l.memo : ''}`, kind: 'Loan', sent,
      attention: !sent && (l.status === 'OFFERED' || l.overdue > 0), amount: l.amount, route: '/loans', replyTo: sent ? l.borrower : l.lender });
  }
  return rows.sort((a,b) => b.at-a.at || b.id.localeCompare(a.id,undefined,{numeric:true}));
}
