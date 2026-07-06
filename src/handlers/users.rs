use axum::{
    extract::State,
    routing::get,
    Json, Router,
};
use serde::Serialize;

use crate::{error::AppResult, AppState};

pub fn routes() -> Router<AppState> {
    Router::new().route("/me", get(get_me))
}

#[derive(Serialize)]
pub struct UserResponse {
    pub id: String,
    pub email: String,
    pub role: String,
}

use axum::Extension;
use crate::middleware::Claims;

pub async fn get_me(
    State(_state): State<AppState>,
    Extension(claims): Extension<Claims>,
) -> AppResult<Json<UserResponse>> {
    Ok(Json(UserResponse {
        id: claims.sub,
        email: claims.email.unwrap_or_default(),
        role: claims.role.unwrap_or_else(|| "user".to_string()),
    }))
}
