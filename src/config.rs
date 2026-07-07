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
    pub supabase_url: String,
    pub supabase_jwt_secret: String,
    pub supabase_webhook_secret: String,
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

    fn normalize_database_url(raw: &str) -> String {
        raw.trim()
            .trim_matches('"')
            .trim_matches('\'')
            .replace('\r', "")
            .replace('\n', "")
            .replace(' ', "")
    }

    fn validate_database_url(url: &str) -> Result<(), String> {
        let authority = url
            .strip_prefix("postgresql://")
            .or_else(|| url.strip_prefix("postgres://"))
            .ok_or_else(|| {
                "must start with postgresql:// or postgres://".to_string()
            })?;

        let at_count = authority.matches('@').count();
        if at_count != 1 {
            return Err(format!(
                "found {at_count} '@' characters in the connection string (expected 1). \
                 If your database password contains '@', '#', or '%', URL-encode it \
                 (for example, '@' becomes %40)."
            ));
        }

        let host_part = authority
            .split('@')
            .nth(1)
            .ok_or_else(|| "missing host after '@'".to_string())?
            .split('/')
            .next()
            .unwrap_or_default();

        // Extract port from host:port format
        if let Some(port_str) = host_part.rsplit(':').next() {
            // Only validate if there's actually a port part
            if !port_str.is_empty() && port_str.chars().all(|c| c.is_ascii_digit()) {
                port_str.parse::<u16>()
                    .map_err(|_| format!("invalid port '{port_str}' (use 5432 for Supabase session pooler)"))?;
            }
        }

        Ok(())
    }

    pub fn from_env() -> Self {
        Self::load_dotenv();

        let port = std::env::var("PORT")
            .ok()
            .and_then(|p| p.parse().ok())
            .unwrap_or(3000);

        let database_url = std::env::var("DATABASE_URL")
            .map(|raw| Self::normalize_database_url(&raw))
            .unwrap_or_else(|_| {
                panic!("DATABASE_URL must be set");
            });

        if let Err(reason) = Self::validate_database_url(&database_url) {
            panic!(
                "DATABASE_URL is invalid: {reason}\n\
                 Example: postgresql://postgres.<project-ref>:<url-encoded-password>@aws-0-eu-west-3.pooler.supabase.com:5432/postgres"
            );
        }

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
            
        let supabase_url = std::env::var("SUPABASE_URL")
            .unwrap_or_default()
            .trim()
            .trim_matches('"')
            .trim_matches('\'')
            .trim_end_matches('/')
            .to_string();

        let supabase_jwt_secret = std::env::var("SUPABASE_JWT_SECRET")
            .unwrap_or_else(|_| "super-secret-jwt-token-with-at-least-32-bytes-long".into());

        let supabase_webhook_secret = std::env::var("SUPABASE_WEBHOOK_SECRET").unwrap_or_default();

        Self {
            database_url,
            port,
            mpesa_callback_secret,
            mpesa_environment,
            mpesa_consumer_key,
            mpesa_consumer_secret,
            mpesa_passkey,
            mpesa_shortcode,
            supabase_url,
            supabase_jwt_secret,
            supabase_webhook_secret,
        }
    }
}

pub async fn create_pool(database_url: &str) -> Result<PgPool, sqlx::Error> {
    PgPoolOptions::new()
        .max_connections(10)
        .connect(database_url)
        .await
}
