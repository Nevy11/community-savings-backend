use axum::{
    extract::State,
    http::HeaderMap,
    routing::post,
    Extension, Json, Router,
};
use serde::{Deserialize, Serialize};

use crate::{
    error::{AppError, AppResult},
    middleware::Claims,
    models::transaction::{AppendLedgerRequest, LedgerEntry, TxType},
    services::{mpesa, validation},
    AppState,
};

use super::transactions::append_ledger_in_tx;

pub fn public_routes() -> Router<AppState> {
    Router::new().route("/callback", post(mpesa_callback))
}

pub fn protected_routes() -> Router<AppState> {
    Router::new()
        .route("/stkpush", post(stk_push))
        .route("/events", axum::routing::get(list_events))
}

#[derive(Deserialize)]
#[allow(dead_code)]
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

#[derive(Serialize, sqlx::FromRow)]
pub struct MpesaEvent {
    pub id: uuid::Uuid,
    pub transaction_id: Option<String>,
    pub phone_number: Option<String>,
    pub member_id: Option<uuid::Uuid>,
    pub group_id: Option<uuid::Uuid>,
    pub amount: i64,
    pub result_code: Option<i32>,
    pub result_desc: Option<String>,
    pub status: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

async fn list_events(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
) -> AppResult<Json<Vec<MpesaEvent>>> {
    let auth_user_id = uuid::Uuid::parse_str(&claims.sub)
        .map_err(|_| AppError::BadRequest("invalid auth user id".into()))?;

    let group_id: uuid::Uuid = sqlx::query_scalar(
        "SELECT group_id FROM members WHERE auth_user_id = $1 ORDER BY joined_at DESC LIMIT 1",
    )
    .bind(auth_user_id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or(AppError::NotFound)?;

    let events = sqlx::query_as::<_, MpesaEvent>(
        r#"
        SELECT id, transaction_id, phone_number, member_id, group_id, amount, result_code, result_desc, status, created_at
        FROM mpesa_callbacks
        WHERE group_id = $1
        ORDER BY created_at DESC
        "#,
    )
    .bind(group_id)
    .fetch_all(&state.pool)
    .await?;

    Ok(Json(events))
}
