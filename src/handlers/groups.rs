use axum::{
    extract::{Path, State},
    routing::{get, patch, post},
    Json, Router,
};
use chrono::{Datelike, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    error::AppResult,
    models::group::{CreateGroupRequest, Group, InterestMethod},
    AppState,
};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/", post(create_group))
        .route("/{id}", get(get_group))
        .route("/{id}/settings", patch(update_settings))
        .route("/{id}/dashboard-metrics", get(dashboard_metrics))
}

async fn create_group(
    State(state): State<AppState>,
    Json(payload): Json<CreateGroupRequest>,
) -> AppResult<Json<Group>> {
    let group = sqlx::query_as::<_, Group>(
        r#"
        INSERT INTO groups (name, annual_interest_rate_bps, absent_fine_amount, late_fine_amount, interest_method)
        VALUES ($1, $2, $3, $4, $5)
        RETURNING *
        "#,
    )
    .bind(&payload.name)
    .bind(payload.annual_interest_rate_bps)
    .bind(payload.absent_fine_amount)
    .bind(payload.late_fine_amount)
    .bind(payload.interest_method.unwrap_or(InterestMethod::FlatRate))
    .fetch_one(&state.pool)
    .await?;

    Ok(Json(group))
}

async fn get_group(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> AppResult<Json<Group>> {
    let group = sqlx::query_as::<_, Group>("SELECT * FROM groups WHERE id = $1")
        .bind(id)
        .fetch_one(&state.pool)
        .await?;
    Ok(Json(group))
}

#[derive(Deserialize)]
pub struct UpdateSettingsRequest {
    pub annual_interest_rate_bps: Option<i32>,
    pub absent_fine_amount: Option<i64>,
    pub late_fine_amount: Option<i64>,
    pub loan_late_penalty_bps: Option<i32>,
}

async fn update_settings(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(payload): Json<UpdateSettingsRequest>,
) -> AppResult<Json<Group>> {
    let group = sqlx::query_as::<_, Group>(
        r#"
        UPDATE groups
        SET
            annual_interest_rate_bps = COALESCE($2, annual_interest_rate_bps),
            absent_fine_amount = COALESCE($3, absent_fine_amount),
            late_fine_amount = COALESCE($4, late_fine_amount),
            loan_late_penalty_bps = COALESCE($5, loan_late_penalty_bps)
        WHERE id = $1
        RETURNING *
        "#,
    )
    .bind(id)
    .bind(payload.annual_interest_rate_bps)
    .bind(payload.absent_fine_amount)
    .bind(payload.late_fine_amount)
    .bind(payload.loan_late_penalty_bps)
    .fetch_one(&state.pool)
    .await?;

    Ok(Json(group))
}

#[derive(Serialize)]
pub struct DashboardMetrics {
    pub total_pool_capital: i64,
    pub total_active_loans: i64,
    pub total_fines_accrued: i64,
    pub group_balance: i64,
    pub active_members_count: i64,
    pub total_contributions_this_month: i64,
    pub pending_reconciliations: i64,
}

async fn dashboard_metrics(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> AppResult<Json<DashboardMetrics>> {
    let now = Utc::now();

    let group = sqlx::query_as::<_, Group>("SELECT * FROM groups WHERE id = $1")
        .bind(id)
        .fetch_optional(&state.pool)
        .await?
        .ok_or(crate::error::AppError::NotFound)?;

    let total_active_loans: i64 = sqlx::query_scalar(
        "SELECT COALESCE(SUM(principal), 0) FROM loans WHERE group_id = $1 AND status IN ('approved', 'disbursed')",
    )
    .bind(id)
    .fetch_one(&state.pool)
    .await?;

    let total_fines_accrued: i64 = sqlx::query_scalar(
        "SELECT COALESCE(SUM(amount), 0) FROM penalties WHERE group_id = $1",
    )
    .bind(id)
    .fetch_one(&state.pool)
    .await?;

    let active_members_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM members WHERE group_id = $1 AND is_active = true",
    )
    .bind(id)
    .fetch_one(&state.pool)
    .await?;

    let total_contributions_this_month: i64 = sqlx::query_scalar(
        r#"
        SELECT COALESCE(SUM(amount), 0)
        FROM ledger_transactions
        WHERE group_id = $1
          AND tx_type IN ('deposit', 'social_fund_payment', 'fine_payment', 'loan_repayment')
          AND EXTRACT(MONTH FROM created_at) = $2
          AND EXTRACT(YEAR FROM created_at) = $3
        "#,
    )
    .bind(id)
    .bind(i64::from(now.month()))
    .bind(i64::from(now.year()))
    .fetch_one(&state.pool)
    .await?;

    Ok(Json(DashboardMetrics {
        total_pool_capital: group.pool_balance,
        total_active_loans,
        total_fines_accrued,
        group_balance: group.pool_balance - total_active_loans,
        active_members_count,
        total_contributions_this_month,
        pending_reconciliations: 0,
    }))
}
