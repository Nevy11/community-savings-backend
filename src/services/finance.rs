use crate::error::{AppError, AppResult};
use crate::models::group::InterestMethod;

const BPS_DENOMINATOR: i128 = 10_000;
const MONTHS_PER_YEAR: i128 = 12;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct AmortizationQuote {
    pub method: InterestMethod,
    pub monthly_payment: i64,
    pub total_repayment: i64,
    pub total_interest: i64,
}

#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DividendShare {
    pub member_id: uuid::Uuid,
    pub weight: i128,
    pub share_amount: i64,
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct ContributionWeight {
    pub member_id: uuid::Uuid,
    pub amount: i64,
    pub months_held: i32,
}

/// Flat-rate total interest: principal × annual_rate_bps × term_months / (10_000 × 12)
pub fn flat_rate_total_interest(
    principal: i64,
    annual_rate_bps: i32,
    term_months: i32,
) -> AppResult<i64> {
    validate_loan_inputs(principal, annual_rate_bps, term_months)?;

    let interest = (principal as i128)
        .checked_mul(annual_rate_bps as i128)
        .and_then(|v| v.checked_mul(term_months as i128))
        .ok_or(AppError::Internal)?
        .checked_div(BPS_DENOMINATOR * MONTHS_PER_YEAR)
        .ok_or(AppError::Internal)?;

    i64::try_from(interest).map_err(|_| AppError::Internal)
}

pub fn flat_rate_quote(
    principal: i64,
    annual_rate_bps: i32,
    term_months: i32,
) -> AppResult<AmortizationQuote> {
    let total_interest = flat_rate_total_interest(principal, annual_rate_bps, term_months)?;
    let total_repayment = principal
        .checked_add(total_interest)
        .ok_or(AppError::Internal)?;
    let monthly_payment = total_repayment
        .checked_div(term_months as i64)
        .ok_or(AppError::Internal)?;

    Ok(AmortizationQuote {
        method: InterestMethod::FlatRate,
        monthly_payment,
        total_repayment,
        total_interest,
    })
}

/// Reducing-balance monthly payment using scaled integer arithmetic (scale = 1_000_000).
pub fn reducing_balance_monthly_payment(
    principal: i64,
    annual_rate_bps: i32,
    term_months: i32,
) -> AppResult<i64> {
    validate_loan_inputs(principal, annual_rate_bps, term_months)?;

    if annual_rate_bps == 0 {
        return principal
            .checked_div(term_months as i64)
            .ok_or(AppError::Internal);
    }

    let scale: i128 = 1_000_000;
    let monthly_rate_scaled =
        (annual_rate_bps as i128 * scale) / (BPS_DENOMINATOR * MONTHS_PER_YEAR);

    let one_plus_r = scale + monthly_rate_scaled;
    let mut compound = scale;
    for _ in 0..term_months {
        compound = compound
            .checked_mul(one_plus_r)
            .ok_or(AppError::Internal)?
            .checked_div(scale)
            .ok_or(AppError::Internal)?;
    }

    let numerator = (principal as i128)
        .checked_mul(monthly_rate_scaled)
        .and_then(|v| v.checked_mul(compound))
        .ok_or(AppError::Internal)?;

    let denominator = compound
        .checked_sub(scale)
        .and_then(|v| v.checked_mul(scale))
        .ok_or(AppError::Internal)?;

    let payment = numerator
        .checked_div(denominator)
        .ok_or(AppError::Internal)?;

    i64::try_from(payment).map_err(|_| AppError::Internal)
}

pub fn reducing_balance_quote(
    principal: i64,
    annual_rate_bps: i32,
    term_months: i32,
) -> AppResult<AmortizationQuote> {
    let monthly_payment =
        reducing_balance_monthly_payment(principal, annual_rate_bps, term_months)?;
    let total_repayment = monthly_payment
        .checked_mul(term_months as i64)
        .ok_or(AppError::Internal)?;
    let total_interest = total_repayment
        .checked_sub(principal)
        .ok_or(AppError::Internal)?;

    Ok(AmortizationQuote {
        method: InterestMethod::ReducingBalance,
        monthly_payment,
        total_repayment,
        total_interest,
    })
}

pub fn amortization_quotes(
    principal: i64,
    annual_rate_bps: i32,
    term_months: i32,
) -> AppResult<Vec<AmortizationQuote>> {
    Ok(vec![
        flat_rate_quote(principal, annual_rate_bps, term_months)?,
        reducing_balance_quote(principal, annual_rate_bps, term_months)?,
    ])
}

pub fn quote_for_method(
    method: InterestMethod,
    principal: i64,
    annual_rate_bps: i32,
    term_months: i32,
) -> AppResult<AmortizationQuote> {
    match method {
        InterestMethod::FlatRate => flat_rate_quote(principal, annual_rate_bps, term_months),
        InterestMethod::ReducingBalance => {
            reducing_balance_quote(principal, annual_rate_bps, term_months)
        }
    }
}

/// Dividend Share = (member_weight / total_weight) × distributable_pool
/// member_weight = Σ(contribution_amount × months_held)
#[allow(dead_code)]
pub fn time_weighted_dividend_shares(
    contributions: &[ContributionWeight],
    distributable_pool: i64,
) -> AppResult<Vec<DividendShare>> {
    if distributable_pool <= 0 {
        return Err(AppError::BadRequest(
            "distributable pool must be positive".into(),
        ));
    }

    if contributions.is_empty() {
        return Ok(Vec::new());
    }

    let mut weights: Vec<(uuid::Uuid, i128)> = contributions
        .iter()
        .map(|c| {
            let weight = (c.amount as i128)
                .checked_mul(c.months_held as i128)
                .ok_or(AppError::Internal)?;
            Ok((c.member_id, weight))
        })
        .collect::<AppResult<_>>()?;

    let total_weight: i128 = weights.iter().map(|(_, w)| *w).sum();
    if total_weight <= 0 {
        return Err(AppError::BadRequest(
            "total time-weighted funds must be greater than zero".into(),
        ));
    }

    let pool = distributable_pool as i128;
    let mut allocated: i64 = 0;
    let mut shares = Vec::with_capacity(weights.len());

    weights.sort_by_key(|(member_id, _)| *member_id);

    for (index, (member_id, weight)) in weights.iter().enumerate() {
        let share_amount = if index == weights.len() - 1 {
            distributable_pool
                .checked_sub(allocated)
                .ok_or(AppError::Internal)?
        } else {
            let share = pool
                .checked_mul(*weight)
                .ok_or(AppError::Internal)?
                .checked_div(total_weight)
                .ok_or(AppError::Internal)?;
            let share_i64 = i64::try_from(share).map_err(|_| AppError::Internal)?;
            allocated = allocated.checked_add(share_i64).ok_or(AppError::Internal)?;
            share_i64
        };

        shares.push(DividendShare {
            member_id: *member_id,
            weight: *weight,
            share_amount,
        });
    }

    Ok(shares)
}

/// Fixed daily penalty: overdue_days × fixed_rate_per_day
pub fn fixed_daily_penalty(overdue_days: i32, rate_per_day: i64) -> AppResult<i64> {
    if overdue_days <= 0 {
        return Err(AppError::BadRequest("overdue_days must be positive".into()));
    }
    validate_positive_amount(rate_per_day, "rate_per_day")?;

    let amount = (overdue_days as i64)
        .checked_mul(rate_per_day)
        .ok_or(AppError::Internal)?;
    Ok(amount)
}

/// Stacked penalty on outstanding principal: principal × penalty_bps × overdue_days / 10_000
pub fn stacked_penalty_on_principal(
    outstanding_principal: i64,
    penalty_bps: i32,
    overdue_days: i32,
) -> AppResult<i64> {
    validate_positive_amount(outstanding_principal, "outstanding_principal")?;
    if overdue_days <= 0 {
        return Err(AppError::BadRequest("overdue_days must be positive".into()));
    }

    let amount = (outstanding_principal as i128)
        .checked_mul(penalty_bps as i128)
        .and_then(|v| v.checked_mul(overdue_days as i128))
        .ok_or(AppError::Internal)?
        .checked_div(BPS_DENOMINATOR)
        .ok_or(AppError::Internal)?;

    i64::try_from(amount).map_err(|_| AppError::Internal)
}

fn validate_loan_inputs(principal: i64, annual_rate_bps: i32, term_months: i32) -> AppResult<()> {
    validate_positive_amount(principal, "principal")?;
    if annual_rate_bps < 0 {
        return Err(AppError::BadRequest(
            "annual_interest_rate_bps cannot be negative".into(),
        ));
    }
    if term_months <= 0 {
        return Err(AppError::BadRequest("term_months must be positive".into()));
    }
    Ok(())
}

fn validate_positive_amount(amount: i64, field: &str) -> AppResult<()> {
    if amount <= 0 {
        return Err(AppError::BadRequest(format!(
            "{field} must be greater than zero"
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};
    use std::thread;

    #[test]
    fn flat_rate_interest_is_integer_safe() {
        let interest = flat_rate_total_interest(100_000, 1200, 12).unwrap();
        assert_eq!(interest, 12_000);
    }

    #[test]
    fn dividend_shares_sum_to_pool() {
        let contributions = vec![
            ContributionWeight {
                member_id: uuid::Uuid::new_v4(),
                amount: 10_000,
                months_held: 6,
            },
            ContributionWeight {
                member_id: uuid::Uuid::new_v4(),
                amount: 20_000,
                months_held: 3,
            },
        ];

        let shares = time_weighted_dividend_shares(&contributions, 9_000).unwrap();
        let total: i64 = shares.iter().map(|s| s.share_amount).sum();
        assert_eq!(total, 9_000);
    }

    #[test]
    fn test_flat_rate_extreme_edge_cases() {
        // High principal, high rate, long term
        let res = flat_rate_total_interest(i64::MAX, i32::MAX, i32::MAX);
        assert!(res.is_err(), "Expected overflow error to be handled gracefully");

        // Zero rate
        let interest = flat_rate_total_interest(100_000, 0, 12).unwrap();
        assert_eq!(interest, 0);

        // Negative values check
        assert!(flat_rate_total_interest(-100, 1000, 12).is_err());
        assert!(flat_rate_total_interest(100, -1000, 12).is_err());
        assert!(flat_rate_total_interest(100, 1000, -12).is_err());
    }

    #[test]
    fn test_reducing_balance_extreme_edge_cases() {
        // Extremely high values
        let res = reducing_balance_monthly_payment(i64::MAX, i32::MAX, 120);
        assert!(res.is_err(), "Expected overflow to be handled gracefully");

        // Zero interest rate
        let payment = reducing_balance_monthly_payment(120_000, 0, 12).unwrap();
        assert_eq!(payment, 10_000);

        // Invalid inputs
        assert!(reducing_balance_monthly_payment(0, 1000, 12).is_err());
        assert!(reducing_balance_monthly_payment(100_000, -10, 12).is_err());
    }

    #[test]
    fn test_dividend_extreme_edge_cases() {
        // Empty contributions
        let shares = time_weighted_dividend_shares(&[], 10_000).unwrap();
        assert!(shares.is_empty());

        // Zero or negative distributable pool
        assert!(time_weighted_dividend_shares(&[ContributionWeight {
            member_id: uuid::Uuid::new_v4(),
            amount: 10_000,
            months_held: 1,
        }], 0).is_err());

        assert!(time_weighted_dividend_shares(&[ContributionWeight {
            member_id: uuid::Uuid::new_v4(),
            amount: 10_000,
            months_held: 1,
        }], -5000).is_err());

        // Extreme pool and amounts (testing i128 overflow protections)
        let extreme_contributions = vec![
            ContributionWeight {
                member_id: uuid::Uuid::new_v4(),
                amount: i64::MAX,
                months_held: i32::MAX,
            },
            ContributionWeight {
                member_id: uuid::Uuid::new_v4(),
                amount: i64::MAX,
                months_held: i32::MAX,
            }
        ];
        
        let shares = time_weighted_dividend_shares(&extreme_contributions, i64::MAX);
        assert!(shares.is_err(), "Expected overflow on massive pool * weight multiplication");
    }

    #[test]
    fn test_penalty_extreme_cases() {
        // Daily penalty max edge cases
        let res = fixed_daily_penalty(i32::MAX, i64::MAX);
        assert!(res.is_err(), "Expected overflow to be handled");

        assert!(fixed_daily_penalty(0, 100).is_err());
        assert!(fixed_daily_penalty(10, 0).is_err());

        // Stacked penalty
        let res2 = stacked_penalty_on_principal(i64::MAX, i32::MAX, i32::MAX);
        assert!(res2.is_err(), "Expected overflow to be handled");
    }

    #[test]
    fn test_concurrent_ledger_dividend_writes() {
        // Simulate concurrent processing of ledgers/dividends
        let num_threads = 50;
        let pool = 1_000_000;
        
        // Mock a shared ledger state that threads will try to write their calculated dividends to.
        let shared_ledger = Arc::new(Mutex::new(0i64));
        let mut handles = vec![];

        for i in 0..num_threads {
            let ledger_clone = Arc::clone(&shared_ledger);
            let handle = thread::spawn(move || {
                // Each thread calculates dividends for a slightly different set of contributions
                let contributions = vec![
                    ContributionWeight {
                        member_id: uuid::Uuid::new_v4(),
                        amount: 10_000 + i as i64,
                        months_held: 12,
                    },
                    ContributionWeight {
                        member_id: uuid::Uuid::new_v4(),
                        amount: 5_000 + i as i64,
                        months_held: 6,
                    },
                ];

                let shares = time_weighted_dividend_shares(&contributions, pool).unwrap();
                let sum: i64 = shares.iter().map(|s| s.share_amount).sum();
                assert_eq!(sum, pool);

                // Safely "write" to the concurrent ledger
                let mut ledger = ledger_clone.lock().unwrap();
                *ledger += sum;
            });
            handles.push(handle);
        }

        for handle in handles {
            handle.join().unwrap();
        }

        let final_ledger = *shared_ledger.lock().unwrap();
        // 50 threads * pool sum
        assert_eq!(final_ledger, pool * (num_threads as i64));
    }

    #[test]
    fn test_amortization_quotes_generation() {
        let quotes = amortization_quotes(100_000, 1200, 12).unwrap();
        assert_eq!(quotes.len(), 2);
        
        assert_eq!(quotes[0].method, InterestMethod::FlatRate);
        assert_eq!(quotes[0].total_interest, 12_000);
        
        assert_eq!(quotes[1].method, InterestMethod::ReducingBalance);
        
        let q_flat = quote_for_method(InterestMethod::FlatRate, 100_000, 1200, 12).unwrap();
        assert_eq!(q_flat, quotes[0]);
    }
}
