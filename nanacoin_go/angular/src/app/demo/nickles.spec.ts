import { DemoLedger, NICKLE_RESERVE } from './ledger';
function setup() {
 const ledger = new DemoLedger(); ledger.provision('nana', 'Nana', 'demo', 'Home');
 const nana = ledger.userByName('nana')!;
 const alice = ledger.addUser('alice', 'Alice', 'demo', 'user');
 const bob = ledger.addUser('bob', 'Bob', 'demo', 'user');
 ledger.issue(nana, alice.account, 20, 'Starting');
 return { ledger, nana, alice, bob };
}
describe('demo bearer vouchers', () => {
 it('reserves existing coins without inflation, redeems once, and never puts a secret in history', () => {
   const { ledger, alice, bob } = setup();
   const before = ledger.status().circulation;
   const voucher = ledger.createNickle(alice, 5);
   expect(voucher.token).toMatch(/^DEMO-NN-[a-f0-9]{64}$/);
   expect(ledger.balanceOf(alice.account)).toBe(15);
   expect(ledger.balanceOf(NICKLE_RESERVE)).toBe(5);
   expect(ledger.status().circulation).toBe(before);
   expect(() => ledger.redeemNickle(alice, voucher.token)).toThrow();
   expect(ledger.balanceOf(NICKLE_RESERVE)).toBe(5);
   ledger.redeemNickle(bob, voucher.token);
   expect(ledger.balanceOf(bob.account)).toBe(5);
   expect(ledger.balanceOf(NICKLE_RESERVE)).toBe(0);
   expect(() => ledger.redeemNickle(alice, voucher.token)).toThrow();
   expect(ledger.status().circulation).toBe(before);
   expect(ledger.balanced()).toBe(true);
   expect(JSON.stringify(ledger.ledger(100))).not.toContain(voucher.token);
 });
 it('permits only Nana to create fresh money and rejects insufficient funds without side effects', () => {
   const { ledger, nana, alice, bob } = setup();
   expect(() => ledger.createNickle(alice, 5, true)).toThrow();
   expect(() => ledger.createNickle(bob, 5)).toThrow();
   expect(() => ledger.createNickle(alice, 0)).toThrow();
   const voucher = ledger.createNickle(nana, 5, true);
   expect(ledger.status().circulation).toBe(25);
   ledger.redeemNickle(bob, voucher.token);
   expect(ledger.status().circulation).toBe(25);
 });
 it('refuses independent reversals of voucher reserve movements', () => {
   const { ledger, nana, alice } = setup();
   ledger.createNickle(alice, 5);
   const txn = ledger.ledger(1).transactions[0];
   expect(() => ledger.reverse(nana, txn.id, 'undo')).toThrow();
   expect(ledger.balanceOf(NICKLE_RESERVE)).toBe(5);
 });
 it('caps outstanding vouchers and rejects disabled holders', () => {
   const { ledger, nana, alice } = setup();
   for (let i = 0; i < 128; i++) ledger.createNickle(nana, 1, true);
   expect(() => ledger.createNickle(nana, 1, true)).toThrow();
   alice.status = 'DISABLED';
   expect(() => ledger.createNickle(alice, 1)).toThrow();
 });
});
