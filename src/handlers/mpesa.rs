use axum::{
    extract::State,
    http::HeaderMap,
    routing::post,
    Json, Router,
};
use serde::{Deserialize, Serialize};

use crate::{
    error::{AppError, AppResult},
    models::transaction::{AppendLedgerRequest, LedgerEntry, TxType},
    services::{mpesa, validation},
    AppState,
};

use super::transactions::append_ledger_in_tx;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/callback", post(mpesa_callback))
        .route("/stkpush", post(stk_push))
}

#[derive(Deserialize)]
pub struct StkPushRequest {
    pub phone_number: String,
    pub amount: i64,
    pub group_id: uuid::Uuid,
    pub member_id: uuid::Uuid,
}

#[derive(Serialize)]
pub struct StkPushResponse {
    pub checkout_request_id: String,
    pub customer_message: String,
}

async fn stk_push(
    State(_state): State<AppState>,
    Json(_payload): Json<StkPushRequest>,
) -> AppResult<Json<StkPushResponse>> {
    // Stub implementation for now
    Ok(Json(StkPushResponse {
        checkout_request_id: "ws_CO_03072026_stk_1234567890".into(),
        customer_message: "Success. Request accepted for processing".into(),
    }))
}

#[derive(Debug, Deserialize, Serialize)]
pub struct MpesaCallbackRequest {
    pub transaction_id: String,
    pub phone_number: String,
    pub member_id: uuid::Uuid,
    pub group_id: uuid::Uuid,
    pub amount: i64,
    pub result_code: i32,
    pub result_desc: String,
}

#[derive(Debug, Serialize)]
pub struct MpesaCallbackResponse {
    pub received: bool,
    pub transaction_id: String,
    pub ledger_entry_id: Option<uuid::Uuid>,
}

async fn mpesa_callback(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<MpesaCallbackRequest>,
) -> AppResult<Json<MpesaCallbackResponse>> {
    let signature = headers
        .get("x-mpesa-signature")
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| AppError::Unauthorized("missing x-mpesa-signature header".into()))?;

    let canonical_payload = serde_json::to_string(&payload).map_err(|_| AppError::Internal)?;
    mpesa::verify_mpesa_signature(
        &canonical_payload,
        signature,
        &state.config.mpesa_callback_secret,
    )?;

    validation::validate_phone_number(&payload.phone_number)?;
    validation::validate_positive_amount(payload.amount, "amount")?;

    if payload.result_code != 0 {
        return Err(AppError::BadRequest(format!(
            "mpesa payment failed: {}",
            payload.result_desc
        )));
    }

    let mut tx = state.pool.begin().await?;

    let ledger_payload = AppendLedgerRequest {
        group_id: payload.group_id,
        member_id: payload.member_id,
        amount: payload.amount,
        tx_type: TxType::Deposit,
        reference: Some(format!("mpesa:{}", payload.transaction_id)),
    };

    let entry: LedgerEntry = append_ledger_in_tx(&mut tx, &ledger_payload).await?;

    tx.commit().await?;

    Ok(Json(MpesaCallbackResponse {
        received: true,
        transaction_id: payload.transaction_id,
        ledger_entry_id: Some(entry.id),
    }))
}
