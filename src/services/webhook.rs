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
