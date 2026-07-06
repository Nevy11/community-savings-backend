use axum::{
    extract::{Request, State},
    http::{header, StatusCode},
    middleware::Next,
    response::Response,
};
use jsonwebtoken::{decode, DecodingKey, Validation, Algorithm};
use serde::{Deserialize, Serialize};

use crate::AppState;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct UserMetadata {
    pub full_name: Option<String>,
    pub name: Option<String>,
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
        self.user_metadata.as_ref().and_then(|meta| {
            meta.full_name
                .clone()
                .or_else(|| meta.name.clone())
        })
    }
}

pub async fn require_auth(
    State(state): State<AppState>,
    mut req: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    let auth_header = req.headers().get(header::AUTHORIZATION);
    
    let auth_header = match auth_header {
        Some(header) => header.to_str().map_err(|_| StatusCode::UNAUTHORIZED)?,
        None => return Err(StatusCode::UNAUTHORIZED),
    };

    if !auth_header.starts_with("Bearer ") {
        return Err(StatusCode::UNAUTHORIZED);
    }

    let token = &auth_header[7..];

    let mut validation = Validation::new(Algorithm::HS256);
    validation.set_audience(&["authenticated"]);

    let token_data = match decode::<Claims>(
        token,
        &DecodingKey::from_secret(state.config.supabase_jwt_secret.as_bytes()),
        &validation,
    ) {
        Ok(c) => c,
        Err(_) => return Err(StatusCode::UNAUTHORIZED),
    };

    // Insert claims into request extensions for handlers to use
    req.extensions_mut().insert(token_data.claims);

    Ok(next.run(req).await)
}
