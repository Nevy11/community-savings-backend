use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "penalty_type", rename_all = "snake_case")]
pub enum PenaltyType {
    Attendance,
    LoanLate,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Penalty {
    pub id: Uuid,
    pub group_id: Uuid,
    pub member_id: Uuid,
    pub loan_id: Option<Uuid>,
    pub penalty_type: PenaltyType,
    pub amount: i64,
    pub applied_at: DateTime<Utc>,
    pub paid: bool,
}

#[derive(Debug, Deserialize)]
pub struct CalculateLoanPenaltyRequest {
    pub loan_id: Uuid,
    pub overdue_days: i32,
}

#[derive(Debug, Deserialize)]
pub struct ApplyPenaltyRequest {
    pub penalty_id: Uuid,
}

#[derive(Debug, Serialize)]
pub struct PenaltyCalculation {
    pub loan_id: Uuid,
    pub overdue_days: i32,
    pub calculated_amount: i64,
}
