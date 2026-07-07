use axum::{
    extract::{Request, State},
    http::{Method, header},
    middleware::Next,
    response::Response,
};
use jsonwebtoken::{
    Algorithm, DecodingKey, Validation, decode, decode_header,
    jwk::{Jwk, JwkSet},
};
use serde::{Deserialize, Serialize};

use crate::{AppState, error::AppError};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct UserMetadata {
    pub full_name: Option<String>,
    pub name: Option<String>,
    pub username: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Claims {
    pub sub: String,
    pub email: Option<String>,
    pub role: Option<String>,
    pub user_metadata: Option<UserMetadata>,
    pub aud: String,
    pub exp: usize,
}

impl Claims {
    pub fn full_name(&self) -> Option<String> {
        self.user_metadata
            .as_ref()
            .and_then(|meta| meta.full_name.clone().or_else(|| meta.name.clone()))
    }
}

fn should_skip_auth(method: &Method, path: &str) -> bool {
    method == Method::OPTIONS || path == "/ping" || path == "/health"
}

async fn fetch_supabase_jwk(supabase_url: &str, kid: &str) -> Result<Jwk, AppError> {
    if supabase_url.is_empty() {
        return Err(AppError::Unauthorized(
            "SUPABASE_URL is required for asymmetric JWT validation".into(),
        ));
    }

    let jwks_url = format!("{supabase_url}/auth/v1/.well-known/jwks.json");
    let jwks = reqwest::get(&jwks_url)
        .await
        .map_err(|err| {
            eprintln!("[auth] failed to fetch Supabase JWKS from {jwks_url}: {err}");
            AppError::Unauthorized("unable to fetch token signing keys".into())
        })?
        .error_for_status()
        .map_err(|err| {
            eprintln!("[auth] Supabase JWKS endpoint returned an error from {jwks_url}: {err}");
            AppError::Unauthorized("unable to fetch token signing keys".into())
        })?
        .json::<JwkSet>()
        .await
        .map_err(|err| {
            eprintln!("[auth] failed to parse Supabase JWKS from {jwks_url}: {err}");
            AppError::Unauthorized("invalid token signing keys".into())
        })?;

    jwks.find(kid).cloned().ok_or_else(|| {
        eprintln!("[auth] no Supabase JWKS key found for kid {kid}");
        AppError::Unauthorized("unknown token signing key".into())
    })
}

async fn decoding_key_for_token(
    token: &str,
    state: &AppState,
) -> Result<(DecodingKey, Algorithm), AppError> {
    let header = decode_header(token).map_err(|err| {
        eprintln!("[auth] failed to decode jwt header: {err}");
        AppError::Unauthorized("invalid access token".into())
    })?;

    match header.alg {
        Algorithm::HS256 | Algorithm::HS384 | Algorithm::HS512 => Ok((
            DecodingKey::from_secret(state.config.supabase_jwt_secret.as_bytes()),
            header.alg,
        )),
        Algorithm::RS256
        | Algorithm::RS384
        | Algorithm::RS512
        | Algorithm::PS256
        | Algorithm::PS384
        | Algorithm::PS512
        | Algorithm::ES256
        | Algorithm::ES384
        | Algorithm::EdDSA => {
            let kid = header.kid.ok_or_else(|| {
                eprintln!(
                    "[auth] token uses {:?} but does not include a kid header",
                    header.alg
                );
                AppError::Unauthorized("missing token signing key id".into())
            })?;
            let jwk = fetch_supabase_jwk(&state.config.supabase_url, &kid).await?;
            let key = DecodingKey::from_jwk(&jwk).map_err(|err| {
                eprintln!("[auth] failed to build decoding key for kid {kid}: {err}");
                AppError::Unauthorized("invalid token signing key".into())
            })?;
            Ok((key, header.alg))
        }
    }
}

pub async fn require_auth(
    State(state): State<AppState>,
    mut req: Request,
    next: Next,
) -> Result<Response, AppError> {
    if should_skip_auth(req.method(), req.uri().path()) {
        return Ok(next.run(req).await);
    }

    let method = req.method().clone();
    let path = req.uri().path().to_string();
    let auth_header = req.headers().get(header::AUTHORIZATION);

    let auth_header = match auth_header {
        Some(header) => header.to_str().map_err(|_| {
            eprintln!("[auth] invalid authorization header encoding for {method} {path}");
            AppError::Unauthorized("invalid authorization header".into())
        })?,
        None => {
            eprintln!("[auth] missing authorization header for {method} {path}");
            return Err(AppError::Unauthorized(
                "missing authorization header".into(),
            ));
        }
    };

    if !auth_header.starts_with("Bearer ") {
        eprintln!("[auth] authorization header is not bearer for {method} {path}");
        return Err(AppError::Unauthorized("expected bearer token".into()));
    }

    let token = &auth_header[7..];

    let (decoding_key, algorithm) = decoding_key_for_token(token, &state).await?;

    let mut validation = Validation::new(algorithm);
    validation.set_audience(&["authenticated"]);

    let token_data = match decode::<Claims>(
        token,
        &decoding_key,
        &validation,
    ) {
        Ok(c) => c,
        Err(err) => {
            eprintln!("[auth] jwt validation failed for {method} {path}: {err}");
            return Err(AppError::Unauthorized(
                "invalid or expired access token".into(),
            ));
        }
    };

    // Insert claims into request extensions for handlers to use
    req.extensions_mut().insert(token_data.claims);

    Ok(next.run(req).await)
}
