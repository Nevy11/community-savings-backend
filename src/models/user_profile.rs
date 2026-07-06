use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[serde(rename_all = "snake_case")]
#[sqlx(type_name = "user_theme", rename_all = "snake_case")]
pub enum UserTheme {
    Light,
    Dark,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[serde(rename_all = "snake_case")]
#[sqlx(type_name = "user_role", rename_all = "snake_case")]
pub enum UserRole {
    Administrator,
    Member,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct UserProfile {
    pub id: Uuid,
    pub auth_user_id: Uuid,
    pub username: String,
    pub full_name: Option<String>,
    pub preferred_theme: UserTheme,
    pub role: UserRole,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct CreateUserProfileRequest {
    pub username: String,
    pub full_name: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateUserProfileRequest {
    pub full_name: Option<String>,
    pub preferred_theme: Option<UserTheme>,
}
