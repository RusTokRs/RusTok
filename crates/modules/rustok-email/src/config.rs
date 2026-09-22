use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct EmailConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub smtp: SmtpConfig,
    #[serde(default = "default_from")]
    pub from: String,
    #[serde(default = "default_reset_base_url")]
    pub reset_base_url: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SmtpConfig {
    #[serde(default = "default_smtp_host")]
    pub host: String,
    #[serde(default = "default_smtp_port")]
    pub port: u16,
    #[serde(default)]
    pub username: String,
    #[serde(default)]
    pub password: String,
}

impl Default for EmailConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            smtp: SmtpConfig::default(),
            from: default_from(),
            reset_base_url: default_reset_base_url(),
        }
    }
}

impl Default for SmtpConfig {
    fn default() -> Self {
        Self {
            host: default_smtp_host(),
            port: default_smtp_port(),
            username: String::new(),
            password: String::new(),
        }
    }
}

fn default_from() -> String {
    "no-reply@rustok.local".to_string()
}

fn default_reset_base_url() -> String {
    "http://localhost:3000/reset-password".to_string()
}

fn default_smtp_host() -> String {
    "localhost".to_string()
}

fn default_smtp_port() -> u16 {
    1025
}
