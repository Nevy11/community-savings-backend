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

pub fn normalize_username_candidate(candidate: &str, fallback_suffix: &str) -> String {
    let mut username = candidate
        .trim()
        .to_lowercase()
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' || c == '.' {
                c
            } else {
                '_'
            }
        })
        .collect::<String>();

    username = username
        .trim_matches(|c| c == '_' || c == '.')
        .chars()
        .take(32)
        .collect();

    if username.len() < 3 {
        let suffix = fallback_suffix.chars().take(8).collect::<String>();
        username = format!("user_{suffix}");
    }

    username
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::transaction::TxType;

    #[test]
    fn test_validate_positive_amount() {
        assert!(validate_positive_amount(1, "field").is_ok());
        assert!(validate_positive_amount(100, "field").is_ok());
        assert!(validate_positive_amount(0, "field").is_err());
        assert!(validate_positive_amount(-1, "field").is_err());
    }

    #[test]
    fn test_validate_non_negative_amount() {
        assert!(validate_non_negative_amount(1, "field").is_ok());
        assert!(validate_non_negative_amount(0, "field").is_ok());
        assert!(validate_non_negative_amount(-1, "field").is_err());
    }

    #[test]
    fn test_validate_phone_number() {
        assert!(validate_phone_number("123456789").is_ok());
        assert!(validate_phone_number("123456789012345").is_ok());
        assert!(validate_phone_number("+254712345678").is_ok()); // 12 digits
        assert!(validate_phone_number("12345678").is_err()); // too short
        assert!(validate_phone_number("1234567890123456").is_err()); // too long
    }

    #[test]
    fn test_validate_username() {
        assert!(validate_username("valid_user").is_ok());
        assert!(validate_username("valid.user").is_ok());
        assert!(validate_username("user123").is_ok());
        
        assert!(validate_username("ab").is_err()); // too short
        assert!(validate_username("a".repeat(33).as_str()).is_err()); // too long
        assert!(validate_username("invalid-user!").is_err()); // invalid chars
        assert!(validate_username("user@name").is_err()); // invalid chars
    }

    #[test]
    fn test_normalize_username_candidate() {
        assert_eq!(normalize_username_candidate("Valid User", "12345678"), "valid_user");
        assert_eq!(normalize_username_candidate("user@name!", "12345678"), "user_name");
        assert_eq!(normalize_username_candidate("a", "12345678"), "user_12345678");
        assert_eq!(normalize_username_candidate("  trim  ", "123"), "trim");
        
        let long_name = "a".repeat(40);
        let normalized = normalize_username_candidate(&long_name, "123");
        assert_eq!(normalized.len(), 32);
    }

    #[test]
    fn test_validate_meeting_day() {
        for i in 0..=6 {
            assert!(validate_meeting_day(i).is_ok());
        }
        assert!(validate_meeting_day(-1).is_err());
        assert!(validate_meeting_day(7).is_err());
    }

    #[test]
    fn test_validate_append_only_tx() {
        // Negative amount required
        assert!(validate_append_only_tx(-100, TxType::Withdrawal).is_ok());
        assert!(validate_append_only_tx(100, TxType::Withdrawal).is_err());
        assert!(validate_append_only_tx(0, TxType::Withdrawal).is_err());

        assert!(validate_append_only_tx(-100, TxType::LoanDisbursement).is_ok());
        assert!(validate_append_only_tx(100, TxType::LoanDisbursement).is_err());

        assert!(validate_append_only_tx(-100, TxType::DividendPayout).is_ok());
        assert!(validate_append_only_tx(100, TxType::DividendPayout).is_err());

        // Positive amount required
        assert!(validate_append_only_tx(100, TxType::Deposit).is_ok());
        assert!(validate_append_only_tx(-100, TxType::Deposit).is_err());
        assert!(validate_append_only_tx(0, TxType::Deposit).is_err());

        assert!(validate_append_only_tx(100, TxType::SocialFundPayment).is_ok());
        assert!(validate_append_only_tx(-100, TxType::SocialFundPayment).is_err());

        assert!(validate_append_only_tx(100, TxType::LoanRepayment).is_ok());
        assert!(validate_append_only_tx(-100, TxType::LoanRepayment).is_err());

        assert!(validate_append_only_tx(100, TxType::FinePayment).is_ok());
        assert!(validate_append_only_tx(-100, TxType::FinePayment).is_err());
    }
}
