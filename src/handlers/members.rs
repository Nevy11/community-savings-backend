use axum::{
    extract::{Path, Query, State},
    routing::get,
    Json, Router,
};
use serde::Deserialize;
use sqlx::{Postgres, Transaction};
use uuid::Uuid;

use crate::{
    error::{AppError, AppResult},
    models::{
        member::{
            AttendanceRecord, AttendanceStatus, CreateMemberRequest, Member, RecordAttendanceRequest,
            UpdateMemberRequest,
        },
        penalty::{Penalty, PenaltyType},
    },
    services::validation,
    AppState,
};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/", get(list_members).post(create_member))
        .route("/{id}", get(get_member).patch(update_member))
        .route("/{id}/attendance", get(list_attendance).post(record_attendance))
}

#[derive(Debug, Deserialize)]
struct AttendanceQuery {
    group_id: Option<Uuid>,
}

async fn list_members(State(state): State<AppState>) -> AppResult<Json<Vec<Member>>> {
    let members = sqlx::query_as::<_, Member>(
        "SELECT * FROM members ORDER BY joined_at DESC",
    )
    .fetch_all(&state.pool)
    .await?;

    Ok(Json(members))
}

async fn get_member(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> AppResult<Json<Member>> {
    let member = sqlx::query_as::<_, Member>("SELECT * FROM members WHERE id = $1")
        .bind(id)
        .fetch_optional(&state.pool)
        .await?
        .ok_or(AppError::NotFound)?;

    Ok(Json(member))
}

async fn create_member(
    State(state): State<AppState>,
    Json(payload): Json<CreateMemberRequest>,
) -> AppResult<Json<Member>> {
    validation::validate_phone_number(&payload.phone_number)?;

    let member = sqlx::query_as::<_, Member>(
        r#"
        INSERT INTO members (group_id, full_name, phone_number)
        VALUES ($1, $2, $3)
        RETURNING *
        "#,
    )
    .bind(payload.group_id)
    .bind(&payload.full_name)
    .bind(&payload.phone_number)
    .fetch_one(&state.pool)
    .await?;

    Ok(Json(member))
}

async fn update_member(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(payload): Json<UpdateMemberRequest>,
) -> AppResult<Json<Member>> {
    if let Some(ref phone) = payload.phone_number {
        validation::validate_phone_number(phone)?;
    }

    let member = sqlx::query_as::<_, Member>(
        r#"
        UPDATE members
        SET
            full_name = COALESCE($2, full_name),
            phone_number = COALESCE($3, phone_number),
            is_active = COALESCE($4, is_active)
        WHERE id = $1
        RETURNING *
        "#,
    )
    .bind(id)
    .bind(payload.full_name)
    .bind(payload.phone_number)
    .bind(payload.is_active)
    .fetch_optional(&state.pool)
    .await?
    .ok_or(AppError::NotFound)?;

    Ok(Json(member))
}

async fn list_attendance(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Query(query): Query<AttendanceQuery>,
) -> AppResult<Json<Vec<AttendanceRecord>>> {
    let records = if let Some(group_id) = query.group_id {
        sqlx::query_as::<_, AttendanceRecord>(
            r#"
            SELECT * FROM attendance_records
            WHERE member_id = $1 AND group_id = $2
            ORDER BY meeting_date DESC
            "#,
        )
        .bind(id)
        .bind(group_id)
        .fetch_all(&state.pool)
        .await?
    } else {
        sqlx::query_as::<_, AttendanceRecord>(
            r#"
            SELECT * FROM attendance_records
            WHERE member_id = $1
            ORDER BY meeting_date DESC
            "#,
        )
        .bind(id)
        .fetch_all(&state.pool)
        .await?
    };

    Ok(Json(records))
}

async fn record_attendance(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(payload): Json<RecordAttendanceRequest>,
) -> AppResult<Json<AttendanceRecord>> {
    let mut tx = state.pool.begin().await?;

    let member = sqlx::query_as::<_, Member>("SELECT * FROM members WHERE id = $1 FOR UPDATE")
        .bind(id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or(AppError::NotFound)?;

    if member.group_id != payload.group_id {
        return Err(AppError::BadRequest(
            "member does not belong to the specified group".into(),
        ));
    }

    if !member.is_active {
        return Err(AppError::Conflict("member is inactive".into()));
    }

    let attendance = sqlx::query_as::<_, AttendanceRecord>(
        r#"
        INSERT INTO attendance_records (group_id, member_id, meeting_date, status)
        VALUES ($1, $2, $3, $4)
        RETURNING *
        "#,
    )
    .bind(payload.group_id)
    .bind(id)
    .bind(payload.meeting_date)
    .bind(payload.status)
    .fetch_one(&mut *tx)
    .await?;

    if matches!(
        payload.status,
        AttendanceStatus::Absent | AttendanceStatus::Late
    ) {
        issue_attendance_fine(&mut tx, &member, payload.status).await?;
    }

    tx.commit().await?;
    Ok(Json(attendance))
}

async fn issue_attendance_fine(
    tx: &mut Transaction<'_, Postgres>,
    member: &Member,
    status: AttendanceStatus,
) -> AppResult<Penalty> {
    let group = sqlx::query_as::<_, crate::models::group::Group>(
        "SELECT * FROM groups WHERE id = $1 FOR UPDATE",
    )
    .bind(member.group_id)
    .fetch_one(&mut **tx)
    .await?;

    let amount = match status {
        AttendanceStatus::Absent => group.absent_fine_amount,
        AttendanceStatus::Late => group.late_fine_amount,
        AttendanceStatus::Present => return Err(AppError::Internal),
    };

    if amount <= 0 {
        return Err(AppError::BadRequest(
            "attendance fine amount is not configured for this group".into(),
        ));
    }

    let penalty = sqlx::query_as::<_, Penalty>(
        r#"
        INSERT INTO penalties (group_id, member_id, penalty_type, amount)
        VALUES ($1, $2, $3, $4)
        RETURNING *
        "#,
    )
    .bind(member.group_id)
    .bind(member.id)
    .bind(PenaltyType::Attendance)
    .bind(amount)
    .fetch_one(&mut **tx)
    .await?;

    Ok(penalty)
}
