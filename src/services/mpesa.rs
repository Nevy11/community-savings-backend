use hmac::{Hmac, Mac};
use sha2::Sha256;
use subtle::ConstantTimeEq;

use crate::error::{AppError, AppResult};

type HmacSha256 = Hmac<Sha256>;

pub fn verify_mpesa_signature(payload: &str, provided_signature: &str, secret: &str) -> AppResult<()> {
    let mut mac = HmacSha256::new_from_slice(secret.as_bytes())
        .map_err(|_| AppError::Internal)?;
    mac.update(payload.as_bytes());

    let expected = mac.finalize().into_bytes();
    let provided = hex::decode(provided_signature)
        .map_err(|_| AppError::Unauthorized("invalid signature encoding".into()))?;

    if expected.as_slice().ct_eq(&provided).into() {
        Ok(())
    } else {
        Err(AppError::Unauthorized("invalid mpesa signature".into()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_verify_mpesa_signature_success() {
        let secret = "my_secret_key";
        let payload = "my_payload_data";
        
        let mut mac = HmacSha256::new_from_slice(secret.as_bytes()).unwrap();
        mac.update(payload.as_bytes());
        let valid_signature = hex::encode(mac.finalize().into_bytes());

        let result = verify_mpesa_signature(payload, &valid_signature, secret);
        assert!(result.is_ok(), "Signature verification should succeed for valid signature");
    }

    #[test]
    fn test_verify_mpesa_signature_invalid_signature() {
        let secret = "my_secret_key";
        let payload = "my_payload_data";
        let invalid_signature = "1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef";

        let result = verify_mpesa_signature(payload, invalid_signature, secret);
        assert!(result.is_err(), "Signature verification should fail for incorrect signature");
    }

    #[test]
    fn test_verify_mpesa_signature_invalid_hex() {
        let secret = "my_secret_key";
        let payload = "my_payload_data";
        let invalid_hex = "not_a_hex_string";

        let result = verify_mpesa_signature(payload, invalid_hex, secret);
        assert!(result.is_err(), "Signature verification should fail for invalid hex encoding");
    }

    #[test]
    fn test_verify_mpesa_signature_different_payload() {
        let secret = "my_secret_key";
        let payload = "my_payload_data";
        let different_payload = "different_payload_data";
        
        let mut mac = HmacSha256::new_from_slice(secret.as_bytes()).unwrap();
        mac.update(payload.as_bytes());
        let valid_signature = hex::encode(mac.finalize().into_bytes());

        let result = verify_mpesa_signature(different_payload, &valid_signature, secret);
        assert!(result.is_err(), "Signature verification should fail if payload was altered");
    }
}
