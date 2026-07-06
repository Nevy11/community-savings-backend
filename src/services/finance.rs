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
}
