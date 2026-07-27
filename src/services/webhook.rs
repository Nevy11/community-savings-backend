use hmac::{Hmac, Mac};
use sha2::Sha256;
use subtle::ConstantTimeEq;
use base64::{engine::general_purpose, Engine as _};

use crate::error::{AppError, AppResult};

type HmacSha256 = Hmac<Sha256>;

/// Verify a webhook payload against a provided signature. The provided signature
/// may be hex-encoded or base64; we try hex first, then base64.
pub fn verify_webhook_signature(payload: &str, provided_signature: &str, secret: &str) -> AppResult<()> {
    let mut mac = HmacSha256::new_from_slice(secret.as_bytes()).map_err(|_| AppError::Internal)?;
    mac.update(payload.as_bytes());

    let expected = mac.finalize().into_bytes();

    // Try hex decode
    if let Ok(provided) = hex::decode(provided_signature) {
        if expected.as_slice().ct_eq(&provided).into() {
            return Ok(());
        }
    }

    // Try base64 (use new Engine API)
    if let Ok(provided) = general_purpose::STANDARD.decode(provided_signature) {
        if expected.as_slice().ct_eq(&provided).into() {
            return Ok(());
        }
    }

    Err(AppError::Unauthorized("invalid webhook signature".into()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_verify_webhook_signature_hex_success() {
        let secret = "my_secret_webhook_key";
        let payload = "my_webhook_payload_data";
        
        let mut mac = HmacSha256::new_from_slice(secret.as_bytes()).unwrap();
        mac.update(payload.as_bytes());
        let valid_signature = hex::encode(mac.finalize().into_bytes());

        let result = verify_webhook_signature(payload, &valid_signature, secret);
        assert!(result.is_ok(), "Webhook hex signature verification should succeed");
    }

    #[test]
    fn test_verify_webhook_signature_base64_success() {
        let secret = "my_secret_webhook_key";
        let payload = "my_webhook_payload_data";
        
        let mut mac = HmacSha256::new_from_slice(secret.as_bytes()).unwrap();
        mac.update(payload.as_bytes());
        let valid_signature = general_purpose::STANDARD.encode(mac.finalize().into_bytes());

        let result = verify_webhook_signature(payload, &valid_signature, secret);
        assert!(result.is_ok(), "Webhook base64 signature verification should succeed");
    }

    #[test]
    fn test_verify_webhook_signature_invalid() {
        let secret = "my_secret_webhook_key";
        let payload = "my_webhook_payload_data";
        let invalid_signature = "1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef";

        let result = verify_webhook_signature(payload, invalid_signature, secret);
        assert!(result.is_err(), "Webhook signature verification should fail for incorrect signature");
    }

    #[test]
    fn test_verify_webhook_signature_invalid_format() {
        let secret = "my_secret_webhook_key";
        let payload = "my_webhook_payload_data";
        let invalid_format = "not_hex_or_base64!";

        let result = verify_webhook_signature(payload, invalid_format, secret);
        assert!(result.is_err(), "Webhook signature verification should fail for invalid encoding");
    }

    #[test]
    fn test_verify_webhook_signature_different_payload() {
        let secret = "my_secret_webhook_key";
        let payload = "my_webhook_payload_data";
        let different_payload = "different_payload_data";
        
        let mut mac = HmacSha256::new_from_slice(secret.as_bytes()).unwrap();
        mac.update(payload.as_bytes());
        let valid_signature = hex::encode(mac.finalize().into_bytes());

        let result = verify_webhook_signature(different_payload, &valid_signature, secret);
        assert!(result.is_err(), "Webhook signature verification should fail for altered payload");
    }
}
