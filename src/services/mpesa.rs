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
