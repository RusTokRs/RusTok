use std::collections::BTreeSet;

use chrono::Utc;
use rustok_api::{Action, Resource};
use rustok_core::SecurityContext;
use rustok_events::DomainEvent;
use rustok_page_builder::runtime_scenario_release::{
    evaluate_page_builder_runtime_scenario_release, PageBuilderRuntimeScenarioReleaseRequest,
    RuntimeScenarioReleaseBaseline, RuntimeScenarioReleaseEvaluation, RuntimeScenarioReleasePolicy,
    PAGE_BUILDER_SCENARIO_REGRESSION_BLOCKED_ERROR_CODE,
};
use rustok_page_builder::{
    compile_materialized_static_landing, sanitize_static_landing_project,
    PageBuilderReviewedPublishRuntime, PageBuilderStaticLandingMaterializationError,
    PageBuilderStaticLandingSanitizationError, StaticLandingPage,
};
use rustok_tenant::entities::tenant_module;
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, ConnectionTrait, DatabaseTransaction, DbBackend,
    EntityTrait, QueryFilter, QueryOrder, QuerySelect, TransactionTrait,
};
use serde::Serialize;
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::dto::{PageBodyRevisionInput, PublishPageInput, PublishPageResult};
use crate::entities::{
    page, page_body, page_builder_scenario_baseline, page_publish_operation,
};
use crate::error::{PagesError, PagesResult};
use crate::services::page_builder_artifact::CompiledLandingArtifact;
use crate::services::rbac::enforce_owned_scope;
use crate::services::PageBuilderArtifactService;

use super::document::document_revision_conflict;
use super::helpers::{
    apply_transition, collect_builder_project_values, collect_builder_sources,
    enforce_expected_version, is_builder_enabled, is_builder_publish_enabled, normalize_locale,
};
use super::{PAGE_KIND, PageService, PageTransition};

const PAGE_PUBLISH_OPERATION_FORMAT: &str = "page_publish_operation_v1";
const MAX_PUBLISH_IDEMPOTENCY_KEY_BYTES: usize = 191;
const MAX_DOCUMENT_HTML_BYTES: usize = 2 * 1024 * 1024;
const MAX_BODY_HTML_BYTES: usize = 1536 * 1024;
const MAX_CSS_BYTES: usize = 512 * 1024;

type BodyRevisionSnapshot = Vec<(String, String)>;

struct ReviewedCompiledLanding {
    compiled: CompiledLandingArtifact,
    sanitized_hash: String,
}

impl PageService {
    /// Publishes the exact reviewed page/runtime snapshot as one idempotent transaction.
    ///
    /// The transaction owns the page/body locks, sanitization evidence, runtime materialization,
    /// immutable artifact staging, binding switch, page state, transactional outbox events and the
    /// durable operation receipt. A successful replay returns the stored receipt without rebuilding
    /// artifacts or emitting duplicate events.
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

        let txn = self.db.begin().await?;
        let existing_page = self.find_page_for_update(&txn, tenant_id, page_id).await?;
        enforce_owned_scope(
            &security,
            Resource::Pages,
            Action::Publish,
            existing_page.author_id,
        )?;

        if let Some(operation) = find_publish_operation_in_tx(
            &txn,
            tenant_id,
            page_id,
            &idempotency_key,
        )
        .await?
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
        let current_bodies = load_bodies_for_reviewed_publish(&txn, tenant_id, page_id).await?;
        let current_revisions = body_revision_snapshot(&current_bodies);
        if current_revisions != expected_body_revisions {
            return Err(document_revision_conflict(
                format_body_revisions(&expected_body_revisions),
                format_body_revisions(&current_revisions),
            ));
        }

        let project_values = collect_builder_project_values(&current_bodies, None, true)?;
        if !project_values.is_empty() {
            ensure_builder_publish_enabled_in_tx(&txn, tenant_id).await?;
            ensure_candidates_allowed_in_tx(
                &txn,
                tenant_id,
                page_id,
                &reviewed,
                project_values,
            )
            .await?;
        }

        let compiled = compile_builder_sources_with_reviewed_runtime(&current_bodies, &reviewed)?;
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
        let mut active: page::ActiveModel = existing_page.into();
        active.updated_at = Set(now.into());
        active.version = Set(active.version.take().unwrap_or(1) + 1);
        apply_transition(&mut active, Some(PageTransition::Publish), now);
        let published_page = active.update(&txn).await?;

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
}

fn compile_builder_sources_with_reviewed_runtime(
    bodies: &[page_body::Model],
    reviewed: &PageBuilderReviewedPublishRuntime,
) -> PagesResult<Vec<ReviewedCompiledLanding>> {
    reviewed.validate().map_err(review_contract_error)?;
    let runtime = reviewed.preview_runtime().map_err(review_contract_error)?;
    let expected_context_hash = reviewed
        .runtime_context_hash()
        .map_err(review_contract_error)?;

    collect_builder_sources(bodies, None, true)
        .into_iter()
        .map(|(locale, content)| {
            let project_data = serde_json::from_str(&content).map_err(|error| {
                PagesError::validation(format!(
                    "Page Builder project for locale `{locale}` is not valid JSON: {error}"
                ))
            })?;
            let sanitized = sanitize_static_landing_project(&project_data)
                .map_err(sanitization_error)?;
            sanitized
                .verify_integrity()
                .map_err(sanitization_integrity_error)?;
            let materialized = compile_materialized_static_landing(
                sanitized.project_data(),
                runtime.clone(),
            )
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
            let materialization_identity = serde_json::to_value(&materialized.identity).map_err(
                |error| {
                    PagesError::artifact_integrity(format!(
                        "unable to encode reviewed landing materialization identity: {error}"
                    ))
                },
            )?;
            let runtime_snapshots = serde_json::to_value(&materialized.runtime_snapshots).map_err(
                |error| {
                    PagesError::artifact_integrity(format!(
                        "unable to encode reviewed landing runtime snapshots: {error}"
                    ))
                },
            )?;
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

async fn ensure_builder_publish_enabled_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
) -> PagesResult<()> {
    let module = tenant_module::Entity::find()
        .filter(tenant_module::Column::TenantId.eq(tenant_id))
        .filter(tenant_module::Column::ModuleSlug.eq("pages"))
        .one(txn)
        .await?;
    let settings = module.as_ref().map(|module| &module.settings);
    if !settings.is_none_or(|settings| {
        is_builder_enabled(settings) && is_builder_publish_enabled(settings)
    }) {
        return Err(PagesError::feature_disabled("builder.publish.enabled"));
    }
    Ok(())
}

async fn ensure_candidates_allowed_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    page_id: Uuid,
    reviewed: &PageBuilderReviewedPublishRuntime,
    project_data: Vec<serde_json::Value>,
) -> PagesResult<()> {
    let Some(model) = page_builder_scenario_baseline::Entity::find()
        .filter(page_builder_scenario_baseline::Column::TenantId.eq(tenant_id))
        .filter(page_builder_scenario_baseline::Column::PageId.eq(page_id))
        .one(txn)
        .await?
    else {
        return Ok(());
    };
    let baseline: RuntimeScenarioReleaseBaseline = serde_json::from_value(model.baseline)
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

async fn load_bodies_for_reviewed_publish(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    page_id: Uuid,
) -> PagesResult<Vec<page_body::Model>> {
    let query = || {
        page_body::Entity::find()
            .filter(page_body::Column::TenantId.eq(tenant_id))
            .filter(page_body::Column::PageId.eq(page_id))
            .order_by_asc(page_body::Column::Locale)
    };
    Ok(match txn.get_database_backend() {
        DbBackend::Sqlite => query().all(txn).await?,
        DbBackend::Postgres | DbBackend::MySql => query().lock_exclusive().all(txn).await?,
    })
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

fn normalize_idempotency_key(value: &str) -> PagesResult<String> {
    let normalized = value.trim();
    if normalized.is_empty() || normalized.len() > MAX_PUBLISH_IDEMPOTENCY_KEY_BYTES {
        return Err(PagesError::validation(format!(
            "publish idempotency_key must contain 1 to {MAX_PUBLISH_IDEMPOTENCY_KEY_BYTES} bytes"
        )));
    }
    Ok(normalized.to_string())
}

fn normalize_expected_body_revisions(
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

fn body_revision_snapshot(bodies: &[page_body::Model]) -> BodyRevisionSnapshot {
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

        assert!(compile_builder_sources_with_reviewed_runtime(&[], &reviewed).is_err());
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
