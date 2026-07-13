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
    Router::new()
        .route("/", post(create_meeting))
        .route("/{id}/attendance", post(bulk_record_attendance))
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

#[derive(Deserialize)]
pub struct BulkAttendanceRequest {
    pub group_id: Uuid,
    pub records: Vec<MemberAttendanceInput>,
}

#[derive(Deserialize)]
pub struct MemberAttendanceInput {
    pub member_id: Uuid,
    pub status: crate::models::member::AttendanceStatus,
}

#[derive(Serialize)]
pub struct BulkAttendanceResponse {
    pub recorded: usize,
    pub fines_issued: usize,
}

async fn bulk_record_attendance(
    State(state): State<AppState>,
    axum::extract::Path(_id): axum::extract::Path<Uuid>, // meeting id
    Json(payload): Json<BulkAttendanceRequest>,
) -> AppResult<Json<BulkAttendanceResponse>> {
    let mut tx = state.pool.begin().await?;
    let mut recorded = 0;
    let mut fines_issued = 0;

    let group = sqlx::query_as::<_, crate::models::group::Group>(
        "SELECT * FROM groups WHERE id = $1 FOR UPDATE",
    )
    .bind(payload.group_id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(crate::error::AppError::NotFound)?;

    // We can iterate over records and process them
    for record in payload.records {
        let member = sqlx::query_as::<_, crate::models::member::Member>(
            "SELECT * FROM members WHERE id = $1 FOR UPDATE",
        )
        .bind(record.member_id)
        .fetch_optional(&mut *tx)
        .await?;

        if let Some(member) = member {
            if member.group_id == payload.group_id && member.is_active {
                // Record attendance
                sqlx::query(
                    r#"
                    INSERT INTO attendance_records (group_id, member_id, meeting_date, status)
                    VALUES ($1, $2, CURRENT_DATE, $3)
                    "#,
                )
                .bind(payload.group_id)
                .bind(record.member_id)
                .bind(record.status)
                .execute(&mut *tx)
                .await?;

                recorded += 1;

                if matches!(
                    record.status,
                    crate::models::member::AttendanceStatus::Absent | crate::models::member::AttendanceStatus::Late
                ) {
                    let amount = match record.status {
                        crate::models::member::AttendanceStatus::Absent => group.absent_fine_amount,
                        crate::models::member::AttendanceStatus::Late => group.late_fine_amount,
                        _ => 0,
                    };

                    if amount > 0 {
                        sqlx::query(
                            r#"
                            INSERT INTO penalties (group_id, member_id, penalty_type, amount)
                            VALUES ($1, $2, $3, $4)
                            "#,
                        )
                        .bind(group.id)
                        .bind(member.id)
                        .bind(crate::models::penalty::PenaltyType::Attendance)
                        .bind(amount)
                        .execute(&mut *tx)
                        .await?;
                        
                        fines_issued += 1;
                    }
                }
            }
        }
    }

    tx.commit().await?;

    Ok(Json(BulkAttendanceResponse { recorded, fines_issued }))
}
