use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Member {
    pub id: Uuid,
    pub group_id: Uuid,
    pub auth_user_id: Option<Uuid>,
    pub full_name: String,
    pub phone_number: String,
    pub is_active: bool,
    pub joined_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[serde(rename_all = "snake_case")]
#[sqlx(type_name = "attendance_status", rename_all = "snake_case")]
pub enum AttendanceStatus {
    Present,
    Absent,
    Late,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct AttendanceRecord {
    pub id: Uuid,
    pub group_id: Uuid,
    pub member_id: Uuid,
    pub meeting_date: NaiveDate,
    pub status: AttendanceStatus,
    pub recorded_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct CreateMemberRequest {
    pub group_id: Uuid,
    pub full_name: String,
    pub phone_number: String,
}

#[derive(Debug, Deserialize)]
pub struct UpdateMemberRequest {
    pub full_name: Option<String>,
    pub phone_number: Option<String>,
    pub is_active: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct RecordAttendanceRequest {
    pub group_id: Uuid,
    pub meeting_date: NaiveDate,
    pub status: AttendanceStatus,
}
