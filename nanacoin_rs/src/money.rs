//! Currency precision and exact, atomic decimal reforms. All cash uses minor units.
use crate::domain::*;

pub fn scale(decimals: u8) -> i64 {
    #[cfg(feature = "cobol-core")]
    {
        crate::cobol::rescale(1, decimals.into(), i64::MAX).expect("validated currency scale")
    }
    #[cfg(not(feature = "cobol-core"))]
    {
        10i64.pow(decimals as u32)
    }
}

pub fn rescale(value: i64, exponent: i16, limit: i64) -> Result<i64, Error> {
    #[cfg(feature = "cobol-core")]
    {
        crate::cobol::rescale(value, exponent, limit)
    }
    #[cfg(not(feature = "cobol-core"))]
    {
        if value == 0 {
            return Ok(0);
        }
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
}

impl State {
    pub(crate) fn validate_reform_request(
        &self,
        decimals: u8,
        power: i8,
        expected_epoch: u64,
        expected_sequence: u64,
    ) -> Result<(), Error> {
        #[cfg(feature = "cobol-core")]
        self.reform_policy(decimals, power, expected_epoch, expected_sequence)?;
        #[cfg(not(feature = "cobol-core"))]
        if expected_epoch != self.money_epoch || expected_sequence != self.sequence {
            return Err(Error::Conflict);
        }
        self.validate_reform(decimals, power)
    }
    pub fn validate_reform(&self, decimals: u8, power: i8) -> Result<(), Error> {
        #[cfg(feature = "cobol-core")]
        {
            self.prepare_reform_numbers(&mut crate::cobol::Prepared::default(), decimals, power)
        }
        #[cfg(not(feature = "cobol-core"))]
        {
            if decimals > 8
                || !(-12..=12).contains(&power)
                || (decimals == self.decimals && power == 0)
            {
                return Err(Error::InvalidInput);
            }
            if self.ledger.epochs.len() == crate::ledger::EPOCHS {
                return Err(Error::Overflow);
            }
            let exponent = decimals as i16 - self.decimals as i16 - power as i16;
            self.commerce.validate_rescale(exponent)?;
            let amount = |v| rescale(v, exponent, MAX_AMOUNT);
            let balance = |v| rescale(v, exponent, MAX_SEQUENCE as i64);
            balance(self.lotto_escrow)?;
            for l in &self.lottos {
                amount(l.terms.ticket_price)?;
                amount(l.pool)?;
                amount(l.escrow)?;
                amount(l.interest)?;
                amount(l.interest_remaining)?;
                if amount(l.interest)? as i128
                    != amount(l.pool)? as i128 * l.terms.rate_bps as i128 / 10_000
                {
                    return Err(Error::Conflict);
                }
            }
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
                {
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
                    if converted / denominator + balance(l.principal)? as i128
                        > MAX_SEQUENCE as i128
                    {
                        return Err(Error::Overflow);
                    }
                }
            }
            Ok(())
        }
    }

    pub(crate) fn apply_reform(&mut self, decimals: u8, power: i8) {
        #[cfg(feature = "cobol-core")]
        self.apply_prepared_reform(decimals, power);
        #[cfg(not(feature = "cobol-core"))]
        {
            let exponent = decimals as i16 - self.decimals as i16 - power as i16;
            self.commerce
                .rescale(|v| rescale(v, exponent, MAX_SEQUENCE as i64))
                .unwrap();
            let convert =
                |value: &mut i64| *value = rescale(*value, exponent, MAX_SEQUENCE as i64).unwrap();
            convert(&mut self.lotto_escrow);
            for l in &mut self.lottos {
                convert(&mut l.terms.ticket_price);
                convert(&mut l.pool);
                convert(&mut l.escrow);
                convert(&mut l.interest);
                convert(&mut l.interest_remaining);
            }
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
            self.ledger.epochs.push(crate::ledger::Epoch {
                decimals,
                exponent,
                flows: [0; crate::ledger::FLOW_COUNT],
            });
            self.decimals = decimals;
            self.money_epoch += 1;
        }
    }

    #[cfg(feature = "cobol-core")]
    pub(crate) fn prepare_reform_numbers(
        &self,
        plan: &mut crate::cobol::Prepared,
        decimals: u8,
        power: i8,
    ) -> Result<(), Error> {
        let header = self.reform_policy(decimals, power, self.money_epoch, self.sequence)?;
        let mut add = |value, kind| {
            plan.conversions
                .push(self.reform_field(value, decimals, power, kind)?)
                .map_err(|_| Error::Capacity)
        };
        for r in &self.commerce.requests {
            if let Some(v) = r.target {
                add(v, 0)?;
            }
            add(r.received, 1)?;
        }
        for a in &self.commerce.artworks {
            if let Some(v) = a.price {
                add(v, 2)?;
            }
        }
        add(self.lotto_escrow, 4)?;
        for l in &self.lottos {
            for v in [
                l.terms.ticket_price,
                l.pool,
                l.escrow,
                l.interest,
                l.interest_remaining,
            ] {
                add(v, 3)?;
            }
            crate::cobol::reform_lotto_consistency(
                self.reform_field(l.pool, decimals, power, 3)?,
                self.reform_field(l.interest, decimals, power, 3)?,
                l.terms.rate_bps,
            )?;
        }
        add(self.initial_grant, 5)?;
        add(self.issuance_balance, 6)?;
        for m in &self.members {
            add(m.balance, 6)?;
        }
        for l in &self.listings {
            add(l.price, 7)?;
        }
        for o in &self.offers {
            add(o.amount, 7)?;
        }
        for q in &self.quotes {
            add(q.coins, 8)?;
            add(q.cents_per_coin, 9)?;
        }
        // Record traversal supplies field identities; COBOL owns their limits/scales.
        for l in &self.loans {
            for (v, kind) in [
                (l.terms.amount, 10),
                (l.terms.installment, 10),
                (l.principal, 11),
                (l.principal_due, 11),
                (l.interest_due, 11),
                (l.attempted_balance, 11),
            ] {
                plan.conversions
                    .push(self.reform_field(v, decimals, power, kind)?)
                    .map_err(|_| Error::Capacity)?;
            }
            let (interest, remainder) = crate::cobol::reform_fraction(
                l.interest,
                l.remainder,
                l.denominator(),
                header.exponent,
                self.reform_field(l.principal, decimals, power, 11)?,
            )?;
            plan.conversions
                .push(interest)
                .map_err(|_| Error::Capacity)?;
            plan.conversions
                .push(remainder as i64)
                .map_err(|_| Error::Capacity)?;
        }
        plan.reform = Some(header);
        Ok(())
    }

    #[cfg(feature = "cobol-core")]
    fn apply_prepared_reform(&mut self, decimals: u8, _power: i8) {
        let header = self.prepared.reform.take().expect("prepared reform header");
        let exponent = header.exponent;
        let mut values = self.prepared.conversions.iter().copied();
        let mut next = || values.next().expect("prepared reform field");
        for r in &mut self.commerce.requests {
            if r.target.is_some() {
                r.target = Some(next());
            }
            r.received = next();
        }
        for a in &mut self.commerce.artworks {
            if a.price.is_some() {
                a.price = Some(next());
            }
        }
        self.lotto_escrow = next();
        for l in &mut self.lottos {
            l.terms.ticket_price = next();
            l.pool = next();
            l.escrow = next();
            l.interest = next();
            l.interest_remaining = next();
        }
        self.initial_grant = next();
        self.issuance_balance = next();
        for m in &mut self.members {
            m.balance = next();
        }
        for l in &mut self.listings {
            l.price = next();
        }
        for o in &mut self.offers {
            o.amount = next();
        }
        for q in &mut self.quotes {
            q.nc_scale = header.nc_scale;
            q.coins = next();
            q.cents_per_coin = next();
        }
        for l in &mut self.loans {
            l.terms.amount = next();
            l.terms.installment = next();
            l.principal = next();
            l.principal_due = next();
            l.interest_due = next();
            l.attempted_balance = next();
            l.interest = next();
            l.remainder = next() as u64;
        }
        assert!(values.next().is_none(), "all reform fields applied");
        self.ledger.epochs.push(crate::ledger::Epoch {
            decimals,
            exponent,
            flows: [0; crate::ledger::FLOW_COUNT],
        });
        self.decimals = decimals;
        self.money_epoch = header.next_epoch;
    }
}
