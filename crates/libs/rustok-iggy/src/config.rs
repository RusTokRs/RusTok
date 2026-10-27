use rustok_iggy_connector::{
    BundledConnectorConfig, ConnectorConfig, ConnectorMode, ExternalConnectorConfig,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, Clone, Default)]
pub struct IggyConfig {
    #[serde(default)]
    pub mode: IggyMode,
    #[serde(default)]
    pub serialization: SerializationFormat,
    #[serde(default)]
    pub bundled: BundledConfig,
    #[serde(default)]
    pub external: ExternalConfig,
    #[serde(default)]
    pub topology: TopologyConfig,
    #[serde(default)]
    pub retention: RetentionConfig,
}

#[derive(Debug, Deserialize, Serialize, Clone, Default, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum IggyMode {
    #[default]
    Bundled,
    External,
}

impl std::fmt::Display for IggyMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            IggyMode::Bundled => write!(f, "bundled"),
            IggyMode::External => write!(f, "external"),
        }
    }
}

#[derive(Debug, Deserialize, Serialize, Clone, Default, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum SerializationFormat {
    #[default]
    Json,
    MessagePack,
}

impl std::fmt::Display for SerializationFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SerializationFormat::Json => write!(f, "json"),
            SerializationFormat::MessagePack => write!(f, "messagepack"),
        }
    }
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct BundledConfig {
    #[serde(default = "default_local_executable")]
    pub executable: String,
    #[serde(default)]
    pub arguments: Vec<String>,
    #[serde(default)]
    pub environment: std::collections::BTreeMap<String, String>,
    pub data_dir: String,
    pub tcp_port: u16,
    pub http_port: u16,
    #[serde(default = "default_startup_timeout_ms")]
    pub startup_timeout_ms: u64,
    #[serde(default = "default_shutdown_timeout_ms")]
    pub shutdown_timeout_ms: u64,
}

impl Default for BundledConfig {
    fn default() -> Self {
        Self {
            executable: default_local_executable(),
            arguments: Vec::new(),
            environment: std::collections::BTreeMap::new(),
            data_dir: "./data/iggy".to_string(),
            tcp_port: 8090,
            http_port: 3000,
            startup_timeout_ms: default_startup_timeout_ms(),
            shutdown_timeout_ms: default_shutdown_timeout_ms(),
        }
    }
}

fn default_local_executable() -> String {
    "iggy-server".to_string()
}

const fn default_startup_timeout_ms() -> u64 {
    30_000
}

const fn default_shutdown_timeout_ms() -> u64 {
    10_000
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ExternalConfig {
    pub addresses: Vec<String>,
    pub protocol: String,
    pub username: String,
    #[serde(default)]
    pub password: String,
    #[serde(default)]
    pub tls_enabled: bool,
    #[serde(default)]
    pub tls_domain: Option<String>,
    #[serde(default)]
    pub tls_ca_file: Option<String>,
}

impl Default for ExternalConfig {
    fn default() -> Self {
        Self {
            addresses: vec!["127.0.0.1:8090".to_string()],
            protocol: "tcp".to_string(),
            username: "iggy".to_string(),
            password: "iggy".to_string(),
            tls_enabled: false,
            tls_domain: None,
            tls_ca_file: None,
        }
    }
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct TopologyConfig {
    #[serde(default = "default_stream_name")]
    pub stream_name: String,
    #[serde(default = "default_domain_partitions")]
    pub domain_partitions: u32,
    #[serde(default = "default_replication_factor")]
    pub replication_factor: u8,
}

impl Default for TopologyConfig {
    fn default() -> Self {
        Self {
            stream_name: "rustok".to_string(),
            domain_partitions: 8,
            replication_factor: 1,
        }
    }
}

fn default_stream_name() -> String {
    "rustok".to_string()
}

fn default_domain_partitions() -> u32 {
    8
}

fn default_replication_factor() -> u8 {
    1
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct RetentionConfig {
    pub domain_max_age_days: u32,
    pub domain_max_size_gb: u32,
    pub system_max_age_days: u32,
    pub dlq_max_age_days: u32,
}

impl Default for RetentionConfig {
    fn default() -> Self {
        Self {
            domain_max_age_days: 30,
            domain_max_size_gb: 10,
            system_max_age_days: 7,
            dlq_max_age_days: 365,
        }
    }
}

impl From<&IggyConfig> for ConnectorConfig {
    fn from(config: &IggyConfig) -> Self {
        let mode = match config.mode {
            IggyMode::Bundled => ConnectorMode::Bundled,
            IggyMode::External => ConnectorMode::External,
        };

        let bundled = BundledConnectorConfig {
            executable: config.bundled.executable.clone(),
            arguments: config.bundled.arguments.clone(),
            environment: config.bundled.environment.clone(),
            data_dir: config.bundled.data_dir.clone(),
            tcp_port: config.bundled.tcp_port,
            http_port: config.bundled.http_port,
            startup_timeout_ms: config.bundled.startup_timeout_ms,
            shutdown_timeout_ms: config.bundled.shutdown_timeout_ms,
        };

        let external = ExternalConnectorConfig {
            addresses: config.external.addresses.clone(),
            protocol: config.external.protocol.clone(),
            username: config.external.username.clone(),
            password: config.external.password.clone(),
            tls_enabled: config.external.tls_enabled,
            tls_domain: config.external.tls_domain.clone(),
            tls_ca_file: config.external.tls_ca_file.clone(),
        };

        ConnectorConfig {
            mode,
            bundled,
            external,
            stream_name: config.topology.stream_name.clone(),
            topic_name: "domain".to_string(),
            partitions: config.topology.domain_partitions,
            replication_factor: config.topology.replication_factor,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn iggy_config_defaults() {
        let config = IggyConfig::default();

        assert_eq!(config.mode, IggyMode::Bundled);
        assert_eq!(config.serialization, SerializationFormat::Json);
        assert_eq!(config.topology.stream_name, "rustok");
        assert_eq!(config.topology.domain_partitions, 8);
    }

    #[test]
    fn iggy_mode_display() {
        assert_eq!(IggyMode::Bundled.to_string(), "bundled");
        assert_eq!(IggyMode::External.to_string(), "external");
    }

    #[test]
    fn serialization_format_display() {
        assert_eq!(SerializationFormat::Json.to_string(), "json");
        assert_eq!(SerializationFormat::MessagePack.to_string(), "messagepack");
    }

    #[test]
    fn local_config_defaults() {
        let config = BundledConfig::default();

        assert_eq!(config.executable, "iggy-server");
        assert_eq!(config.data_dir, "./data/iggy");
        assert_eq!(config.tcp_port, 8090);
        assert_eq!(config.http_port, 3000);
    }

    #[test]
    fn remote_config_defaults() {
        let config = ExternalConfig::default();

        assert_eq!(config.addresses, vec!["127.0.0.1:8090"]);
        assert_eq!(config.protocol, "tcp");
        assert_eq!(config.username, "iggy");
        assert!(!config.tls_enabled);
    }

    #[test]
    fn config_to_connector_config_local() {
        let iggy_config = IggyConfig {
            mode: IggyMode::Bundled,
            ..Default::default()
        };

        let connector_config: ConnectorConfig = (&iggy_config).into();

        assert_eq!(connector_config.mode, ConnectorMode::Bundled);
        assert_eq!(connector_config.stream_name, "rustok");
        assert_eq!(connector_config.partitions, 8);
    }

    #[test]
    fn config_to_connector_config_remote() {
        let iggy_config = IggyConfig {
            mode: IggyMode::External,
            external: ExternalConfig {
                addresses: vec!["192.168.1.1:8090".to_string()],
                username: "admin".to_string(),
                password: "secret".to_string(),
                tls_enabled: true,
                ..Default::default()
            },
            ..Default::default()
        };

        let connector_config: ConnectorConfig = (&iggy_config).into();

        assert_eq!(connector_config.mode, ConnectorMode::External);
        assert_eq!(
            connector_config.external.addresses,
            vec!["192.168.1.1:8090"]
        );
        assert!(connector_config.external.tls_enabled);
    }

    #[test]
    fn config_serialization_roundtrip() {
        let config = IggyConfig {
            mode: IggyMode::External,
            serialization: SerializationFormat::MessagePack,
            topology: TopologyConfig {
                stream_name: "custom-stream".to_string(),
                domain_partitions: 16,
                replication_factor: 3,
            },
            ..Default::default()
        };

        let json = serde_json::to_string(&config).unwrap();
        let parsed: IggyConfig = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.mode, IggyMode::External);
        assert_eq!(parsed.serialization, SerializationFormat::MessagePack);
        assert_eq!(parsed.topology.stream_name, "custom-stream");
        assert_eq!(parsed.topology.domain_partitions, 16);
    }
}
