use chrono::Utc;
use rustok_api::{Action, Resource};
use rustok_core::SecurityContext;
use rustok_events::DomainEvent;
use rustok_page_builder::{
    PageBuilderReviewedPublishRuntime, PageBuilderStaticLandingMaterializationError,
    StaticLandingPage, compile_materialized_static_landing,
};
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, DatabaseTransaction, DbBackend, EntityTrait,
    QueryFilter, QueryOrder, QuerySelect, TransactionTrait,
};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::dto::PageResponse;
use crate::entities::{page, page_body};
use crate::error::{
    PagesError, PagesResult, PAGE_BUILDER_PUBLISH_RUNTIME_MATERIALIZATION_MISMATCH,
    PAGE_BUILDER_PUBLISH_RUNTIME_REVIEW_INVALID,
};
use crate::services::page_builder_artifact::CompiledLandingArtifact;
use crate::services::rbac::enforce_owned_scope;
use crate::services::{PageBuilderArtifactService, PageBuilderScenarioBaselineService};

use super::document::document_revision_conflict;
use super::helpers::{
    apply_transition, collect_builder_project_values, collect_builder_sources,
    enforce_expected_version, transition_event,
};
use super::{PAGE_KIND, PageService, PageTransition};

const MAX_DOCUMENT_HTML_BYTES: usize = 2 * 1024 * 1024;
const MAX_BODY_HTML_BYTES: usize = 1536 * 1024;
const MAX_CSS_BYTES: usize = 512 * 1024;

type BodyRevisionSnapshot = Vec<(String, String)>;

impl PageService {
    pub async fn publish_reviewed(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        page_id: Uuid,
        runtime: PageBuilderReviewedPublishRuntime,
    ) -> PagesResult<PageResponse> {
        self.publish_reviewed_if_current(tenant_id, security, page_id, None, runtime)
            .await
    }

    pub async fn publish_reviewed_if_current(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        page_id: Uuid,
        expected_version: Option<i32>,
        runtime: PageBuilderReviewedPublishRuntime,
    ) -> PagesResult<PageResponse> {
        let observed = self.find_page(tenant_id, page_id).await?;
        enforce_owned_scope(
            &security,
            Resource::Pages,
            Action::Publish,
            observed.author_id,
        )?;
        enforce_expected_version(expected_version, observed.version)?;
        runtime.validate().map_err(review_contract_error)?;

        let bodies = self.load_bodies(tenant_id, page_id).await?;
        let body_revisions = body_revision_snapshot(&bodies);
        let project_values = collect_builder_project_values(&bodies, None, true)?;
        if !project_values.is_empty() {
            self.ensure_builder_enabled(tenant_id).await?;
            self.ensure_builder_publish_enabled(tenant_id).await?;
            PageBuilderScenarioBaselineService::new(self.db.clone())
                .ensure_candidates_allowed(tenant_id, page_id, project_values)
                .await?;
        }

        self.transition_page_with_reviewed_runtime(
            tenant_id,
            security,
            page_id,
            observed.version,
            body_revisions,
            runtime,
        )
        .await
    }

    async fn transition_page_with_reviewed_runtime(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        page_id: Uuid,
        expected_version: i32,
        expected_body_revisions: BodyRevisionSnapshot,
        runtime: PageBuilderReviewedPublishRuntime,
    ) -> PagesResult<PageResponse> {
        let txn = self.db.begin().await?;
        let existing = self.find_page_for_update(&txn, tenant_id, page_id).await?;
        enforce_expected_version(Some(expected_version), existing.version)?;
        enforce_owned_scope(
            &security,
            Resource::Pages,
            Action::Publish,
            existing.author_id,
        )?;

        let current_bodies = load_bodies_for_reviewed_publish(&txn, tenant_id, page_id).await?;
        let current_revisions = body_revision_snapshot(&current_bodies);
        if current_revisions != expected_body_revisions {
            return Err(document_revision_conflict(
                format_body_revisions(&expected_body_revisions),
                format_body_revisions(&current_revisions),
            ));
        }

        let compiled = compile_builder_sources_with_reviewed_runtime(&current_bodies, &runtime)?;
        for compiled in &compiled {
            let artifact_id = PageBuilderArtifactService::stage_compiled_in_tx(
                &txn, tenant_id, page_id, compiled,
            )
            .await?;
            PageBuilderArtifactService::bind_existing_body_in_tx(
                &txn,
                tenant_id,
                page_id,
                &compiled.locale,
                artifact_id,
            )
            .await?;
        }

        let now = Utc::now();
        let mut active: page::ActiveModel = existing.into();
        active.updated_at = Set(now.into());
        active.version = Set(active.version.take().unwrap_or(1) + 1);
        apply_transition(&mut active, Some(PageTransition::Publish), now);
        active.update(&txn).await?;

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
        if let Some(event) = transition_event(Some(PageTransition::Publish), page_id) {
            self.event_bus
                .publish_in_tx(&txn, tenant_id, security.user_id, event)
                .await?;
        }
        txn.commit().await?;
        self.get(tenant_id, security, page_id).await
    }
}

fn compile_builder_sources_with_reviewed_runtime(
    bodies: &[page_body::Model],
    reviewed: &PageBuilderReviewedPublishRuntime,
) -> PagesResult<Vec<CompiledLandingArtifact>> {
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
            let materialized = compile_materialized_static_landing(&project_data, runtime.clone())
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
            Ok(CompiledLandingArtifact {
                locale,
                artifact: materialized.artifact,
                page,
                materialization_hash,
                materialization_identity,
                runtime_snapshots,
            })
        })
        .collect()
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

fn body_revision_snapshot(bodies: &[page_body::Model]) -> BodyRevisionSnapshot {
    let mut revisions = bodies
        .iter()
        .map(|body| {
            let digest = Sha256::digest(format!("{}\0{}", body.format, body.content).as_bytes());
            (
                body.locale.clone(),
                format!("{}:{digest:x}", body.updated_at),
            )
        })
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
}
