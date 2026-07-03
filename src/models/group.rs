use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "interest_method", rename_all = "snake_case")]
pub enum InterestMethod {
    FlatRate,
    ReducingBalance,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Group {
    pub id: Uuid,
    pub name: String,
    pub pool_balance: i64,
    pub interest_method: InterestMethod,
    pub annual_interest_rate_bps: i32,
    pub absent_fine_amount: i64,
    pub late_fine_amount: i64,
    pub loan_late_penalty_bps: i32,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct CreateGroupRequest {
    pub name: String,
    pub annual_interest_rate_bps: i32,
    pub absent_fine_amount: i64,
    pub late_fine_amount: i64,
    pub interest_method: Option<InterestMethod>,
}
