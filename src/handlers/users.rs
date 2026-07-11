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
    pub username: Option<String>,
    pub full_name: Option<String>,
    pub preferred_theme: UserTheme,
    pub role: UserRole,
    pub phone_number: Option<String>,
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
            phone_number: profile.phone_number,
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

fn username_candidates(claims: &Claims) -> Vec<String> {
    let fallback_suffix = claims.sub.replace('-', "");
    let mut candidates = Vec::new();

    if let Some(base) = username_from_claims(claims) {
        candidates.push(base.clone());

        let suffix = fallback_suffix.chars().take(8).collect::<String>();
        let alternate = validation::normalize_username_candidate(
            &format!("{base}_{suffix}"),
            &fallback_suffix,
        );
        if !candidates.contains(&alternate) {
            candidates.push(alternate);
        }

        let reversed_suffix = fallback_suffix.chars().rev().take(8).collect::<String>();
        let alternate = validation::normalize_username_candidate(
            &format!("{base}_{reversed_suffix}"),
            &fallback_suffix,
        );
        if !candidates.contains(&alternate) {
            candidates.push(alternate);
        }
    }

    let fallback = validation::normalize_username_candidate(
        &format!("user_{}", fallback_suffix.chars().take(8).collect::<String>()),
        &fallback_suffix,
    );
    if !candidates.contains(&fallback) {
        candidates.push(fallback);
    }

    candidates
}

async fn insert_profile_with_retry(
    pool: &sqlx::PgPool,
    auth_user_id: Uuid,
    claims: &Claims,
    role: UserRole,
) -> AppResult<UserProfile> {
    let full_name = claims.full_name();
    let phone_number = claims.phone_number();

    for username in username_candidates(claims) {
        validation::validate_username(&username)?;

        let result = sqlx::query_as::<_, UserProfile>(
            r#"
            INSERT INTO user_profiles (auth_user_id, username, full_name, role, phone_number)
            VALUES ($1, $2, $3, $4, $5)
            ON CONFLICT (auth_user_id) DO UPDATE SET
                username = COALESCE(EXCLUDED.username, user_profiles.username),
                full_name = COALESCE(EXCLUDED.full_name, user_profiles.full_name),
                role = EXCLUDED.role,
                phone_number = COALESCE(EXCLUDED.phone_number, user_profiles.phone_number),
                updated_at = NOW()
            RETURNING *
            "#,
        )
        .bind(auth_user_id)
        .bind(Some(&username))
        .bind(&full_name)
        .bind(role)
        .bind(&phone_number)
        .fetch_one(pool)
        .await;

        match result {
            Ok(profile) => return Ok(profile),
            Err(sqlx::Error::Database(db_err))
                if db_err.constraint() == Some("user_profiles_username_key") =>
            {
                continue;
            }
            Err(other) => return Err(other.into()),
        }
    }

    Err(AppError::Conflict("username is already taken".into()))
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
        None => insert_profile_with_retry(
            &state.pool,
            auth_user_id,
            &claims,
            UserRole::Member,
        )
        .await?,
    };

    Ok(Json(to_response(profile, claims.email)))
}

pub async fn upsert_profile(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Json(payload): Json<CreateUserProfileRequest>,
) -> AppResult<Json<UserProfileResponse>> {
    if let Some(ref un) = payload.username {
        validation::validate_username(un)?;
    }

    let auth_user_id = parse_auth_user_id(&claims.sub)?;
    let full_name = payload.full_name.or_else(|| claims.full_name());
    let role = payload.role.unwrap_or(UserRole::Member);

    let profile_result = sqlx::query_as::<_, UserProfile>(
        r#"
        INSERT INTO user_profiles (auth_user_id, username, full_name, role, phone_number)
        VALUES ($1, $2, $3, $4, $5)
        ON CONFLICT (auth_user_id) DO UPDATE SET
            username = COALESCE(EXCLUDED.username, user_profiles.username),
            full_name = COALESCE(EXCLUDED.full_name, user_profiles.full_name),
            role = EXCLUDED.role,
            phone_number = COALESCE(EXCLUDED.phone_number, user_profiles.phone_number),
            updated_at = NOW()
        RETURNING *
        "#,
    )
    .bind(auth_user_id)
    .bind(&payload.username)
    .bind(&full_name)
    .bind(role)
    .bind(&payload.phone_number)
    .fetch_one(&state.pool)
    .await;

    let profile = match profile_result {
        Ok(p) => p,
        Err(sqlx::Error::Database(db_err))
            if db_err.constraint() == Some("user_profiles_username_key") =>
        {
            let exists = load_requester_profile(&state.pool, &claims).await?.is_some();
            if exists {
                return Err(AppError::Conflict("username is already taken".into()));
            } else {
                insert_profile_with_retry(&state.pool, auth_user_id, &claims, role).await?
            }
        }
        Err(other) => return Err(AppError::from(other)),
    };

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
    let phone_number = payload.phone_number.or(existing.phone_number);

    let profile = sqlx::query_as::<_, UserProfile>(
        r#"
        UPDATE user_profiles
        SET full_name = $2,
            preferred_theme = $3,
            phone_number = $4,
            updated_at = NOW()
        WHERE auth_user_id = $1
        RETURNING *
        "#,
    )
    .bind(auth_user_id)
    .bind(full_name)
    .bind(preferred_theme)
    .bind(phone_number)
    .fetch_one(&state.pool)
    .await?;

    Ok(Json(to_response(profile, claims.email)))
}
