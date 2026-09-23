import { Loan } from '../api/models';
import { annualLoanPercent, averageOfferRate } from './loans';
describe('annual simple interest labels',()=>{
  it('normalizes weekly, daily and yearly rates without compounding',()=>{
    expect(annualLoanPercent(500,7)).toBeCloseTo(260.7142857);
    expect(annualLoanPercent(100,1)).toBe(365);
    expect(annualLoanPercent(500,365)).toBe(5);
    expect(annualLoanPercent(0,7)).toBe(0);
  });
  it('averages only open offers in the requested direction, including zero rates',()=>{
    const loan={lender:'a',borrower:'b',rate_bps:500,rate_days:365,status:'OFFERED'} as Loan;
    const loans=[loan,{...loan,rate_bps:0},{...loan,status:'ACTIVE' as const,rate_bps:900},{...loan,lender:'c',rate_bps:1000}];
    expect(averageOfferRate(loans,'a','lender')).toBe(2.5);
    expect(averageOfferRate(loans,'b','borrower')).toBe(5);
    expect(averageOfferRate(loans,'a','borrower')).toBeNull();
  });
});
