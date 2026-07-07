use axum::{
    Extension, Json, Router,
    extract::{Path, State},
    routing::{get, post},
};
use serde::Serialize;
use uuid::Uuid;

use crate::{
    AppState,
    error::{AppError, AppResult},
    middleware::Claims,
    models::user_profile::{
        CreateUserProfileRequest, UpdateUserProfileRequest, UserProfile, UserRole, UserTheme,
    },
    services::validation,
};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/me", get(get_me))
        .route("/profile", post(upsert_profile).patch(update_profile))
        .route("/profile/{username}", get(get_profile_by_username))
}

#[derive(Serialize)]
pub struct UserProfileResponse {
    pub id: Uuid,
    pub username: String,
    pub full_name: Option<String>,
    pub preferred_theme: UserTheme,
    pub role: UserRole,
    pub email: Option<String>,
}

impl From<UserProfile> for UserProfileResponse {
    fn from(profile: UserProfile) -> Self {
        Self {
            id: profile.id,
            username: profile.username,
            full_name: profile.full_name,
            preferred_theme: profile.preferred_theme,
            role: profile.role,
            email: None,
        }
    }
}

fn parse_auth_user_id(sub: &str) -> AppResult<Uuid> {
    Uuid::parse_str(sub).map_err(|_| AppError::BadRequest("invalid auth user id".into()))
}

fn username_from_claims(claims: &Claims) -> Option<String> {
    claims
        .user_metadata
        .as_ref()
        .and_then(|meta| meta.username.clone())
        .or_else(|| {
            claims
                .email
                .as_ref()
                .and_then(|email| email.split('@').next())
                .map(|username| username.to_lowercase())
        })
        .map(|username| {
            validation::normalize_username_candidate(
                &username,
                &claims.sub.replace('-', ""),
            )
        })
}

fn to_response(profile: UserProfile, email: Option<String>) -> UserProfileResponse {
    let mut response = UserProfileResponse::from(profile);
    response.email = email;
    response
}

async fn load_requester_profile(
    pool: &sqlx::PgPool,
    claims: &Claims,
) -> AppResult<Option<UserProfile>> {
    let auth_user_id = parse_auth_user_id(&claims.sub)?;
    let profile =
        sqlx::query_as::<_, UserProfile>("SELECT * FROM user_profiles WHERE auth_user_id = $1")
            .bind(auth_user_id)
            .fetch_optional(pool)
            .await?;

    Ok(profile)
}

fn authorize_profile_access(
    claims: &Claims,
    profile: &UserProfile,
    requester: Option<&UserProfile>,
) -> AppResult<()> {
    let auth_user_id = parse_auth_user_id(&claims.sub)?;

    if profile.auth_user_id == auth_user_id {
        return Ok(());
    }

    if let Some(requester) = requester {
        if requester.role == UserRole::Administrator {
            return Ok(());
        }
    }

    Err(AppError::Unauthorized(
        "you can only access your own profile".into(),
    ))
}

pub async fn get_profile_by_username(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(username): Path<String>,
) -> AppResult<Json<UserProfileResponse>> {
    let profile =
        sqlx::query_as::<_, UserProfile>("SELECT * FROM user_profiles WHERE username = $1")
            .bind(&username)
            .fetch_optional(&state.pool)
            .await?
            .ok_or(AppError::NotFound)?;

    let requester = load_requester_profile(&state.pool, &claims).await?;
    authorize_profile_access(&claims, &profile, requester.as_ref())?;

    Ok(Json(to_response(profile, claims.email)))
}

pub async fn get_me(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
) -> AppResult<Json<UserProfileResponse>> {
    let auth_user_id = parse_auth_user_id(&claims.sub)?;

    let profile = match sqlx::query_as::<_, UserProfile>(
        "SELECT * FROM user_profiles WHERE auth_user_id = $1",
    )
    .bind(auth_user_id)
    .fetch_optional(&state.pool)
    .await?
    {
        Some(profile) => profile,
        None => {
            let username = username_from_claims(&claims)
                .ok_or_else(|| AppError::BadRequest("username is required".into()))?;
            validation::validate_username(&username)?;

            sqlx::query_as::<_, UserProfile>(
                r#"
                INSERT INTO user_profiles (auth_user_id, username, full_name, role)
                VALUES ($1, $2, $3, $4)
                ON CONFLICT (auth_user_id) DO UPDATE SET
                    username = EXCLUDED.username,
                    full_name = COALESCE(EXCLUDED.full_name, user_profiles.full_name),
                    updated_at = NOW()
                RETURNING *
                "#,
            )
            .bind(auth_user_id)
            .bind(&username)
            .bind(claims.full_name())
            .bind(UserRole::Administrator)
            .fetch_one(&state.pool)
            .await
            .map_err(|err| match err {
                sqlx::Error::Database(db_err)
                    if db_err.constraint() == Some("user_profiles_username_key") =>
                {
                    AppError::Conflict("username is already taken".into())
                }
                other => AppError::from(other),
            })?
        }
    };

    Ok(Json(to_response(profile, claims.email)))
}

pub async fn upsert_profile(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Json(payload): Json<CreateUserProfileRequest>,
) -> AppResult<Json<UserProfileResponse>> {
    validation::validate_username(&payload.username)?;

    let auth_user_id = parse_auth_user_id(&claims.sub)?;
    let full_name = payload.full_name.or_else(|| claims.full_name());
    let role = payload.role.unwrap_or(UserRole::Administrator);

    let profile = sqlx::query_as::<_, UserProfile>(
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
    .bind(&payload.username)
    .bind(&full_name)
    .bind(role)
    .fetch_one(&state.pool)
    .await
    .map_err(|err| match err {
        sqlx::Error::Database(db_err)
            if db_err.constraint() == Some("user_profiles_username_key") =>
        {
            AppError::Conflict("username is already taken".into())
        }
        other => AppError::from(other),
    })?;

    Ok(Json(to_response(profile, claims.email)))
}

pub async fn update_profile(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Json(payload): Json<UpdateUserProfileRequest>,
) -> AppResult<Json<UserProfileResponse>> {
    let auth_user_id = parse_auth_user_id(&claims.sub)?;

    let existing =
        sqlx::query_as::<_, UserProfile>("SELECT * FROM user_profiles WHERE auth_user_id = $1")
            .bind(auth_user_id)
            .fetch_optional(&state.pool)
            .await?
            .ok_or(AppError::NotFound)?;

    let full_name = payload.full_name.or(existing.full_name);
    let preferred_theme = payload.preferred_theme.unwrap_or(existing.preferred_theme);

    let profile = sqlx::query_as::<_, UserProfile>(
        r#"
        UPDATE user_profiles
        SET full_name = $2,
            preferred_theme = $3,
            updated_at = NOW()
        WHERE auth_user_id = $1
        RETURNING *
        "#,
    )
    .bind(auth_user_id)
    .bind(full_name)
    .bind(preferred_theme)
    .fetch_one(&state.pool)
    .await?;

    Ok(Json(to_response(profile, claims.email)))
}
