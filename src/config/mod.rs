#![allow(dead_code)]

#[derive(Debug, Clone)]
pub struct Config {
    pub log_level: String,
}

impl Config {
    pub fn from_env() -> anyhow::Result<Self> {
        // dotenvy loads .env, but doesn't override already-set env vars
        dotenvy::dotenv().ok();

        let log_level = std::env::var("RUST_LOG").unwrap_or_else(|_| "info".to_string());

        Ok(Self { log_level })
    }
}
