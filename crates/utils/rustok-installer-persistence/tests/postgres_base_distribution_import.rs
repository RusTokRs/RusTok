use std::{error::Error, io::Write};

use base64::{Engine, engine::general_purpose::STANDARD};
use chrono::{Duration, Utc};
use ed25519_dalek::{Signer, SigningKey};
use rustok_installer::{
    AdminBootstrap, InstallBootstrapPort, InstallComposition, InstallDistributionBinding,
    InstallPlan, InstallSchemaPort, InstancePlacement, SecretValue, TenantBootstrap,
    load_base_distribution_receipt,
};
use rustok_installer_persistence::SeaOrmInstallerPorts;
use rustok_migrations::{Migrator, verification::validate_journal};
use rustok_modules::{
    MODULE_STATIC_DISTRIBUTION_BOOTSTRAP_RECEIPT_CONTRACT,
    ModuleStaticDistributionBootstrapPreparation, ModuleStaticDistributionBootstrapReceipt,
    ModuleStaticDistributionBootstrapReceiptPayload, ModuleStaticDistributionBuildEvidence,
    ModuleStaticDistributionExecutorMode, ModuleStaticDistributionItem,
    ModuleStaticDistributionReleaseAdmission, ModuleStaticDistributionRole,
    ModuleStaticDistributionRoleArtifact, module_static_distribution_composition_digest,
    resolve_static_distribution_install_binding,
};
use rustok_test_utils::db::{
    assert_postgres_table_missing, connect_postgres, create_postgres_database,
    drop_postgres_database_if_exists, postgres_database_url, unique_postgres_database_name,
};
use sea_orm_migration::MigratorTrait;
use sha2::{Digest, Sha256};
use uuid::Uuid;

#[tokio::test]
#[ignore = "requires isolated PostgreSQL and RUSTOK_MIGRATION_SMOKE_ADMIN_URL"]
async fn postgres_signed_base_distribution_survives_install_schema() -> Result<(), Box<dyn Error>> {
    let admin_url = std::env::var("RUSTOK_MIGRATION_SMOKE_ADMIN_URL")?;
    let (binding, public_key) = signed_base_distribution()?;
    let receipt = binding
        .bootstrap_receipt
        .as_ref()
        .ok_or("missing signed receipt")?;
    let release_id = receipt.payload.distribution_release_id;
    let digest = receipt.digest()?;
    let composition = InstallComposition {
        revision: receipt.payload.host_composition_revision.clone(),
        hash: receipt.payload.host_composition_hash.clone(),
    };
    let name = unique_postgres_database_name("rustok_signed_base");
    let db_url = postgres_database_url(&admin_url, &name);
    let plan = InstallPlan::production_minimal(
        InstancePlacement::new("."),
        SecretValue::Plaintext {
            value: db_url.clone(),
        },
        TenantBootstrap {
            slug: "installer".into(),
            name: "Installer".into(),
        },
        AdminBootstrap {
            email: "installer@example.com".into(),
            password: SecretValue::Plaintext {
                value: "unused-fixture-password".into(),
            },
        },
        composition,
        binding.clone(),
    );

    let admin = connect_postgres(&admin_url).await?;
    create_postgres_database(&admin, &name).await?;
    let result: Result<(), Box<dyn Error>> = async {
        let db = connect_postgres(&db_url).await?;
        let ports = SeaOrmInstallerPorts;
        ports.apply_owner_schema(&db).await?;
        assert_postgres_table_missing(&db, "users").await?;

        let mut tampered = plan.clone();
        tampered
            .topology
            .distribution
            .as_mut()
            .and_then(|distribution| distribution.bootstrap_receipt.as_mut())
            .ok_or("missing receipt to tamper with")?
            .payload
            .data_contract_digest = test_digest('f');
        let error = ports
            .import_base_distribution(&db, &tampered, Some(&public_key))
            .await
            .err()
            .ok_or("owner admitted a tampered signed receipt")?;
        if !error.to_string().contains("verification") {
            return Err(error.into());
        }

        let imported = ports
            .import_base_distribution(&db, &plan, Some(&public_key))
            .await?
            .ok_or("owner did not import the signed release")?;
        if !imported.created
            || imported.receipt_digest != digest
            || imported.preparation_id != binding.preparation_id
            || imported.distribution_release_id != release_id
            || imported.release_revision != 1
        {
            return Err(format!("unexpected base distribution import {imported:?}").into());
        }
        let before = resolve_static_distribution_install_binding(&db, release_id).await?;
        if before.preparation_id != binding.preparation_id
            || before.distribution_release_id != release_id
            || before.bundle_reference != binding.bundle_reference
            || before.bundle_root_digest != binding.bundle_root_digest
            || before.role_set_digest != binding.role_set_digest
            || before.roles != binding.roles
        {
            return Err(
                "owner returned a different binding from the signed base distribution".into(),
            );
        }
        assert_postgres_table_missing(&db, "users").await?;

        ports.apply_remaining_schema(&db).await?;
        let journal: Vec<_> = Migrator::get_migration_models(&db)
            .await?
            .into_iter()
            .map(|row| row.version)
            .collect();
        validate_journal(&journal, Migrator::inventory()?.migrations())?;
        let replay = ports
            .import_base_distribution(&db, &plan, Some(&public_key))
            .await?
            .ok_or("missing signed import replay")?;
        if replay
            != (rustok_modules::ModuleStaticDistributionBootstrapImportReceipt {
                created: false,
                ..imported
            })
            || resolve_static_distribution_install_binding(&db, release_id).await? != before
        {
            return Err("schema completion changed the signed import or its binding".into());
        }
        db.close().await?;
        Ok(())
    }
    .await;

    let cleanup = drop_postgres_database_if_exists(&admin, &name).await;
    match (result, cleanup) {
        (Err(error), Err(cleanup)) => Err(format!("{error}; database cleanup: {cleanup}").into()),
        (Err(error), Ok(())) => Err(error),
        (Ok(()), Err(error)) => Err(error.into()),
        (Ok(()), Ok(())) => Ok(()),
    }
}

fn signed_base_distribution() -> Result<(InstallDistributionBinding, String), Box<dyn Error>> {
    let key = SigningKey::from_bytes(&[7; 32]);
    let public_key = key.verifying_key().to_bytes();
    let platform_source_digest = test_digest('1');
    let platform_source_reference = format!("cas://{platform_source_digest}");
    let toolchain_digest = test_digest('3');
    let build_target = "x86_64-unknown-linux-gnu";
    let source_digest = test_digest('9');
    let items = vec![ModuleStaticDistributionItem {
        promotion_id: Uuid::new_v4(),
        promotion_revision: 1,
        release_id: "fixture@0.1.0".into(),
        module_slug: "fixture".into(),
        module_version: "0.1.0".into(),
        cargo_package: "rustok-fixture".into(),
        entry_type: "FixtureModule".into(),
        source_reference: format!("cas://{source_digest}"),
        source_digest,
        dependency_lock_digest: test_digest('a'),
        executor_mode: ModuleStaticDistributionExecutorMode::StaticNative,
    }];
    let roles = vec![ModuleStaticDistributionRoleArtifact {
        role: ModuleStaticDistributionRole::Monolith,
        artifact_digest: test_digest('2'),
    }];
    let bundle_root_digest = test_digest('b');
    let now = Utc::now();
    let payload = ModuleStaticDistributionBootstrapReceiptPayload {
        contract: MODULE_STATIC_DISTRIBUTION_BOOTSTRAP_RECEIPT_CONTRACT.into(),
        preparation_id: Uuid::new_v4(),
        distribution_release_id: Uuid::new_v4(),
        host_composition_revision: "test-distribution@1".into(),
        host_composition_hash: "a".repeat(64),
        preparation: ModuleStaticDistributionBootstrapPreparation {
            composition_revision: 1,
            composition_digest: module_static_distribution_composition_digest(
                &platform_source_reference,
                &platform_source_digest,
                &toolchain_digest,
                build_target,
                &items,
            )?,
            platform_source_reference,
            platform_source_digest,
            toolchain_digest,
            build_target: build_target.into(),
            items,
            evidence: ModuleStaticDistributionBuildEvidence {
                bundle_reference: format!("registry.example/rustok/base@{bundle_root_digest}"),
                bundle_root_digest,
                role_set_digest: ModuleStaticDistributionBuildEvidence::role_set_digest(&roles)?,
                roles,
                sbom_reference: "oci://base/sbom".into(),
                sbom_digest: test_digest('4'),
                provenance_reference: "oci://base/provenance".into(),
                provenance_digest: test_digest('5'),
                signature_reference: "oci://base/signature".into(),
                signature_digest: test_digest('6'),
                test_evidence_reference: "oci://base/tests".into(),
                test_evidence_digest: test_digest('7'),
            },
            admission: ModuleStaticDistributionReleaseAdmission {
                verifier_identity: "bootstrap-signer-fixture".into(),
                policy_revision: "policy@1".into(),
                evidence_reference: "oci://base/admission".into(),
                evidence_digest: test_digest('8'),
                signature_verified: true,
                provenance_verified: true,
                sbom_verified: true,
                test_evidence_verified: true,
                dependency_policy_verified: true,
            },
        },
        migration_plan_digest: test_digest('d'),
        data_contract_digest: test_digest('e'),
        signer_key_digest: format!("sha256:{}", hex::encode(Sha256::digest(public_key))),
        issued_at: now - Duration::minutes(1),
        expires_at: now + Duration::hours(1),
    };
    let signature = key.sign(&rustok_api::manifest_hash::canonical_json_bytes(&payload)?);
    let receipt = ModuleStaticDistributionBootstrapReceipt {
        payload,
        signature: STANDARD.encode(signature.to_bytes()),
    };
    let public_key = STANDARD.encode(public_key);
    let bytes = serde_json::to_vec(&receipt)?;
    let path = std::env::temp_dir().join(format!("rustok-signed-base-{}.json", Uuid::new_v4()));
    let mut file = std::fs::File::create_new(&path)?;
    let written = file.write_all(&bytes);
    drop(file);
    let loaded = written
        .map_err(Into::<Box<dyn Error>>::into)
        .and_then(
            |()| Ok(load_base_distribution_receipt(&path, &public_key, now)?.into_binding()?),
        );
    std::fs::remove_file(&path)?;
    Ok((loaded?, public_key))
}

fn test_digest(digit: char) -> String {
    format!("sha256:{}", digit.to_string().repeat(64))
}
