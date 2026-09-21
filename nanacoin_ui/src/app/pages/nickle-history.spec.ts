import { loadSavedNickles, saveNickle } from './nickle-history';

describe('saved Nana-nickles', () => {
  beforeEach(() => localStorage.clear());

  it('keeps issued voucher secrets in this browser and replaces a duplicate serial', () => {
    const first = {
      token: 'secret-one', serial: 'NN-1', amount: 5, issuerId: 'alice',
      issuerAccount: 'account-alice', issuerName: 'Alice', createdAt: 10,
    };
    saveNickle(first);
    saveNickle({ ...first, token: 'secret-two' });
    expect(loadSavedNickles()).toEqual([{ ...first, token: 'secret-two' }]);
  });

  it('ignores damaged browser data', () => {
    localStorage.setItem('nanacoin:nickles:issued:v1', '{bad json');
    expect(loadSavedNickles()).toEqual([]);
  });
});
