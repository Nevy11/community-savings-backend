use axum::{
    extract::State,
    routing::post,
    Json, Router,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use chrono::NaiveDate;

use crate::{error::AppResult, AppState};

pub fn routes() -> Router<AppState> {
    Router::new().route("/", post(create_cycle))
}

#[derive(Deserialize)]
pub struct CreateCycleRequest {
    pub group_id: Uuid,
    pub start_date: NaiveDate,
    pub end_date: NaiveDate,
}

#[derive(Serialize, sqlx::FromRow)]
pub struct CycleResponse {
    pub id: Uuid,
    pub group_id: Uuid,
    pub start_date: NaiveDate,
    pub end_date: NaiveDate,
    pub status: String,
}

async fn create_cycle(
    State(state): State<AppState>,
    Json(payload): Json<CreateCycleRequest>,
) -> AppResult<Json<CycleResponse>> {
    let cycle = sqlx::query_as::<_, CycleResponse>(
        r#"
        INSERT INTO cycles (group_id, start_date, end_date)
        VALUES ($1, $2, $3)
        RETURNING id, group_id, start_date, end_date, status
        "#,
    )
    .bind(payload.group_id)
    .bind(payload.start_date)
    .bind(payload.end_date)
    .fetch_one(&state.pool)
    .await?;

    Ok(Json(cycle))
}
