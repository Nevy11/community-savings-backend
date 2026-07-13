use axum::{
    extract::{Path, Query, State},
    routing::get,
    Json, Router,
};
use serde::Deserialize;
use sqlx::{Postgres, Transaction};
use uuid::Uuid;

use crate::{
    error::{AppError, AppResult},
    models::transaction::{AppendLedgerRequest, LedgerEntry, TxType},
    services::validation,
    AppState,
};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/", get(list_ledger).post(append_ledger))
        .route("/{id}", get(get_ledger_entry))
}

#[derive(Debug, Deserialize)]
struct LedgerQuery {
    group_id: Option<Uuid>,
    member_id: Option<Uuid>,
}

async fn list_ledger(
    State(state): State<AppState>,
    Query(query): Query<LedgerQuery>,
) -> AppResult<Json<Vec<LedgerEntry>>> {
    let entries = match (query.group_id, query.member_id) {
        (Some(group_id), Some(member_id)) => {
            sqlx::query_as::<_, LedgerEntry>(
                r#"
                SELECT * FROM ledger_transactions
                WHERE group_id = $1 AND member_id = $2
                ORDER BY created_at DESC
                "#,
            )
            .bind(group_id)
            .bind(member_id)
            .fetch_all(&state.pool)
            .await?
        }
        (Some(group_id), None) => {
            sqlx::query_as::<_, LedgerEntry>(
                r#"
                SELECT * FROM ledger_transactions
                WHERE group_id = $1
                ORDER BY created_at DESC
                "#,
            )
            .bind(group_id)
            .fetch_all(&state.pool)
            .await?
        }
        (None, Some(member_id)) => {
            sqlx::query_as::<_, LedgerEntry>(
                r#"
                SELECT * FROM ledger_transactions
                WHERE member_id = $1
                ORDER BY created_at DESC
                "#,
            )
            .bind(member_id)
            .fetch_all(&state.pool)
            .await?
        }
        (None, None) => {
            sqlx::query_as::<_, LedgerEntry>(
                "SELECT * FROM ledger_transactions ORDER BY created_at DESC",
            )
            .fetch_all(&state.pool)
            .await?
        }
    };

    Ok(Json(entries))
}

async fn get_ledger_entry(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> AppResult<Json<LedgerEntry>> {
    let entry = sqlx::query_as::<_, LedgerEntry>(
        "SELECT * FROM ledger_transactions WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or(AppError::NotFound)?;

    Ok(Json(entry))
}

async fn append_ledger(
    State(state): State<AppState>,
    Json(payload): Json<AppendLedgerRequest>,
) -> AppResult<Json<LedgerEntry>> {
    validation::validate_append_only_tx(payload.amount, payload.tx_type)?;

    let mut tx = state.pool.begin().await?;

    let entry = append_ledger_in_tx(&mut tx, &payload).await?;

    tx.commit().await?;
    Ok(Json(entry))
}

pub async fn append_ledger_in_tx(
    tx: &mut Transaction<'_, Postgres>,
    payload: &AppendLedgerRequest,
) -> AppResult<LedgerEntry> {
    validation::validate_append_only_tx(payload.amount, payload.tx_type)?;

    let pool_balance: i64 = sqlx::query_scalar("SELECT pool_balance FROM groups WHERE id = $1 FOR UPDATE")
        .bind(payload.group_id)
        .fetch_one(&mut **tx)
        .await?;

    let member_group_id: Uuid = sqlx::query_scalar("SELECT group_id FROM members WHERE id = $1")
        .bind(payload.member_id)
        .fetch_optional(&mut **tx)
        .await?
        .ok_or(AppError::NotFound)?;

    if member_group_id != payload.group_id {
        return Err(AppError::BadRequest(
            "member does not belong to the specified group".into(),
        ));
    }

    let next_balance = pool_balance
        .checked_add(payload.amount)
        .ok_or(AppError::Internal)?;
    if next_balance < 0 {
        return Err(AppError::InsufficientFunds {
            available: pool_balance,
            required: payload.amount.abs(),
        });
    }

    let entry = sqlx::query_as::<_, LedgerEntry>(
        r#"
        INSERT INTO ledger_transactions (group_id, member_id, amount, tx_type, reference)
        VALUES ($1, $2, $3, $4, $5)
        RETURNING *
        "#,
    )
    .bind(payload.group_id)
    .bind(payload.member_id)
    .bind(payload.amount)
    .bind(payload.tx_type)
    .bind(&payload.reference)
    .fetch_one(&mut **tx)
    .await?;

    sqlx::query(
        r#"
        UPDATE groups
        SET pool_balance = $2
        WHERE id = $1
        "#,
    )
    .bind(payload.group_id)
    .bind(next_balance)
    .execute(&mut **tx)
    .await?;

    if payload.tx_type == TxType::LoanRepayment {
        if let Some(ref_str) = &payload.reference {
            if let Some(uuid_str) = ref_str.strip_prefix("loan_repayment:") {
                if let Ok(loan_id) = Uuid::parse_str(uuid_str) {
                    let loan: crate::models::loan::Loan = sqlx::query_as::<_, crate::models::loan::Loan>(
                        "SELECT * FROM loans WHERE id = $1 FOR UPDATE"
                    )
                    .bind(loan_id)
                    .fetch_optional(&mut **tx)
                    .await?
                    .ok_or(AppError::NotFound)?;
                    
                    let new_balance = loan.outstanding_balance - payload.amount.abs();
                    let new_status = if new_balance <= 0 { crate::models::loan::LoanStatus::Repaid } else { loan.status };
                    
                    sqlx::query(
                        r#"
                        UPDATE loans
                        SET outstanding_balance = $2, status = $3
                        WHERE id = $1
                        "#
                    )
                    .bind(loan_id)
                    .bind(std::cmp::max(0, new_balance))
                    .bind(new_status as crate::models::loan::LoanStatus)
                    .execute(&mut **tx)
                    .await?;
                }
            }
        }
    }

    Ok(entry)
}
