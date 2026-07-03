use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;

#[derive(Clone, Debug)]
pub struct AppConfig {
    pub database_url: String,
    pub port: u16,
    pub mpesa_callback_secret: String,
    pub mpesa_environment: String,
    pub mpesa_consumer_key: String,
    pub mpesa_consumer_secret: String,
    pub mpesa_passkey: String,
    pub mpesa_shortcode: String,
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

        let mpesa_environment = std::env::var("MPESA_ENVIRONMENT")
            .unwrap_or_else(|_| "sandbox".into());
            
        let mpesa_consumer_key = std::env::var("MPESA_CONSUMER_KEY")
            .unwrap_or_default();
            
        let mpesa_consumer_secret = std::env::var("MPESA_CONSUMER_SECRET")
            .unwrap_or_default();
            
        let mpesa_passkey = std::env::var("MPESA_PASSKEY")
            .unwrap_or_default();
            
        let mpesa_shortcode = std::env::var("MPESA_SHORTCODE")
            .unwrap_or_default();

        Self {
            database_url,
            port,
            mpesa_callback_secret,
            mpesa_environment,
            mpesa_consumer_key,
            mpesa_consumer_secret,
            mpesa_passkey,
            mpesa_shortcode,
        }
    }
}

pub async fn create_pool(database_url: &str) -> Result<PgPool, sqlx::Error> {
    PgPoolOptions::new()
        .max_connections(10)
        .connect(database_url)
        .await
}
