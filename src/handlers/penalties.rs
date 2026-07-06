use axum::{
    extract::{Path, State},
    routing::get,
    Json, Router,
};
use uuid::Uuid;

use crate::{
    error::{AppError, AppResult},
    models::{
        group::Group,
        loan::Loan,
        penalty::{
            ApplyPenaltyRequest, CalculateLoanPenaltyRequest, Penalty, PenaltyCalculation,
            PenaltyType,
        },
        transaction::{AppendLedgerRequest, TxType},
    },
    services::finance,
    services::validation,
    AppState,
};

use super::transactions::append_ledger_in_tx;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/", get(list_penalties))
        .route("/calculate", axum::routing::post(calculate_loan_penalty))
        .route("/apply", axum::routing::post(apply_penalty))
        .route("/{id}", get(get_penalty))
}

async fn list_penalties(State(state): State<AppState>) -> AppResult<Json<Vec<Penalty>>> {
    let penalties = sqlx::query_as::<_, Penalty>(
        "SELECT * FROM penalties ORDER BY applied_at DESC",
    )
    .fetch_all(&state.pool)
    .await?;

    Ok(Json(penalties))
}

async fn get_penalty(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> AppResult<Json<Penalty>> {
    let penalty = sqlx::query_as::<_, Penalty>("SELECT * FROM penalties WHERE id = $1")
        .bind(id)
        .fetch_optional(&state.pool)
        .await?
        .ok_or(AppError::NotFound)?;

    Ok(Json(penalty))
}

async fn calculate_loan_penalty(
    State(state): State<AppState>,
    Json(payload): Json<CalculateLoanPenaltyRequest>,
) -> AppResult<Json<PenaltyCalculation>> {
    let calculation = compute_loan_penalty(&state.pool, payload.loan_id, payload.overdue_days).await?;
    Ok(Json(calculation))
}

async fn compute_loan_penalty(
    pool: &sqlx::PgPool,
    loan_id: Uuid,
    overdue_days: i32,
) -> AppResult<PenaltyCalculation> {
    if overdue_days <= 0 {
        return Err(AppError::BadRequest("overdue_days must be positive".into()));
    }

    let loan = sqlx::query_as::<_, Loan>("SELECT * FROM loans WHERE id = $1")
        .bind(loan_id)
        .fetch_optional(pool)
        .await?
        .ok_or(AppError::NotFound)?;

    let group = sqlx::query_as::<_, Group>("SELECT * FROM groups WHERE id = $1")
        .bind(loan.group_id)
        .fetch_one(pool)
        .await?;

    let stacked = finance::stacked_penalty_on_principal(
        loan.principal,
        group.loan_late_penalty_bps,
        overdue_days,
    )?;

    let fixed_daily_rate = group.late_fine_amount.max(1).checked_div(7).unwrap_or(1);
    let fixed = finance::fixed_daily_penalty(overdue_days, fixed_daily_rate)?;

    Ok(PenaltyCalculation {
        loan_id: loan.id,
        overdue_days,
        calculated_amount: stacked.max(fixed),
    })
}

async fn apply_penalty(
    State(state): State<AppState>,
    Json(payload): Json<ApplyPenaltyRequest>,
) -> AppResult<Json<Penalty>> {
    let mut tx = state.pool.begin().await?;

    let penalty = sqlx::query_as::<_, Penalty>(
        "SELECT * FROM penalties WHERE id = $1 FOR UPDATE",
    )
    .bind(payload.penalty_id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(AppError::NotFound)?;

    if penalty.paid {
        return Err(AppError::Conflict("penalty has already been paid".into()));
    }

    validation::validate_positive_amount(penalty.amount, "penalty_amount")?;

    let ledger_payload = AppendLedgerRequest {
        group_id: penalty.group_id,
        member_id: penalty.member_id,
        amount: penalty.amount,
        tx_type: TxType::FinePayment,
        reference: Some(format!("penalty_payment:{}", penalty.id)),
    };
    append_ledger_in_tx(&mut tx, &ledger_payload).await?;

    let penalty = sqlx::query_as::<_, Penalty>(
        r#"
        UPDATE penalties
        SET paid = TRUE
        WHERE id = $1
        RETURNING *
        "#,
    )
    .bind(penalty.id)
    .fetch_one(&mut *tx)
    .await?;

    tx.commit().await?;
    Ok(Json(penalty))
}

#[allow(dead_code)]
pub async fn create_loan_late_penalty(
    state: &AppState,
    loan_id: Uuid,
    overdue_days: i32,
) -> AppResult<Penalty> {
    let calculation = compute_loan_penalty(&state.pool, loan_id, overdue_days).await?;

    let loan = sqlx::query_as::<_, Loan>("SELECT * FROM loans WHERE id = $1")
        .bind(loan_id)
        .fetch_one(&state.pool)
        .await?;

    let penalty = sqlx::query_as::<_, Penalty>(
        r#"
        INSERT INTO penalties (group_id, member_id, loan_id, penalty_type, amount)
        VALUES ($1, $2, $3, $4, $5)
        RETURNING *
        "#,
    )
    .bind(loan.group_id)
    .bind(loan.member_id)
    .bind(loan.id)
    .bind(PenaltyType::LoanLate)
    .bind(calculation.calculated_amount)
    .fetch_one(&state.pool)
    .await?;

    Ok(penalty)
}
