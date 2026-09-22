//! Configuration Management
//!
//! Provides utilities for loading and managing application configuration.
//!
//! # Features
//!
//! - **Environment-based config**: Load from environment variables
//! - **File-based config**: Load from YAML/JSON/TOML files
//! - **Validation**: Built-in validation for configuration values
//! - **Secrets handling**: Secure handling of sensitive configuration
//! - **Hot reload**: Runtime configuration updates
//!
//! # Example
//!
//! ```ignore
//! use rustok_core::config::{ConfigLoader, ConfigSource, AppConfig};
//!
//! let config = ConfigLoader::new()
//!     .with_source(ConfigSource::File("config.yaml"))
//!     .with_source(ConfigSource::Env)
//!     .load::<AppConfig>()?;
//! ```

use std::collections::HashMap;
use std::env;
use std::fmt;

/// Configuration source
#[derive(Debug, Clone)]
pub enum ConfigSource {
    /// Load from environment variables with optional prefix
    Env { prefix: Option<String> },
    /// Load from a file (format detected by extension)
    File(String),
    /// Load from a string (format specified)
    String {
        content: String,
        format: ConfigFormat,
    },
    /// In-memory configuration
    Memory(HashMap<String, String>),
}

impl ConfigSource {
    /// Create an environment source with no prefix
    pub fn env() -> Self {
        Self::Env { prefix: None }
    }

    /// Create an environment source with a prefix
    pub fn env_with_prefix(prefix: impl Into<String>) -> Self {
        Self::Env {
            prefix: Some(prefix.into()),
        }
    }

    /// Create a file source
    pub fn file(path: impl Into<String>) -> Self {
        Self::File(path.into())
    }
}

/// Configuration file format
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigFormat {
    Yaml,
    Json,
    Toml,
}

impl ConfigFormat {
    /// Detect format from file extension
    pub fn from_extension(path: &str) -> Option<Self> {
        let lower = path.to_lowercase();
        if lower.ends_with(".yaml") || lower.ends_with(".yml") {
            Some(Self::Yaml)
        } else if lower.ends_with(".json") {
            Some(Self::Json)
        } else if lower.ends_with(".toml") {
            Some(Self::Toml)
        } else {
            None
        }
    }
}

impl fmt::Display for ConfigFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConfigFormat::Yaml => write!(f, "YAML"),
            ConfigFormat::Json => write!(f, "JSON"),
            ConfigFormat::Toml => write!(f, "TOML"),
        }
    }
}

/// Configuration loader
#[derive(Debug, Default)]
pub struct ConfigLoader {
    sources: Vec<ConfigSource>,
}

impl ConfigLoader {
    /// Create a new config loader
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a configuration source
    pub fn with_source(mut self, source: ConfigSource) -> Self {
        self.sources.push(source);
        self
    }

    /// Add multiple configuration sources
    pub fn with_sources(mut self, sources: impl IntoIterator<Item = ConfigSource>) -> Self {
        self.sources.extend(sources);
        self
    }

    /// Load configuration into a type that implements Config
    pub fn load<T: Config>(self) -> Result<T, ConfigError> {
        let mut merged = HashMap::new();

        for source in &self.sources {
            let values = match source {
                ConfigSource::Env { prefix } => load_from_env(prefix.as_deref()),
                ConfigSource::File(path) => load_from_file(path),
                ConfigSource::String { content, format } => parse_content(content, *format),
                ConfigSource::Memory(values) => Ok(values.clone()),
            }?;

            // Merge values (later sources override earlier ones)
            merged.extend(values);
        }

        T::from_map(merged)
    }

    /// Load configuration as a raw HashMap
    pub fn load_raw(self) -> Result<HashMap<String, String>, ConfigError> {
        let mut merged = HashMap::new();

        for source in &self.sources {
            let values = match source {
                ConfigSource::Env { prefix } => load_from_env(prefix.as_deref()),
                ConfigSource::File(path) => load_from_file(path),
                ConfigSource::String { content, format } => parse_content(content, *format),
                ConfigSource::Memory(values) => Ok(values.clone()),
            }?;

            merged.extend(values);
        }

        Ok(merged)
    }
}

/// Trait for types that can be loaded from configuration
pub trait Config: Sized {
    /// Load from a key-value map
    fn from_map(map: HashMap<String, String>) -> Result<Self, ConfigError>;

    /// Validate the configuration
    fn validate(&self) -> Result<(), ConfigError>;
}

/// Configuration error
#[derive(Debug, Clone)]
pub enum ConfigError {
    /// Missing required key
    MissingKey(String),
    /// Invalid value for key
    InvalidValue {
        key: String,
        value: String,
        reason: String,
    },
    /// Failed to parse file
    ParseError {
        source: String,
        format: ConfigFormat,
        message: String,
    },
    /// Failed to read file
    ReadError { path: String, message: String },
    /// Environment variable error
    EnvError(String),
    /// Validation error
    ValidationError(String),
    /// Other error
    Other(String),
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConfigError::MissingKey(key) => {
                write!(f, "Missing required configuration key: {}", key)
            }
            ConfigError::InvalidValue { key, value, reason } => {
                write!(f, "Invalid value '{}' for key '{}': {}", value, key, reason)
            }
            ConfigError::ParseError {
                source,
                format,
                message,
            } => {
                write!(
                    f,
                    "Failed to parse {} config from '{}': {}",
                    format, source, message
                )
            }
            ConfigError::ReadError { path, message } => {
                write!(f, "Failed to read config file '{}': {}", path, message)
            }
            ConfigError::EnvError(msg) => write!(f, "Environment error: {}", msg),
            ConfigError::ValidationError(msg) => write!(f, "Validation error: {}", msg),
            ConfigError::Other(msg) => write!(f, "Configuration error: {}", msg),
        }
    }
}

impl std::error::Error for ConfigError {}

/// Load configuration from environment variables
fn load_from_env(prefix: Option<&str>) -> Result<HashMap<String, String>, ConfigError> {
    let mut result = HashMap::new();
    let prefix = prefix.map(|p| format!("{}_", p.to_uppercase()));

    for (key, value) in env::vars() {
        if let Some(ref p) = prefix {
            if let Some(stripped) = key.strip_prefix(p) {
                result.insert(stripped.to_lowercase().replace('_', "."), value);
            }
        } else {
            result.insert(key.to_lowercase().replace('_', "."), value);
        }
    }

    Ok(result)
}

/// Load configuration from a file
fn load_from_file(path: &str) -> Result<HashMap<String, String>, ConfigError> {
    let format = ConfigFormat::from_extension(path).ok_or_else(|| {
        ConfigError::Other(format!("Cannot detect config format from path: {}", path))
    })?;

    let content = std::fs::read_to_string(path).map_err(|e| ConfigError::ReadError {
        path: path.to_string(),
        message: e.to_string(),
    })?;

    parse_content(&content, format)
}

/// Parse configuration content
fn parse_content(
    content: &str,
    format: ConfigFormat,
) -> Result<HashMap<String, String>, ConfigError> {
    match format {
        ConfigFormat::Json => parse_json(content),
        ConfigFormat::Yaml => parse_yaml(content),
        ConfigFormat::Toml => parse_toml(content),
    }
}

/// Parse JSON content
fn parse_json(content: &str) -> Result<HashMap<String, String>, ConfigError> {
    let value: serde_json::Value =
        serde_json::from_str(content).map_err(|e| ConfigError::ParseError {
            source: "JSON".to_string(),
            format: ConfigFormat::Json,
            message: e.to_string(),
        })?;

    flatten_json(&value, "")
}

/// Parse YAML content
fn parse_yaml(content: &str) -> Result<HashMap<String, String>, ConfigError> {
    let value: serde_yaml::Value =
        serde_yaml::from_str(content).map_err(|e| ConfigError::ParseError {
            source: "YAML".to_string(),
            format: ConfigFormat::Yaml,
            message: e.to_string(),
        })?;

    flatten_yaml(&value, "")
}

/// Parse TOML content
fn parse_toml(content: &str) -> Result<HashMap<String, String>, ConfigError> {
    let value: toml::Value =
        content
            .parse()
            .map_err(|e: toml::de::Error| ConfigError::ParseError {
                source: "TOML".to_string(),
                format: ConfigFormat::Toml,
                message: e.to_string(),
            })?;

    flatten_toml(&value, "")
}

/// Flatten JSON value into dot-notation keys
fn flatten_json(
    value: &serde_json::Value,
    prefix: &str,
) -> Result<HashMap<String, String>, ConfigError> {
    let mut result = HashMap::new();

    match value {
        serde_json::Value::Object(map) => {
            for (key, val) in map {
                let new_prefix = if prefix.is_empty() {
                    key.clone()
                } else {
                    format!("{}.{}", prefix, key)
                };

                match val {
                    serde_json::Value::String(s) => {
                        result.insert(new_prefix, s.clone());
                    }
                    serde_json::Value::Number(n) => {
                        result.insert(new_prefix, n.to_string());
                    }
                    serde_json::Value::Bool(b) => {
                        result.insert(new_prefix, b.to_string());
                    }
                    _ => {
                        result.extend(flatten_json(val, &new_prefix)?);
                    }
                }
            }
        }
        serde_json::Value::String(s) => {
            result.insert(prefix.to_string(), s.clone());
        }
        serde_json::Value::Number(n) => {
            result.insert(prefix.to_string(), n.to_string());
        }
        serde_json::Value::Bool(b) => {
            result.insert(prefix.to_string(), b.to_string());
        }
        _ => {}
    }

    Ok(result)
}

/// Flatten YAML value into dot-notation keys
fn flatten_yaml(
    value: &serde_yaml::Value,
    prefix: &str,
) -> Result<HashMap<String, String>, ConfigError> {
    let mut result = HashMap::new();

    match value {
        serde_yaml::Value::Mapping(map) => {
            for (key, val) in map {
                let key_str = key.as_str().unwrap_or("");
                let new_prefix = if prefix.is_empty() {
                    key_str.to_string()
                } else {
                    format!("{}.{}", prefix, key_str)
                };

                match val {
                    serde_yaml::Value::String(s) => {
                        result.insert(new_prefix, s.clone());
                    }
                    serde_yaml::Value::Number(n) => {
                        result.insert(new_prefix, n.to_string());
                    }
                    serde_yaml::Value::Bool(b) => {
                        result.insert(new_prefix, b.to_string());
                    }
                    _ => {
                        result.extend(flatten_yaml(val, &new_prefix)?);
                    }
                }
            }
        }
        serde_yaml::Value::String(s) => {
            result.insert(prefix.to_string(), s.clone());
        }
        serde_yaml::Value::Number(n) => {
            result.insert(prefix.to_string(), n.to_string());
        }
        serde_yaml::Value::Bool(b) => {
            result.insert(prefix.to_string(), b.to_string());
        }
        _ => {}
    }

    Ok(result)
}

/// Flatten TOML value into dot-notation keys
fn flatten_toml(value: &toml::Value, prefix: &str) -> Result<HashMap<String, String>, ConfigError> {
    let mut result = HashMap::new();

    match value {
        toml::Value::Table(table) => {
            for (key, val) in table {
                let new_prefix = if prefix.is_empty() {
                    key.clone()
                } else {
                    format!("{}.{}", prefix, key)
                };

                match val {
                    toml::Value::String(s) => {
                        result.insert(new_prefix, s.clone());
                    }
                    toml::Value::Integer(n) => {
                        result.insert(new_prefix, n.to_string());
                    }
                    toml::Value::Float(n) => {
                        result.insert(new_prefix, n.to_string());
                    }
                    toml::Value::Boolean(b) => {
                        result.insert(new_prefix, b.to_string());
                    }
                    _ => {
                        result.extend(flatten_toml(val, &new_prefix)?);
                    }
                }
            }
        }
        toml::Value::String(s) => {
            result.insert(prefix.to_string(), s.clone());
        }
        toml::Value::Integer(n) => {
            result.insert(prefix.to_string(), n.to_string());
        }
        toml::Value::Float(n) => {
            result.insert(prefix.to_string(), n.to_string());
        }
        toml::Value::Boolean(b) => {
            result.insert(prefix.to_string(), b.to_string());
        }
        _ => {}
    }

    Ok(result)
}

/// Configuration value helper
pub struct ConfigValue {
    key: String,
    value: Option<String>,
}

impl ConfigValue {
    /// Create a new config value
    pub fn new(key: impl Into<String>, value: Option<String>) -> Self {
        Self {
            key: key.into(),
            value,
        }
    }

    /// Get the value as a string
    pub fn as_string(self) -> Result<String, ConfigError> {
        self.value.ok_or(ConfigError::MissingKey(self.key))
    }

    /// Get the value as an integer
    pub fn as_i64(self) -> Result<i64, ConfigError> {
        let key = self.key.clone();
        let s = self.as_string()?;
        s.parse::<i64>().map_err(|e| ConfigError::InvalidValue {
            key,
            value: s,
            reason: e.to_string(),
        })
    }

    /// Get the value as a boolean
    pub fn as_bool(self) -> Result<bool, ConfigError> {
        let key = self.key.clone();
        let s = self.as_string()?;
        match s.to_lowercase().as_str() {
            "true" | "yes" | "1" | "on" => Ok(true),
            "false" | "no" | "0" | "off" => Ok(false),
            _ => Err(ConfigError::InvalidValue {
                key,
                value: s,
                reason: "Expected boolean (true/false)".to_string(),
            }),
        }
    }

    /// Get the value with a default
    pub fn or_default(self, default: impl Into<String>) -> String {
        self.value.unwrap_or_else(|| default.into())
    }

    /// Get the value or None
    pub fn optional(self) -> Option<String> {
        self.value
    }
}

/// Secret value that masks its content in debug output
#[derive(Clone)]
pub struct Secret {
    value: String,
}

impl Secret {
    /// Create a new secret
    pub fn new(value: impl Into<String>) -> Self {
        Self {
            value: value.into(),
        }
    }

    /// Get the secret value
    pub fn expose(&self) -> &str {
        &self.value
    }

    /// Create from environment variable
    pub fn from_env(key: impl AsRef<str>) -> Result<Self, ConfigError> {
        let value = env::var(key.as_ref())
            .map_err(|_| ConfigError::MissingKey(key.as_ref().to_string()))?;
        Ok(Self::new(value))
    }
}

impl fmt::Debug for Secret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Secret(<redacted>)")
    }
}

impl fmt::Display for Secret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "<redacted>")
    }
}

/// Application configuration trait
pub trait AppConfig: Config {
    /// Application name
    fn app_name() -> &'static str;

    /// Default configuration values
    fn defaults() -> HashMap<String, String>;

    /// Load with defaults and environment
    fn load_with_defaults() -> Result<Self, ConfigError> {
        let mut defaults = Self::defaults();
        let env = load_from_env(Some(Self::app_name()))?;

        defaults.extend(env);
        Self::from_map(defaults)
    }
}

/// Common database configuration
#[derive(Debug, Clone)]
pub struct DatabaseConfig {
    pub url: Secret,
    pub pool_size: u32,
    pub timeout_seconds: u64,
    pub enable_logging: bool,
}

impl Config for DatabaseConfig {
    fn from_map(map: HashMap<String, String>) -> Result<Self, ConfigError> {
        let url = ConfigValue::new("database.url", map.get("database.url").cloned()).as_string()?;

        let pool_size =
            ConfigValue::new("database.pool_size", map.get("database.pool_size").cloned())
                .as_string()
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(10);

        let timeout_seconds = ConfigValue::new(
            "database.timeout_seconds",
            map.get("database.timeout_seconds").cloned(),
        )
        .as_string()
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(30);

        let enable_logging = ConfigValue::new(
            "database.enable_logging",
            map.get("database.enable_logging").cloned(),
        )
        .as_bool()
        .unwrap_or(false);

        let config = Self {
            url: Secret::new(url),
            pool_size,
            timeout_seconds,
            enable_logging,
        };

        config.validate()?;
        Ok(config)
    }

    fn validate(&self) -> Result<(), ConfigError> {
        if self.url.expose().is_empty() {
            return Err(ConfigError::ValidationError(
                "Database URL cannot be empty".to_string(),
            ));
        }
        if self.pool_size == 0 {
            return Err(ConfigError::ValidationError(
                "Database pool size must be greater than 0".to_string(),
            ));
        }
        Ok(())
    }
}

/// Common server configuration
#[derive(Debug, Clone)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
    pub workers: usize,
    pub request_timeout: u64,
    pub keep_alive: bool,
}

impl Config for ServerConfig {
    fn from_map(map: HashMap<String, String>) -> Result<Self, ConfigError> {
        let host =
            ConfigValue::new("server.host", map.get("server.host").cloned()).or_default("0.0.0.0");

        let port = ConfigValue::new("server.port", map.get("server.port").cloned())
            .as_string()
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(8080);

        let workers = ConfigValue::new("server.workers", map.get("server.workers").cloned())
            .as_string()
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(4);

        let request_timeout = ConfigValue::new(
            "server.request_timeout",
            map.get("server.request_timeout").cloned(),
        )
        .as_string()
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(30);

        let keep_alive =
            ConfigValue::new("server.keep_alive", map.get("server.keep_alive").cloned())
                .as_bool()
                .unwrap_or(true);

        let config = Self {
            host,
            port,
            workers,
            request_timeout,
            keep_alive,
        };

        config.validate()?;
        Ok(config)
    }

    fn validate(&self) -> Result<(), ConfigError> {
        if self.port == 0 {
            return Err(ConfigError::ValidationError(
                "Server port cannot be 0".to_string(),
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_source_env() {
        let source = ConfigSource::env();
        assert!(matches!(source, ConfigSource::Env { prefix: None }));

        let source = ConfigSource::env_with_prefix("APP");
        assert!(matches!(source, ConfigSource::Env { prefix: Some(p) } if p == "APP"));
    }

    #[test]
    fn test_config_format_from_extension() {
        assert_eq!(
            ConfigFormat::from_extension("config.yaml"),
            Some(ConfigFormat::Yaml)
        );
        assert_eq!(
            ConfigFormat::from_extension("config.yml"),
            Some(ConfigFormat::Yaml)
        );
        assert_eq!(
            ConfigFormat::from_extension("config.json"),
            Some(ConfigFormat::Json)
        );
        assert_eq!(
            ConfigFormat::from_extension("config.toml"),
            Some(ConfigFormat::Toml)
        );
        assert_eq!(ConfigFormat::from_extension("config.txt"), None);
    }

    #[test]
    fn test_parse_json() {
        let json = r#"{"database": {"url": "postgres://localhost", "port": 5432}}"#;
        let result = parse_json(json).unwrap();

        assert_eq!(
            result.get("database.url"),
            Some(&"postgres://localhost".to_string())
        );
        assert_eq!(result.get("database.port"), Some(&"5432".to_string()));
    }

    #[test]
    fn test_config_value() {
        let value = ConfigValue::new("key", Some("value".to_string()));
        assert_eq!(value.as_string().unwrap(), "value");

        let value = ConfigValue::new("key", Some("123".to_string()));
        assert_eq!(value.as_i64().unwrap(), 123);

        let value = ConfigValue::new("key", Some("true".to_string()));
        assert!(value.as_bool().unwrap());

        let value = ConfigValue::new("key", Some("yes".to_string()));
        assert!(value.as_bool().unwrap());

        let value = ConfigValue::new("key", None::<String>);
        assert!(value.as_string().is_err());
    }

    #[test]
    fn test_secret() {
        let secret = Secret::new("my-secret");
        assert_eq!(secret.expose(), "my-secret");

        let debug = format!("{:?}", secret);
        assert!(debug.contains("<redacted>"));
    }

    #[test]
    fn test_database_config() {
        let mut map = HashMap::new();
        map.insert(
            "database.url".to_string(),
            "postgres://localhost/db".to_string(),
        );
        map.insert("database.pool_size".to_string(), "20".to_string());

        let config = DatabaseConfig::from_map(map).unwrap();
        assert_eq!(config.pool_size, 20);
        assert_eq!(config.timeout_seconds, 30); // default
        assert!(!config.enable_logging); // default
    }

    #[test]
    fn test_database_config_validation() {
        let mut map = HashMap::new();
        map.insert("database.url".to_string(), "".to_string());

        let result = DatabaseConfig::from_map(map);
        assert!(result.is_err());
    }

    #[test]
    fn test_server_config() {
        let mut map = HashMap::new();
        map.insert("server.host".to_string(), "127.0.0.1".to_string());
        map.insert("server.port".to_string(), "3000".to_string());

        let config = ServerConfig::from_map(map).unwrap();
        assert_eq!(config.host, "127.0.0.1");
        assert_eq!(config.port, 3000);
    }
}
