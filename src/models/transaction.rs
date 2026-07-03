use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[serde(rename_all = "snake_case")]
#[sqlx(type_name = "tx_type", rename_all = "snake_case")]
pub enum TxType {
    Deposit,
    SocialFundPayment,
    Withdrawal,
    LoanRepayment,
    FinePayment,
    DividendPayout,
    LoanDisbursement,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct LedgerEntry {
    pub id: Uuid,
    pub group_id: Uuid,
    pub member_id: Uuid,
    pub amount: i64,
    pub tx_type: TxType,
    pub reference: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct AppendLedgerRequest {
    pub group_id: Uuid,
    pub member_id: Uuid,
    pub amount: i64,
    pub tx_type: TxType,
    pub reference: Option<String>,
}
