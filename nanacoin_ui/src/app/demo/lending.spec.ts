import { DemoLending } from './lending';
import { LoanOfferInput, User } from '../api/models';
describe('demo loan applications',()=>{
  it('publishes desired terms without moving money, then requires borrower consent',()=>{
    const borrower={account:'a',display_name:'Alice',status:'ACTIVE',role:'user'} as User;
    const lender={account:'b',display_name:'Bob',status:'ACTIVE',role:'user'} as User;
    const users=[borrower,lender]; const posts: unknown[]=[];
    const book=new DemoLending({balance:()=>10000,user:a=>users.find(u=>u.account===a)!,post:(...p)=>posts.push(p)});
    const terms:LoanOfferInput={borrower:'a',amount:100,installment:10,rate_bps:500,rate_days:365,payment_days:7,credit:false,memo:'Kitchen project'};
    const wanted=book.offer(borrower,terms,1700000000,true);
    expect(wanted.status).toBe('REQUESTED'); expect(wanted.waiting_reason).toBe('');
    expect(book.book(lender,4,0,1,1700000000).loans[0].id).toBe(wanted.id);
    expect(posts.length).toBe(0);
    expect(()=>book.accept(borrower,wanted.id,1700000000)).toThrow();
    const offered=book.offer(lender,{...terms,rate_bps:250},1700000000,false,wanted.id);
    expect(offered.status).toBe('OFFERED'); expect(posts.length).toBe(0);
    expect(()=>book.accept(lender,wanted.id,1700000000)).toThrow();
    expect(book.accept(borrower,wanted.id,1700000000).status).toBe('ACTIVE'); expect(posts.length).toBe(1);
  });
});
