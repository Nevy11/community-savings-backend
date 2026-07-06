mod config;
mod error;
mod handlers;
mod middleware;
mod models;
mod services;

use std::net::SocketAddr;

use axum::{routing::get, Router};

use config::{create_pool, AppConfig};

#[derive(Clone)]
pub struct AppState {
    pub pool: sqlx::PgPool,
    pub config: AppConfig,
}

#[tokio::main]
async fn main() {
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

    let app = Router::new()
        .route("/ping", get(|| async { "pong" }))
        .route("/health", get(|| async { JsonHealth::ok() }))
        .nest("/api", handlers::routes(state.clone()))
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
