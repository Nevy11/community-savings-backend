use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;

#[derive(Clone, Debug)]
pub struct AppConfig {
    pub database_url: String,
    pub port: u16,
    pub mpesa_callback_secret: String,
}

impl AppConfig {
    pub fn from_env() -> Self {
        let port = std::env::var("PORT")
            .ok()
            .and_then(|p| p.parse().ok())
            .unwrap_or(3000);

        let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");

        let mpesa_callback_secret = std::env::var("MPESA_CALLBACK_SECRET")
            .unwrap_or_else(|_| "dev-mpesa-secret-change-in-production".into());

        Self {
            database_url,
            port,
            mpesa_callback_secret,
        }
    }
}

pub async fn create_pool(database_url: &str) -> Result<PgPool, sqlx::Error> {
    PgPoolOptions::new()
        .max_connections(10)
        .connect(database_url)
        .await
}
