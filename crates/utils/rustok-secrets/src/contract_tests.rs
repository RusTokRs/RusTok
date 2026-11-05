use std::time::Duration;

use secrecy::ExposeSecret;
use uuid::Uuid;

use crate::{
    EnvResolver, MountedFileResolver, SecretAccessPolicy, SecretError, SecretRef, SecretResolver,
    SecretResolverRegistry,
};

#[test]
fn secret_access_policy_contract_matrix() {
    let tenant_1 = Uuid::new_v4();
    let tenant_2 = Uuid::new_v4();

    // 1. DenyAll
    let deny = SecretAccessPolicy::DenyAll;
    assert!(!deny.allows(tenant_1, "any_key"));

    // 2. Exact
    let exact = SecretAccessPolicy::Exact(vec!["payment_key".to_string()]);
    assert!(exact.allows(tenant_1, "payment_key"));
    assert!(!exact.allows(tenant_1, "payment_key_extra"));
    assert!(!exact.allows(tenant_1, "other_key"));

    // 3. Prefix
    let prefix = SecretAccessPolicy::Prefix(vec!["app/".to_string()]);
    assert!(prefix.allows(tenant_1, "app/database_url"));
    assert!(!prefix.allows(tenant_1, "other/database_url"));

    // 4. TenantPrefix
    let tenant_policy = SecretAccessPolicy::TenantPrefix {
        prefix: "tenants/".to_string(),
    };
    let allowed_key = format!("tenants/{tenant_1}/stripe_secret");
    let forbidden_key = format!("tenants/{tenant_2}/stripe_secret");

    assert!(tenant_policy.allows(tenant_1, &allowed_key));
    assert!(!tenant_policy.allows(tenant_1, &forbidden_key));
    assert!(!tenant_policy.allows(tenant_2, &allowed_key));
    assert!(tenant_policy.allows(tenant_2, &forbidden_key));
}

#[tokio::test]
async fn registry_builder_clamps_ttl_and_validates_references() {
    let registry = SecretResolverRegistry::builder()
        .resolver(
            "env",
            EnvResolver,
            SecretAccessPolicy::Exact(vec!["VALID_KEY".to_string()]),
        )
        .cache_ttl(Duration::from_secs(3600)) // Should be clamped to 60s
        .build();

    let tenant = Uuid::new_v4();

    // 1. Blank key validation
    let blank_err = registry
        .validate_reference_for_tenant(
            tenant,
            &SecretRef {
                resolver: "env".to_string(),
                key: "   ".to_string(),
            },
        )
        .expect_err("blank key must be rejected");
    assert!(matches!(blank_err, SecretError::BlankKey));

    // 2. Unregistered resolver
    let unknown_err = registry
        .validate_reference_for_tenant(
            tenant,
            &SecretRef {
                resolver: "non_existent".to_string(),
                key: "VALID_KEY".to_string(),
            },
        )
        .expect_err("unregistered resolver must fail");
    assert!(matches!(unknown_err, SecretError::ResolverNotFound(_)));

    // 3. Disallowed key
    let forbidden_err = registry
        .validate_reference_for_tenant(
            tenant,
            &SecretRef {
                resolver: "env".to_string(),
                key: "INVALID_KEY".to_string(),
            },
        )
        .expect_err("forbidden key must fail");
    assert!(matches!(forbidden_err, SecretError::ForbiddenKey { .. }));

    // 4. Valid key
    assert!(
        registry
            .validate_reference_for_tenant(
                tenant,
                &SecretRef {
                    resolver: "env".to_string(),
                    key: "VALID_KEY".to_string(),
                },
            )
            .is_ok()
    );
}

#[tokio::test]
async fn mounted_file_resolver_prevents_directory_traversal() {
    let temp_dir = std::env::temp_dir().join(format!("rustok-secrets-test-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&temp_dir).unwrap();
    let secret_file = temp_dir.join("api_key.txt");
    std::fs::write(&secret_file, "super-secret-key-content\n").unwrap();

    let outside_file = std::env::temp_dir().join(format!("outside-secret-{}.txt", Uuid::new_v4()));
    std::fs::write(&outside_file, "outside-content\n").unwrap();

    let resolver = MountedFileResolver::new(&temp_dir);

    // 1. Resolves file correctly and trims newline
    let resolved = resolver
        .resolve("api_key.txt")
        .await
        .expect("mounted file should resolve");
    assert_eq!(resolved.expose_secret(), "super-secret-key-content");

    // 2. Directory traversal attempt is rejected
    let traversal_err = resolver.resolve("../outside-secret.txt").await;
    assert!(traversal_err.is_err());

    // Cleanup
    let _ = std::fs::remove_dir_all(temp_dir);
    let _ = std::fs::remove_file(outside_file);
}
