use crate::error::{AppError, AppResult};
use crate::models::transaction::TxType;

pub fn validate_positive_amount(amount: i64, field: &str) -> AppResult<()> {
    if amount <= 0 {
        return Err(AppError::BadRequest(format!(
            "{field} must be greater than zero"
        )));
    }
    Ok(())
}

#[allow(dead_code)]
pub fn validate_non_negative_amount(amount: i64, field: &str) -> AppResult<()> {
    if amount < 0 {
        return Err(AppError::BadRequest(format!(
            "{field} cannot be negative"
        )));
    }
    Ok(())
}

pub fn validate_phone_number(phone: &str) -> AppResult<()> {
    let digits: String = phone.chars().filter(|c| c.is_ascii_digit()).collect();

    if digits.len() < 9 || digits.len() > 15 {
        return Err(AppError::BadRequest("invalid phone number".into()));
    }

    Ok(())
}

pub fn validate_username(username: &str) -> AppResult<()> {
    let len = username.len();
    if len < 3 || len > 32 {
        return Err(AppError::BadRequest(
            "username must be between 3 and 32 characters".into(),
        ));
    }

    if !username
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '.')
    {
        return Err(AppError::BadRequest(
            "username may only contain letters, numbers, underscores, and dots".into(),
        ));
    }

    Ok(())
}

#[allow(dead_code)]
pub fn validate_meeting_day(day: i32) -> AppResult<()> {
    if !(0..=6).contains(&day) {
        return Err(AppError::BadRequest(
            "meeting_day must be between 0 (Sunday) and 6 (Saturday)".into(),
        ));
    }
    Ok(())
}

pub fn validate_append_only_tx(amount: i64, tx_type: TxType) -> AppResult<()> {
    match tx_type {
        TxType::Withdrawal | TxType::LoanDisbursement | TxType::DividendPayout => {
            if amount >= 0 {
                return Err(AppError::BadRequest(format!(
                    "{tx_type:?} requires a negative amount"
                )));
            }
        }
        TxType::Deposit
        | TxType::SocialFundPayment
        | TxType::LoanRepayment
        | TxType::FinePayment => {
            if amount <= 0 {
                return Err(AppError::BadRequest(format!(
                    "{tx_type:?} requires a positive amount"
                )));
            }
        }
    }
    Ok(())
}
