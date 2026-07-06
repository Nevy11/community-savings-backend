use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;
use std::path::PathBuf;

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
    pub supabase_jwt_secret: String,
}

impl AppConfig {
    fn load_dotenv() {
        let env_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(".env");
        if env_path.exists() {
            dotenvy::from_path(&env_path).ok();
        } else {
            dotenvy::dotenv().ok();
        }
    }

    pub fn from_env() -> Self {
        Self::load_dotenv();

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
            
        let supabase_jwt_secret = std::env::var("SUPABASE_JWT_SECRET")
            .unwrap_or_else(|_| "super-secret-jwt-token-with-at-least-32-bytes-long".into());

        Self {
            database_url,
            port,
            mpesa_callback_secret,
            mpesa_environment,
            mpesa_consumer_key,
            mpesa_consumer_secret,
            mpesa_passkey,
            mpesa_shortcode,
            supabase_jwt_secret,
        }
    }
}

pub async fn create_pool(database_url: &str) -> Result<PgPool, sqlx::Error> {
    PgPoolOptions::new()
        .max_connections(10)
        .connect(database_url)
        .await
}
