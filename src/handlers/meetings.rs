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
    Router::new().route("/", post(create_meeting))
}

#[derive(Deserialize)]
pub struct CreateMeetingRequest {
    pub group_id: Uuid,
    pub cycle_id: Uuid,
    pub meeting_date: NaiveDate,
}

#[derive(Serialize, sqlx::FromRow)]
pub struct MeetingResponse {
    pub id: Uuid,
    pub group_id: Uuid,
    pub cycle_id: Uuid,
    pub meeting_date: NaiveDate,
}

async fn create_meeting(
    State(state): State<AppState>,
    Json(payload): Json<CreateMeetingRequest>,
) -> AppResult<Json<MeetingResponse>> {
    let meeting = sqlx::query_as::<_, MeetingResponse>(
        r#"
        INSERT INTO meetings (group_id, cycle_id, meeting_date)
        VALUES ($1, $2, $3)
        RETURNING id, group_id, cycle_id, meeting_date
        "#,
    )
    .bind(payload.group_id)
    .bind(payload.cycle_id)
    .bind(payload.meeting_date)
    .fetch_one(&state.pool)
    .await?;

    Ok(Json(meeting))
}
