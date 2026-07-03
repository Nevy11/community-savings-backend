use axum::{
    extract::State,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};

use crate::{error::AppResult, AppState};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/login", post(login))
        .route("/me", get(get_me))
}

#[derive(Deserialize)]
pub struct LoginRequest {
    pub email: String,
    pub password: String,
}

#[derive(Serialize)]
pub struct LoginResponse {
    pub token: String,
    pub user: UserResponse,
}

#[derive(Serialize)]
pub struct UserResponse {
    pub id: String,
    pub email: String,
    pub role: String,
}

async fn login(
    State(_state): State<AppState>,
    Json(_payload): Json<LoginRequest>,
) -> AppResult<Json<LoginResponse>> {
    // Stubbed login logic
    Ok(Json(LoginResponse {
        token: "stub-jwt-token".into(),
        user: UserResponse {
            id: "stub-id".into(),
            email: "admin@example.com".into(),
            role: "admin".into(),
        },
    }))
}

async fn get_me(State(_state): State<AppState>) -> AppResult<Json<UserResponse>> {
    Ok(Json(UserResponse {
        id: "stub-id".into(),
        email: "admin@example.com".into(),
        role: "admin".into(),
    }))
}
