#![allow(unused_imports)]

use sea_orm::{ConnectionTrait, Database, DbBackend, Statement, TransactionTrait, Value};
use semver::Version;
use sha2::{Digest, Sha256};
use uuid::Uuid;

use super::fixtures::*;
use super::*;
use crate::build::{
    ModuleBuildAuthoring, ModuleBuildComponentInterface, ModuleBuildDependencyPolicy,
    ModuleBuildEvidence, ModuleBuildLimits, ModuleBuildMetrics, ModuleBuildNetworkPolicy,
    ModuleBuildNextAction, ModuleBuildOutcome, ModuleBuildPublicationReceipt,
    ModuleBuildRequest, ModuleBuildResult, ModuleBuildScenario, ModuleBuildSignatureAuthority,
    ModuleBuildSource, ModuleBuildToolchain, ModuleBuildValidationOutcome,
    ModuleBuildValidationProfile, ModuleBuildValidationResult, ModuleBuildWitContract,
};
use crate::installation::{ArtifactVerificationEvidence, OciArtifactReference};
use crate::{
    ArtifactBlobStore, ArtifactModuleKind, ArtifactPayloadKind, ArtifactReleaseRef,
    ControlPlaneInfrastructure, InMemoryArtifactBlobStore, ModuleArtifactDescriptor,
    ModuleCommandContext, ModuleMarketplaceArtifactOrigin, ModuleMarketplaceArtifactRelease,
    ModuleMarketplaceEntry, ModuleMarketplaceEvidenceKind, ModuleMarketplaceEvidenceReference,
    TrustEvidenceKind, TrustEvidenceReference, MODULE_BUILD_COMPONENT_TARGET,
    MODULE_BUILD_PROTOCOL_VERSION, MODULE_BUILD_RUNTIME_ABI, MODULE_BUILD_WIT_VERSION,
    MODULE_BUILD_WIT_WORLD,
};


    #[tokio::test]
    async fn marketplace_projection_reads_yanked_release_versions_in_the_owner() {
        let database = Database::connect("sqlite::memory:")
            .await
            .expect("database");
        for statement in [
            "CREATE TABLE registry_module_releases (\
                id TEXT PRIMARY KEY, slug TEXT NOT NULL, version TEXT NOT NULL,\
                status TEXT NOT NULL, publisher_principal TEXT NOT NULL,\
                checksum_sha256 TEXT NULL, default_locale TEXT NOT NULL,\
                published_at TEXT NOT NULL\
             )",
            "CREATE TABLE registry_module_release_artifacts (\
                release_id TEXT PRIMARY KEY, request_id TEXT NOT NULL UNIQUE,\
                artifact JSON NOT NULL, descriptor JSON NOT NULL, lineage JSON NOT NULL\
             )",
            "CREATE TABLE registry_publish_platform_admissions (\
                request_id TEXT PRIMARY KEY, media_type TEXT NOT NULL\
             )",
            "INSERT INTO registry_module_releases (\
                id, slug, version, status, publisher_principal, checksum_sha256, default_locale, published_at\
             ) VALUES (\
                'release-1', 'sample_module', '1.0.0', 'yanked',\
                '{\"subject\":\"publisher\"}',\
                'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa',\
                'en', datetime('now')\
             )",
        ] {
            database
                .execute_raw(Statement::from_string(
                    DbBackend::Sqlite,
                    statement.to_string(),
                ))
                .await
                .expect("schema or fixture");
        }

        let entry = serde_json::from_value::<ModuleMarketplaceEntry>(serde_json::json!({
            "slug": "sample_module",
            "name": "Static name",
            "latest_version": "0.9.0",
            "description": "Static description retained without an active release.",
            "source": "local",
            "kind": "optional",
            "category": "tools",
            "tags": [],
            "icon_url": null,
            "banner_url": null,
            "screenshots": [],
            "crate_name": "sample-module",
            "dependencies": [],
            "ownership": "third_party",
            "trust_level": "unverified",
            "rustok_min_version": null,
            "rustok_max_version": null,
            "publisher": null,
            "checksum_sha256": null,
            "signature_present": false,
            "versions": [],
            "has_admin_ui": false,
            "has_storefront_ui": false,
            "ui_classification": "no_ui",
            "registry_lifecycle": null,
            "compatible": true,
            "recommended_admin_surfaces": [],
            "showcase_admin_surfaces": [],
            "settings_schema": {},
            "installed": false,
            "installed_version": null,
            "update_available": false
        }))
        .expect("marketplace entry");

        let projected = SeaOrmModuleGovernanceService::new(database)
            .apply_marketplace_projection(vec![entry], Some("en-US"), Some("en"))
            .await
            .expect("owner projection");
        assert_eq!(projected.len(), 1);
        assert_eq!(projected[0].latest_version, "0.9.0");
        assert_eq!(projected[0].versions.len(), 1);
        assert_eq!(projected[0].versions[0].version, "1.0.0");
        assert!(projected[0].versions[0].yanked);
    }


    #[tokio::test]
    async fn published_rhai_workspace_requires_an_exact_active_contract_and_canonical_cas_bytes() {
        let database = Database::connect("sqlite::memory:")
            .await
            .expect("database");
        for statement in [
            "CREATE TABLE registry_module_releases (\
                id TEXT PRIMARY KEY, slug TEXT NOT NULL, version TEXT NOT NULL,\
                status TEXT NOT NULL, published_at TEXT NOT NULL\
             )",
            "CREATE TABLE registry_module_release_artifacts (\
                release_id TEXT PRIMARY KEY, request_id TEXT NOT NULL UNIQUE,\
                artifact JSON NOT NULL, descriptor JSON NOT NULL, lineage JSON NOT NULL\
             )",
            "CREATE TABLE registry_publish_platform_admissions (\
                request_id TEXT PRIMARY KEY, media_type TEXT NOT NULL\
             )",
        ] {
            database
                .execute_raw(Statement::from_string(
                    DbBackend::Sqlite,
                    statement.to_string(),
                ))
                .await
                .expect("schema");
        }

        let workspace = rustok_sandbox::RhaiWorkspace::single_source("40 + 2");
        let workspace_bytes = workspace.canonical_bytes().expect("workspace bytes");
        let digest = workspace.digest().expect("workspace digest");
        let descriptor = ModuleArtifactDescriptor {
            schema_version: crate::MODULE_ARTIFACT_DESCRIPTOR_SCHEMA_VERSION,
            slug: "published_rhai".to_string(),
            version: "1.0.0".to_string(),
            payload_kind: ArtifactPayloadKind::Rhai,
            module_kind: ArtifactModuleKind::Optional,
            runtime_abi: "rustok:module/runtime@1".to_string(),
            platform_compatibility: "^0.1".to_string(),
            required_features: Vec::new(),
            artifact_digest: digest.clone(),
            entrypoint: "main".to_string(),
            capabilities: Vec::new(),
            bindings: Vec::new(),
            dependencies: Vec::new(),
            permissions: Vec::new(),
            schema_documents: Vec::new(),
            settings_schema_digest: None,
            data_schema_digest: None,
            localization_catalogs: Vec::new(),
            ui_contributions: Vec::new(),
            persistence_contract: None,
        };
        descriptor.validate().expect("descriptor");
        let evidence = [
            ModuleMarketplaceEvidenceKind::AuthorSignature,
            ModuleMarketplaceEvidenceKind::Sbom,
            ModuleMarketplaceEvidenceKind::Provenance,
            ModuleMarketplaceEvidenceKind::PlatformAdmission,
            ModuleMarketplaceEvidenceKind::MarketplaceApproval,
        ]
        .into_iter()
        .enumerate()
        .map(|(index, kind)| {
            let digit = char::from(b'a' + index as u8);
            ModuleMarketplaceEvidenceReference {
                kind,
                reference: format!("evidence://published-rhai/{index}"),
                digest: format!("sha256:{}", digit.to_string().repeat(64)),
            }
        })
        .collect::<Vec<_>>();
        let artifact = ModuleMarketplaceArtifactRelease {
            registry_id: "local".to_string(),
            repository: "modules/published_rhai".to_string(),
            origin: ModuleMarketplaceArtifactOrigin::AlloyAuthored,
            runtime_kind: crate::ModuleMarketplaceRuntimeKind::Rhai,
            oci_manifest_digest: format!("sha256:{}", "f".repeat(64)),
            payload_digest: digest.clone(),
            descriptor_digest: crate::canonical_artifact_descriptor_digest(&descriptor),
            source_reference: "alloy://tenant/script/1".to_string(),
            source_digest: digest.clone(),
            evidence,
        };
        artifact.validate().expect("artifact contract");
        let lineage = crate::ArtifactSourceLineage {
            origin: crate::ArtifactOrigin::Marketplace,
            source_digest: digest.clone(),
            parent_release: Some(ArtifactReleaseRef {
                slug: "published_rhai".to_string(),
                version: "0.9.0".to_string(),
                digest: format!("sha256:{}", "e".repeat(64)),
            }),
        };
        database
            .execute_raw(Statement::from_sql_and_values(
                DbBackend::Sqlite,
                "INSERT INTO registry_module_releases \
                 (id, slug, version, status, published_at) \
                 VALUES ('release-1', 'published_rhai', '1.0.0', 'active', datetime('now'))"
                    .to_string(),
                Vec::new(),
            ))
            .await
            .expect("release");
        database
            .execute_raw(Statement::from_sql_and_values(
                DbBackend::Sqlite,
                "INSERT INTO registry_module_release_artifacts \
                 (release_id, request_id, artifact, descriptor, lineage) VALUES ('release-1', 'request-1', ?, ?, ?)"
                    .to_string(),
                vec![
                    Value::Json(Some(Box::new(
                        serde_json::to_value(&artifact).expect("artifact JSON"),
                    ))),
                    Value::Json(Some(Box::new(
                        serde_json::to_value(&descriptor).expect("descriptor JSON"),
                    ))),
                    Value::Json(Some(Box::new(
                        serde_json::to_value(&lineage).expect("lineage JSON"),
                    ))),
                ],
            ))
            .await
            .expect("artifact projection");
        database
            .execute_raw(Statement::from_string(
                DbBackend::Sqlite,
                format!(
                    "INSERT INTO registry_publish_platform_admissions (request_id, media_type) \
                     VALUES ('request-1', '{}')",
                    rustok_sandbox::RHAI_WORKSPACE_MEDIA_TYPE
                ),
            ))
            .await
            .expect("admission projection");

        let blobs = InMemoryArtifactBlobStore::default();
        blobs
            .put_verified(&digest, &workspace_bytes)
            .await
            .expect("CAS payload");
        let release = ArtifactReleaseRef {
            slug: "published_rhai".to_string(),
            version: "1.0.0".to_string(),
            digest: digest.clone(),
        };
        let service = SeaOrmModuleGovernanceService::new(database.clone());
        let source = service
            .published_rhai_workspace(&release, &blobs)
            .await
            .expect("published workspace");
        assert_eq!(source.release.descriptor.release_ref(), release);
        assert_eq!(source.release.lineage, lineage);
        assert_eq!(source.workspace, workspace);

        database
            .execute_raw(Statement::from_string(
                DbBackend::Sqlite,
                "UPDATE registry_publish_platform_admissions \
                 SET media_type = 'application/json' WHERE request_id = 'request-1'"
                    .to_string(),
            ))
            .await
            .expect("wrong admission media type");
        assert_eq!(
            service.published_rhai_workspace(&release, &blobs).await,
            Err(ModuleGovernanceError::PublishRequestMissingCanonicalArtifactContract)
        );

        let missing = ArtifactReleaseRef {
            version: "1.0.1".to_string(),
            ..source.release.descriptor.release_ref()
        };
        assert_eq!(
            service.published_rhai_workspace(&missing, &blobs).await,
            Err(ModuleGovernanceError::ReleaseNotFound)
        );
    }

