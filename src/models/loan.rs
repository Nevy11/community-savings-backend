use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[serde(rename_all = "snake_case")]
#[sqlx(type_name = "loan_status", rename_all = "snake_case")]
pub enum LoanStatus {
    Pending,
    Approved,
    Disbursed,
    Repaid,
    Defaulted,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Loan {
    pub id: Uuid,
    pub group_id: Uuid,
    pub member_id: Uuid,
    pub principal: i64,
    pub term_months: i32,
    pub status: LoanStatus,
    pub approved_at: Option<DateTime<Utc>>,
    pub disbursed_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct LoanGuarantor {
    pub loan_id: Uuid,
    pub member_id: Uuid,
}

#[derive(Debug, Deserialize)]
pub struct CreateLoanRequest {
    pub group_id: Uuid,
    pub member_id: Uuid,
    pub principal: i64,
    pub term_months: i32,
}

#[derive(Debug, Deserialize)]
pub struct AddGuarantorRequest {
    pub member_id: Uuid,
}
