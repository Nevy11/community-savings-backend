mod config;
mod error;
mod handlers;
mod middleware;
mod models;
mod services;

use std::net::SocketAddr;

use axum::{routing::get, Router};
use std::convert::Infallible;
use axum::http::Method;
use axum::http::Request;
use axum::body::Body;
use axum::middleware::Next;
use axum::response::{Response, IntoResponse};
use axum::http::HeaderValue;

use config::{create_pool, AppConfig};

#[derive(Clone)]
pub struct AppState {
    pub pool: sqlx::PgPool,
    pub config: AppConfig,
}

#[tokio::main]
async fn main() {
    let _ = jsonwebtoken::crypto::rust_crypto::DEFAULT_PROVIDER.install_default();

    let config = AppConfig::from_env();
    let pool = create_pool(&config.database_url)
        .await
        .unwrap_or_else(|err| {
            panic!(
                "failed to connect to database: {err}\n\
                 Check DATABASE_URL on Render — password must be URL-encoded and port must be 5432."
            );
        });

    let state = AppState {
        pool,
        config: config.clone(),
    };

    async fn cors(req: Request<Body>, next: Next) -> Result<Response, Infallible> {
        let origin = req
            .headers()
            .get("origin")
            .and_then(|value| value.to_str().ok())
            .unwrap_or("http://localhost:4200");

        let allow_origin = if origin.starts_with("http://localhost:")
            || origin.starts_with("http://127.0.0.1:")
            || origin.contains(".netlify.app")
            || origin.contains(".vercel.app")
        {
            origin.to_string()
        } else {
            "http://localhost:4200".to_string()
        };

        if req.method() == Method::OPTIONS {
            let mut res = ("").into_response();
            let headers = res.headers_mut();
            headers.insert(
                "Access-Control-Allow-Origin",
                HeaderValue::from_str(&allow_origin).unwrap_or_else(|_| HeaderValue::from_static("http://localhost:4200")),
            );
            headers.insert(
                "Access-Control-Allow-Methods",
                HeaderValue::from_static("GET,POST,PUT,PATCH,DELETE,OPTIONS"),
            );
            headers.insert(
                "Access-Control-Allow-Headers",
                HeaderValue::from_static("Authorization,Content-Type,X-Requested-With"),
            );
            headers.insert("Access-Control-Allow-Credentials", HeaderValue::from_static("true"));
            return Ok(res);
        }

        let mut response = next.run(req).await;
        let headers = response.headers_mut();
        headers.insert(
            "Access-Control-Allow-Origin",
            HeaderValue::from_str(&allow_origin).unwrap_or_else(|_| HeaderValue::from_static("http://localhost:4200")),
        );
        headers.insert("Access-Control-Allow-Credentials", HeaderValue::from_static("true"));
        Ok(response)
    }

    let app = Router::new()
        .route("/ping", get(|| async { "pong" }))
        .route("/health", get(|| async { JsonHealth::ok() }))
        .nest("/api", handlers::routes(state.clone()))
        .layer(axum::middleware::from_fn(cors))
        .with_state(state);

    let addr = SocketAddr::from(([0, 0, 0, 0], config.port));
    println!("Listening on http://{}", addr);

    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

#[derive(serde::Serialize)]
struct JsonHealth {
    status: &'static str,
}

impl JsonHealth {
    fn ok() -> axum::Json<Self> {
        axum::Json(Self { status: "ok" })
    }
}
