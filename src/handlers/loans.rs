use axum::{
    extract::{Path, State},
    routing::get,
    Json, Router,
};
use chrono::Utc;
use serde::Serialize;
use uuid::Uuid;

use crate::{
    error::{AppError, AppResult},
    models::{
        group::Group,
        loan::{
            AddGuarantorRequest, CreateLoanRequest, Loan, LoanGuarantor, LoanStatus,
            RepayLoanRequest,
        },
        transaction::{AppendLedgerRequest, TxType},
    },
    services::finance::{self, AmortizationQuote},
    services::validation,
    AppState,
};

use super::transactions::append_ledger_in_tx;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/", get(list_loans).post(request_loan))
        .route("/{id}", get(get_loan))
        .route("/{id}/guarantors", get(list_guarantors).post(add_guarantor))
        .route("/{id}/approve", axum::routing::post(approve_loan))
        .route("/{id}/disburse", axum::routing::post(disburse_loan))
        .route("/{id}/repay", axum::routing::post(repay_loan))
        .route("/{id}/schedule", get(loan_schedule))
}

async fn list_loans(State(state): State<AppState>) -> AppResult<Json<Vec<Loan>>> {
    let loans = sqlx::query_as::<_, Loan>("SELECT * FROM loans ORDER BY created_at DESC")
        .fetch_all(&state.pool)
        .await?;

    Ok(Json(loans))
}

async fn get_loan(State(state): State<AppState>, Path(id): Path<Uuid>) -> AppResult<Json<Loan>> {
    let loan = sqlx::query_as::<_, Loan>("SELECT * FROM loans WHERE id = $1")
        .bind(id)
        .fetch_optional(&state.pool)
        .await?
        .ok_or(AppError::NotFound)?;

    Ok(Json(loan))
}

async fn request_loan(
    State(state): State<AppState>,
    Json(payload): Json<CreateLoanRequest>,
) -> AppResult<Json<Loan>> {
    validation::validate_positive_amount(payload.principal, "principal")?;
    if payload.term_months <= 0 {
        return Err(AppError::BadRequest("term_months must be positive".into()));
    }

    let mut tx = state.pool.begin().await?;

    let loan = sqlx::query_as::<_, Loan>(
        r#"
        INSERT INTO loans (group_id, member_id, principal, outstanding_balance, term_months)
        VALUES ($1, $2, $3, $3, $4)
        RETURNING *
        "#,
    )
    .bind(payload.group_id)
    .bind(payload.member_id)
    .bind(payload.principal)
    .bind(payload.term_months)
    .fetch_one(&mut *tx)
    .await?;

    if let Some(guarantors) = payload.guarantors {
        for guarantor in guarantors {
            sqlx::query(
                r#"
                INSERT INTO loan_guarantors (loan_id, member_id, guaranteed_amount)
                VALUES ($1, $2, $3)
                "#,
            )
            .bind(loan.id)
            .bind(guarantor.member_id)
            .bind(guarantor.guaranteed_amount)
            .execute(&mut *tx)
            .await?;
        }
    }

    tx.commit().await?;

    Ok(Json(loan))
}

async fn list_guarantors(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> AppResult<Json<Vec<LoanGuarantor>>> {
    let guarantors = sqlx::query_as::<_, LoanGuarantor>(
        "SELECT * FROM loan_guarantors WHERE loan_id = $1",
    )
    .bind(id)
    .fetch_all(&state.pool)
    .await?;

    Ok(Json(guarantors))
}

async fn add_guarantor(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(payload): Json<AddGuarantorRequest>,
) -> AppResult<Json<LoanGuarantor>> {
    let mut tx = state.pool.begin().await?;

    let loan = sqlx::query_as::<_, Loan>("SELECT * FROM loans WHERE id = $1 FOR UPDATE")
        .bind(id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or(AppError::NotFound)?;

    if loan.status != LoanStatus::Pending {
        return Err(AppError::Conflict(
            "guarantors can only be added to pending loans".into(),
        ));
    }

    let guarantor = sqlx::query_as::<_, LoanGuarantor>(
        r#"
        INSERT INTO loan_guarantors (loan_id, member_id, guaranteed_amount)
        VALUES ($1, $2, $3)
        RETURNING *
        "#,
    )
    .bind(id)
    .bind(payload.member_id)
    .bind(payload.guaranteed_amount)
    .fetch_one(&mut *tx)
    .await?;

    tx.commit().await?;
    Ok(Json(guarantor))
}

async fn approve_loan(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> AppResult<Json<Loan>> {
    let mut tx = state.pool.begin().await?;

    let loan = sqlx::query_as::<_, Loan>("SELECT * FROM loans WHERE id = $1 FOR UPDATE")
        .bind(id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or(AppError::NotFound)?;

    if loan.status != LoanStatus::Pending {
        return Err(AppError::Conflict("loan is not pending approval".into()));
    }

    let guarantor_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM loan_guarantors WHERE loan_id = $1")
            .bind(id)
            .fetch_one(&mut *tx)
            .await?;

    if guarantor_count == 0 {
        return Err(AppError::BadRequest(
            "at least one guarantor is required before approval".into(),
        ));
    }

    let group = sqlx::query_as::<_, Group>(
        "SELECT * FROM groups WHERE id = $1 FOR UPDATE",
    )
    .bind(loan.group_id)
    .fetch_one(&mut *tx)
    .await?;

    if group.pool_balance < loan.principal {
        return Err(AppError::InsufficientFunds {
            available: group.pool_balance,
            required: loan.principal,
        });
    }

    let loan = sqlx::query_as::<_, Loan>(
        r#"
        UPDATE loans
        SET status = 'approved', approved_at = $2
        WHERE id = $1
        RETURNING *
        "#,
    )
    .bind(id)
    .bind(Utc::now())
    .fetch_one(&mut *tx)
    .await?;

    tx.commit().await?;
    Ok(Json(loan))
}

async fn disburse_loan(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> AppResult<Json<Loan>> {
    let mut tx = state.pool.begin().await?;

    let loan = sqlx::query_as::<_, Loan>("SELECT * FROM loans WHERE id = $1 FOR UPDATE")
        .bind(id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or(AppError::NotFound)?;

    if loan.status != LoanStatus::Approved {
        return Err(AppError::Conflict("loan must be approved before disbursement".into()));
    }

    let group = sqlx::query_as::<_, Group>(
        "SELECT * FROM groups WHERE id = $1 FOR UPDATE",
    )
    .bind(loan.group_id)
    .fetch_one(&mut *tx)
    .await?;

    if group.pool_balance < loan.principal {
        return Err(AppError::InsufficientFunds {
            available: group.pool_balance,
            required: loan.principal,
        });
    }

    let ledger_payload = AppendLedgerRequest {
        group_id: loan.group_id,
        member_id: loan.member_id,
        amount: -loan.principal,
        tx_type: TxType::LoanDisbursement,
        reference: Some(format!("loan_disbursement:{id}")),
    };
    append_ledger_in_tx(&mut tx, &ledger_payload).await?;

    let loan = sqlx::query_as::<_, Loan>(
        r#"
        UPDATE loans
        SET status = 'disbursed', disbursed_at = $2
        WHERE id = $1
        RETURNING *
        "#,
    )
    .bind(id)
    .bind(Utc::now())
    .fetch_one(&mut *tx)
    .await?;

    tx.commit().await?;
    Ok(Json(loan))
}

#[derive(Debug, Serialize)]
struct LoanScheduleResponse {
    loan_id: Uuid,
    principal: i64,
    term_months: i32,
    group_interest_method: crate::models::group::InterestMethod,
    quotes: Vec<AmortizationQuote>,
    selected_quote: AmortizationQuote,
}

async fn repay_loan(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(payload): Json<RepayLoanRequest>,
) -> AppResult<Json<Loan>> {
    validation::validate_positive_amount(payload.amount, "amount")?;

    let mut tx = state.pool.begin().await?;

    let loan = sqlx::query_as::<_, Loan>("SELECT * FROM loans WHERE id = $1 FOR UPDATE")
        .bind(id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or(AppError::NotFound)?;

    if loan.status != LoanStatus::Disbursed {
        return Err(AppError::Conflict(
            "only disbursed loans can be repaid".into(),
        ));
    }

    if payload.amount > loan.outstanding_balance {
        return Err(AppError::BadRequest(format!(
            "repayment amount exceeds outstanding balance of {}",
            loan.outstanding_balance
        )));
    }

    let ledger_payload = AppendLedgerRequest {
        group_id: loan.group_id,
        member_id: loan.member_id,
        amount: payload.amount,
        tx_type: TxType::LoanRepayment,
        reference: Some(format!("loan_repayment:{id}")),
    };
    append_ledger_in_tx(&mut tx, &ledger_payload).await?;

    let loan = sqlx::query_as::<_, Loan>("SELECT * FROM loans WHERE id = $1")
        .bind(id)
        .fetch_one(&mut *tx)
        .await?;

    tx.commit().await?;
    Ok(Json(loan))
}

async fn loan_schedule(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> AppResult<Json<LoanScheduleResponse>> {
    let loan = sqlx::query_as::<_, Loan>("SELECT * FROM loans WHERE id = $1")
        .bind(id)
        .fetch_optional(&state.pool)
        .await?
        .ok_or(AppError::NotFound)?;

    let group = sqlx::query_as::<_, Group>("SELECT * FROM groups WHERE id = $1")
        .bind(loan.group_id)
        .fetch_one(&state.pool)
        .await?;

    let quotes = finance::amortization_quotes(
        loan.principal,
        group.annual_interest_rate_bps,
        loan.term_months,
    )?;

    let selected_quote = finance::quote_for_method(
        group.interest_method,
        loan.principal,
        group.annual_interest_rate_bps,
        loan.term_months,
    )?;

    Ok(Json(LoanScheduleResponse {
        loan_id: loan.id,
        principal: loan.principal,
        term_months: loan.term_months,
        group_interest_method: group.interest_method,
        quotes,
        selected_quote,
    }))
}
