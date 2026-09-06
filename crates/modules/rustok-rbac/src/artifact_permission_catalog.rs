//! Durable RBAC vocabulary for permissions declared by admitted artifacts.

use std::collections::{BTreeSet, HashSet};

use async_trait::async_trait;
use hex::ToHex;
use rustok_api::{
    ArtifactPermissionContinuityReceipt, ArtifactPermissionDiff,
    ArtifactPermissionRegistration, ArtifactPermissionRegistrationPort,
    ArtifactPermissionScope, PermissionContinuityEvaluationRequest, PortError,
    ReleasePermissionAdmissionRequest, ScopedPermissionProjectionRequest,
    compute_canonical_authorization_fingerprint, normalize_locale_tag,
};
use sea_orm::{
    ConnectionTrait, DatabaseConnection, DatabaseTransaction, DbBackend, Statement,
    TransactionTrait,
};
use uuid::Uuid;

use crate::invalidation_generation::read_permission_invalidation_generation;

const MAX_PERMISSION_KEY_LENGTH: usize = 256;

/// RBAC-owned durable adapter for admitted artifact permission vocabulary.
///
/// It intentionally writes neither `roles` nor `role_permissions`: registration
/// makes a permission available for later policy assignment, never grants it.
/// Language-neutral identity is persisted once and localized copy is stored in
/// the owner translations table.
#[derive(Clone)]
pub struct RbacArtifactPermissionCatalog {
    db: DatabaseConnection,
}

impl RbacArtifactPermissionCatalog {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }
}

#[async_trait]
impl ArtifactPermissionRegistrationPort for RbacArtifactPermissionCatalog {
    async fn admit_release_permissions(
        &self,
        request: ReleasePermissionAdmissionRequest,
    ) -> Result<(), PortError> {
        validate_admission_request(&request)?;
        let backend = self.db.get_database_backend();
        let transaction = self.db.begin().await.map_err(storage_error)?;

        for permission in &request.permissions {
            let release_permission_id = rustok_core::generate_id();
            transaction
                .execute_raw(Statement::from_sql_and_values(
                    backend,
                    release_definition_insert_sql(backend)?,
                    vec![
                        release_permission_id.into(),
                        request.release_digest.clone().into(),
                        request.module_slug.clone().into(),
                        permission.key.clone().into(),
                    ],
                ))
                .await
                .map_err(storage_error)?;

            let definition = transaction
                .query_one_raw(Statement::from_sql_and_values(
                    backend,
                    release_definition_select_sql(backend)?,
                    vec![
                        request.release_digest.clone().into(),
                        permission.key.clone().into(),
                    ],
                ))
                .await
                .map_err(storage_error)?
                .ok_or_else(|| {
                    PortError::invariant_violation(
                        "rbac.artifact_release_permission_definition_missing",
                        "release permission definition disappeared during admission",
                    )
                })?;

            let persisted_id: Uuid = definition.try_get("", "id").map_err(storage_error)?;
            let persisted_module_slug: String = definition
                .try_get("", "module_slug")
                .map_err(storage_error)?;

            if persisted_module_slug != request.module_slug {
                return Err(PortError::conflict(
                    "rbac.artifact_permission_identity_conflict",
                    "release permission definition is already bound to different module slug",
                ));
            }

            for localization in &permission.localizations {
                let normalized_locale =
                    normalize_locale_tag(&localization.locale).ok_or_else(|| {
                        PortError::invariant_violation(
                            "rbac.artifact_permission_locale_not_normalized",
                            "validated artifact permission locale could not be normalized",
                        )
                    })?;
                transaction
                    .execute_raw(Statement::from_sql_and_values(
                        backend,
                        release_translation_upsert_sql(backend)?,
                        vec![
                            rustok_core::generate_id().into(),
                            persisted_id.into(),
                            normalized_locale.into(),
                            localization.label.clone().into(),
                            localization.description.clone().into(),
                        ],
                    ))
                    .await
                    .map_err(storage_error)?;
            }
        }

        transaction.commit().await.map_err(storage_error)
    }

    async fn project_scoped_permissions(
        &self,
        request: ScopedPermissionProjectionRequest,
    ) -> Result<(), PortError> {
        validate_projection_request(&request)?;
        let scope_key = scope_key(&request.scope);
        let backend = self.db.get_database_backend();
        let transaction = self.db.begin().await.map_err(storage_error)?;

        ensure_installation_identity(
            &transaction,
            backend,
            &scope_key,
            request.installation_id,
            &request.module_slug,
            &request.release_digest,
        )
        .await?;

        // Query admitted release definitions
        let release_definitions = transaction
            .query_all_raw(Statement::from_sql_and_values(
                backend,
                release_definitions_by_release_sql(backend)?,
                vec![
                    request.release_digest.clone().into(),
                    request.module_slug.clone().into(),
                ],
            ))
            .await
            .map_err(storage_error)?;

        for row in release_definitions {
            let release_permission_id: Uuid = row.try_get("", "id").map_err(storage_error)?;
            let permission_key: String =
                row.try_get("", "permission_key").map_err(storage_error)?;

            let scoped_permission_id = rustok_core::generate_id();
            transaction
                .execute_raw(Statement::from_sql_and_values(
                    backend,
                    definition_insert_sql(backend)?,
                    vec![
                        scoped_permission_id.into(),
                        scope_key.clone().into(),
                        request.installation_id.into(),
                        request.module_slug.clone().into(),
                        request.release_digest.clone().into(),
                        permission_key.clone().into(),
                    ],
                ))
                .await
                .map_err(storage_error)?;

            let definition = transaction
                .query_one_raw(Statement::from_sql_and_values(
                    backend,
                    definition_select_sql(backend)?,
                    vec![
                        scope_key.clone().into(),
                        request.installation_id.into(),
                        permission_key.clone().into(),
                    ],
                ))
                .await
                .map_err(storage_error)?
                .ok_or_else(|| {
                    PortError::invariant_violation(
                        "rbac.artifact_permission_definition_missing",
                        "scoped artifact permission definition disappeared during projection",
                    )
                })?;

            let persisted_scoped_id: Uuid =
                definition.try_get("", "id").map_err(storage_error)?;

            // Copy translations from release definition
            let translations = transaction
                .query_all_raw(Statement::from_sql_and_values(
                    backend,
                    release_translations_by_definition_sql(backend)?,
                    vec![release_permission_id.into()],
                ))
                .await
                .map_err(storage_error)?;

            for trans_row in translations {
                let locale: String = trans_row.try_get("", "locale").map_err(storage_error)?;
                let label: String = trans_row.try_get("", "label").map_err(storage_error)?;
                let description: String =
                    trans_row.try_get("", "description").map_err(storage_error)?;

                transaction
                    .execute_raw(Statement::from_sql_and_values(
                        backend,
                        translation_upsert_sql(backend)?,
                        vec![
                            rustok_core::generate_id().into(),
                            persisted_scoped_id.into(),
                            locale.into(),
                            label.into(),
                            description.into(),
                        ],
                    ))
                    .await
                    .map_err(storage_error)?;
            }
        }

        transaction.commit().await.map_err(storage_error)
    }

    async fn evaluate_permission_continuity(
        &self,
        request: PermissionContinuityEvaluationRequest,
    ) -> Result<ArtifactPermissionContinuityReceipt, PortError> {
        let candidate_fingerprint =
            compute_canonical_authorization_fingerprint(&request.candidate_permissions);
        let predecessor_fingerprint =
            compute_canonical_authorization_fingerprint(&request.predecessor_permissions);

        let pred_keys: BTreeSet<String> = request
            .predecessor_permissions
            .iter()
            .map(|p| p.key.clone())
            .collect();
        let cand_keys: BTreeSet<String> = request
            .candidate_permissions
            .iter()
            .map(|p| p.key.clone())
            .collect();

        let unchanged_keys: Vec<String> =
            pred_keys.intersection(&cand_keys).cloned().collect();
        let added_keys: Vec<String> = cand_keys.difference(&pred_keys).cloned().collect();
        let removed_dormant_keys: Vec<String> =
            pred_keys.difference(&cand_keys).cloned().collect();
        let modified_keys = Vec::new();

        let diff = ArtifactPermissionDiff {
            unchanged_keys,
            modified_keys,
            added_keys: added_keys.clone(),
            removed_dormant_keys: removed_dormant_keys.clone(),
        };

        let current_epoch = read_permission_invalidation_generation(&self.db)
            .await
            .unwrap_or(request.expected_rbac_epoch);

        let approved = (candidate_fingerprint == predecessor_fingerprint)
            && added_keys.is_empty()
            && removed_dormant_keys.is_empty()
            && (current_epoch == request.expected_rbac_epoch);

        let receipt_digest = compute_receipt_digest(
            &request.scope,
            &request.predecessor_release_digest,
            &request.candidate_release_digest,
            &candidate_fingerprint,
            current_epoch,
            approved,
            &diff,
        );

        Ok(ArtifactPermissionContinuityReceipt {
            scope: request.scope,
            predecessor_release_digest: request.predecessor_release_digest,
            candidate_release_digest: request.candidate_release_digest,
            authorization_fingerprint: candidate_fingerprint,
            rbac_epoch: current_epoch,
            diff,
            approved,
            receipt_digest,
        })
    }
}

fn compute_receipt_digest(
    scope: &ArtifactPermissionScope,
    predecessor_release_digest: &str,
    candidate_release_digest: &str,
    authorization_fingerprint: &str,
    rbac_epoch: u64,
    approved: bool,
    diff: &ArtifactPermissionDiff,
) -> String {
    let mut parts: Vec<&[u8]> = Vec::new();
    let scope_str = scope_key(scope);
    parts.push(scope_str.as_bytes());
    parts.push(predecessor_release_digest.as_bytes());
    parts.push(candidate_release_digest.as_bytes());
    parts.push(authorization_fingerprint.as_bytes());
    let epoch_bytes = rbac_epoch.to_be_bytes();
    parts.push(&epoch_bytes);
    let approved_byte = if approved { &[1_u8] } else { &[0_u8] };
    parts.push(approved_byte);
    for key in &diff.unchanged_keys {
        parts.push(key.as_bytes());
    }
    for key in &diff.added_keys {
        parts.push(key.as_bytes());
    }
    for key in &diff.removed_dormant_keys {
        parts.push(key.as_bytes());
    }
    let digest = rustok_api::digest::sha256_digest(&parts);
    format!("sha256:{}", digest.encode_hex::<String>())
}

async fn ensure_installation_identity(
    transaction: &DatabaseTransaction,
    backend: DbBackend,
    scope_key: &str,
    installation_id: Uuid,
    module_slug: &str,
    release_digest: &str,
) -> Result<(), PortError> {
    transaction
        .execute_raw(Statement::from_sql_and_values(
            backend,
            installation_insert_sql(backend)?,
            vec![
                installation_id.into(),
                scope_key.into(),
                module_slug.into(),
                release_digest.into(),
            ],
        ))
        .await
        .map_err(storage_error)?;

    let installation = transaction
        .query_one_raw(Statement::from_sql_and_values(
            backend,
            installation_select_sql(backend)?,
            vec![installation_id.into()],
        ))
        .await
        .map_err(storage_error)?
        .ok_or_else(|| {
            PortError::invariant_violation(
                "rbac.artifact_permission_installation_missing",
                "artifact permission installation identity disappeared during registration",
            )
        })?;

    let persisted_scope_key: String = installation
        .try_get("", "scope_key")
        .map_err(storage_error)?;
    let persisted_module_slug: String = installation
        .try_get("", "module_slug")
        .map_err(storage_error)?;
    let persisted_release_digest: String = installation
        .try_get("", "release_digest")
        .map_err(storage_error)?;

    if persisted_scope_key != scope_key
        || persisted_module_slug != module_slug
        || persisted_release_digest != release_digest
    {
        return Err(PortError::conflict(
            "rbac.artifact_permission_identity_conflict",
            "artifact installation identity is already bound to different admitted scope or metadata",
        ));
    }
    Ok(())
}

fn storage_error(error: impl std::fmt::Display) -> PortError {
    PortError::unavailable("rbac.artifact_permission_catalog", error.to_string())
}

fn validate_admission_request(request: &ReleasePermissionAdmissionRequest) -> Result<(), PortError> {
    if request.module_slug.trim().is_empty()
        || request.release_digest.trim().is_empty()
        || request.permissions.is_empty()
    {
        return Err(PortError::validation(
            "rbac.artifact_permission_registration_invalid",
            "release permission admission requires module slug, release digest, and permissions",
        ));
    }
    validate_permissions(&request.module_slug, &request.permissions)
}

fn validate_projection_request(
    request: &ScopedPermissionProjectionRequest,
) -> Result<(), PortError> {
    if request.installation_id.is_nil()
        || matches!(
            &request.scope,
            ArtifactPermissionScope::Tenant { tenant_id } if tenant_id.is_nil()
        )
        || request.module_slug.trim().is_empty()
        || request.release_digest.trim().is_empty()
    {
        return Err(PortError::validation(
            "rbac.artifact_permission_registration_invalid",
            "scoped permission projection requires valid installation identity, scope, slug, and release digest",
        ));
    }
    Ok(())
}

fn validate_permissions(
    module_slug: &str,
    permissions: &[ArtifactPermissionRegistration],
) -> Result<(), PortError> {
    let prefix = format!("{module_slug}.");
    let mut permission_keys = HashSet::new();
    for permission in permissions {
        if !permission.key.starts_with(&prefix)
            || permission.key.len() > MAX_PERMISSION_KEY_LENGTH
            || permission.key.trim() != permission.key
            || permission.key.chars().any(char::is_control)
            || !permission_keys.insert(permission.key.as_str())
            || permission.localizations.is_empty()
        {
            return Err(PortError::validation(
                "rbac.artifact_permission_registration_invalid",
                "artifact permissions must remain module-owned, bounded, unique and localized",
            ));
        }
        let mut normalized_locales = HashSet::new();
        for localization in &permission.localizations {
            let Some(normalized_locale) = normalize_locale_tag(&localization.locale) else {
                return Err(PortError::validation(
                    "rbac.artifact_permission_registration_invalid",
                    "artifact permission localizations must use valid canonical locale tags",
                ));
            };
            if localization.label.trim().is_empty()
                || localization.description.trim().is_empty()
                || !normalized_locales.insert(normalized_locale)
            {
                return Err(PortError::validation(
                    "rbac.artifact_permission_registration_invalid",
                    "artifact permission localizations must be non-empty and unique after locale normalization",
                ));
            }
        }
    }
    Ok(())
}

fn scope_key(scope: &ArtifactPermissionScope) -> String {
    match scope {
        ArtifactPermissionScope::Platform => "platform".to_string(),
        ArtifactPermissionScope::Tenant { tenant_id } => format!("tenant:{tenant_id}"),
    }
}

fn release_definition_insert_sql(backend: DbBackend) -> Result<&'static str, PortError> {
    match backend {
        DbBackend::Sqlite => Ok(
            "INSERT INTO rbac_artifact_release_permission_definitions (id, release_digest, module_slug, permission_key) VALUES (?1, ?2, ?3, ?4) ON CONFLICT (release_digest, permission_key) DO NOTHING",
        ),
        DbBackend::Postgres => Ok(
            "INSERT INTO rbac_artifact_release_permission_definitions (id, release_digest, module_slug, permission_key) VALUES ($1, $2, $3, $4) ON CONFLICT (release_digest, permission_key) DO NOTHING",
        ),
        backend => Err(PortError::validation(
            "rbac.artifact_permission_backend_unsupported",
            format!("artifact permission catalog does not support {backend:?}"),
        )),
    }
}

fn release_definition_select_sql(backend: DbBackend) -> Result<&'static str, PortError> {
    match backend {
        DbBackend::Sqlite => Ok(
            "SELECT id, module_slug FROM rbac_artifact_release_permission_definitions WHERE release_digest = ?1 AND permission_key = ?2 LIMIT 1",
        ),
        DbBackend::Postgres => Ok(
            "SELECT id, module_slug FROM rbac_artifact_release_permission_definitions WHERE release_digest = $1 AND permission_key = $2 LIMIT 1",
        ),
        backend => Err(PortError::validation(
            "rbac.artifact_permission_backend_unsupported",
            format!("artifact permission catalog does not support {backend:?}"),
        )),
    }
}

fn release_translation_upsert_sql(backend: DbBackend) -> Result<&'static str, PortError> {
    match backend {
        DbBackend::Sqlite => Ok(
            "INSERT INTO rbac_artifact_release_permission_translations (id, release_permission_id, locale, label, description) VALUES (?1, ?2, ?3, ?4, ?5) ON CONFLICT (release_permission_id, locale) DO UPDATE SET label = excluded.label, description = excluded.description",
        ),
        DbBackend::Postgres => Ok(
            "INSERT INTO rbac_artifact_release_permission_translations (id, release_permission_id, locale, label, description) VALUES ($1, $2, $3, $4, $5) ON CONFLICT (release_permission_id, locale) DO UPDATE SET label = EXCLUDED.label, description = EXCLUDED.description",
        ),
        backend => Err(PortError::validation(
            "rbac.artifact_permission_backend_unsupported",
            format!("artifact permission catalog does not support {backend:?}"),
        )),
    }
}

fn release_definitions_by_release_sql(backend: DbBackend) -> Result<&'static str, PortError> {
    match backend {
        DbBackend::Sqlite => Ok(
            "SELECT id, permission_key FROM rbac_artifact_release_permission_definitions WHERE release_digest = ?1 AND module_slug = ?2",
        ),
        DbBackend::Postgres => Ok(
            "SELECT id, permission_key FROM rbac_artifact_release_permission_definitions WHERE release_digest = $1 AND module_slug = $2",
        ),
        backend => Err(PortError::validation(
            "rbac.artifact_permission_backend_unsupported",
            format!("artifact permission catalog does not support {backend:?}"),
        )),
    }
}

fn release_translations_by_definition_sql(backend: DbBackend) -> Result<&'static str, PortError> {
    match backend {
        DbBackend::Sqlite => Ok(
            "SELECT locale, label, description FROM rbac_artifact_release_permission_translations WHERE release_permission_id = ?1",
        ),
        DbBackend::Postgres => Ok(
            "SELECT locale, label, description FROM rbac_artifact_release_permission_translations WHERE release_permission_id = $1",
        ),
        backend => Err(PortError::validation(
            "rbac.artifact_permission_backend_unsupported",
            format!("artifact permission catalog does not support {backend:?}"),
        )),
    }
}

fn installation_insert_sql(backend: DbBackend) -> Result<&'static str, PortError> {
    match backend {
        DbBackend::Sqlite => Ok(
            "INSERT INTO rbac_artifact_permission_installations (installation_id, scope_key, module_slug, release_digest) VALUES (?1, ?2, ?3, ?4) ON CONFLICT (installation_id) DO NOTHING",
        ),
        DbBackend::Postgres => Ok(
            "INSERT INTO rbac_artifact_permission_installations (installation_id, scope_key, module_slug, release_digest) VALUES ($1, $2, $3, $4) ON CONFLICT (installation_id) DO NOTHING",
        ),
        backend => Err(PortError::validation(
            "rbac.artifact_permission_backend_unsupported",
            format!("artifact permission catalog does not support {backend:?}"),
        )),
    }
}

fn installation_select_sql(backend: DbBackend) -> Result<&'static str, PortError> {
    match backend {
        DbBackend::Sqlite => Ok(
            "SELECT scope_key, module_slug, release_digest FROM rbac_artifact_permission_installations WHERE installation_id = ?1 LIMIT 1",
        ),
        DbBackend::Postgres => Ok(
            "SELECT scope_key, module_slug, release_digest FROM rbac_artifact_permission_installations WHERE installation_id = $1 LIMIT 1",
        ),
        backend => Err(PortError::validation(
            "rbac.artifact_permission_backend_unsupported",
            format!("artifact permission catalog does not support {backend:?}"),
        )),
    }
}

fn definition_insert_sql(backend: DbBackend) -> Result<&'static str, PortError> {
    match backend {
        DbBackend::Sqlite => Ok(
            "INSERT INTO rbac_artifact_permission_definitions (id, scope_key, installation_id, module_slug, release_digest, permission_key) VALUES (?1, ?2, ?3, ?4, ?5, ?6) ON CONFLICT (scope_key, installation_id, permission_key) DO NOTHING",
        ),
        DbBackend::Postgres => Ok(
            "INSERT INTO rbac_artifact_permission_definitions (id, scope_key, installation_id, module_slug, release_digest, permission_key) VALUES ($1, $2, $3, $4, $5, $6) ON CONFLICT (scope_key, installation_id, permission_key) DO NOTHING",
        ),
        backend => Err(PortError::validation(
            "rbac.artifact_permission_backend_unsupported",
            format!("artifact permission catalog does not support {backend:?}"),
        )),
    }
}

fn definition_select_sql(backend: DbBackend) -> Result<&'static str, PortError> {
    match backend {
        DbBackend::Sqlite => Ok(
            "SELECT id, module_slug, release_digest FROM rbac_artifact_permission_definitions WHERE scope_key = ?1 AND installation_id = ?2 AND permission_key = ?3 LIMIT 1",
        ),
        DbBackend::Postgres => Ok(
            "SELECT id, module_slug, release_digest FROM rbac_artifact_permission_definitions WHERE scope_key = $1 AND installation_id = $2 AND permission_key = $3 LIMIT 1",
        ),
        backend => Err(PortError::validation(
            "rbac.artifact_permission_backend_unsupported",
            format!("artifact permission catalog does not support {backend:?}"),
        )),
    }
}

fn translation_upsert_sql(backend: DbBackend) -> Result<&'static str, PortError> {
    match backend {
        DbBackend::Sqlite => Ok(
            "INSERT INTO rbac_artifact_permission_translations (id, artifact_permission_id, locale, label, description) VALUES (?1, ?2, ?3, ?4, ?5) ON CONFLICT (artifact_permission_id, locale) DO UPDATE SET label = excluded.label, description = excluded.description",
        ),
        DbBackend::Postgres => Ok(
            "INSERT INTO rbac_artifact_permission_translations (id, artifact_permission_id, locale, label, description) VALUES ($1, $2, $3, $4, $5) ON CONFLICT (artifact_permission_id, locale) DO UPDATE SET label = EXCLUDED.label, description = EXCLUDED.description",
        ),
        backend => Err(PortError::validation(
            "rbac.artifact_permission_backend_unsupported",
            format!("artifact permission catalog does not support {backend:?}"),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustok_api::{ArtifactPermissionLocalization, ArtifactPermissionRegistration};
    use sea_orm::{ConnectionTrait, Database, DatabaseConnection};
    use sea_orm_migration::prelude::{MigrationTrait, SchemaManager};

    fn sample_permissions() -> Vec<ArtifactPermissionRegistration> {
        vec![ArtifactPermissionRegistration {
            key: "sample_module.events.handle".to_string(),
            localizations: vec![ArtifactPermissionLocalization {
                locale: "en".to_string(),
                label: "Handle event".to_string(),
                description: "Allows handling an admitted event".to_string(),
            }],
        }]
    }

    async fn migrate_catalog(database: &DatabaseConnection) {
        database
            .execute_unprepared("PRAGMA foreign_keys = ON")
            .await
            .expect("enable SQLite foreign keys");
        for statement in [
            "CREATE TABLE users (id TEXT PRIMARY KEY, tenant_id TEXT NOT NULL)",
            "CREATE TABLE roles (id TEXT PRIMARY KEY, tenant_id TEXT NOT NULL)",
        ] {
            database
                .execute_unprepared(statement)
                .await
                .expect("create RBAC catalog parent table");
        }
        let manager = SchemaManager::new(database);
        super::super::m20260714_900002_create_rbac_invalidation_state::Migration
            .up(&manager)
            .await
            .expect("apply invalidation state migration");
        super::super::m20260716_000001_artifact_permission_catalog::Migration
            .up(&manager)
            .await
            .expect("apply legacy catalog migration");
        super::super::m20260717_000001_artifact_role_permissions::Migration
            .up(&manager)
            .await
            .expect("apply legacy artifact grant migration");
        super::super::m20260803_000001_canonicalize_artifact_permissions::Migration
            .up(&manager)
            .await
            .expect("apply canonical artifact permission cutover");
    }

    #[tokio::test]
    async fn admission_and_scoped_projection_are_idempotent() {
        let database = Database::connect("sqlite::memory:")
            .await
            .expect("database");
        migrate_catalog(&database).await;
        let catalog = RbacArtifactPermissionCatalog::new(database.clone());
        let release_digest = format!("sha256:{}", "a".repeat(64));

        let mut permissions = sample_permissions();
        permissions[0].localizations[0].locale = "EN_us".to_string();

        catalog
            .admit_release_permissions(ReleasePermissionAdmissionRequest {
                module_slug: "sample_module".to_string(),
                release_digest: release_digest.clone(),
                permissions: permissions.clone(),
            })
            .await
            .expect("initial admission");

        // Idempotent retry with normalized locale
        permissions[0].localizations[0].locale = "en-US".to_string();
        catalog
            .admit_release_permissions(ReleasePermissionAdmissionRequest {
                module_slug: "sample_module".to_string(),
                release_digest: release_digest.clone(),
                permissions,
            })
            .await
            .expect("idempotent admission retry");

        // Now project into scoped installation
        let installation_id = Uuid::new_v4();
        catalog
            .project_scoped_permissions(ScopedPermissionProjectionRequest {
                scope: ArtifactPermissionScope::Platform,
                installation_id,
                module_slug: "sample_module".to_string(),
                release_digest: release_digest.clone(),
            })
            .await
            .expect("initial projection");

        // Idempotent retry of projection
        catalog
            .project_scoped_permissions(ScopedPermissionProjectionRequest {
                scope: ArtifactPermissionScope::Platform,
                installation_id,
                module_slug: "sample_module".to_string(),
                release_digest: release_digest.clone(),
            })
            .await
            .expect("idempotent projection retry");

        for (table, expected) in [
            ("rbac_artifact_release_permission_definitions", 1_i64),
            ("rbac_artifact_release_permission_translations", 1_i64),
            ("rbac_artifact_permission_definitions", 1_i64),
            ("rbac_artifact_permission_translations", 1_i64),
        ] {
            let row = database
                .query_one_raw(Statement::from_string(
                    DbBackend::Sqlite,
                    format!("SELECT COUNT(*) AS count FROM {table}"),
                ))
                .await
                .expect("catalog query")
                .expect("catalog row");
            let count: i64 = row.try_get("", "count").expect("count");
            assert_eq!(count, expected);
        }
    }

    #[tokio::test]
    async fn projection_rejects_identity_rebinding() {
        let database = Database::connect("sqlite::memory:")
            .await
            .expect("database");
        migrate_catalog(&database).await;
        let catalog = RbacArtifactPermissionCatalog::new(database);
        let installation_id = Uuid::new_v4();
        let release_digest = format!("sha256:{}", "a".repeat(64));

        catalog
            .admit_release_permissions(ReleasePermissionAdmissionRequest {
                module_slug: "sample_module".to_string(),
                release_digest: release_digest.clone(),
                permissions: sample_permissions(),
            })
            .await
            .expect("admission");

        catalog
            .project_scoped_permissions(ScopedPermissionProjectionRequest {
                scope: ArtifactPermissionScope::Platform,
                installation_id,
                module_slug: "sample_module".to_string(),
                release_digest: release_digest.clone(),
            })
            .await
            .expect("projection");

        let error = catalog
            .project_scoped_permissions(ScopedPermissionProjectionRequest {
                scope: ArtifactPermissionScope::Platform,
                installation_id,
                module_slug: "sample_module".to_string(),
                release_digest: format!("sha256:{}", "b".repeat(64)),
            })
            .await
            .expect_err("identity rebinding must fail closed");

        assert_eq!(error.code, "rbac.artifact_permission_identity_conflict");
    }

    #[tokio::test]
    async fn projection_rejects_installation_scope_rebinding() {
        let database = Database::connect("sqlite::memory:")
            .await
            .expect("database");
        migrate_catalog(&database).await;
        let catalog = RbacArtifactPermissionCatalog::new(database);
        let installation_id = Uuid::new_v4();
        let release_digest = format!("sha256:{}", "a".repeat(64));

        catalog
            .admit_release_permissions(ReleasePermissionAdmissionRequest {
                module_slug: "sample_module".to_string(),
                release_digest: release_digest.clone(),
                permissions: sample_permissions(),
            })
            .await
            .expect("admission");

        catalog
            .project_scoped_permissions(ScopedPermissionProjectionRequest {
                scope: ArtifactPermissionScope::Platform,
                installation_id,
                module_slug: "sample_module".to_string(),
                release_digest: release_digest.clone(),
            })
            .await
            .expect("projection");

        let error = catalog
            .project_scoped_permissions(ScopedPermissionProjectionRequest {
                scope: ArtifactPermissionScope::Tenant {
                    tenant_id: Uuid::new_v4(),
                },
                installation_id,
                module_slug: "sample_module".to_string(),
                release_digest: release_digest.clone(),
            })
            .await
            .expect_err("scope rebinding must fail closed");

        assert_eq!(error.code, "rbac.artifact_permission_identity_conflict");
    }

    #[tokio::test]
    async fn continuity_evaluation_verifies_authorization_fingerprint_invariance() {
        let database = Database::connect("sqlite::memory:")
            .await
            .expect("database");
        migrate_catalog(&database).await;
        let catalog = RbacArtifactPermissionCatalog::new(database);

        let pred_permissions = sample_permissions();
        let mut cand_permissions = sample_permissions();
        // Change ONLY display text/localizations in candidate
        cand_permissions[0].localizations[0].label = "Updated event label".to_string();
        cand_permissions[0].localizations[0].description = "New description".to_string();

        let receipt = catalog
            .evaluate_permission_continuity(PermissionContinuityEvaluationRequest {
                scope: ArtifactPermissionScope::Platform,
                predecessor_release_digest: format!("sha256:{}", "1".repeat(64)),
                candidate_release_digest: format!("sha256:{}", "2".repeat(64)),
                predecessor_permissions: pred_permissions,
                candidate_permissions: cand_permissions,
                expected_rbac_epoch: 0,
            })
            .await
            .expect("continuity evaluation");

        // Even though labels changed, authorization fingerprint is identical and approved is true!
        assert!(receipt.approved);
        assert_eq!(receipt.diff.unchanged_keys, vec!["sample_module.events.handle"]);
        assert!(receipt.diff.added_keys.is_empty());
        assert!(receipt.diff.removed_dormant_keys.is_empty());

        // Now add a new permission key in candidate
        let mut cand_permissions_modified = sample_permissions();
        cand_permissions_modified.push(ArtifactPermissionRegistration {
            key: "sample_module.admin.delete".to_string(),
            localizations: vec![ArtifactPermissionLocalization {
                locale: "en".to_string(),
                label: "Delete".to_string(),
                description: "Delete item".to_string(),
            }],
        });

        let receipt_modified = catalog
            .evaluate_permission_continuity(PermissionContinuityEvaluationRequest {
                scope: ArtifactPermissionScope::Platform,
                predecessor_release_digest: format!("sha256:{}", "1".repeat(64)),
                candidate_release_digest: format!("sha256:{}", "3".repeat(64)),
                predecessor_permissions: sample_permissions(),
                candidate_permissions: cand_permissions_modified,
                expected_rbac_epoch: 0,
            })
            .await
            .expect("continuity evaluation");

        // Key set changed -> approved must be false, requiring explicit operator approval!
        assert!(!receipt_modified.approved);
        assert_eq!(receipt_modified.diff.added_keys, vec!["sample_module.admin.delete"]);
    }
}
