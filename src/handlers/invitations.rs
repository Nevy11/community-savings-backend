use axum::{
    extract::{Path, State},
    routing::get,
    Json, Router, Extension,
};
use uuid::Uuid;

use crate::{
    error::{AppError, AppResult},
    models::invitation::{CreateInvitationRequest, GroupInvitation},
    middleware::Claims,
    AppState,
};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/", get(list_my_invitations).post(create_invitation))
        .route("/{id}/accept", axum::routing::post(accept_invitation))
        .route("/{id}/reject", axum::routing::post(reject_invitation))
}

async fn create_invitation(
    State(state): State<AppState>,
    Json(payload): Json<CreateInvitationRequest>,
) -> AppResult<Json<GroupInvitation>> {
    let inv = sqlx::query_as::<_, GroupInvitation>(
        r#"
        INSERT INTO group_invitations (group_id, email, phone_number, status)
        VALUES ($1, $2, $3, 'pending')
        RETURNING *
        "#,
    )
    .bind(payload.group_id)
    .bind(&payload.email)
    .bind(&payload.phone_number)
    .fetch_one(&state.pool)
    .await?;
    Ok(Json(inv))
}

async fn list_my_invitations(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
) -> AppResult<Json<Vec<GroupInvitation>>> {
    let email = claims.email.clone().ok_or_else(|| AppError::Unauthorized("No email in claims".into()))?;
    
    let invs = sqlx::query_as::<_, GroupInvitation>(
        "SELECT * FROM group_invitations WHERE email = $1 AND status = 'pending' ORDER BY created_at DESC"
    )
    .bind(email)
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(invs))
}

async fn accept_invitation(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(id): Path<Uuid>,
) -> AppResult<Json<GroupInvitation>> {
    let email = claims.email.clone().ok_or_else(|| AppError::Unauthorized("No email in claims".into()))?;
    let mut tx = state.pool.begin().await?;

    let inv = sqlx::query_as::<_, GroupInvitation>(
        "SELECT * FROM group_invitations WHERE id = $1 AND email = $2 AND status = 'pending' FOR UPDATE"
    )
    .bind(id)
    .bind(&email)
    .fetch_optional(&mut *tx)
    .await?.ok_or_else(|| AppError::NotFound)?;

    let full_name = claims.full_name().unwrap_or_else(|| "Unknown Member".into());

    let exists = sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM members WHERE group_id = $1 AND phone_number = $2)"
    )
    .bind(inv.group_id)
    .bind(&inv.phone_number)
    .fetch_one(&mut *tx)
    .await?;

    if !exists {
        sqlx::query(
            "INSERT INTO members (group_id, full_name, phone_number, is_active) VALUES ($1, $2, $3, true)"
        )
        .bind(inv.group_id)
        .bind(&full_name)
        .bind(&inv.phone_number)
        .execute(&mut *tx)
        .await?;
    }

    let updated_inv = sqlx::query_as::<_, GroupInvitation>(
        "UPDATE group_invitations SET status = 'accepted' WHERE id = $1 RETURNING *"
    )
    .bind(id)
    .fetch_one(&mut *tx)
    .await?;

    tx.commit().await?;

    Ok(Json(updated_inv))
}

async fn reject_invitation(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(id): Path<Uuid>,
) -> AppResult<Json<GroupInvitation>> {
    let email = claims.email.clone().ok_or_else(|| AppError::Unauthorized("No email in claims".into()))?;
    
    let inv = sqlx::query_as::<_, GroupInvitation>(
        "UPDATE group_invitations SET status = 'rejected' WHERE id = $1 AND email = $2 AND status = 'pending' RETURNING *"
    )
    .bind(id)
    .bind(email)
    .fetch_optional(&state.pool)
    .await?.ok_or_else(|| AppError::NotFound)?;
    
    Ok(Json(inv))
}
