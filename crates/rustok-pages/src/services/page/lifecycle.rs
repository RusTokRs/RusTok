use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, ConnectionTrait, DbBackend, EntityTrait,
    QueryFilter, QueryOrder, QuerySelect, TransactionTrait,
};
use sha2::{Digest, Sha256};
use tracing::instrument;
use uuid::Uuid;

use rustok_api::{Action, Resource};
use rustok_core::SecurityContext;
use rustok_events::DomainEvent;
use rustok_tenant::TenantService;

use crate::dto::PageResponse;
use crate::entities::{page, page_body, page_translation};
use crate::error::{
    FEATURE_BUILDER_ENABLED, FEATURE_BUILDER_PREVIEW_ENABLED, FEATURE_BUILDER_PROPERTIES_ENABLED,
    FEATURE_BUILDER_PUBLISH_ENABLED, PagesError, PagesResult,
};
use crate::services::rbac::enforce_owned_scope;
use crate::services::{PageBuilderArtifactService, PageBuilderScenarioBaselineService};

use super::document::document_revision_conflict;
use super::helpers::{
    apply_transition, collect_builder_project_values, compile_builder_sources,
    enforce_expected_version, is_builder_enabled, is_builder_preview_enabled,
    is_builder_properties_enabled, is_builder_publish_enabled, transition_event,
};
use super::{PAGE_KIND, PageService, PageTransition};

type BodyRevisionSnapshot = Vec<(String, String)>;

impl PageService {
    #[instrument(skip(self))]
    pub async fn publish(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        page_id: Uuid,
    ) -> PagesResult<PageResponse> {
        self.publish_if_current(tenant_id, security, page_id, None)
            .await
    }

    #[instrument(skip(self))]
    pub async fn publish_if_current(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        page_id: Uuid,
        expected_version: Option<i32>,
    ) -> PagesResult<PageResponse> {
        let observed = self.find_page(tenant_id, page_id).await?;
        enforce_owned_scope(
            &security,
            Resource::Pages,
            Action::Publish,
            observed.author_id,
        )?;
        enforce_expected_version(expected_version, observed.version)?;

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

        self.transition_page(
            tenant_id,
            security,
            page_id,
            PageTransition::Publish,
            observed.version,
            Some(body_revisions),
        )
        .await
    }

    #[instrument(skip(self))]
    pub async fn unpublish(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        page_id: Uuid,
    ) -> PagesResult<PageResponse> {
        self.unpublish_if_current(tenant_id, security, page_id, None)
            .await
    }

    #[instrument(skip(self))]
    pub async fn unpublish_if_current(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        page_id: Uuid,
        expected_version: Option<i32>,
    ) -> PagesResult<PageResponse> {
        let observed = self.find_page(tenant_id, page_id).await?;
        enforce_owned_scope(
            &security,
            Resource::Pages,
            Action::Publish,
            observed.author_id,
        )?;
        enforce_expected_version(expected_version, observed.version)?;
        self.transition_page(
            tenant_id,
            security,
            page_id,
            PageTransition::Unpublish,
            observed.version,
            None,
        )
        .await
    }

    #[instrument(skip(self))]
    pub async fn archive(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        page_id: Uuid,
    ) -> PagesResult<PageResponse> {
        self.archive_if_current(tenant_id, security, page_id, None)
            .await
    }

    #[instrument(skip(self))]
    pub async fn archive_if_current(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        page_id: Uuid,
        expected_version: Option<i32>,
    ) -> PagesResult<PageResponse> {
        let observed = self.find_page(tenant_id, page_id).await?;
        enforce_owned_scope(
            &security,
            Resource::Pages,
            Action::Publish,
            observed.author_id,
        )?;
        enforce_expected_version(expected_version, observed.version)?;
        self.transition_page(
            tenant_id,
            security,
            page_id,
            PageTransition::Archive,
            observed.version,
            None,
        )
        .await
    }

    #[instrument(skip(self))]
    pub async fn ensure_builder_preview_enabled_for_tenant(
        &self,
        tenant_id: Uuid,
    ) -> PagesResult<()> {
        let module = self.load_tenant_pages_module(tenant_id).await?;
        let enabled = module
            .as_ref()
            .map(is_builder_preview_enabled)
            .unwrap_or(true);
        if !enabled {
            return Err(PagesError::feature_disabled(
                FEATURE_BUILDER_PREVIEW_ENABLED,
            ));
        }
        Ok(())
    }

    #[instrument(skip(self))]
    pub async fn ensure_builder_properties_enabled_for_tenant(
        &self,
        tenant_id: Uuid,
    ) -> PagesResult<()> {
        let module = self.load_tenant_pages_module(tenant_id).await?;
        let enabled = module
            .as_ref()
            .map(is_builder_properties_enabled)
            .unwrap_or(true);
        if !enabled {
            return Err(PagesError::feature_disabled(
                FEATURE_BUILDER_PROPERTIES_ENABLED,
            ));
        }
        Ok(())
    }

    pub async fn delete(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        page_id: Uuid,
    ) -> PagesResult<()> {
        let txn = self.db.begin().await?;
        let existing = self.find_page_for_update(&txn, tenant_id, page_id).await?;
        enforce_owned_scope(
            &security,
            Resource::Pages,
            Action::Delete,
            existing.author_id,
        )?;
        if existing.status == "published" {
            return Err(PagesError::cannot_delete_published());
        }

        page_body::Entity::delete_many()
            .filter(page_body::Column::TenantId.eq(tenant_id))
            .filter(page_body::Column::PageId.eq(page_id))
            .exec(&txn)
            .await?;
        page_translation::Entity::delete_many()
            .filter(page_translation::Column::TenantId.eq(tenant_id))
            .filter(page_translation::Column::PageId.eq(page_id))
            .exec(&txn)
            .await?;
        page::Entity::delete_by_id(page_id).exec(&txn).await?;
        self.event_bus
            .publish_in_tx(
                &txn,
                tenant_id,
                security.user_id,
                DomainEvent::NodeDeleted {
                    node_id: page_id,
                    kind: PAGE_KIND.to_string(),
                },
            )
            .await?;
        txn.commit().await?;
        Ok(())
    }

    async fn transition_page(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        page_id: Uuid,
        transition: PageTransition,
        expected_version: i32,
        expected_body_revisions: Option<BodyRevisionSnapshot>,
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

        if transition == PageTransition::Publish {
            let current_bodies = load_bodies_for_publish(&txn, tenant_id, page_id).await?;
            let current_revisions = body_revision_snapshot(&current_bodies);
            let expected_revisions = expected_body_revisions.unwrap_or_default();
            if current_revisions != expected_revisions {
                return Err(document_revision_conflict(
                    format_body_revisions(&expected_revisions),
                    format_body_revisions(&current_revisions),
                ));
            }

            let compiled = compile_builder_sources(&current_bodies, None, true)?;
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
        }

        let now = Utc::now();
        let mut active: page::ActiveModel = existing.into();
        active.updated_at = Set(now.into());
        active.version = Set(active.version.take().unwrap_or(1) + 1);
        apply_transition(&mut active, Some(transition), now);
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
        if let Some(event) = transition_event(Some(transition), page_id) {
            self.event_bus
                .publish_in_tx(&txn, tenant_id, security.user_id, event)
                .await?;
        }
        txn.commit().await?;
        self.get(tenant_id, security, page_id).await
    }

    pub(super) async fn ensure_builder_publish_enabled(&self, tenant_id: Uuid) -> PagesResult<()> {
        let module = self.load_tenant_pages_module(tenant_id).await?;
        let enabled = module
            .as_ref()
            .map(is_builder_publish_enabled)
            .unwrap_or(true);
        if !enabled {
            return Err(PagesError::feature_disabled(
                FEATURE_BUILDER_PUBLISH_ENABLED,
            ));
        }
        Ok(())
    }

    pub(super) async fn ensure_builder_enabled(&self, tenant_id: Uuid) -> PagesResult<()> {
        let module = self.load_tenant_pages_module(tenant_id).await?;
        let enabled = module.as_ref().map(is_builder_enabled).unwrap_or(true);
        if !enabled {
            return Err(PagesError::feature_disabled(FEATURE_BUILDER_ENABLED));
        }
        Ok(())
    }

    async fn load_tenant_pages_module(
        &self,
        tenant_id: Uuid,
    ) -> PagesResult<Option<serde_json::Value>> {
        TenantService::new(self.db.clone())
            .find_tenant_module(tenant_id, "pages")
            .await
            .map(|module| module.map(|module| module.settings))
            .map_err(Into::into)
    }
}

async fn load_bodies_for_publish(
    txn: &sea_orm::DatabaseTransaction,
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
                format!("{}:{}", body.updated_at, encode_digest(&digest)),
            )
        })
        .collect::<Vec<_>>();
    revisions.sort();
    revisions
}

fn encode_digest(digest: &[u8]) -> String {
    use std::fmt::Write as _;

    let mut encoded = String::with_capacity(digest.len() * 2);
    for byte in digest {
        write!(&mut encoded, "{byte:02x}").expect("writing to a String cannot fail");
    }
    encoded
}

fn format_body_revisions(revisions: &BodyRevisionSnapshot) -> String {
    revisions
        .iter()
        .map(|(locale, revision)| format!("{locale}:{revision}"))
        .collect::<Vec<_>>()
        .join(",")
}
