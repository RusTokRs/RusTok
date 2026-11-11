use std::collections::{BTreeMap, BTreeSet};

use chrono::Utc;
use rustok_api::{Action, Resource};
use rustok_core::SecurityContext;
use rustok_events::DomainEvent;
use rustok_page_builder::runtime_scenario_release::{
    PAGE_BUILDER_SCENARIO_REGRESSION_BLOCKED_ERROR_CODE, PageBuilderRuntimeScenarioReleaseRequest,
    RuntimeScenarioReleaseBaseline, RuntimeScenarioReleaseEvaluation, RuntimeScenarioReleasePolicy,
    evaluate_page_builder_runtime_scenario_release,
};
use rustok_page_builder::{
    PageBuilderReviewedPublishRuntime, PageBuilderStaticLandingMaterializationError,
    PageBuilderStaticLandingSanitizationError, StaticLandingPage,
    compile_materialized_static_landing, sanitize_static_landing_project,
};
use rustok_tenant::TenantService;
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, ConnectionTrait, DatabaseTransaction,
    DbBackend, EntityTrait, QueryFilter, QuerySelect, Select, TransactionTrait,
};
use serde::Serialize;
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::dto::{PageBodyRevisionInput, PublishPageInput, PublishPageResult};
use crate::entities::{
    page, page_builder_scenario_baseline, page_publish_operation, page_template,
};
use crate::error::{PagesError, PagesResult};
use crate::services::page_builder_artifact::CompiledLandingArtifact;
use crate::services::rbac::enforce_owned_scope;
use crate::services::{PageBuilderArtifactService, PageTemplateRecord};
use crate::translation_evidence::{TranslationChangeEvidence, record_translation_change_in_tx};

use super::document::document_revision_conflict;
use super::helpers::{
    apply_transition, collect_builder_sources, enforce_expected_version, is_builder_enabled,
    is_builder_publish_enabled, next_page_version, normalize_locale,
};
use super::{PAGE_KIND, PageService, PageTransition, WorkingBody};

const PAGE_PUBLISH_OPERATION_FORMAT: &str = "page_publish_operation_v1";
const REVIEWED_PUBLISH_EVALUATION_FORMAT: &str = "page_reviewed_publish_evaluation_v1";
const MAX_PUBLISH_IDEMPOTENCY_KEY_BYTES: usize = 191;
const MAX_DOCUMENT_HTML_BYTES: usize = 2 * 1024 * 1024;
const MAX_BODY_HTML_BYTES: usize = 1536 * 1024;
const MAX_CSS_BYTES: usize = 512 * 1024;

type BodyRevisionSnapshot = Vec<(String, String)>;
type BuilderSourceSet = BTreeMap<String, String>;
/// Site symbol definitions per locale.
type SiteSymbolCatalog = BTreeMap<String, Vec<serde_json::Value>>;
/// Layout template rows of one template key, per locale.
type TemplateRecordSet = BTreeMap<String, PageTemplateRecord>;

struct ReviewedCompiledLanding {
    compiled: CompiledLandingArtifact,
    sanitized_hash: String,
}

/// The promoted scenario baseline row as stored, before integrity checks.
#[derive(Serialize)]
struct StoredScenarioBaseline {
    baseline_id: String,
    baseline_hash: String,
    source_project_hash: String,
    baseline: serde_json::Value,
}

impl From<page_builder_scenario_baseline::Model> for StoredScenarioBaseline {
    fn from(model: page_builder_scenario_baseline::Model) -> Self {
        Self {
            baseline_id: model.baseline_id,
            baseline_hash: model.baseline_hash,
            source_project_hash: model.source_project_hash,
            baseline: model.baseline,
        }
    }
}

/// The scenario gate and compilation of a reviewed publish, computed before
/// the publish transaction took its locks (audit §4.2).
struct PrecompiledReviewedPublish {
    /// [`reviewed_publish_fingerprint`] of the inputs `outcome` was computed
    /// from.
    fingerprint: String,
    /// What the gate followed by the compilation returned for those inputs.
    outcome: PagesResult<Vec<ReviewedCompiledLanding>>,
}

impl PageService {
    /// Publishes the exact reviewed Page Builder/runtime snapshot as one idempotent transaction.
    ///
    /// The transaction owns page/body locks, feature and promoted-scenario gates, sanitization,
    /// runtime materialization, immutable artifact staging, binding switch, page state,
    /// transactional outbox events and the durable operation receipt. An exact replay returns the
    /// stored receipt without rebuilding artifacts or emitting duplicate events.
    ///
    /// The scenario gate, sanitization and materialization are CPU-heavy, so they are first
    /// attempted before the transaction opens (see `precompile_reviewed_publish`). The
    /// transaction still reads every input under its locks, in the same order and with the same
    /// checks as without that attempt, and reuses the attempt's result only when the expanded
    /// documents and the promoted baseline are the ones it was computed from; otherwise it gates
    /// and compiles under the locks. The attempt changes how long the locks are held, not what
    /// is published or which error is reported.
    pub async fn publish_reviewed(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        page_id: Uuid,
        input: PublishPageInput,
    ) -> PagesResult<PublishPageResult> {
        let idempotency_key = normalize_idempotency_key(&input.idempotency_key)?;
        let expected_body_revisions =
            normalize_expected_body_revisions(input.expected_body_revisions)?;
        let reviewed: PageBuilderReviewedPublishRuntime =
            input.runtime.try_into().map_err(review_contract_error)?;
        let request_hash = stable_hash(&(
            PAGE_PUBLISH_OPERATION_FORMAT,
            tenant_id,
            page_id,
            input.expected_version,
            &expected_body_revisions,
            reviewed.review_hash.as_str(),
        ))?;

        let precompiled = self
            .precompile_reviewed_publish(
                tenant_id,
                &security,
                page_id,
                input.expected_version,
                &expected_body_revisions,
                &idempotency_key,
                &reviewed,
            )
            .await;

        let txn = self.db.begin().await?;
        let existing_page = self.find_page_for_update(&txn, tenant_id, page_id).await?;
        enforce_owned_scope(
            &security,
            Resource::Pages,
            Action::Publish,
            existing_page.author_id,
        )?;

        if let Some(operation) =
            find_publish_operation_in_tx(&txn, tenant_id, page_id, &idempotency_key).await?
        {
            ensure_same_publish_request(
                &operation,
                tenant_id,
                page_id,
                &idempotency_key,
                &request_hash,
                &reviewed.review_hash,
            )?;
            let result = publish_result_from_record(operation, true)?;
            txn.commit().await?;
            return Ok(result);
        }

        enforce_expected_version(Some(input.expected_version), existing_page.version)?;
        let current_bodies = self
            .load_working_bodies_in_tx(&txn, tenant_id, page_id)
            .await?;
        let current_revisions = body_revision_snapshot(&current_bodies);
        if current_revisions != expected_body_revisions {
            return Err(document_revision_conflict(
                format_body_revisions(&expected_body_revisions),
                format_body_revisions(&current_revisions),
            ));
        }

        // Fold unpublished working copies into their current bodies before artifact binding,
        // which addresses the current body row of each locale.
        self.promote_drafts_in_tx(&txn, tenant_id, page_id, security.user_id)
            .await?;

        let builder_sources =
            require_builder_sources(collect_builder_sources(&current_bodies, None, true))?;
        // Layout templates and site symbols resolve freshly at publish: the stored catalogs win over
        // each body's embedded working copy, and every instance expands into its
        // definition before sanitization so re-publishing re-issues occurrences.
        let (site_symbols, template_records) =
            load_reviewed_publish_catalogs(&txn, tenant_id, &existing_page.template).await?;
        let builder_sources = expand_builder_sources(
            &existing_page.template,
            builder_sources,
            &site_symbols,
            &template_records,
        )?;
        let project_values = parse_builder_project_values(&builder_sources)?;
        ensure_builder_publish_enabled_in_tx(&txn, tenant_id).await?;
        let baseline = load_scenario_baseline_in_tx(&txn, tenant_id, page_id).await?;

        // The gate and the compilation read nothing but the expanded documents, the promoted
        // baseline and the reviewed runtime of this request. A result computed before the locks
        // were taken is used only if it was computed from these documents and this baseline;
        // otherwise both steps run here.
        let fingerprint = reviewed_publish_fingerprint(&builder_sources, baseline.as_ref())?;
        let compiled = match precompiled {
            Some(precompiled) if precompiled.fingerprint == fingerprint => precompiled.outcome?,
            _ => {
                ensure_candidates_allowed(baseline, &reviewed, project_values)?;
                compile_builder_sources_with_reviewed_runtime(builder_sources, &reviewed)?
            }
        };
        let sanitized_set_hash = sanitized_set_hash(&compiled)?;
        let artifact_set_hash = artifact_set_hash(&compiled)?;
        for item in &compiled {
            let artifact_id = PageBuilderArtifactService::stage_compiled_in_tx(
                &txn,
                tenant_id,
                page_id,
                &item.compiled,
            )
            .await?;
            PageBuilderArtifactService::bind_existing_body_in_tx(
                &txn,
                tenant_id,
                page_id,
                &item.compiled.locale,
                artifact_id,
            )
            .await?;
        }

        let now = Utc::now();
        let next_version = next_page_version(page_id, existing_page.version)?;
        let mut active: page::ActiveModel = existing_page.into();
        active.updated_at = Set(now.into());
        active.version = Set(next_version);
        apply_transition(&mut active, Some(PageTransition::Publish), now);
        let published_page = active.update(&txn).await?;

        record_translation_change_in_tx(
            &txn,
            TranslationChangeEvidence {
                tenant_id,
                page_id,
                resource_revision: i64::from(published_page.version),
                operation: "lifecycle",
                lifecycle: "active",
            },
        )
        .await?;

        self.event_bus
            .publish_in_tx(
                &txn,
                tenant_id,
                security.user_id,
                DomainEvent::NodeUpdated {
                    node_id: page_id,
                    kind: PAGE_KIND.to_string(),
                },
            )
            .await?;
        self.event_bus
            .publish_in_tx(
                &txn,
                tenant_id,
                security.user_id,
                DomainEvent::NodePublished {
                    node_id: page_id,
                    kind: PAGE_KIND.to_string(),
                },
            )
            .await?;

        let operation = insert_publish_operation_in_tx(
            &txn,
            tenant_id,
            page_id,
            idempotency_key,
            request_hash,
            reviewed.review_hash,
            sanitized_set_hash,
            artifact_set_hash,
            published_page.version,
            now,
        )
        .await?;
        let result = publish_result_from_record(operation, false)?;
        txn.commit().await?;
        Ok(result)
    }

    /// Runs the scenario gate and the compilation of a reviewed publish before the publish
    /// transaction, without locks (audit §4.2).
    ///
    /// Returns `None` whenever the publish transaction would not reach those steps anyway
    /// (missing page, missing permission, replay, version or body-revision conflict, feature
    /// switched off, a document that does not expand or parse) or a read fails; such a failure
    /// is logged at debug level. Nothing decided here is authoritative: the transaction repeats
    /// every check under its locks and reports the real error, so a failure in this method only
    /// means the gate and the compilation run under the locks.
    #[allow(clippy::too_many_arguments)]
    async fn precompile_reviewed_publish(
        &self,
        tenant_id: Uuid,
        security: &SecurityContext,
        page_id: Uuid,
        expected_version: i32,
        expected_body_revisions: &BodyRevisionSnapshot,
        idempotency_key: &str,
        reviewed: &PageBuilderReviewedPublishRuntime,
    ) -> Option<PrecompiledReviewedPublish> {
        match self
            .try_precompile_reviewed_publish(
                tenant_id,
                security,
                page_id,
                expected_version,
                expected_body_revisions,
                idempotency_key,
                reviewed,
            )
            .await
        {
            Ok(precompiled) => precompiled,
            Err(error) => {
                tracing::debug!(
                    %error,
                    %tenant_id,
                    %page_id,
                    "Reviewed publish was not precompiled; the publish transaction decides"
                );
                None
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    async fn try_precompile_reviewed_publish(
        &self,
        tenant_id: Uuid,
        security: &SecurityContext,
        page_id: Uuid,
        expected_version: i32,
        expected_body_revisions: &BodyRevisionSnapshot,
        idempotency_key: &str,
        reviewed: &PageBuilderReviewedPublishRuntime,
    ) -> PagesResult<Option<PrecompiledReviewedPublish>> {
        let page = self.find_page(tenant_id, page_id).await?;
        // Authorize before spending CPU: a caller who may not publish this page must not be
        // able to make the server compile it.
        enforce_owned_scope(security, Resource::Pages, Action::Publish, page.author_id)?;
        if page.version != expected_version {
            return Ok(None);
        }
        let replayed = page_publish_operation::Entity::find()
            .filter(page_publish_operation::Column::TenantId.eq(tenant_id))
            .filter(page_publish_operation::Column::PageId.eq(page_id))
            .filter(page_publish_operation::Column::IdempotencyKey.eq(idempotency_key))
            .one(&self.db)
            .await?;
        if replayed.is_some() {
            return Ok(None);
        }
        let bodies = self.load_working_bodies(tenant_id, page_id).await?;
        if body_revision_snapshot(&bodies) != *expected_body_revisions {
            return Ok(None);
        }
        let builder_sources = collect_builder_sources(&bodies, None, true);
        if builder_sources.is_empty() {
            return Ok(None);
        }
        let module = TenantService::new(self.db.clone())
            .find_tenant_module(tenant_id, "pages")
            .await?;
        if !builder_publish_enabled(module.as_ref().map(|module| &module.settings)) {
            return Ok(None);
        }

        let (site_symbols, template_records) =
            load_reviewed_publish_catalogs(&self.db, tenant_id, &page.template).await?;
        let builder_sources = expand_builder_sources(
            &page.template,
            builder_sources,
            &site_symbols,
            &template_records,
        )?;
        let project_values = parse_builder_project_values(&builder_sources)?;
        let baseline = scenario_baseline_query(tenant_id, page_id)
            .one(&self.db)
            .await?
            .map(StoredScenarioBaseline::from);
        let fingerprint = reviewed_publish_fingerprint(&builder_sources, baseline.as_ref())?;
        // The same two steps, in the same order, as the transaction runs when it cannot reuse
        // this result.
        let outcome =
            ensure_candidates_allowed(baseline, reviewed, project_values).and_then(|()| {
                compile_builder_sources_with_reviewed_runtime(builder_sources, reviewed)
            });
        Ok(Some(PrecompiledReviewedPublish {
            fingerprint,
            outcome,
        }))
    }
}

/// Whether a page uses the built-in layout, which has no stored template rows.
fn uses_default_template(template_key: &str) -> bool {
    template_key.is_empty() || template_key == "default"
}

/// Identity of everything the scenario gate and the compilation read besides
/// the reviewed runtime of the request: every locale's expanded document and
/// the promoted baseline as stored. A result computed for an equal
/// fingerprint can be reused; anything else is recomputed, so a spurious
/// mismatch costs time and never correctness.
fn reviewed_publish_fingerprint(
    builder_sources: &BuilderSourceSet,
    baseline: Option<&StoredScenarioBaseline>,
) -> PagesResult<String> {
    stable_hash(&(
        REVIEWED_PUBLISH_EVALUATION_FORMAT,
        builder_sources,
        baseline,
    ))
}

/// Loads the site symbol catalog and the page's layout template rows, keyed by
/// locale. Runs on the pool for the attempt before the publish transaction and
/// on the transaction for the authoritative read.
async fn load_reviewed_publish_catalogs<C: ConnectionTrait>(
    conn: &C,
    tenant_id: Uuid,
    template_key: &str,
) -> PagesResult<(SiteSymbolCatalog, TemplateRecordSet)> {
    let site_symbols: SiteSymbolCatalog =
        super::symbols::load_site_symbols_by_locale(conn, tenant_id)
            .await?
            .into_iter()
            .collect();
    let mut template_records = TemplateRecordSet::new();
    if !uses_default_template(template_key) {
        let rows = page_template::Entity::find()
            .filter(page_template::Column::TenantId.eq(tenant_id))
            .filter(page_template::Column::TemplateKey.eq(template_key))
            .all(conn)
            .await?;
        for row in rows {
            let record = PageTemplateRecord::try_from(row)?;
            template_records.insert(record.locale.clone(), record);
        }
    }
    Ok((site_symbols, template_records))
}

fn scenario_baseline_query(
    tenant_id: Uuid,
    page_id: Uuid,
) -> Select<page_builder_scenario_baseline::Entity> {
    page_builder_scenario_baseline::Entity::find()
        .filter(page_builder_scenario_baseline::Column::TenantId.eq(tenant_id))
        .filter(page_builder_scenario_baseline::Column::PageId.eq(page_id))
}

/// Loads the promoted scenario baseline of a page under the shared row lock
/// that keeps it stable until the publish commits.
async fn load_scenario_baseline_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    page_id: Uuid,
) -> PagesResult<Option<StoredScenarioBaseline>> {
    let query = || scenario_baseline_query(tenant_id, page_id);
    let model = match txn.get_database_backend() {
        DbBackend::Sqlite => query().one(txn).await?,
        DbBackend::Postgres | DbBackend::MySql => query().lock_shared().one(txn).await?,
        _ => unreachable!("unsupported SeaORM database backend"),
    };
    Ok(model.map(StoredScenarioBaseline::from))
}

/// Expands site symbols and the page's layout template into every locale's
/// document. The stored catalogs win over each body's embedded working copy.
/// No I/O.
fn expand_builder_sources(
    template_key: &str,
    mut builder_sources: BuilderSourceSet,
    site_symbols: &SiteSymbolCatalog,
    template_records: &TemplateRecordSet,
) -> PagesResult<BuilderSourceSet> {
    for (locale, content) in builder_sources.iter_mut() {
        let symbols = site_symbols.get(locale).cloned().unwrap_or_default();
        super::symbols::assert_reviewed_symbol_snapshot(content, &symbols)?;
        if uses_default_template(template_key) {
            *content = super::symbols::apply_site_symbols_to_content(content, &symbols)?;
        } else {
            let template_record = template_records.get(locale).ok_or_else(|| {
                PagesError::validation(format!(
                    "Page template `{template_key}` is not defined for locale `{locale}`"
                ))
            })?;
            *content = super::layout::compose_layout_document(
                content,
                &template_record.header_symbol_ids,
                &template_record.footer_symbol_ids,
                &symbols,
            )?;
        }
    }
    Ok(builder_sources)
}

fn require_builder_sources(sources: BuilderSourceSet) -> PagesResult<BuilderSourceSet> {
    if sources.is_empty() {
        return Err(PagesError::validation(
            "atomic reviewed publish requires at least one Page Builder body",
        ));
    }
    Ok(sources)
}

fn parse_builder_project_values(sources: &BuilderSourceSet) -> PagesResult<Vec<serde_json::Value>> {
    sources
        .iter()
        .map(|(locale, content)| {
            serde_json::from_str(content).map_err(|error| {
                PagesError::validation(format!(
                    "Page Builder project for locale `{locale}` is not valid JSON: {error}"
                ))
            })
        })
        .collect()
}

fn compile_builder_sources_with_reviewed_runtime(
    sources: BuilderSourceSet,
    reviewed: &PageBuilderReviewedPublishRuntime,
) -> PagesResult<Vec<ReviewedCompiledLanding>> {
    reviewed.validate().map_err(review_contract_error)?;
    let runtime = reviewed.preview_runtime().map_err(review_contract_error)?;
    let expected_context_hash = reviewed
        .runtime_context_hash()
        .map_err(review_contract_error)?;

    sources
        .into_iter()
        .map(|(locale, content)| {
            let project_data = serde_json::from_str(&content).map_err(|error| {
                PagesError::validation(format!(
                    "Page Builder project for locale `{locale}` is not valid JSON: {error}"
                ))
            })?;
            let sanitized =
                sanitize_static_landing_project(&project_data).map_err(sanitization_error)?;
            sanitized
                .verify_integrity()
                .map_err(sanitization_integrity_error)?;
            let materialized =
                compile_materialized_static_landing(sanitized.project_data(), runtime.clone())
                    .map_err(artifact_compile_error)?;
            materialized
                .verify_integrity()
                .map_err(artifact_integrity_error)?;
            if materialized.identity.runtime_scenario_id.as_deref()
                != Some(reviewed.scenario_id.as_str())
                || materialized.identity.runtime_context_hash != expected_context_hash
            {
                return Err(materialization_mismatch_error(&locale));
            }
            if materialized.artifact.pages.len() != 1 {
                return Err(PagesError::validation(format!(
                    "A Pages Page Builder body must contain exactly one Fly page; found {}",
                    materialized.artifact.pages.len()
                )));
            }
            let page = materialized.artifact.pages[0].clone();
            enforce_size_limits(&page)?;
            let materialization_hash = materialized.identity.materialization_hash.clone();
            let materialization_identity =
                serde_json::to_value(&materialized.identity).map_err(|error| {
                    PagesError::artifact_integrity(format!(
                        "unable to encode reviewed landing materialization identity: {error}"
                    ))
                })?;
            let runtime_snapshots =
                serde_json::to_value(&materialized.runtime_snapshots).map_err(|error| {
                    PagesError::artifact_integrity(format!(
                        "unable to encode reviewed landing runtime snapshots: {error}"
                    ))
                })?;
            Ok(ReviewedCompiledLanding {
                sanitized_hash: sanitized.sanitized_hash().to_string(),
                compiled: CompiledLandingArtifact {
                    locale,
                    artifact: materialized.artifact,
                    page,
                    materialization_hash,
                    materialization_identity,
                    runtime_snapshots,
                },
            })
        })
        .collect()
}

/// Whether the tenant's `pages` module settings allow Page Builder publication.
/// A tenant without a `pages` module row runs on the defaults, which allow it.
fn builder_publish_enabled(settings: Option<&serde_json::Value>) -> bool {
    settings
        .is_none_or(|settings| is_builder_enabled(settings) && is_builder_publish_enabled(settings))
}

async fn ensure_builder_publish_enabled_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
) -> PagesResult<()> {
    let module = TenantService::find_tenant_module_locked_in_tx(txn, tenant_id, "pages").await?;
    if !builder_publish_enabled(module.as_ref().map(|module| &module.settings)) {
        return Err(PagesError::feature_disabled("builder.publish.enabled"));
    }
    Ok(())
}

/// Gates the publish candidates against the promoted scenario baseline, when
/// the page has one.
fn ensure_candidates_allowed(
    stored: Option<StoredScenarioBaseline>,
    reviewed: &PageBuilderReviewedPublishRuntime,
    project_data: Vec<serde_json::Value>,
) -> PagesResult<()> {
    let Some(model) = stored else {
        return Ok(());
    };
    let baseline: RuntimeScenarioReleaseBaseline = serde_json::from_value(model.baseline.clone())
        .map_err(|error| {
        PagesError::validation(format!(
            "Stored Page Builder scenario baseline is invalid: {error}"
        ))
    })?;
    if !baseline.validate().is_empty()
        || baseline.baseline_hash != model.baseline_hash
        || baseline.source_project_hash != model.source_project_hash
        || baseline.baseline_id != model.baseline_id
    {
        return Err(PagesError::validation(
            "Stored Page Builder scenario baseline failed integrity validation",
        ));
    }
    let Some(selected) = baseline
        .scenarios
        .iter()
        .find(|scenario| scenario.id == reviewed.scenario_id)
    else {
        return Err(PagesError::publish_runtime_review_invalid(format!(
            "reviewed scenario `{}` is not present in the promoted runtime baseline",
            reviewed.scenario_id
        )));
    };
    if selected.context != reviewed.context {
        return Err(PagesError::publish_runtime_review_invalid(format!(
            "reviewed scenario `{}` context does not match the promoted runtime baseline",
            reviewed.scenario_id
        )));
    }
    for candidate in project_data {
        let response = evaluate_page_builder_runtime_scenario_release(
            PageBuilderRuntimeScenarioReleaseRequest {
                project_data: candidate,
                baseline: Some(baseline.clone()),
                policy: RuntimeScenarioReleasePolicy::block_broken(),
            },
        )
        .map_err(|error| PagesError::validation(error.to_string()))?;
        ensure_evaluation_allowed(response.evaluation)?;
    }
    Ok(())
}

fn ensure_evaluation_allowed(evaluation: RuntimeScenarioReleaseEvaluation) -> PagesResult<()> {
    if evaluation.allowed {
        return Ok(());
    }
    let details = evaluation
        .blocking_diagnostics()
        .take(4)
        .map(|diagnostic| format!("{}: {}", diagnostic.code, diagnostic.message))
        .collect::<Vec<_>>()
        .join("; ");
    Err(PagesError::validation(format!(
        "{PAGE_BUILDER_SCENARIO_REGRESSION_BLOCKED_ERROR_CODE}: {}",
        if details.is_empty() {
            format!("release status {:?} is not allowed", evaluation.status)
        } else {
            details
        }
    )))
}

async fn find_publish_operation_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    page_id: Uuid,
    idempotency_key: &str,
) -> PagesResult<Option<page_publish_operation::Model>> {
    let query = || {
        page_publish_operation::Entity::find()
            .filter(page_publish_operation::Column::TenantId.eq(tenant_id))
            .filter(page_publish_operation::Column::PageId.eq(page_id))
            .filter(page_publish_operation::Column::IdempotencyKey.eq(idempotency_key))
    };
    Ok(match txn.get_database_backend() {
        DbBackend::Sqlite => query().one(txn).await?,
        DbBackend::Postgres | DbBackend::MySql => query().lock_exclusive().one(txn).await?,
        _ => unreachable!("unsupported SeaORM database backend"),
    })
}

#[allow(clippy::too_many_arguments)]
async fn insert_publish_operation_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    page_id: Uuid,
    idempotency_key: String,
    request_hash: String,
    review_hash: String,
    sanitized_set_hash: String,
    artifact_set_hash: String,
    result_version: i32,
    published_at: chrono::DateTime<Utc>,
) -> PagesResult<page_publish_operation::Model> {
    let timestamp: sea_orm::prelude::DateTimeWithTimeZone = published_at.into();
    page_publish_operation::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(tenant_id),
        page_id: Set(page_id),
        idempotency_key: Set(idempotency_key),
        request_hash: Set(request_hash),
        review_hash: Set(review_hash),
        sanitized_set_hash: Set(sanitized_set_hash),
        artifact_set_hash: Set(artifact_set_hash),
        result_version: Set(result_version),
        published_at: Set(timestamp),
        created_at: Set(timestamp),
    }
    .insert(txn)
    .await
    .map_err(Into::into)
}

fn ensure_same_publish_request(
    operation: &page_publish_operation::Model,
    tenant_id: Uuid,
    page_id: Uuid,
    idempotency_key: &str,
    request_hash: &str,
    review_hash: &str,
) -> PagesResult<()> {
    verify_publish_operation(operation)?;
    if operation.tenant_id != tenant_id
        || operation.page_id != page_id
        || operation.idempotency_key != idempotency_key
        || operation.request_hash != request_hash
        || operation.review_hash != review_hash
    {
        return Err(PagesError::publish_idempotency_conflict(format!(
            "idempotency key `{idempotency_key}` is already bound to a different page publish request"
        )));
    }
    Ok(())
}

fn verify_publish_operation(operation: &page_publish_operation::Model) -> PagesResult<()> {
    if operation.id.is_nil()
        || operation.tenant_id.is_nil()
        || operation.page_id.is_nil()
        || operation.idempotency_key.trim().is_empty()
        || operation.result_version <= 0
        || !is_sha256(&operation.request_hash)
        || !is_sha256(&operation.review_hash)
        || !is_sha256(&operation.sanitized_set_hash)
        || !is_sha256(&operation.artifact_set_hash)
    {
        return Err(PagesError::publish_operation_integrity(
            "stored page publish operation contains invalid identity or hash evidence",
        ));
    }
    Ok(())
}

fn publish_result_from_record(
    operation: page_publish_operation::Model,
    replayed: bool,
) -> PagesResult<PublishPageResult> {
    verify_publish_operation(&operation)?;
    Ok(PublishPageResult {
        operation_id: operation.id,
        page_id: operation.page_id,
        version: operation.result_version,
        idempotency_key: operation.idempotency_key,
        review_hash: operation.review_hash,
        sanitized_set_hash: operation.sanitized_set_hash,
        artifact_set_hash: operation.artifact_set_hash,
        replayed,
        published_at: operation.published_at.to_string(),
    })
}

pub(super) fn normalize_idempotency_key(value: &str) -> PagesResult<String> {
    let normalized = value.trim();
    if normalized.is_empty() || normalized.len() > MAX_PUBLISH_IDEMPOTENCY_KEY_BYTES {
        return Err(PagesError::validation(format!(
            "publish idempotency_key must contain 1 to {MAX_PUBLISH_IDEMPOTENCY_KEY_BYTES} bytes"
        )));
    }
    Ok(normalized.to_string())
}

pub(super) fn normalize_expected_body_revisions(
    revisions: Vec<PageBodyRevisionInput>,
) -> PagesResult<BodyRevisionSnapshot> {
    if revisions.is_empty() {
        return Err(PagesError::validation(
            "publish expected_body_revisions must not be empty",
        ));
    }
    let mut locales = BTreeSet::new();
    let mut normalized = Vec::with_capacity(revisions.len());
    for revision in revisions {
        let locale = normalize_locale(&revision.locale)?;
        if !locales.insert(locale.clone()) {
            return Err(PagesError::validation(format!(
                "duplicate publish body revision locale `{locale}`"
            )));
        }
        let value = revision.revision.trim();
        if value.is_empty() {
            return Err(PagesError::validation(format!(
                "publish body revision for locale `{locale}` must not be empty"
            )));
        }
        normalized.push((locale, value.to_string()));
    }
    normalized.sort();
    Ok(normalized)
}

fn body_revision_snapshot(bodies: &[WorkingBody]) -> BodyRevisionSnapshot {
    let mut revisions = bodies
        .iter()
        .map(|body| (body.locale.clone(), body.updated_at.to_string()))
        .collect::<Vec<_>>();
    revisions.sort();
    revisions
}

fn format_body_revisions(revisions: &BodyRevisionSnapshot) -> String {
    revisions
        .iter()
        .map(|(locale, revision)| format!("{locale}:{revision}"))
        .collect::<Vec<_>>()
        .join(",")
}

fn sanitized_set_hash(compiled: &[ReviewedCompiledLanding]) -> PagesResult<String> {
    stable_hash(
        &compiled
            .iter()
            .map(|item| (&item.compiled.locale, &item.sanitized_hash))
            .collect::<Vec<_>>(),
    )
}

fn artifact_set_hash(compiled: &[ReviewedCompiledLanding]) -> PagesResult<String> {
    stable_hash(
        &compiled
            .iter()
            .map(|item| {
                (
                    &item.compiled.locale,
                    &item.compiled.artifact.artifact_hash,
                    &item.compiled.materialization_hash,
                )
            })
            .collect::<Vec<_>>(),
    )
}

fn stable_hash(value: &impl Serialize) -> PagesResult<String> {
    let bytes = serde_json::to_vec(value).map_err(|error| {
        PagesError::publish_operation_integrity(format!(
            "unable to encode page publish identity: {error}"
        ))
    })?;
    Ok(Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect())
}

fn is_sha256(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn enforce_size_limits(page: &StaticLandingPage) -> PagesResult<()> {
    enforce_max(
        "document HTML",
        page.document_html.len(),
        MAX_DOCUMENT_HTML_BYTES,
    )?;
    enforce_max("body HTML", page.body_html.len(), MAX_BODY_HTML_BYTES)?;
    enforce_max("CSS", page.css.len(), MAX_CSS_BYTES)
}

fn enforce_max(label: &str, actual: usize, maximum: usize) -> PagesResult<()> {
    if actual > maximum {
        return Err(PagesError::validation(format!(
            "static landing {label} exceeds the {maximum}-byte limit"
        )));
    }
    Ok(())
}

/// Validates one reviewed runtime exactly as the interactive publish path does.
pub(super) fn validate_reviewed_runtime(
    runtime: crate::dto::ReviewedPagePublishRuntimeInput,
) -> PagesResult<PageBuilderReviewedPublishRuntime> {
    runtime.try_into().map_err(review_contract_error)
}

fn review_contract_error(error: impl std::fmt::Display) -> PagesError {
    PagesError::publish_runtime_review_invalid(error.to_string())
}

fn sanitization_error(error: PageBuilderStaticLandingSanitizationError) -> PagesError {
    PagesError::publish_sanitize(error.to_string())
}

fn sanitization_integrity_error(error: impl std::fmt::Display) -> PagesError {
    PagesError::publish_sanitize(format!(
        "Page Builder publish sanitization integrity error: {error}"
    ))
}

fn materialization_mismatch_error(locale: &str) -> PagesError {
    PagesError::publish_runtime_materialization_mismatch(format!(
        "reviewed runtime does not match the materialized landing for locale `{locale}`"
    ))
}

fn artifact_compile_error(error: PageBuilderStaticLandingMaterializationError) -> PagesError {
    PagesError::validation(format!("Page Builder static artifact error: {error}"))
}

fn artifact_integrity_error(error: impl std::fmt::Display) -> PagesError {
    PagesError::artifact_integrity(format!(
        "Page Builder static artifact integrity error: {error}"
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn reviewed_runtime_hash_must_match_context() {
        let mut reviewed = PageBuilderReviewedPublishRuntime::new(
            "landing-primary",
            json!({ "page": { "title": "Reviewed" } }),
        )
        .expect("reviewed runtime");
        reviewed.context = json!({ "page": { "title": "Changed" } });

        assert!(compile_builder_sources_with_reviewed_runtime(BTreeMap::new(), &reviewed).is_err());
    }

    fn sources(locale: &str, source: &str) -> BuilderSourceSet {
        BTreeMap::from([(locale.to_string(), source.to_string())])
    }

    fn stored_baseline(baseline_hash: &str) -> StoredScenarioBaseline {
        StoredScenarioBaseline {
            baseline_id: "baseline-1".to_string(),
            baseline_hash: baseline_hash.to_string(),
            source_project_hash: "source".to_string(),
            baseline: json!({}),
        }
    }

    fn reviewed_runtime() -> PageBuilderReviewedPublishRuntime {
        PageBuilderReviewedPublishRuntime::new(
            "landing-primary",
            json!({ "page": { "title": "Reviewed" } }),
        )
        .expect("reviewed runtime")
    }

    /// A result precompiled outside the publish locks is reused only for an
    /// equal fingerprint, so the fingerprint has to move with everything the
    /// gate and the compilation read (audit §4.2).
    #[test]
    fn evaluation_fingerprint_tracks_documents_and_baseline() {
        const SOURCE: &str = r#"{"pages":[]}"#;
        let base = reviewed_publish_fingerprint(&sources("en", SOURCE), None).expect("fingerprint");
        assert!(is_sha256(&base));
        assert_eq!(
            base,
            reviewed_publish_fingerprint(&sources("en", SOURCE), None).expect("fingerprint")
        );

        let changed = [
            reviewed_publish_fingerprint(&sources("en", r#"{"pages":[{}]}"#), None),
            reviewed_publish_fingerprint(&sources("de", SOURCE), None),
            reviewed_publish_fingerprint(&sources("en", SOURCE), Some(&stored_baseline("hash"))),
            reviewed_publish_fingerprint(&sources("en", SOURCE), Some(&stored_baseline("other"))),
        ];
        let mut fingerprints = BTreeSet::from([base]);
        for fingerprint in changed {
            fingerprints.insert(fingerprint.expect("fingerprint"));
        }
        assert_eq!(fingerprints.len(), 5);
    }

    #[test]
    fn expansion_requires_the_layout_template_for_every_locale() {
        let error = expand_builder_sources(
            "landing",
            sources("en", r#"{"pages":[]}"#),
            &BTreeMap::new(),
            &BTreeMap::new(),
        )
        .expect_err("a missing template must fail the expansion");
        assert!(
            error
                .to_string()
                .contains("Page template `landing` is not defined for locale `en`")
        );
    }

    #[test]
    fn scenario_gate_applies_only_to_pages_with_a_promoted_baseline() {
        let reviewed = reviewed_runtime();
        assert!(ensure_candidates_allowed(None, &reviewed, vec![json!({})]).is_ok());

        // A stored baseline that does not decode is rejected, not skipped.
        let corrupt = StoredScenarioBaseline {
            baseline_id: "baseline-1".to_string(),
            baseline_hash: "hash".to_string(),
            source_project_hash: "source".to_string(),
            baseline: json!("not a baseline"),
        };
        assert!(ensure_candidates_allowed(Some(corrupt), &reviewed, vec![json!({})]).is_err());
    }

    #[test]
    fn builder_publish_gate_reads_the_pages_module_settings() {
        assert!(builder_publish_enabled(None));
        assert!(builder_publish_enabled(Some(&json!({}))));
        assert!(!builder_publish_enabled(Some(
            &json!({ "builder": { "enabled": false } })
        )));
        assert!(!builder_publish_enabled(Some(
            &json!({ "builder": { "publish": { "enabled": false } } })
        )));
    }

    #[test]
    fn default_template_keys_have_no_stored_rows() {
        assert!(uses_default_template(""));
        assert!(uses_default_template("default"));
        assert!(!uses_default_template("landing"));
    }

    #[test]
    fn reviewed_publish_requires_builder_sources() {
        assert!(require_builder_sources(BTreeMap::new()).is_err());
    }

    #[test]
    fn expected_body_revisions_are_normalized_and_unique() {
        let revisions = normalize_expected_body_revisions(vec![PageBodyRevisionInput {
            locale: "en-US".to_string(),
            revision: "2026-07-21T10:00:00Z".to_string(),
        }])
        .expect("normalized revisions");
        assert_eq!(revisions[0].0, "en-US");
        assert!(normalize_expected_body_revisions(vec![]).is_err());
    }

    #[test]
    fn publish_identity_hashes_are_sha256() {
        let hash = stable_hash(&(
            PAGE_PUBLISH_OPERATION_FORMAT,
            Uuid::nil(),
            1_i32,
            vec![("en", "rev")],
            "review",
        ))
        .expect("request hash");
        assert!(is_sha256(&hash));
    }
}
