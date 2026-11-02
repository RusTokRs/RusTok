use chrono::{TimeZone, Utc};
use object_store::path::Path;
use uuid::Uuid;

use crate::key::{DigestObjectKey, KeyError, ObjectKey, ObjectScope, ObjectZone};
use crate::runtime::{S3StorageConfig, StorageConfig, StorageDriver, StorageKind, StorageRuntime};

#[test]
fn test_chronological_key_construction_and_formatting() {
    let tenant_id = Uuid::parse_str("11111111-1111-4111-8111-111111111111").unwrap();
    let object_id = Uuid::parse_str("22222222-2222-4222-8222-22222222223a").unwrap();
    let created_at = Utc.with_ymd_and_hms(2026, 10, 31, 12, 0, 0).unwrap();

    let key = ObjectKey::chronological(
        "media",
        ObjectZone::Objects,
        ObjectScope::Tenant(tenant_id),
        created_at,
        object_id,
        "webp",
    )
    .expect("chronological key should construct cleanly");

    let expected_shard = "3a";
    let expected = format!(
        "media/objects/tenants/{tenant_id}/2026/10/31/{expected_shard}/{object_id}.webp"
    );
    assert_eq!(key.as_path().as_ref(), expected);
    assert_eq!(key.to_string(), expected);
}

#[test]
fn test_chronological_key_validation_errors() {
    let tenant_id = Uuid::new_v4();
    let object_id = Uuid::new_v4();
    let now = Utc::now();

    // Invalid namespace: empty
    assert!(matches!(
        ObjectKey::chronological("", ObjectZone::Objects, ObjectScope::Tenant(tenant_id), now, object_id, "png"),
        Err(KeyError::InvalidSegment { name: "namespace", .. })
    ));

    // Invalid namespace: uppercase or illegal character
    assert!(matches!(
        ObjectKey::chronological("Media_Uploads", ObjectZone::Objects, ObjectScope::Tenant(tenant_id), now, object_id, "png"),
        Err(KeyError::InvalidSegment { name: "namespace", .. })
    ));

    // Invalid namespace: path traversal sequence
    assert!(matches!(
        ObjectKey::chronological("../escape", ObjectZone::Objects, ObjectScope::Tenant(tenant_id), now, object_id, "png"),
        Err(KeyError::InvalidSegment { name: "namespace", .. })
    ));

    // Invalid extension: empty
    assert!(matches!(
        ObjectKey::chronological("media", ObjectZone::Objects, ObjectScope::Tenant(tenant_id), now, object_id, ""),
        Err(KeyError::InvalidExtension(_))
    ));

    // Invalid extension: uppercase
    assert!(matches!(
        ObjectKey::chronological("media", ObjectZone::Objects, ObjectScope::Tenant(tenant_id), now, object_id, "PNG"),
        Err(KeyError::InvalidExtension(_))
    ));

    // Invalid extension: contains dot
    assert!(matches!(
        ObjectKey::chronological("media", ObjectZone::Objects, ObjectScope::Tenant(tenant_id), now, object_id, ".png"),
        Err(KeyError::InvalidExtension(_))
    ));
}

#[test]
fn test_digest_key_validation_and_formatting() {
    let tenant_id = Uuid::new_v4();
    let valid_digest = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";

    let key = DigestObjectKey::sha256("artifacts", ObjectScope::Tenant(tenant_id), valid_digest)
        .expect("valid digest key should build");
    let expected = format!("artifacts/objects/tenants/{tenant_id}/sha256/e3/b0/{valid_digest}");
    assert_eq!(key.as_path().as_ref(), expected);

    // Invalid digest: uppercase hex
    let upper_digest = "E3B0C44298FC1C149AFBF4C8996FB92427AE41E4649B934CA495991B7852B855";
    assert_eq!(
        DigestObjectKey::sha256("artifacts", ObjectScope::Tenant(tenant_id), upper_digest),
        Err(KeyError::InvalidSha256Digest)
    );

    // Invalid digest: wrong length
    assert_eq!(
        DigestObjectKey::sha256("artifacts", ObjectScope::Tenant(tenant_id), "tooshort"),
        Err(KeyError::InvalidSha256Digest)
    );
}

#[test]
fn test_scope_segment_nil_uuid_rejection() {
    let nil_id = Uuid::nil();
    let valid_id = Uuid::new_v4();
    let now = Utc::now();

    assert_eq!(
        ObjectKey::chronological(
            "media",
            ObjectZone::Objects,
            ObjectScope::Namespace {
                tenant_id: nil_id,
                owner_id: valid_id,
                instance_id: valid_id,
            },
            now,
            valid_id,
            "bin",
        ),
        Err(KeyError::InvalidNamespaceIdentity)
    );

    assert_eq!(
        ObjectKey::chronological(
            "media",
            ObjectZone::Objects,
            ObjectScope::Namespace {
                tenant_id: valid_id,
                owner_id: nil_id,
                instance_id: valid_id,
            },
            now,
            valid_id,
            "bin",
        ),
        Err(KeyError::InvalidNamespaceIdentity)
    );
}

#[test]
fn test_s3_storage_config_debug_redaction() {
    let config = S3StorageConfig {
        bucket: "my-bucket".to_string(),
        region: Some("eu-central-1".to_string()),
        endpoint_url: Some("https://s3.example.com".to_string()),
        access_key_id: Some("AKIAEXAMPLE".to_string()),
        secret_access_key: Some("SUPER_SECRET_KEY_12345".to_string()),
        session_token: Some("SESSION_TOKEN_XYZ".to_string()),
        public_base_url: Some("https://cdn.example.com".to_string()),
        allow_http: false,
        virtual_hosted_style_request: true,
    };

    let debug_str = format!("{config:?}");
    assert!(!debug_str.contains("SUPER_SECRET_KEY_12345"));
    assert!(!debug_str.contains("AKIAEXAMPLE"));
    assert!(!debug_str.contains("SESSION_TOKEN_XYZ"));
    assert!(debug_str.contains("secret_access_key_configured: true"));
    assert!(debug_str.contains("access_key_id_configured: true"));
    assert!(debug_str.contains("session_token_configured: true"));
}

#[test]
fn test_storage_runtime_in_memory_and_options() {
    let runtime = StorageRuntime::in_memory();
    assert_eq!(runtime.kind, StorageKind::Local);
    assert_eq!(runtime.kind.as_str(), "local");

    // Local runtime put_options does not attach custom ContentType
    let opts = runtime.put_options("image/png");
    assert!(opts.attributes.is_empty());

    // Public URL construction
    let path = Path::from("media/test.png");
    let url = runtime.public_url(&path).expect("public url should be generated");
    assert_eq!(url, "/media/media/test.png");
}

#[test]
fn test_storage_config_bind_local_base_dir() {
    let mut config = StorageConfig::default();
    assert_eq!(config.driver, StorageDriver::Local);
    assert_eq!(config.local.base_dir, "storage");

    config.bind_local_base_dir("/var/data/rustok-storage");
    assert_eq!(config.local.base_dir, "/var/data/rustok-storage");
}
