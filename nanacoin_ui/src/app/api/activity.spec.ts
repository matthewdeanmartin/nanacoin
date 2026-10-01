import { Activity } from './activity';
describe('server activity',()=>{
  afterEach(()=>vi.useRealTimers());
  it('keeps working visible until concurrent requests finish',()=>{ const a=new Activity(); a.begin(); a.begin(); a.end(); expect(a.pending()).toBe(true); a.end(); expect(a.pending()).toBe(false); });
  it('shows failure for three seconds and renews it on another failure',()=>{ vi.useFakeTimers(); const a=new Activity(); a.begin(); a.fail(); a.end(); expect(a.failed()).toBe(true); vi.advanceTimersByTime(2000); a.fail(); vi.advanceTimersByTime(2000); expect(a.failed()).toBe(true); vi.advanceTimersByTime(1000); expect(a.failed()).toBe(false); });
});
