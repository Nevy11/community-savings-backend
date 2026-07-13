use axum::{
    extract::{Path, State},
    routing::get,
    Json, Router,
};
use serde::Serialize;
use uuid::Uuid;

use crate::{error::AppResult, AppState};

pub fn routes() -> Router<AppState> {
    Router::new().route("/dividends/:group_id", get(get_dividends))
}

#[derive(Serialize)]
pub struct DividendRecord {
    pub member_id: Uuid,
    pub member_name: String,
    pub total_contributions: i64,
    pub weighted_contribution: i64,
    pub dividend_share: i64,
    pub rank: i32,
}

#[derive(Serialize)]
pub struct DividendsAnalytics {
    pub group_id: Uuid,
    pub total_dividend_pool: i64,
    pub total_weighted_funds: i64,
    pub records: Vec<DividendRecord>,
}

async fn get_dividends(
    State(state): State<AppState>,
    Path(group_id): Path<Uuid>,
) -> AppResult<Json<DividendsAnalytics>> {
    let mut records = Vec::new();
    
    let total_fines: i64 = sqlx::query_scalar("SELECT COALESCE(SUM(amount), 0) FROM penalties WHERE group_id = $1 AND paid = true")
        .bind(group_id)
        .fetch_one(&state.pool)
        .await?;
        
    let total_interest: i64 = sqlx::query_scalar("SELECT COALESCE(SUM(amount), 0) FROM ledger_transactions WHERE group_id = $1 AND tx_type = 'loan_repayment'")
        .bind(group_id)
        .fetch_one(&state.pool)
        .await?;
        
    let total_dividend_pool = total_fines + total_interest;
    
#[derive(sqlx::FromRow)]
struct MemberRow {
    id: Uuid,
    full_name: String,
    joined_at: chrono::DateTime<chrono::Utc>,
}

    let members = sqlx::query_as::<_, MemberRow>("SELECT id, full_name, joined_at FROM members WHERE group_id = $1")
        .bind(group_id)
        .fetch_all(&state.pool)
        .await?;
        
    let mut total_weighted = 0;
    
    for m in &members {
        let total_contributions: i64 = sqlx::query_scalar("SELECT COALESCE(SUM(amount), 0) FROM ledger_transactions WHERE member_id = $1 AND tx_type = 'deposit'")
            .bind(m.id)
            .fetch_one(&state.pool)
            .await?;
            
        use chrono::Datelike;
        let now = chrono::Utc::now();
        let joined = m.joined_at;
        
        let months_active = (now.year() as i64 - joined.year() as i64) * 12 + (now.month() as i64 - joined.month() as i64);
        let months_active = if months_active < 1 { 1 } else { months_active };
        
        let weighted = total_contributions * months_active;
        total_weighted += weighted;
        
        records.push(DividendRecord {
            member_id: m.id,
            member_name: m.full_name.clone(),
            total_contributions,
            weighted_contribution: weighted,
            dividend_share: 0,
            rank: 0,
        });
    }
    
    for r in &mut records {
        if total_weighted > 0 {
            r.dividend_share = ((r.weighted_contribution as f64 / total_weighted as f64) * total_dividend_pool as f64).round() as i64;
        }
    }
    
    records.sort_by(|a, b| b.weighted_contribution.cmp(&a.weighted_contribution));
    for (i, r) in records.iter_mut().enumerate() {
        r.rank = (i + 1) as i32;
    }
    
    Ok(Json(DividendsAnalytics {
        group_id,
        total_dividend_pool,
        total_weighted_funds: total_weighted,
        records,
    }))
}
