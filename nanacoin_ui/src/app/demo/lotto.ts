import { Lotto, LottoTerms, Posting, User } from '../api/models';

const MONTH = 30 * 86400;
interface Draw { view: Lotto; entries: Map<string, number> }
export class DemoLotto {
  private draws: Draw[] = [];
  constructor(private readonly host: {
    balance(account: string): number;
    name(account: string): string;
    post(id: number, memo: string, postings: Posting[]): void;
  }) {}
  private active(actor: User): void { if (actor.status !== 'ACTIVE') throw new Error('This account is disabled.'); }
  book(actor: User) { return this.draws.map(d => ({...d.view, terms:{...d.view.terms}, my_tickets:d.entries.get(actor.account) ?? 0})).reverse(); }
  create(actor: User, terms: LottoTerms, now: number): Lotto {
    this.active(actor);
    if (actor.role !== 'nana') throw new Error('Only Nana can create a lotto.');
    if (!['SIMPLE','DELAYED','SAVINGS'].includes(terms.kind) || !terms.title?.trim() || terms.title.length>80 || !Number.isSafeInteger(terms.ticket_price) || terms.ticket_price<=0 || !Number.isSafeInteger(terms.closes_at) || terms.closes_at<=now || !Number.isInteger(terms.rate_bps) || terms.rate_bps<0 || terms.rate_bps>10000 || (terms.kind==='SIMPLE' && terms.rate_bps!==0)) throw new Error('Invalid lotto terms.');
    const view: Lotto = {id:this.draws.length+1,terms:{...terms},house:actor.account,pool:0,interest:0,tickets:0,my_tickets:0,winner:null,winner_name:null,due_at:terms.closes_at+(terms.kind==='SIMPLE'?0:MONTH),status:'OPEN'};
    this.draws.push({view,entries:new Map()}); return {...view};
  }
  buy(actor: User, id: number, count: number, now: number): Lotto {
    this.active(actor);
    const d=this.draws.find(d=>d.view.id===id);
    if (!d || d.view.status!=='OPEN' || now>=d.view.terms.closes_at) throw new Error('Ticket sales are closed.');
    const l=d.view, cost=count*l.terms.ticket_price;
    if (actor.account===l.house) throw new Error('Nana cannot buy tickets in her own draw.');
    if (!Number.isInteger(count) || count<1 || count+l.tickets>4294967295 || !Number.isSafeInteger(cost+l.pool) || cost>this.host.balance(actor.account)) throw new Error('Enter a whole ticket count you can afford.');
    const pool=cost+l.pool, interest=Number(BigInt(pool)*BigInt(l.terms.rate_bps)/10000n);
    if (!Number.isSafeInteger(pool+interest)) throw new Error('Pool exceeds the accounting limit.');
    this.host.post(id,'Lotto tickets', [{account:actor.account,name:'',amount:-cost},{account:`lotto-pool-${id}`,name:'Lotto pool',amount:cost}]);
    d.entries.set(actor.account,(d.entries.get(actor.account) ?? 0)+count);
    l.pool=pool;l.interest=interest;l.tickets+=count;
    return {...l,my_tickets:d.entries.get(actor.account)!};
  }
  resolveNow(actor: User, now: number): number {
    this.active(actor);
    if (actor.role !== 'nana') throw new Error('Only Nana can resolve demo lottos.');
    const pending=this.draws.filter(d=>d.view.status!=='SETTLED').length;
    this.settle(now,true);
    return pending;
  }
  tick(now: number): void { this.settle(now,false); }
  private settle(now: number, force: boolean): void {
    for (const d of this.draws) {
      const l=d.view;
      if (l.status==='SETTLED' || (!force && now<l.terms.closes_at)) continue;
      l.status='WAITING';
      if (!force && now<l.due_at) continue;
      if (force) { l.terms.closes_at=Math.min(l.terms.closes_at,now);l.due_at=now; }
      if (!l.tickets) { l.status='SETTLED'; continue; }
      // Rejection sampling gives every ticket an equal chance without modulo bias.
      const bound=Math.floor(4294967296/l.tickets)*l.tickets;
      let random: number; do { random=crypto.getRandomValues(new Uint32Array(1))[0]; } while(random>=bound);
      let ticket=random%l.tickets, winner='';
      for (const [account,count] of d.entries) { if(ticket<count) {winner=account;break;} ticket-=count; }
      const totals=new Map<string,number>();
      const add=(account:string,amount:number)=>totals.set(account,(totals.get(account)??0)+amount);
      add(`lotto-pool-${l.id}`,-l.pool);
      if (l.terms.kind==='SAVINGS') for(const [account,count] of d.entries) add(account,count*l.terms.ticket_price);
      else add(winner,l.pool);
      this.host.post(l.id,'Lotto principal payout',Array.from(totals,([account,amount])=>({account,name:this.host.name(account),amount})).filter(p=>p.amount!==0));
      const funded=Math.min(Math.max(0,this.host.balance(l.house)),l.interest);
      if(l.interest) this.host.post(l.id,'Lotto interest',[
        {account:l.house,name:this.host.name(l.house),amount:-funded},
        {account:'account:system-issuance',name:'Issuance',amount:-(l.interest-funded)},
        {account:winner,name:this.host.name(winner),amount:l.interest},
      ].filter(p=>p.amount!==0));
      l.winner=winner;l.winner_name=this.host.name(winner);l.status='SETTLED';
    }
  }
  reform(convert: (value:number)=>number): ()=>void {
    const changes=this.draws.map(({view:l})=>({l,price:convert(l.terms.ticket_price),pool:convert(l.pool),interest:convert(l.interest)}));
    return ()=>{for(const c of changes) {c.l.terms.ticket_price=c.price;c.l.pool=c.pool;c.l.interest=c.interest;}};
  }
}
