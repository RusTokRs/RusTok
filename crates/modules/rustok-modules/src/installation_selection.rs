//! Shared tenant/platform precedence for active artifact installations.
//!
//! Observe every eligible row before parsing descriptor contents. A corrupt
//! or ambiguous winning scope must never cause a fallback to a lower scope.

use std::collections::{BTreeMap, btree_map::Entry};

use sea_orm::{DbErr, QueryResult};

use crate::ModuleArtifactDescriptor;

/// Decode a descriptor and compare it to its persisted immutable identity.
/// Projection callers must resolve scope precedence before invoking this.
/// None denotes corrupt or inconsistent identity; SQL decoding errors remain
/// storage failures rather than being silently omitted.
pub(crate) fn descriptor_with_identity(
    row: &QueryResult,
) -> Result<Option<ModuleArtifactDescriptor>, DbErr> {
    let slug: String = row.try_get("", "slug")?;
    let version: String = row.try_get("", "version")?;
    let payload_digest: String = row.try_get("", "payload_digest")?;
    let admission_digest: String = row.try_get("", "admission_payload_digest")?;
    let descriptor_text: String = row.try_get("", "descriptor")?;
    let Ok(descriptor) = serde_json::from_str::<ModuleArtifactDescriptor>(&descriptor_text) else {
        return Ok(None);
    };
    if descriptor.validate().is_err()
        || descriptor.slug != slug
        || descriptor.version != version
        || descriptor.artifact_digest != payload_digest
        || descriptor.artifact_digest != admission_digest
    {
        return Ok(None);
    }
    Ok(Some(descriptor))
}

/// Reduce already tenant-scoped, active rows before parsing descriptors. A
/// malformed winning descriptor will be handled by the caller without ever
/// exposing a lower-priority candidate for that slug.
pub(crate) struct ProjectionSelection {
    pub(crate) rows: Vec<QueryResult>,
    pub(crate) ambiguous_count: usize,
}

pub(crate) fn select_projection_rows(rows: Vec<QueryResult>) -> Result<ProjectionSelection, DbErr> {
    let mut selection = EffectiveInstallationSelection::new();
    for row in rows {
        let slug = row.try_get::<String>("", "slug")?;
        let tenant_scoped = match row.try_get::<String>("", "scope_kind")?.as_str() {
            "tenant" => true,
            "platform" => false,
            _ => return Err(DbErr::Custom("active artifact has an invalid scope".to_string())),
        };
        selection.observe(slug, tenant_scoped, row);
    }
    let mut result = ProjectionSelection {
        rows: Vec::new(),
        ambiguous_count: 0,
    };
    for candidate in selection.finish().into_values() {
        match candidate {
            Some(row) => result.rows.push(row),
            None => result.ambiguous_count += 1,
        }
    }
    Ok(result)
}

struct Candidate<T> {
    tenant_scoped: bool,
    // None means two or more eligible installations at the winning scope.
    value: Option<T>,
}

pub(crate) struct EffectiveInstallationSelection<T> {
    candidates: BTreeMap<String, Candidate<T>>,
}

impl<T> EffectiveInstallationSelection<T> {
    pub(crate) fn new() -> Self {
        Self {
            candidates: BTreeMap::new(),
        }
    }

    pub(crate) fn observe(&mut self, slug: String, tenant_scoped: bool, value: T) {
        match self.candidates.entry(slug) {
            Entry::Vacant(entry) => {
                entry.insert(Candidate {
                    tenant_scoped,
                    value: Some(value),
                });
            }
            Entry::Occupied(mut entry) => {
                let current = entry.get_mut();
                if tenant_scoped && !current.tenant_scoped {
                    *current = Candidate {
                        tenant_scoped,
                        value: Some(value),
                    };
                } else if tenant_scoped == current.tenant_scoped {
                    current.value = None;
                }
            }
        }
    }

    /// Each slug maps to its sole winner or `None` if the winning scope is
    /// ambiguous. Ordering is deterministic and independent of row order.
    pub(crate) fn finish(self) -> BTreeMap<String, Option<T>> {
        self.candidates
            .into_iter()
            .map(|(slug, candidate)| (slug, candidate.value))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn permutations<T: Clone>(values: &[T]) -> Vec<Vec<T>> {
        if values.is_empty() {
            return vec![Vec::new()];
        }
        let mut result = Vec::new();
        for (index, value) in values.iter().enumerate() {
            let mut remainder = values.to_vec();
            remainder.remove(index);
            for mut suffix in permutations(&remainder) {
                suffix.insert(0, value.clone());
                result.push(suffix);
            }
        }
        result
    }

    #[test]
    fn tenant_wins_over_duplicate_platform_in_every_order() {
        let rows = [(false, "platform-1"), (false, "platform-2"), (true, "tenant")];
        for order in permutations(&rows) {
            let mut selection = EffectiveInstallationSelection::new();
            for (tenant, id) in order {
                selection.observe("module".into(), tenant, id);
            }
            assert_eq!(selection.finish().get("module"), Some(&Some("tenant")));
        }
    }

    #[test]
    fn duplicate_tenant_is_ambiguous_in_every_order() {
        let rows = [(false, "platform"), (true, "tenant-1"), (true, "tenant-2")];
        for order in permutations(&rows) {
            let mut selection = EffectiveInstallationSelection::new();
            for (tenant, id) in order {
                selection.observe("module".into(), tenant, id);
            }
            assert_eq!(selection.finish().get("module"), Some(&None));
        }
    }

    #[test]
    fn corrupt_winner_is_not_replaced_by_a_valid_lower_scope() {
        for order in permutations(&[(false, Some("platform")), (true, None)]) {
            let mut selection = EffectiveInstallationSelection::new();
            for (tenant, descriptor) in order {
                selection.observe("module".into(), tenant, descriptor);
            }
            assert_eq!(selection.finish().get("module"), Some(&Some(None)));
        }
    }

    #[test]
    fn ambiguity_is_local_to_one_slug_and_does_not_clear_other_winners() {
        let mut selection = EffectiveInstallationSelection::new();
        selection.observe("ambiguous".into(), false, 1);
        selection.observe("other".into(), false, 2);
        selection.observe("ambiguous".into(), false, 3);
        let selected = selection.finish();
        assert_eq!(selected.get("ambiguous"), Some(&None));
        assert_eq!(selected.get("other"), Some(&Some(2)));
    }
}

#[cfg(test)]
pub(crate) mod test_support {
    use sea_orm::{ConnectionTrait, Database, DatabaseConnection, DbBackend, Statement};
    use serde_json::json;
    use uuid::Uuid;

    use crate::{
        ArtifactModuleKind, ArtifactPayloadKind, ArtifactPermissionDescriptor,
        ArtifactSchemaDocument, ModuleArtifactDescriptor, ModuleBindingIdempotency,
        ModuleRuntimeBinding, ModuleRuntimeBindingKind, ModuleScheduleBinding,
        ModuleScheduleDeduplication, ModuleScheduleMisfirePolicy, ModuleScheduleOverlapPolicy,
        canonical_schema_digest,
    };

    pub(crate) struct ProjectionFixture {
        pub db: DatabaseConnection,
        pub tenant: Uuid,
        pub platform: Uuid,
        pub platform_duplicate: Uuid,
        pub tenant_winner: Uuid,
        pub tenant_duplicate: Uuid,
        pub other: Uuid,
    }

    fn descriptor(slug: &str) -> String {
        let schema = json!({"$schema": "https://json-schema.org/draft/2020-12/schema", "type": "object"});
        let digest = canonical_schema_digest(&schema);
        let permission = format!("{slug}.deliver");
        let binding = |id: &str, kind, event_topics, schedule| ModuleRuntimeBinding {
            id: id.to_string(),
            kind,
            entrypoint: "main".to_string(),
            input_schema_digest: digest.clone(),
            output_schema_digest: digest.clone(),
            permission: permission.clone(),
            idempotency: ModuleBindingIdempotency::Required,
            limit_profile: "event".to_string(),
            capabilities: Vec::new(),
            event_topics,
            schedule,
            http: None,
        };
        let descriptor = ModuleArtifactDescriptor {
            schema_version: crate::MODULE_ARTIFACT_DESCRIPTOR_SCHEMA_VERSION,
            slug: slug.to_string(),
            version: "1.0.0".to_string(),
            payload_kind: ArtifactPayloadKind::Rhai,
            module_kind: ArtifactModuleKind::Optional,
            runtime_abi: "rustok:module/runtime@1".to_string(),
            platform_compatibility: "^0.1".to_string(),
            required_features: Vec::new(),
            artifact_digest: format!("sha256:{}", "a".repeat(64)),
            entrypoint: "main".to_string(),
            capabilities: Vec::new(),
            bindings: vec![
                binding("event_handler", ModuleRuntimeBindingKind::Event, vec!["sample.event".into()], None),
                binding("schedule_handler", ModuleRuntimeBindingKind::Schedule, Vec::new(), Some(ModuleScheduleBinding {
                    cron: "*/10 * * * *".to_string(),
                    timezone: "UTC".to_string(),
                    misfire: ModuleScheduleMisfirePolicy::Skip,
                    overlap: ModuleScheduleOverlapPolicy::Queue,
                    deduplication: ModuleScheduleDeduplication::PerSlot,
                })),
            ],
            dependencies: Vec::new(),
            permissions: vec![ArtifactPermissionDescriptor {
                key: permission.clone(),
                localizations: vec![rustok_api::ArtifactPermissionLocalization {
                    locale: "en".into(),
                    label: "Deliver".into(),
                    description: "Deliver work".into(),
                }],
            }],
            schema_documents: vec![ArtifactSchemaDocument { digest: digest.clone(), document: schema }],
            settings_schema_digest: None,
            data_schema_digest: None,
            localization_catalogs: Vec::new(),
            ui_contributions: Vec::new(),
            persistence_contract: None,
        };
        descriptor.validate().expect("valid projection fixture");
        serde_json::to_string(&descriptor).unwrap()
    }

    impl ProjectionFixture {
        pub(crate) async fn new() -> Self {
            let db = Database::connect("sqlite::memory:").await.unwrap();
            for sql in [
                "CREATE TABLE module_artifact_installations (installation_id TEXT PRIMARY KEY, slug TEXT NOT NULL, scope_kind TEXT NOT NULL, tenant_id TEXT, descriptor TEXT NOT NULL, version TEXT NOT NULL DEFAULT '1.0.0', payload_digest TEXT NOT NULL DEFAULT 'sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa')",
                "CREATE TABLE module_artifact_admissions (installation_id TEXT PRIMARY KEY, status TEXT NOT NULL, payload_digest TEXT NOT NULL DEFAULT 'sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa')",
                "CREATE TABLE module_artifact_tenant_lifecycle (installation_id TEXT, tenant_id TEXT, enabled INTEGER)",
                "CREATE TABLE module_artifact_uninstall_operations (installation_id TEXT)",
            ] {
                db.execute_raw(Statement::from_string(DbBackend::Sqlite, sql.to_string())).await.unwrap();
            }
            let fixture = Self {
                db,
                tenant: Uuid::new_v4(),
                platform: Uuid::new_v4(),
                platform_duplicate: Uuid::new_v4(),
                tenant_winner: Uuid::new_v4(),
                tenant_duplicate: Uuid::new_v4(),
                other: Uuid::new_v4(),
            };
            // Insert duplicates before the tenant row to expose the previous
            // order-dependent exclusion bug.
            for (id, slug, tenant_scoped, value, status) in [
                (fixture.platform, "sample_module", false, descriptor("sample_module"), "active"),
                (fixture.platform_duplicate, "sample_module", false, descriptor("sample_module"), "active"),
                (fixture.tenant_winner, "sample_module", true, "broken JSON".to_string(), "active"),
                (fixture.tenant_duplicate, "sample_module", true, descriptor("sample_module"), "inactive"),
                (fixture.other, "other_module", false, descriptor("other_module"), "active"),
            ] {
                fixture.db.execute_raw(Statement::from_sql_and_values(
                    DbBackend::Sqlite,
                    "INSERT INTO module_artifact_installations (installation_id, slug, scope_kind, tenant_id, descriptor) VALUES (?1, ?2, ?3, ?4, ?5)",
                    vec![id.to_string().into(), slug.into(), if tenant_scoped { "tenant" } else { "platform" }.into(),
                         tenant_scoped.then(|| fixture.tenant.to_string()).into(), value.into()],
                )).await.unwrap();
                fixture.db.execute_raw(Statement::from_sql_and_values(
                    DbBackend::Sqlite,
                    "INSERT INTO module_artifact_admissions (installation_id, status) VALUES (?1, ?2)",
                    vec![id.to_string().into(), status.into()],
                )).await.unwrap();
            }
            fixture
        }

        /// Add many eligible duplicate candidates with small invalid descriptors.
        /// The duplicate slug avoids parsing a descriptor for each row.
        pub(crate) async fn add_limit_candidates(&self, count: usize) {
            assert!(count > 0);
            self.db.execute_raw(Statement::from_sql_and_values(
                DbBackend::Sqlite,
                "WITH RECURSIVE sequence(n) AS (SELECT 1 UNION ALL SELECT n + 1 FROM sequence WHERE n < ?1) \
                 INSERT INTO module_artifact_installations (installation_id, slug, scope_kind, tenant_id, descriptor) \
                 SELECT 'limit-' || n, 'limit_module', 'platform', NULL, 'invalid' FROM sequence",
                vec![i64::try_from(count).unwrap().into()],
            )).await.unwrap();
            self.db.execute_raw(Statement::from_string(
                DbBackend::Sqlite,
                "INSERT INTO module_artifact_admissions (installation_id, status) \
                 SELECT installation_id, 'active' FROM module_artifact_installations WHERE slug = 'limit_module'".to_string(),
            )).await.unwrap();
        }

        pub(crate) async fn repair_tenant(&self) {
            self.db.execute_raw(Statement::from_sql_and_values(
                DbBackend::Sqlite,
                "UPDATE module_artifact_installations SET descriptor = ?1 WHERE installation_id = ?2",
                vec![descriptor("sample_module").into(), self.tenant_winner.to_string().into()],
            )).await.unwrap();
        }

        pub(crate) async fn set_tenant_enabled(&self, enabled: bool) {
            self.db.execute_raw(Statement::from_sql_and_values(
                DbBackend::Sqlite,
                "DELETE FROM module_artifact_tenant_lifecycle WHERE installation_id = ?1 AND tenant_id = ?2",
                vec![self.tenant_winner.to_string().into(), self.tenant.to_string().into()],
            )).await.unwrap();
            self.db.execute_raw(Statement::from_sql_and_values(
                DbBackend::Sqlite,
                "INSERT INTO module_artifact_tenant_lifecycle (installation_id, tenant_id, enabled) VALUES (?1, ?2, ?3)",
                vec![self.tenant_winner.to_string().into(), self.tenant.to_string().into(), (if enabled { 1_i32 } else { 0_i32 }).into()],
            )).await.unwrap();
        }

        pub(crate) async fn set_active(&self, id: Uuid, active: bool) {
            self.db.execute_raw(Statement::from_sql_and_values(
                DbBackend::Sqlite,
                "UPDATE module_artifact_admissions SET status = ?1 WHERE installation_id = ?2",
                vec![if active { "active" } else { "inactive" }.into(), id.to_string().into()],
            )).await.unwrap();
        }
    }
}
