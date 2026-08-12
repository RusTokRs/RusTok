use std::io::{self, Write};

use rustok_api::{Action, Resource};
use rustok_core::{PermissionScope, SecurityContext};
use rustok_page_builder::{
    ComponentRegistryManifest, LandingSectionSnapshot,
    PageBuilderMaterializedStaticLandingArtifact, PageBuilderStaticLandingMaterializationIdentity,
    PageHead, StaticLandingArtifact, StaticLandingBuildIdentity, StaticLandingPage,
};
use sea_orm::{
    ColumnTrait, ConnectionTrait, DbBackend, EntityTrait, QueryFilter, QueryOrder, QuerySelect,
    TransactionTrait,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use tracing::instrument;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::entities::{page, page_static_landing_artifact};
use crate::error::{PagesError, PagesResult};

use super::PageService;

pub const DEFAULT_PAGE_ARTIFACT_AUDIT_RECORDS: u32 = 128;
pub const MAX_PAGE_ARTIFACT_AUDIT_RECORDS: u32 = 512;
pub const MAX_PAGE_ARTIFACT_AUDIT_FINDINGS: usize = 64;
pub const PAGE_ARTIFACT_INTEGRITY_INVALID: &str = "PAGE_ARTIFACT_INTEGRITY_INVALID";
pub const PAGE_ARTIFACT_INTEGRITY_AUDIT_FORMAT: &str =
    "pages_immutable_artifact_integrity_audit_v1";

const MAX_DOCUMENT_HTML_BYTES: usize = 2 * 1024 * 1024;
const MAX_BODY_HTML_BYTES: usize = 1536 * 1024;
const MAX_CSS_BYTES: usize = 512 * 1024;
const CANONICAL_ARTIFACT_INSTANCE_KEY: &str = "canonical";
const REBUILD_ARTIFACT_INSTANCE_PREFIX: &str = "rebuild:";

#[derive(Debug, Clone, Default, Serialize, Deserialize, ToSchema)]
pub struct AuditPageArtifactsInput {
    pub max_records: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct PageArtifactIntegrityFinding {
    pub artifact_id: Uuid,
    pub locale_hash: String,
    pub record_identity_hash: String,
    pub code: String,
    pub diagnostic_hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct PageArtifactIntegrityAuditResult {
    pub format: String,
    pub page_id: Uuid,
    pub max_records: u32,
    pub scanned_artifact_count: u32,
    pub valid_artifact_count: u32,
    pub invalid_artifact_count: u32,
    pub truncated: bool,
    pub findings_truncated: bool,
    pub findings: Vec<PageArtifactIntegrityFinding>,
    pub audit_hash: String,
}

#[derive(Debug, Clone, Serialize)]
struct ArtifactAuditEntry {
    artifact_id: Uuid,
    locale_hash: String,
    record_identity_hash: String,
    status: &'static str,
    diagnostic_hash: Option<String>,
}

impl PageService {
    /// Audits immutable Page Builder artifacts for one Pages page without mutating any record.
    ///
    /// The command is tenant-scoped, requires tenant-wide `pages:manage`, reads through one
    /// transaction, and intentionally returns only bounded hashed identity and diagnostics. It never
    /// returns locale text, stored hashes, document HTML, CSS, runtime snapshots, materialization
    /// identity payloads or internal errors.
    #[instrument(skip(self, security, input), fields(tenant_id = %tenant_id, page_id = %page_id))]
    pub async fn audit_immutable_artifact_integrity(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        page_id: Uuid,
        input: AuditPageArtifactsInput,
    ) -> PagesResult<PageArtifactIntegrityAuditResult> {
        enforce_tenant_wide_manage(&security)?;
        if tenant_id.is_nil() {
            return Err(PagesError::validation(
                "Immutable artifact audit tenant must not be nil",
            ));
        }
        if page_id.is_nil() {
            return Err(PagesError::validation(
                "Immutable artifact audit page must not be nil",
            ));
        }
        let max_records = normalize_max_records(input.max_records)?;
        let fetch_limit = u64::from(max_records).saturating_add(1);
        let txn = self.db.begin().await?;

        let page_query =
            || page::Entity::find_by_id(page_id).filter(page::Column::TenantId.eq(tenant_id));
        let page = match txn.get_database_backend() {
            DbBackend::Sqlite => page_query().one(&txn).await?,
            DbBackend::Postgres | DbBackend::MySql => page_query().lock_shared().one(&txn).await?,
        };
        if page.is_none() {
            return Err(PagesError::PageNotFound(page_id));
        }

        let artifact_id_query = || {
            page_static_landing_artifact::Entity::find()
                .filter(page_static_landing_artifact::Column::TenantId.eq(tenant_id))
                .filter(page_static_landing_artifact::Column::PageId.eq(page_id))
                .select_only()
                .column(page_static_landing_artifact::Column::Id)
                .order_by_asc(page_static_landing_artifact::Column::CreatedAt)
                .order_by_asc(page_static_landing_artifact::Column::Id)
                .limit(fetch_limit)
        };
        let mut artifact_ids = match txn.get_database_backend() {
            DbBackend::Sqlite => artifact_id_query().into_tuple::<Uuid>().all(&txn).await?,
            DbBackend::Postgres | DbBackend::MySql => {
                artifact_id_query()
                    .lock_shared()
                    .into_tuple::<Uuid>()
                    .all(&txn)
                    .await?
            }
        };
        let truncated = artifact_ids.len() > max_records as usize;
        if truncated {
            artifact_ids.pop();
        }

        let mut valid_artifact_count = 0_u32;
        let mut invalid_artifact_count = 0_u32;
        let mut findings = Vec::new();
        let mut findings_truncated = false;
        let mut entries = Vec::with_capacity(artifact_ids.len());

        for artifact_id in &artifact_ids {
            let record_query = || {
                page_static_landing_artifact::Entity::find_by_id(*artifact_id)
                    .filter(page_static_landing_artifact::Column::TenantId.eq(tenant_id))
                    .filter(page_static_landing_artifact::Column::PageId.eq(page_id))
            };
            let record = match txn.get_database_backend() {
                DbBackend::Sqlite => record_query().one(&txn).await?,
                DbBackend::Postgres | DbBackend::MySql => {
                    record_query().lock_shared().one(&txn).await?
                }
            }
            .ok_or_else(|| {
                PagesError::artifact_integrity(
                    "Immutable artifact audit selected a record that is no longer readable",
                )
            })?;

            let locale_hash = hex_sha256(record.locale.as_bytes());
            let record_identity_hash = artifact_record_identity_hash(&record)?;
            let (status, diagnostic_hash) =
                match verify_artifact_record(&record, tenant_id, page_id) {
                    Ok(()) => {
                        valid_artifact_count = valid_artifact_count.saturating_add(1);
                        ("valid", None)
                    }
                    Err(error) => {
                        invalid_artifact_count = invalid_artifact_count.saturating_add(1);
                        let diagnostic_hash = hex_sha256(error.to_string().as_bytes());
                        if findings.len() < MAX_PAGE_ARTIFACT_AUDIT_FINDINGS {
                            findings.push(PageArtifactIntegrityFinding {
                                artifact_id: record.id,
                                locale_hash: locale_hash.clone(),
                                record_identity_hash: record_identity_hash.clone(),
                                code: PAGE_ARTIFACT_INTEGRITY_INVALID.to_string(),
                                diagnostic_hash: diagnostic_hash.clone(),
                            });
                        } else {
                            findings_truncated = true;
                        }
                        ("invalid", Some(diagnostic_hash))
                    }
                };
            entries.push(ArtifactAuditEntry {
                artifact_id: record.id,
                locale_hash,
                record_identity_hash,
                status,
                diagnostic_hash,
            });
        }

        let audit_hash = stable_hash(&(
            PAGE_ARTIFACT_INTEGRITY_AUDIT_FORMAT,
            tenant_id,
            page_id,
            max_records,
            truncated,
            &entries,
        ))?;
        let scanned_artifact_count = u32::try_from(artifact_ids.len()).map_err(|_| {
            PagesError::artifact_integrity("Immutable artifact audit record count overflow")
        })?;
        txn.commit().await?;

        Ok(PageArtifactIntegrityAuditResult {
            format: PAGE_ARTIFACT_INTEGRITY_AUDIT_FORMAT.to_string(),
            page_id,
            max_records,
            scanned_artifact_count,
            valid_artifact_count,
            invalid_artifact_count,
            truncated,
            findings_truncated,
            findings,
            audit_hash,
        })
    }
}

fn enforce_tenant_wide_manage(security: &SecurityContext) -> PagesResult<()> {
    if matches!(
        security.get_scope(Resource::Pages, Action::Manage),
        PermissionScope::All
    ) {
        Ok(())
    } else {
        Err(PagesError::forbidden(
            "Immutable artifact audit requires tenant-wide pages:manage",
        ))
    }
}

fn normalize_max_records(value: Option<u32>) -> PagesResult<u32> {
    let value = value.unwrap_or(DEFAULT_PAGE_ARTIFACT_AUDIT_RECORDS);
    if value == 0 || value > MAX_PAGE_ARTIFACT_AUDIT_RECORDS {
        return Err(PagesError::validation(format!(
            "Immutable artifact audit max_records must be between 1 and {MAX_PAGE_ARTIFACT_AUDIT_RECORDS}",
        )));
    }
    Ok(value)
}

fn artifact_record_identity_hash(
    record: &page_static_landing_artifact::Model,
) -> PagesResult<String> {
    stable_hash(&(
        record.id,
        record.tenant_id,
        record.page_id,
        &record.locale,
        &record.source_hash,
        &record.build_hash,
        &record.artifact_hash,
        record.materialization_hash.as_deref(),
        &record.content_hash,
        &record.instance_key,
    ))
}

fn verify_artifact_record(
    record: &page_static_landing_artifact::Model,
    tenant_id: Uuid,
    page_id: Uuid,
) -> PagesResult<()> {
    if record.id.is_nil()
        || record.tenant_id != tenant_id
        || record.page_id != page_id
        || record.locale.trim().is_empty()
        || !is_valid_artifact_instance_key(&record.instance_key)
    {
        return Err(PagesError::artifact_integrity(
            "Stored static landing artifact has invalid owner or instance identity",
        ));
    }
    enforce_record_size_limits(record)?;

    let identity: StaticLandingBuildIdentity =
        from_json(&record.identity, "landing build identity")?;
    let registry: ComponentRegistryManifest = from_json(&record.registry, "component registry")?;
    let head: PageHead = from_json(&record.head, "page head")?;
    let landing_sections: Vec<LandingSectionSnapshot> =
        from_json(&record.landing_sections, "landing section manifest")?;
    let page_index = usize::try_from(record.page_index)
        .map_err(|_| PagesError::artifact_integrity("Stored landing page index is negative"))?;
    let artifact = StaticLandingArtifact {
        identity,
        artifact_hash: record.artifact_hash.clone(),
        registry,
        pages: vec![StaticLandingPage {
            page_index,
            page_id: record.fly_page_id.clone(),
            slug: record.slug.clone(),
            head,
            document_html: record.document_html.clone(),
            body_html: record.body_html.clone(),
            css: record.css.clone(),
            content_hash: record.content_hash.clone(),
            landing_sections,
        }],
    };
    artifact
        .verify_integrity()
        .map_err(artifact_integrity_error)?;
    if record.source_hash != artifact.identity.source_hash
        || record.build_hash != artifact.identity.build_hash
        || record.renderer_id != artifact.identity.renderer.id
        || record.renderer_release != artifact.identity.renderer.release
    {
        return Err(PagesError::artifact_integrity(
            "Stored static landing artifact metadata does not match its payload",
        ));
    }

    match (
        record.materialization_hash.as_ref(),
        record.materialization_identity.as_ref(),
        record.runtime_snapshots.as_ref(),
    ) {
        (None, None, None) => Ok(()),
        (Some(materialization_hash), Some(materialization_identity), Some(runtime_snapshots)) => {
            let identity: PageBuilderStaticLandingMaterializationIdentity = from_json(
                materialization_identity,
                "Page Builder landing materialization identity",
            )?;
            let runtime_snapshots =
                from_json(runtime_snapshots, "Page Builder landing runtime snapshots")?;
            let materialized = PageBuilderMaterializedStaticLandingArtifact {
                identity,
                runtime_snapshots,
                artifact,
            };
            materialized
                .verify_integrity()
                .map_err(artifact_integrity_error)?;
            if materialization_hash != &materialized.identity.materialization_hash {
                return Err(PagesError::artifact_integrity(
                    "Stored landing materialization hash does not match its identity",
                ));
            }
            Ok(())
        }
        _ => Err(PagesError::artifact_integrity(
            "Stored landing materialization evidence is partial",
        )),
    }
}

fn is_valid_artifact_instance_key(value: &str) -> bool {
    if value == CANONICAL_ARTIFACT_INSTANCE_KEY {
        return true;
    }
    value
        .strip_prefix(REBUILD_ARTIFACT_INSTANCE_PREFIX)
        .and_then(|value| Uuid::parse_str(value).ok())
        .is_some_and(|operation_id| !operation_id.is_nil())
}

fn enforce_record_size_limits(record: &page_static_landing_artifact::Model) -> PagesResult<()> {
    enforce_max(
        "document HTML",
        record.document_html.len(),
        MAX_DOCUMENT_HTML_BYTES,
    )?;
    enforce_max("body HTML", record.body_html.len(), MAX_BODY_HTML_BYTES)?;
    enforce_max("CSS", record.css.len(), MAX_CSS_BYTES)
}

fn enforce_max(label: &str, actual: usize, maximum: usize) -> PagesResult<()> {
    if actual > maximum {
        return Err(PagesError::artifact_integrity(format!(
            "Stored static landing {label} exceeds the {maximum}-byte limit",
        )));
    }
    Ok(())
}

fn from_json<T>(value: &Value, label: &str) -> PagesResult<T>
where
    T: serde::de::DeserializeOwned,
{
    T::deserialize(value).map_err(|error| {
        PagesError::artifact_integrity(format!("Unable to decode stored {label}: {error}"))
    })
}

fn stable_hash(value: &impl Serialize) -> PagesResult<String> {
    let mut writer = Sha256Writer::default();
    serde_json::to_writer(&mut writer, value).map_err(|error| {
        PagesError::artifact_integrity(format!(
            "Unable to encode immutable artifact audit identity: {error}",
        ))
    })?;
    Ok(writer.finish())
}

#[derive(Default)]
struct Sha256Writer(Sha256);

impl Sha256Writer {
    fn finish(self) -> String {
        self.0
            .finalize()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect()
    }
}

impl Write for Sha256Writer {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        self.0.update(buffer);
        Ok(buffer.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

fn hex_sha256(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn artifact_integrity_error(error: impl std::fmt::Display) -> PagesError {
    PagesError::artifact_integrity(format!(
        "Page Builder static artifact integrity error: {error}",
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn audit_limit_is_bounded() {
        assert_eq!(
            normalize_max_records(None).expect("default limit"),
            DEFAULT_PAGE_ARTIFACT_AUDIT_RECORDS,
        );
        assert!(normalize_max_records(Some(0)).is_err());
        assert!(normalize_max_records(Some(MAX_PAGE_ARTIFACT_AUDIT_RECORDS + 1)).is_err());
    }

    #[test]
    fn audit_identity_is_deterministic() {
        let page_id = Uuid::new_v4();
        let tenant_id = Uuid::new_v4();
        let entries = vec![ArtifactAuditEntry {
            artifact_id: Uuid::new_v4(),
            locale_hash: hex_sha256(b"en"),
            record_identity_hash: hex_sha256(b"record"),
            status: "valid",
            diagnostic_hash: None,
        }];
        let first = stable_hash(&(
            PAGE_ARTIFACT_INTEGRITY_AUDIT_FORMAT,
            tenant_id,
            page_id,
            10_u32,
            false,
            &entries,
        ))
        .expect("audit hash");
        let second = stable_hash(&(
            PAGE_ARTIFACT_INTEGRITY_AUDIT_FORMAT,
            tenant_id,
            page_id,
            10_u32,
            false,
            &entries,
        ))
        .expect("audit hash");
        assert_eq!(first, second);
        assert_eq!(first.len(), 64);
    }

    #[test]
    fn audit_accepts_only_canonical_or_operation_bound_instances() {
        assert!(is_valid_artifact_instance_key("canonical"));
        assert!(is_valid_artifact_instance_key(&format!(
            "rebuild:{}",
            Uuid::new_v4()
        )));
        assert!(!is_valid_artifact_instance_key("rebuild:not-a-uuid"));
        assert!(!is_valid_artifact_instance_key("other"));
    }
}
