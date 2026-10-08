use serde::{Deserialize, Serialize};

pub const DEFAULT_CACHE_MODE: &str = "in-memory";
pub const DEFAULT_REDIS_HOST: &str = "127.0.0.1";
pub const DEFAULT_REDIS_PORT: u16 = 6379;
pub const DEFAULT_REDIS_DB: u32 = 0;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CacheSettings {
    #[serde(default = "default_mode")]
    pub mode: String,
    #[serde(default = "default_host")]
    pub redis_host: String,
    #[serde(default = "default_port")]
    pub redis_port: u16,
    #[serde(default)]
    pub redis_password: String,
    #[serde(default)]
    pub redis_db: u32,
    #[serde(default)]
    pub redis_url: String,
}

fn default_mode() -> String {
    DEFAULT_CACHE_MODE.to_string()
}

fn default_host() -> String {
    DEFAULT_REDIS_HOST.to_string()
}

fn default_port() -> u16 {
    DEFAULT_REDIS_PORT
}

impl Default for CacheSettings {
    fn default() -> Self {
        Self {
            mode: default_mode(),
            redis_host: default_host(),
            redis_port: default_port(),
            redis_password: String::new(),
            redis_db: DEFAULT_REDIS_DB,
            redis_url: String::new(),
        }
    }
}

impl CacheSettings {
    pub fn validate(&self) -> Result<(), Vec<String>> {
        let mut errors = Vec::new();
        let valid_modes = ["in-memory", "redis", "hybrid"];
        if !valid_modes.contains(&self.mode.as_str()) {
            errors.push(format!(
                "Invalid cache mode '{}'. Must be one of: {}",
                self.mode,
                valid_modes.join(", ")
            ));
        }

        if (self.mode == "redis" || self.mode == "hybrid") && self.redis_port == 0 {
            errors.push("redis_port must be greater than 0".to_string());
        }

        if self.redis_db > 255 {
            errors.push("redis_db must be between 0 and 255".to_string());
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    pub fn redacted_for_client(&self) -> Self {
        let mut clone = self.clone();
        if !clone.redis_password.is_empty() {
            clone.redis_password = "********".to_string();
        }
        clone
    }

    pub fn merge_with_existing(&mut self, existing: &Self) {
        if self.redis_password.is_empty() || self.redis_password == "********" {
            self.redis_password = existing.redis_password.clone();
        }
    }
}
