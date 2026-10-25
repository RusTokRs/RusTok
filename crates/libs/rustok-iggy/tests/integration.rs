//! Integration tests for rustok-iggy module.
//!
//! These tests require a running Iggy backend or use mock implementations.

use rustok_iggy::config::{IggyConfig, IggyMode, SerializationFormat};
use rustok_iggy::transport::IggyTransport;

type TestResult<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

#[tokio::test]
#[ignore = "Integration test requires running iggy backend"]
async fn test_iggy_transport_lifecycle() -> TestResult<()> {
    let config = IggyConfig {
        mode: IggyMode::Bundled,
        serialization: SerializationFormat::Json,
        ..IggyConfig::default()
    };

    let transport = IggyTransport::new(config).await?;
    transport.shutdown().await?;

    Ok(())
}

#[tokio::test]
#[ignore = "Integration test requires running iggy backend"]
async fn test_iggy_transport_remote_mode() -> TestResult<()> {
    let config = IggyConfig {
        mode: IggyMode::External,
        serialization: SerializationFormat::Json,
        ..IggyConfig::default()
    };

    let transport = IggyTransport::new(config).await?;
    assert!(!transport.is_connected()); // Depends on whether server is running
    transport.shutdown().await?;

    Ok(())
}

#[tokio::test]
#[ignore = "Integration test requires running iggy backend"]
async fn test_iggy_transport_message_pack() -> TestResult<()> {
    let config = IggyConfig {
        mode: IggyMode::Bundled,
        serialization: SerializationFormat::MessagePack,
        ..IggyConfig::default()
    };

    let transport = IggyTransport::new(config).await?;
    transport.shutdown().await?;

    Ok(())
}

mod config_tests {
    use rustok_iggy::config::{
        BundledConfig, ExternalConfig, IggyConfig, IggyMode, RetentionConfig, SerializationFormat,
        TopologyConfig,
    };

    #[test]
    fn config_serialization_roundtrip() {
        let original = IggyConfig {
            mode: IggyMode::External,
            serialization: SerializationFormat::MessagePack,
            bundled: BundledConfig {
                executable: "iggy-server".to_string(),
                arguments: Vec::new(),
                environment: Default::default(),
                data_dir: "/custom/data".to_string(),
                tcp_port: 9000,
                http_port: 4000,
                startup_timeout_ms: 1_000,
                shutdown_timeout_ms: 1_000,
            },
            external: ExternalConfig {
                addresses: vec!["10.0.0.1:8090".to_string(), "10.0.0.2:8090".to_string()],
                protocol: "http".to_string(),
                username: "admin".to_string(),
                password: "secret".to_string(),
                tls_enabled: true,
                tls_domain: Some("iggy.internal".to_string()),
                tls_ca_file: Some("/etc/iggy/ca.pem".to_string()),
            },
            topology: TopologyConfig {
                stream_name: "production".to_string(),
                domain_partitions: 32,
                replication_factor: 3,
            },
            retention: RetentionConfig {
                domain_max_age_days: 90,
                domain_max_size_gb: 100,
                system_max_age_days: 30,
                dlq_max_age_days: 365,
            },
        };

        let json = serde_json::to_string(&original).unwrap();
        let parsed: IggyConfig = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.mode, IggyMode::External);
        assert_eq!(parsed.serialization, SerializationFormat::MessagePack);
        assert_eq!(parsed.topology.stream_name, "production");
        assert_eq!(parsed.topology.domain_partitions, 32);
        assert_eq!(parsed.external.addresses.len(), 2);
        assert!(parsed.external.tls_enabled);
        assert_eq!(parsed.external.tls_domain.as_deref(), Some("iggy.internal"));
        assert_eq!(
            parsed.external.tls_ca_file.as_deref(),
            Some("/etc/iggy/ca.pem")
        );
    }

    #[test]
    fn config_yaml_parsing() {
        let yaml = r#"
mode: external
serialization: json
topology:
  stream_name: test-stream
  domain_partitions: 16
"#;

        let parsed: IggyConfig = serde_yaml::from_str(yaml).unwrap();

        assert_eq!(parsed.mode, IggyMode::External);
        assert_eq!(parsed.serialization, SerializationFormat::Json);
        assert_eq!(parsed.topology.stream_name, "test-stream");
        assert_eq!(parsed.topology.domain_partitions, 16);
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
        assert!(!config.tls_enabled);
    }
}

mod serialization_tests {
    use rustok_core::events::{DomainEvent, EventEnvelope};
    use rustok_iggy::config::SerializationFormat;
    use rustok_iggy::serialization::{EventSerializer, JsonSerializer, MessagePackSerializer};
    use uuid::Uuid;

    fn create_test_envelope() -> EventEnvelope {
        EventEnvelope::new(
            Uuid::new_v4(),
            Some(Uuid::new_v4()),
            DomainEvent::NodeCreated {
                node_id: Uuid::new_v4(),
                kind: "article".to_string(),
                author_id: None,
            },
        )
    }

    #[test]
    fn json_serializer_format() {
        let serializer = JsonSerializer;
        assert_eq!(serializer.format(), SerializationFormat::Json);
    }

    #[test]
    fn message_pack_serializer_format() {
        let serializer = MessagePackSerializer;
        assert_eq!(serializer.format(), SerializationFormat::MessagePack);
    }

    #[test]
    fn json_serialization_works() {
        let serializer = JsonSerializer;
        let envelope = create_test_envelope();

        let result = serializer.serialize(&envelope);
        assert!(result.is_ok());

        let bytes = result.unwrap();
        assert!(!bytes.is_empty());
    }

    #[test]
    fn message_pack_serialization_works() {
        let serializer = MessagePackSerializer;
        let envelope = create_test_envelope();

        let result = serializer.serialize(&envelope);
        assert!(result.is_ok());

        let bytes = result.unwrap();
        assert!(!bytes.is_empty());
    }
}

mod partitioning_tests {
    use rustok_iggy::partitioning::{calculate_partition, partition_key};
    use uuid::Uuid;

    #[test]
    fn partition_key_uses_tenant_id() {
        let tenant_id = Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap();
        let key = partition_key(tenant_id);

        assert_eq!(key, "550e8400-e29b-41d4-a716-446655440000");
    }

    #[test]
    fn partition_calculation_is_deterministic() {
        let key = "tenant-123";
        let p1 = calculate_partition(key, 8);
        let p2 = calculate_partition(key, 8);

        assert_eq!(p1, p2);
    }

    #[test]
    fn partition_is_within_range() {
        for i in 0..1000 {
            let key = format!("tenant-{}", i);
            let partition = calculate_partition(&key, 16);
            assert!(partition < 16, "Partition {} out of range", partition);
        }
    }
}
