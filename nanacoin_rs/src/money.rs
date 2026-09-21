//! Currency precision and exact, atomic decimal reforms. All cash uses minor units.
use crate::domain::*;

pub fn scale(decimals: u8) -> i64 {
    10i64.pow(decimals as u32)
}

pub fn rescale(value: i64, exponent: i16, limit: i64) -> Result<i64, Error> {
    let factor = 10i128
        .checked_pow(exponent.unsigned_abs() as u32)
        .ok_or(Error::Overflow)?;
    let value = value as i128;
    let result = if exponent >= 0 {
        value.checked_mul(factor).ok_or(Error::Overflow)?
    } else {
        if value % factor != 0 {
            return Err(Error::Conflict);
        }
        value / factor
    };
    if result.unsigned_abs() > limit as u128 {
        return Err(Error::Overflow);
    }
    Ok(result as i64)
}

impl State {
    pub fn validate_reform(&self, decimals: u8, power: i8) -> Result<(), Error> {
        if decimals > 8 || !(-12..=12).contains(&power) || (decimals == self.decimals && power == 0)
        {
            return Err(Error::InvalidInput);
        }
        if self.money_epoch >= MAX_SEQUENCE {
            return Err(Error::Overflow);
        }
        let exponent = decimals as i16 - self.decimals as i16 - power as i16;
        let amount = |v| rescale(v, exponent, MAX_AMOUNT);
        let balance = |v| rescale(v, exponent, MAX_SEQUENCE as i64);
        amount(self.initial_grant)?;
        balance(self.issuance_balance)?;
        for m in &self.members {
            balance(m.balance)?;
        }
        for l in &self.listings {
            amount(l.price)?;
        }
        for o in &self.offers {
            amount(o.amount)?;
        }
        for t in &self.history {
            if !t.usd {
                amount(t.amount)?;
            }
        }
        for q in &self.quotes {
            amount(q.coins)?;
            rescale(q.cents_per_coin, power as i16, MAX_AMOUNT)?;
        }
        for l in &self.loans {
            amount(l.terms.amount)?;
            amount(l.terms.installment)?;
            balance(l.principal)?;
            balance(l.principal_due)?;
            balance(l.interest_due)?;
            balance(l.attempted_balance)?;
            let denominator = l.denominator() as i128;
            let numerator = l.interest as i128 * denominator + l.remainder as i128;
            let factor = 10i128.pow(exponent.unsigned_abs() as u32);
            let converted = if exponent >= 0 {
                numerator.checked_mul(factor).ok_or(Error::Overflow)?
            } else {
                if numerator % factor != 0 {
                    return Err(Error::Conflict);
                }
                numerator / factor
            };
            if converted / denominator + balance(l.principal)? as i128 > MAX_SEQUENCE as i128 {
                return Err(Error::Overflow);
            }
        }
        Ok(())
    }

    pub(crate) fn apply_reform(&mut self, decimals: u8, power: i8) {
        let exponent = decimals as i16 - self.decimals as i16 - power as i16;
        let convert =
            |value: &mut i64| *value = rescale(*value, exponent, MAX_SEQUENCE as i64).unwrap();
        convert(&mut self.initial_grant);
        convert(&mut self.issuance_balance);
        for m in &mut self.members {
            convert(&mut m.balance);
        }
        for l in &mut self.listings {
            convert(&mut l.price);
        }
        for o in &mut self.offers {
            convert(&mut o.amount);
        }
        for t in &mut self.history {
            if !t.usd {
                convert(&mut t.amount);
            }
        }
        for q in &mut self.quotes {
            q.nc_scale = scale(decimals);
            convert(&mut q.coins);
            q.cents_per_coin = rescale(q.cents_per_coin, power as i16, MAX_AMOUNT).unwrap();
        }
        for l in &mut self.loans {
            convert(&mut l.terms.amount);
            convert(&mut l.terms.installment);
            convert(&mut l.principal);
            convert(&mut l.principal_due);
            convert(&mut l.interest_due);
            convert(&mut l.attempted_balance);
            let denominator = l.denominator() as i128;
            let numerator = l.interest as i128 * denominator + l.remainder as i128;
            let factor = 10i128.pow(exponent.unsigned_abs() as u32);
            let converted = if exponent >= 0 {
                numerator * factor
            } else {
                numerator / factor
            };
            l.interest = (converted / denominator) as i64;
            l.remainder = (converted % denominator) as u64;
        }
        self.decimals = decimals;
        self.money_epoch += 1;
    }
}
