use std::net::SocketAddr;
use std::time::Duration;

use super::{
    MutualTlsClientConfig, MutualTlsListenerConfig, PeerCertificateFingerprint, WorkerAdmission,
};

#[test]
fn cargo_manifest_contract_is_standardized() {
    let manifest = include_str!("../Cargo.toml");
    assert!(
        manifest.contains("readme = \"README.md\""),
        "manifest must declare readme"
    );
    assert!(
        manifest.contains("repository.workspace = true"),
        "manifest must inherit repository from workspace"
    );
    assert!(
        manifest.contains("rust-version.workspace = true"),
        "manifest must inherit rust-version from workspace"
    );
    assert!(
        manifest.contains("[package.metadata.docs.rs]"),
        "manifest must configure docs.rs metadata"
    );
}

#[test]
fn crate_api_contract_specification_is_complete() {
    let api_doc = include_str!("../CRATE_API.md");
    assert!(
        api_doc.contains("# `rustok-worker-transport` Crate API & Architecture Contract"),
        "CRATE_API.md must contain the main title"
    );
    assert!(
        api_doc.contains("## 1. Architectural Purpose & Boundaries"),
        "CRATE_API.md must document purpose and boundaries"
    );
    assert!(
        api_doc.contains("## 2. Mutual TLS (mTLS) Listener & Client Configurations"),
        "CRATE_API.md must document mTLS configuration"
    );
    assert!(
        api_doc.contains("## 3. Peer Certificate Fingerprint & Deployment Identity"),
        "CRATE_API.md must document peer fingerprint identity"
    );
    assert!(
        api_doc.contains("## 4. Worker Admission Controller & Concurrency Bounds"),
        "CRATE_API.md must document worker admission"
    );
    assert!(
        api_doc.contains("## 5. Universal Logic Dimensions Verification"),
        "CRATE_API.md must document universal logic dimensions"
    );
}

#[test]
fn debug_formatting_redacts_private_keys() {
    let listener = MutualTlsListenerConfig::new(
        "127.0.0.1:9090".parse::<SocketAddr>().unwrap(),
        b"cert-data".to_vec(),
        b"super-secret-private-key".to_vec(),
        b"ca-data".to_vec(),
        Duration::from_secs(10),
        Duration::from_millis(500),
        8,
        64 * 1024,
        1024 * 1024,
    )
    .expect("valid listener");

    let listener_debug = format!("{listener:?}");
    assert!(listener_debug.contains("MutualTlsListenerConfig"));
    assert!(listener_debug.contains("<redacted>"));
    assert!(!listener_debug.contains("super-secret-private-key"));

    let client = MutualTlsClientConfig::new(
        b"client-cert".to_vec(),
        b"client-secret-key".to_vec(),
        b"server-ca".to_vec(),
        "worker.service.internal",
    )
    .expect("valid client");

    let client_debug = format!("{client:?}");
    assert!(client_debug.contains("MutualTlsClientConfig"));
    assert!(client_debug.contains("<redacted>"));
    assert!(!client_debug.contains("client-secret-key"));
    assert!(client_debug.contains("worker.service.internal"));

    let admission = WorkerAdmission::new(4, Duration::from_millis(100)).expect("valid admission");
    let admission_debug = format!("{admission:?}");
    assert!(admission_debug.contains("WorkerAdmission"));
    assert!(admission_debug.contains("available_permits: 4"));
}

#[test]
fn peer_certificate_fingerprint_traits_and_display() {
    let hash = "a1b2c3d4e5f60718293a4b5c6d7e8f90a1b2c3d4e5f60718293a4b5c6d7e8f90";
    let formatted = format!("sha256:{hash}");
    let fingerprint = PeerCertificateFingerprint::parse(&formatted).expect("valid fingerprint");

    assert_eq!(fingerprint.as_str(), formatted);
    assert_eq!(fingerprint.to_string(), formatted);
    assert_eq!(fingerprint.as_ref(), formatted.as_str());

    // Comparison & ordering
    let cloned = fingerprint.clone();
    assert_eq!(fingerprint, cloned);

    let other_hash = "b1b2c3d4e5f60718293a4b5c6d7e8f90a1b2c3d4e5f60718293a4b5c6d7e8f90";
    let other_fp =
        PeerCertificateFingerprint::parse(format!("sha256:{other_hash}")).expect("valid");
    assert!(fingerprint < other_fp);
}

#[tokio::test]
async fn worker_admission_lifecycle_and_shutdown() {
    let admission = WorkerAdmission::new(2, Duration::from_millis(50)).expect("valid admission");
    assert_eq!(admission.available_permits(), 2);
    assert_eq!(admission.admission_timeout(), Duration::from_millis(50));

    let permit1 = admission.acquire().await.expect("permit 1");
    assert_eq!(admission.available_permits(), 1);

    {
        let permit2 = admission.acquire().await.expect("permit 2");
        assert_eq!(admission.available_permits(), 0);

        // Saturation check
        let saturated_err = admission.acquire().await.expect_err("should saturate");
        assert_eq!(saturated_err.code(), tonic::Code::ResourceExhausted);
        drop(permit2);
    }

    assert_eq!(admission.available_permits(), 1);
    drop(permit1);
    assert_eq!(admission.available_permits(), 2);

    // Close on shutdown
    admission.close();
    let shutdown_err = admission
        .acquire()
        .await
        .expect_err("should be shutting down");
    assert_eq!(shutdown_err.code(), tonic::Code::Unavailable);
}

#[test]
fn mutual_tls_client_trust_anchor_digest_determinism() {
    let client = MutualTlsClientConfig::new(
        b"cert".to_vec(),
        b"key".to_vec(),
        b"root-ca-cert-pem".to_vec(),
        "build-worker.prod.internal",
    )
    .expect("valid client config");

    assert_eq!(client.server_domain(), "build-worker.prod.internal");
    let digest1 = client.trust_anchor_digest();
    let digest2 = client.trust_anchor_digest();
    assert_eq!(digest1, digest2);
    assert!(digest1.starts_with("sha256:"));
    assert_eq!(digest1.len(), 71);

    let client_different_domain = MutualTlsClientConfig::new(
        b"cert".to_vec(),
        b"key".to_vec(),
        b"root-ca-cert-pem".to_vec(),
        "build-worker-2.prod.internal",
    )
    .expect("valid client config");
    assert_ne!(digest1, client_different_domain.trust_anchor_digest());
}

#[test]
fn listener_validation_invariants() {
    let addr = "127.0.0.1:8080".parse::<SocketAddr>().unwrap();

    // Zero request timeout
    assert!(
        MutualTlsListenerConfig::new(
            addr,
            b"cert".to_vec(),
            b"key".to_vec(),
            b"ca".to_vec(),
            Duration::ZERO,
            Duration::from_millis(100),
            1,
            1024,
            1024 * 1024
        )
        .is_err()
    );

    // Admission timeout > request timeout
    assert!(
        MutualTlsListenerConfig::new(
            addr,
            b"cert".to_vec(),
            b"key".to_vec(),
            b"ca".to_vec(),
            Duration::from_millis(50),
            Duration::from_millis(100),
            1,
            1024,
            1024 * 1024
        )
        .is_err()
    );

    // Concurrency limit 0
    assert!(
        MutualTlsListenerConfig::new(
            addr,
            b"cert".to_vec(),
            b"key".to_vec(),
            b"ca".to_vec(),
            Duration::from_secs(1),
            Duration::from_millis(100),
            0,
            1024,
            1024 * 1024
        )
        .is_err()
    );
}
