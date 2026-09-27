use serde::{Deserialize, Serialize};

#[derive(Clone, Deserialize, Serialize)]
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

#[derive(Clone, Deserialize, Serialize)]
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


impl std::fmt::Debug for EmailConfig {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("EmailConfig")
            .field("enabled", &self.enabled)
            .field("smtp", &self.smtp)
            .field("from", &self.from)
            .field("reset_base_url", &"<redacted>")
            .finish()
    }
}

impl std::fmt::Debug for SmtpConfig {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("SmtpConfig")
            .field("host", &self.host)
            .field("port", &self.port)
            .field("username", &self.username)
            .field("password", &"<redacted>")
            .finish()
    }
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
