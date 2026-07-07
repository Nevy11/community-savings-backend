use axum::{Json, Router, extract::State, http::HeaderMap, routing::post};
use serde::Deserialize;
use serde_json::Value;

use crate::{AppState, error::AppResult, models::user_profile::UserRole};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/supabase-auth", post(handle_supabase_auth))
        .route("/supabase-auth/handshake", post(handshake))
}

#[derive(Debug, Deserialize)]
struct SupabaseUser {
    id: String,
    email: Option<String>,
    user_metadata: Option<Value>,
}

async fn handle_supabase_auth(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<Value>,
) -> AppResult<String> {
    // Verify signature
    let signature = match headers
        .get("x-hook-signature")
        .or_else(|| headers.get("x-supabase-signature"))
        .or_else(|| headers.get("x-signature"))
        .and_then(|v| v.to_str().ok())
    {
        Some(s) => s,
        None => {
            eprintln!("[webhook] missing signature header; headers: {:?}", headers);
            return Err(crate::error::AppError::Unauthorized(
                "missing webhook signature".into(),
            ));
        }
    };

    if state.config.supabase_webhook_secret.is_empty() {
        eprintln!("[webhook] SUPABASE_WEBHOOK_SECRET not configured");
        return Err(crate::error::AppError::Unauthorized(
            "webhook secret not configured".into(),
        ));
    }

    let canonical = serde_json::to_string(&payload).map_err(|_| {
        eprintln!("[webhook] failed to serialize payload for signature verification");
        crate::error::AppError::BadRequest("invalid payload".into())
    })?;

    if let Err(err) = crate::services::webhook::verify_webhook_signature(
        &canonical,
        signature,
        &state.config.supabase_webhook_secret,
    ) {
        eprintln!(
            "[webhook] signature verification failed: {:?}; signature: {}",
            err, signature
        );
        return Err(err);
    }
    // Try to extract user object from known keys
    let user_val = payload
        .get("user")
        .or_else(|| payload.get("record"))
        .or_else(|| payload.get("new"))
        .unwrap_or(&payload);

    let user: SupabaseUser = serde_json::from_value(user_val.clone())
        .map_err(|_| crate::error::AppError::BadRequest("invalid webhook payload".into()))?;

    let auth_user_id = uuid::Uuid::parse_str(&user.id)
        .map_err(|_| crate::error::AppError::BadRequest("invalid user id".into()))?;

    let email = user.email.clone();

    // derive username from metadata or email
    let mut username = None;
    if let Some(meta) = user.user_metadata.as_ref() {
        if let Some(u) = meta.get("username").and_then(|v| v.as_str()) {
            username = Some(u.to_string());
        }
        if username.is_none() {
            if let Some(fnm) = meta.get("full_name").and_then(|v| v.as_str()) {
                username = Some(fnm.to_lowercase().replace(' ', "_"));
            }
        }
    }
    if username.is_none() {
        if let Some(ref e) = email {
            if let Some(pos) = e.find('@') {
                username = Some(e[..pos].to_string());
            }
        }
    }

    let username =
        username.unwrap_or_else(|| format!("user_{}", &auth_user_id.simple().to_string()[..8]));
    let full_name = user.user_metadata.as_ref().and_then(|m| {
        m.get("full_name")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
    });
    let role = match user
        .user_metadata
        .as_ref()
        .and_then(|m| m.get("role"))
        .and_then(|v| v.as_str())
    {
        Some("member") => UserRole::Member,
        _ => UserRole::Administrator,
    };

    // Insert into users and user_profiles in a transaction
    let mut tx = state.pool.begin().await?;
    let tx_ref = &mut tx;

    // insert into users table if not exists
    sqlx::query("INSERT INTO users (id, email) VALUES ($1, $2) ON CONFLICT (id) DO NOTHING")
        .bind(auth_user_id)
        .bind(&email)
        .execute(&mut **tx_ref)
        .await?;

    // upsert profile
    let _profile = sqlx::query_as::<_, crate::models::user_profile::UserProfile>(
        r#"
        INSERT INTO user_profiles (auth_user_id, username, full_name, role)
        VALUES ($1, $2, $3, $4)
        ON CONFLICT (auth_user_id) DO UPDATE SET
            username = EXCLUDED.username,
            full_name = COALESCE(EXCLUDED.full_name, user_profiles.full_name),
            role = EXCLUDED.role,
            updated_at = NOW()
        RETURNING *
        "#,
    )
    .bind(auth_user_id)
    .bind(&username)
    .bind(&full_name)
    .bind(role)
    .fetch_one(&mut **tx_ref)
    .await?;

    tx.commit().await?;

    Ok("ok".into())
}

async fn handshake(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<Value>,
) -> AppResult<String> {
    // Simple handshake: verify signature and return ok
    let signature = headers
        .get("x-hook-signature")
        .or_else(|| headers.get("x-supabase-signature"))
        .or_else(|| headers.get("x-signature"))
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| crate::error::AppError::Unauthorized("missing webhook signature".into()))?;

    if state.config.supabase_webhook_secret.is_empty() {
        eprintln!("[webhook/handshake] SUPABASE_WEBHOOK_SECRET not configured");
        return Err(crate::error::AppError::Unauthorized(
            "webhook secret not configured".into(),
        ));
    }

    let canonical = serde_json::to_string(&payload)
        .map_err(|_| crate::error::AppError::BadRequest("invalid payload".into()))?;
    println!("[webhook/handshake] canonical payload: {}", &canonical);
    crate::services::webhook::verify_webhook_signature(
        &canonical,
        signature,
        &state.config.supabase_webhook_secret,
    )?;

    println!("[webhook/handshake] successful signature verification");
    Ok("handshake ok".into())
}
