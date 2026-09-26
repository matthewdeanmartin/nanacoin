import { Transaction, User } from '../api/models';
import { MAX_MONEY } from '../api/money';

export interface GiftRequest { id: number; owner: number; title: string; description: string; target: number | null; deadline: number | null; received: number; closed: boolean; created_at:number }
export interface Artwork { id: number; creator: number; owner: number; title: string; license: string; sha256: string; locator: string; price: number | null; revision: number; equipped: boolean; created_at:number }
export type CommerceAction =
  | {create_request: {title:string;description:string;target:number|null;deadline:number|null}}
  | {close_request: {request:number}}
  | {contribute: {request:number;amount:number;memo:string}}
  | {mint_art: {title:string;license:string;sha256:string;locator:string}}
  | {list_art: {art:number;price:number|null}}
  | {buy_art: {art:number;expected_owner:number;expected_revision:number;expected_price:number}}
  | {gift_art: {art:number;to:number}}
  | {equip_art: {art:number;equipped:boolean}};
interface Host {
  user(id:number): User | undefined;
  now(): number;
  sequence(): number;
  pay(actor:User,to:string,amount:number,memo:string,reference:{gift_request?:number;art?:number}): Transaction;
}
/** Browser-only commerce using the same command payloads and atomic cash/ownership rules. */
export class DemoCommerce {
  private requests: GiftRequest[] = [];
  private artworks: Artwork[] = [];
  constructor(private readonly host: Host) {}
  book(): {requests:GiftRequest[];artworks:Artwork[]} { return structuredClone({requests:this.requests,artworks:this.artworks}); }
  private active(id:number): User { const u=this.host.user(id);if(!u || u.status!=='ACTIVE') throw new Error('Member is unavailable.');return u; }
  private amount(n:number): void { if(!Number.isSafeInteger(n) || n<=0 || n>MAX_MONEY) throw new Error('Invalid amount.'); }
  private text(s:string,max:number,required=false): void { if(typeof s!=='string' || (required&&!s.trim()) || new TextEncoder().encode(s).length>max || /[\u0000-\u001f\u007f]/.test(s)) throw new Error('Invalid text.'); }
  command(actor:User, action:CommerceAction): {sequence:number;replayed:false} {
    const owner=Number(actor.id.replace('user-',''));this.active(owner);
    if(Object.keys(action).length!==1) throw new Error('Choose one commerce command.');
    let apply: (sequence:number)=>void;
    if('create_request' in action) {
      const a=action.create_request;this.text(a.title,80,true);this.text(a.description,96);
      if(a.target!==null) this.amount(a.target);
      if(a.deadline!==null && (!Number.isSafeInteger(a.deadline)||a.deadline<=this.host.now())) throw new Error('Deadline must be in the future.');
      if(this.requests.length>=32) throw new Error('Gift request capacity reached.');
      const created_at=this.host.now();
      apply=id=>this.requests.push({...a,id,owner,received:0,closed:false,created_at});
    } else if('close_request' in action || 'contribute' in action) {
      const a='close_request' in action?action.close_request:action.contribute;
      const r=this.requests.find(r=>r.id===a.request);if(!r) throw new Error('Gift request not found.');
      if('close_request' in action) {
        if(r.owner!==owner || r.closed) throw new Error('Only the owner can close an open request.');
        apply=()=>{r.closed=true;};
      } else {
        const a=action.contribute;this.amount(a.amount);this.text(a.memo,96);
        if(r.closed || (r.deadline!==null&&r.deadline<=this.host.now()) || r.owner===owner) throw new Error('This request cannot receive your contribution.');
        const to=this.active(r.owner);
        if(!Number.isSafeInteger(r.received+a.amount)) throw new Error('Contribution limit exceeded.');
        // Payment validates before appending; everything after it is infallible.
        this.host.pay(actor,to.account,a.amount,a.memo||r.title,{gift_request:r.id});
        apply=()=>{r.received+=a.amount;};
      }
    } else if('mint_art' in action) {
      const a=action.mint_art;this.text(a.title,80,true);this.text(a.license,96,true);this.text(a.locator,192,true);
      if(!/^[a-fA-F0-9]{64}$/.test(a.sha256) || !a.locator.startsWith('https://')) throw new Error('Art needs a SHA-256 digest and HTTPS locator.');
      if(this.artworks.length>=64) throw new Error('Art capacity reached.');
      const created_at=this.host.now();
      apply=id=>this.artworks.push({...a,id,creator:owner,owner,price:null,revision:id,equipped:false,created_at});
    } else {
      const a='list_art' in action?action.list_art:'buy_art' in action?action.buy_art:'gift_art' in action?action.gift_art:'equip_art' in action?action.equip_art:null;
      if(!a) throw new Error('Unknown commerce command.');
      const art=this.artworks.find(art=>art.id===a.art);if(!art) throw new Error('Artwork not found.');
      if('buy_art' in action) {
        const a=action.buy_art;
        if(art.owner===owner || art.price===null || a.expected_owner!==art.owner || a.expected_revision!==art.revision || a.expected_price!==art.price) throw new Error('The artwork listing changed.');
        const seller=this.active(art.owner);
        this.host.pay(actor,seller.account,art.price,`Digital art: ${art.title}`,{art:art.id});
        apply=sequence=>{art.owner=owner;art.price=null;art.equipped=false;art.revision=sequence;};
      } else {
        if(art.owner!==owner) throw new Error('Only the owner can change this artwork.');
        if('list_art' in action) {
          const price=action.list_art.price;if(price!==null)this.amount(price);
          apply=sequence=>{art.price=price;art.revision=sequence;};
        } else if('gift_art' in action) {
          const to=action.gift_art.to;this.active(to);if(to===owner)throw new Error('Choose another member.');
          apply=sequence=>{art.owner=to;art.price=null;art.equipped=false;art.revision=sequence;};
        } else {
          const equipped=action.equip_art.equipped;if(typeof equipped!=='boolean')throw new Error('Invalid equipment choice.');
          apply=sequence=>{if(equipped)for(const other of this.artworks)if(other.owner===owner)other.equipped=false;art.equipped=equipped;art.revision=sequence;};
        }
      }
    }
    const sequence=this.host.sequence();apply(sequence);return {sequence,replayed:false};
  }
  refunded(id:number,amount:number): void { const r=this.requests.find(r=>r.id===id)!;r.received-=amount; }
  reform(convert:(n:number)=>number): ()=>void {
    const requests=this.requests.map(r=>({...r,target:r.target===null?null:convert(r.target),received:convert(r.received)}));
    const artworks=this.artworks.map(a=>({...a,price:a.price===null?null:convert(a.price)}));
    return ()=>{this.requests=requests;this.artworks=artworks;};
  }
}
