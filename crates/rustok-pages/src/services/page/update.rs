use chrono::Utc;
use sea_orm::{
    ActiveModelTrait,
    ActiveValue::Set,
    ColumnTrait, EntityTrait, QueryFilter, TransactionTrait,
};
use tracing::instrument;
use uuid::Uuid;

use rustok_api::{Action, Resource, PLATFORM_FALLBACK_LOCALE};
use rustok_content::entities::node::ContentStatus;
use rustok_core::{SecurityContext, CONTENT_FORMAT_GRAPESJS};
use rustok_events::DomainEvent;
use rustok_tenant::TenantService;

use crate::dto::{PageResponse, UpdatePageInput};
use crate::entities::{page, page_body, page_translation};
use crate::error::{
    PagesError, PagesResult, FEATURE_BUILDER_ENABLED, FEATURE_BUILDER_PREVIEW_ENABLED,
    FEATURE_BUILDER_PROPERTIES_ENABLED, FEATURE_BUILDER_PUBLISH_ENABLED,
};
use crate::services::page_builder_artifact::CompiledLandingArtifact;
use crate::services::rbac::{enforce_owned_scope, enforce_scope};
use crate::services::{PageBuilderArtifactService, PageBuilderScenarioBaselineService};

use super::helpers::{
    apply_transition, body_uses_builder_capability, build_page_metadata,
    collect_builder_project_values, compile_builder_sources, enforce_expected_version,
    is_builder_enabled, is_builder_preview_enabled, is_builder_properties_enabled,
    is_builder_publish_enabled, normalize_channel_slugs, normalize_page_body_input, normalize_slug,
    storage_to_status, transition_event, validate_page_translations,
};
use super::{PageService, PageTransition, PAGE_KIND};

impl PageService {
    #[instrument(skip(self, input))]
    pub async fn update(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        page_id: Uuid,
        input: UpdatePageInput,
    ) -> PagesResult<PageResponse> {
        if let Some(ref translations) = input.translations {
            validate_page_translations(translations)?;
        }

        let observed = self.find_page(tenant_id, page_id).await?;
        enforce_owned_scope(
            &security,
            Resource::Pages,
            Action::Update,
            observed.author_id,
        )?;
        enforce_expected_version(input.expected_version, observed.version)?;

        let transition = PageTransition::from_status(input.status.as_ref());
        let current_status = storage_to_status(&observed.status)?;
        let body = normalize_page_body_input(input.body)?;
        let candidate_uses_builder = body_uses_builder_capability(body.as_ref());
        let changes_page_content = input.translations.is_some()
            || input.template.is_some()
            || body.is_some()
            || input.channel_slugs.is_some();
        let mutates_public_page =
            current_status == ContentStatus::Published && changes_page_content;
        if transition.is_some() || mutates_public_page {
            enforce_scope(&security, Resource::Pages, Action::Publish)?;
        }
        if candidate_uses_builder {
            self.ensure_builder_enabled(tenant_id).await?;
        }

        let existing_bodies = if transition == Some(PageTransition::Publish) {
            self.load_bodies(page_id).await?
        } else {
            Vec::new()
        };
        let effective_builder_projects = if transition == Some(PageTransition::Publish) {
            collect_builder_project_values(&existing_bodies, body.as_ref(), true)?
        } else if transition.is_none()
            && current_status == ContentStatus::Published
            && candidate_uses_builder
        {
            collect_builder_project_values(&[], body.as_ref(), false)?
        } else {
            Vec::new()
        };
        if !effective_builder_projects.is_empty() {
            self.ensure_builder_enabled(tenant_id).await?;
            self.ensure_builder_publish_enabled(tenant_id).await?;
            PageBuilderScenarioBaselineService::new(self.db.clone())
                .ensure_candidates_allowed(tenant_id, page_id, effective_builder_projects)
                .await?;
        }

        let compiled = if transition == Some(PageTransition::Publish) {
            compile_builder_sources(&existing_bodies, body.as_ref(), true)?
        } else if transition.is_none()
            && current_status == ContentStatus::Published
            && candidate_uses_builder
        {
            compile_builder_sources(&[], body.as_ref(), false)?
        } else {
            Vec::new()
        };

        let template = input
            .template
            .clone()
            .unwrap_or_else(|| observed.template.clone());
        let metadata = build_page_metadata(
            &template,
            input.translations.as_deref().unwrap_or(&[]),
            Some(&observed.metadata),
        );
        let channel_slugs = input
            .channel_slugs
            .as_ref()
            .map(|items| normalize_channel_slugs(items))
            .unwrap_or_default();
        let replace_channel_visibility = input.channel_slugs.is_some();
        let response_locale = body
            .as_ref()
            .map(|body| body.locale.clone())
            .or_else(|| {
                input
                    .translations
                    .as_ref()
                    .and_then(|items| items.first().map(|item| item.locale.clone()))
            })
            .unwrap_or_else(|| PLATFORM_FALLBACK_LOCALE.to_string());

        let txn = self.db.begin().await?;
        let locked = self.find_page_for_update(&txn, tenant_id, page_id).await?;
        enforce_expected_version(Some(observed.version), locked.version)?;
        enforce_owned_scope(&security, Resource::Pages, Action::Update, locked.author_id)?;

        for translation in input.translations.as_deref().unwrap_or(&[]) {
            let slug = normalize_slug(
                translation
                    .slug
                    .as_deref()
                    .unwrap_or(translation.title.as_str()),
            );
            self.ensure_slug_unique_in_tx(
                &txn,
                tenant_id,
                &translation.locale,
                &slug,
                Some(page_id),
            )
            .await?;
        }

        let mut staged_artifacts = Vec::with_capacity(compiled.len());
        for compiled in &compiled {
            let artifact_id = PageBuilderArtifactService::stage_compiled_in_tx(
                &txn, tenant_id, page_id, compiled,
            )
            .await?;
            staged_artifacts.push((compiled.locale.clone(), artifact_id));
        }

        let now = Utc::now();
        let mut active: page::ActiveModel = locked.into();
        active.template = Set(template);
        active.metadata = Set(metadata);
        active.updated_at = Set(now.into());
        active.version = Set(active.version.take().unwrap_or(1) + 1);
        apply_transition(&mut active, transition, now);
        active.update(&txn).await?;

        if let Some(ref translations) = input.translations {
            self.replace_translations_in_tx(&txn, tenant_id, page_id, translations)
                .await?;
        }
        if replace_channel_visibility {
            self.replace_channel_visibility_in_tx(&txn, tenant_id, page_id, &channel_slugs)
                .await?;
        }
        if let Some(body) = body
            .as_ref()
            .filter(|body| body.format != CONTENT_FORMAT_GRAPESJS)
        {
            PageBuilderArtifactService::clear_existing_body_binding_in_tx(
                &txn,
                page_id,
                &body.locale,
            )
            .await?;
        }
        self.upsert_body_in_tx(&txn, page_id, body, now).await?;
        for (locale, artifact_id) in staged_artifacts {
            PageBuilderArtifactService::bind_existing_body_in_tx(
                &txn,
                tenant_id,
                page_id,
                &locale,
                artifact_id,
            )
            .await?;
        }

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
        if let Some(event) = transition_event(transition, page_id) {
            self.event_bus
                .publish_in_tx(&txn, tenant_id, security.user_id, event)
                .await?;
        }
        txn.commit().await?;
        self.get_with_locale_fallback(
            tenant_id,
            security,
            page_id,
            &response_locale,
            Some(PLATFORM_FALLBACK_LOCALE),
        )
        .await
    }

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
        let bodies = self.load_bodies(page_id).await?;
        let project_values = collect_builder_project_values(&bodies, None, true)?;
        if !project_values.is_empty() {
            self.ensure_builder_enabled(tenant_id).await?;
            self.ensure_builder_publish_enabled(tenant_id).await?;
            PageBuilderScenarioBaselineService::new(self.db.clone())
                .ensure_candidates_allowed(tenant_id, page_id, project_values)
                .await?;
        }
        let compiled = compile_builder_sources(&bodies, None, true)?;
        self.transition_page(
            tenant_id,
            security,
            page_id,
            PageTransition::Publish,
            observed.version,
            &compiled,
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
            &[],
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
        let existing = self.find_page(tenant_id, page_id).await?;
        enforce_owned_scope(
            &security,
            Resource::Pages,
            Action::Delete,
            existing.author_id,
        )?;
        let txn = self.db.begin().await?;
        page_body::Entity::delete_many()
            .filter(page_body::Column::PageId.eq(page_id))
            .exec(&txn)
            .await?;
        page_translation::Entity::delete_many()
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
        compiled: &[CompiledLandingArtifact],
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
            for compiled in compiled {
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

    pub(super) async fn ensure_builder_publish_enabled(
        &self,
        tenant_id: Uuid,
    ) -> PagesResult<()> {
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
