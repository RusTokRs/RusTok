use chrono::Utc;
use rustok_page_builder::{
    ComponentRegistryManifest, LandingSectionSnapshot, PageHead, StaticLandingArtifact,
    StaticLandingBuildIdentity, StaticLandingPage, static_landing::StaticLandingCompiler,
};
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, ConnectionTrait, DatabaseConnection,
    DatabaseTransaction, DbBackend, EntityTrait, QueryFilter, QuerySelect, TransactionTrait,
    sea_query::OnConflict,
};
use serde_json::Value;
use uuid::Uuid;

use crate::entities::{
    page, page_body, page_channel_visibility, page_published_landing_artifact,
    page_static_landing_artifact,
};
use crate::error::{PagesError, PagesResult};

const MAX_DOCUMENT_HTML_BYTES: usize = 2 * 1024 * 1024;
const MAX_BODY_HTML_BYTES: usize = 1536 * 1024;
const MAX_CSS_BYTES: usize = 512 * 1024;

#[derive(Debug, Clone)]
pub(crate) struct CompiledLandingArtifact {
    pub locale: String,
    pub artifact: StaticLandingArtifact,
    pub page: StaticLandingPage,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PublishedLandingArtifact {
    pub page_id: Uuid,
    pub locale: String,
    pub build_hash: String,
    pub artifact_hash: String,
    pub document_html: String,
    pub css: String,
    pub content_hash: String,
}

#[derive(Clone)]
pub struct PageBuilderArtifactService {
    db: DatabaseConnection,
}

impl PageBuilderArtifactService {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    pub(crate) fn compile_source(
        locale: &str,
        content: &str,
    ) -> PagesResult<CompiledLandingArtifact> {
        let project_data: Value = serde_json::from_str(content).map_err(|error| {
            PagesError::validation(format!(
                "Page Builder project for locale `{locale}` is not valid JSON: {error}"
            ))
        })?;
        let artifact = StaticLandingCompiler::default()
            .compile_publish(&project_data)
            .map_err(artifact_compile_error)?;
        artifact
            .verify_integrity()
            .map_err(artifact_integrity_error)?;
        if artifact.pages.len() != 1 {
            return Err(PagesError::validation(format!(
                "A Pages Page Builder body must contain exactly one Fly page; found {}",
                artifact.pages.len()
            )));
        }
        let page = artifact.pages[0].clone();
        enforce_size_limits(&page)?;
        Ok(CompiledLandingArtifact {
            locale: locale.to_string(),
            artifact,
            page,
        })
    }

    pub(crate) async fn stage_compiled_in_tx(
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        page_id: Uuid,
        compiled: &CompiledLandingArtifact,
    ) -> PagesResult<Uuid> {
        compiled
            .artifact
            .verify_integrity()
            .map_err(artifact_integrity_error)?;
        enforce_size_limits(&compiled.page)?;
        let identity = &compiled.artifact.identity;
        if let Some(existing) = find_artifact_in_tx(
            txn,
            tenant_id,
            page_id,
            &compiled.locale,
            &identity.build_hash,
        )
        .await?
        {
            verify_record(&existing)?;
            ensure_same_artifact(&existing, compiled)?;
            return Ok(existing.id);
        }

        let artifact_id = Uuid::new_v4();
        let model = page_static_landing_artifact::ActiveModel {
            id: Set(artifact_id),
            tenant_id: Set(tenant_id),
            page_id: Set(page_id),
            locale: Set(compiled.locale.clone()),
            source_hash: Set(identity.source_hash.clone()),
            build_hash: Set(identity.build_hash.clone()),
            artifact_hash: Set(compiled.artifact.artifact_hash.clone()),
            renderer_id: Set(identity.renderer.id.clone()),
            renderer_release: Set(identity.renderer.release.clone()),
            identity: Set(to_json(identity, "landing build identity")?),
            registry: Set(to_json(&compiled.artifact.registry, "component registry")?),
            page_index: Set(i32::try_from(compiled.page.page_index)
                .map_err(|_| PagesError::validation("static landing page index exceeds i32"))?),
            fly_page_id: Set(compiled.page.page_id.clone()),
            slug: Set(compiled.page.slug.clone()),
            head: Set(to_json(&compiled.page.head, "page head")?),
            document_html: Set(compiled.page.document_html.clone()),
            body_html: Set(compiled.page.body_html.clone()),
            css: Set(compiled.page.css.clone()),
            content_hash: Set(compiled.page.content_hash.clone()),
            landing_sections: Set(to_json(
                &compiled.page.landing_sections,
                "landing section manifest",
            )?),
            created_at: Set(Utc::now().into()),
        };

        page_static_landing_artifact::Entity::insert(model)
            .on_conflict(
                OnConflict::columns([
                    page_static_landing_artifact::Column::TenantId,
                    page_static_landing_artifact::Column::PageId,
                    page_static_landing_artifact::Column::Locale,
                    page_static_landing_artifact::Column::BuildHash,
                ])
                .do_nothing()
                .to_owned(),
            )
            .exec_without_returning(txn)
            .await?;

        let stored = find_artifact_in_tx(
            txn,
            tenant_id,
            page_id,
            &compiled.locale,
            &identity.build_hash,
        )
        .await?
        .ok_or_else(|| {
            PagesError::artifact_integrity(
                "static landing artifact insert completed without a readable record",
            )
        })?;
        verify_record(&stored)?;
        ensure_same_artifact(&stored, compiled)?;
        Ok(stored.id)
    }

    pub(crate) async fn bind_existing_body_in_tx(
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        page_id: Uuid,
        locale: &str,
        artifact_id: Uuid,
    ) -> PagesResult<()> {
        let body = page_body::Entity::find()
            .filter(page_body::Column::PageId.eq(page_id))
            .filter(page_body::Column::Locale.eq(locale))
            .one(txn)
            .await?
            .ok_or_else(|| {
                PagesError::artifact_integrity(format!(
                    "cannot bind landing artifact: page body `{page_id}/{locale}` does not exist"
                ))
            })?;
        let artifact = page_static_landing_artifact::Entity::find_by_id(artifact_id)
            .filter(page_static_landing_artifact::Column::TenantId.eq(tenant_id))
            .filter(page_static_landing_artifact::Column::PageId.eq(page_id))
            .filter(page_static_landing_artifact::Column::Locale.eq(locale))
            .one(txn)
            .await?
            .ok_or_else(|| {
                PagesError::artifact_integrity(format!(
                    "cannot bind unknown landing artifact `{artifact_id}` to `{page_id}/{locale}`"
                ))
            })?;
        verify_record(&artifact)?;

        let now: sea_orm::prelude::DateTimeWithTimeZone = Utc::now().into();
        match page_published_landing_artifact::Entity::find_by_id(body.id)
            .one(txn)
            .await?
        {
            Some(existing) => {
                let mut active: page_published_landing_artifact::ActiveModel = existing.into();
                active.artifact_id = Set(artifact_id);
                active.published_at = Set(now);
                active.update(txn).await?;
            }
            None => {
                page_published_landing_artifact::ActiveModel {
                    page_body_id: Set(body.id),
                    artifact_id: Set(artifact_id),
                    published_at: Set(now),
                }
                .insert(txn)
                .await?;
            }
        }
        Ok(())
    }

    pub(crate) async fn clear_existing_body_binding_in_tx(
        txn: &DatabaseTransaction,
        page_id: Uuid,
        locale: &str,
    ) -> PagesResult<()> {
        let body = page_body::Entity::find()
            .filter(page_body::Column::PageId.eq(page_id))
            .filter(page_body::Column::Locale.eq(locale))
            .one(txn)
            .await?;
        if let Some(body) = body {
            page_published_landing_artifact::Entity::delete_by_id(body.id)
                .exec(txn)
                .await?;
        }
        Ok(())
    }

    /// Loads the currently published artifact for a public storefront request.
    ///
    /// Page status, channel visibility, locale binding and artifact integrity are evaluated in one
    /// transaction. This prevents an unpublish or visibility update from completing between the
    /// authorization decision and the returned HTML snapshot.
    pub async fn load_public_bound_artifact_with_fallback(
        &self,
        tenant_id: Uuid,
        page_id: Uuid,
        locale: &str,
        fallback_locale: Option<&str>,
        channel_slug: Option<&str>,
    ) -> PagesResult<Option<PublishedLandingArtifact>> {
        let txn = self.db.begin().await?;
        let page_query =
            || page::Entity::find_by_id(page_id).filter(page::Column::TenantId.eq(tenant_id));
        let page = match txn.get_database_backend() {
            DbBackend::Sqlite => page_query().one(&txn).await?,
            DbBackend::Postgres | DbBackend::MySql => page_query().lock_shared().one(&txn).await?,
        };

        let is_published = page.is_some_and(|page| page.status == "published");
        let is_visible = is_published
            && page_is_visible_for_channel_in_tx(&txn, tenant_id, page_id, channel_slug).await?;
        let result = if is_visible {
            if let Some(artifact) =
                load_bound_artifact_in_tx(&txn, tenant_id, page_id, locale).await?
            {
                Some(artifact)
            } else if let Some(fallback_locale) =
                fallback_locale.filter(|fallback| *fallback != locale)
            {
                load_bound_artifact_in_tx(&txn, tenant_id, page_id, fallback_locale).await?
            } else {
                None
            }
        } else {
            None
        };

        txn.commit().await?;
        Ok(result)
    }
}

async fn page_is_visible_for_channel_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    page_id: Uuid,
    channel_slug: Option<&str>,
) -> PagesResult<bool> {
    let channel_slugs = page_channel_visibility::Entity::find()
        .filter(page_channel_visibility::Column::TenantId.eq(tenant_id))
        .filter(page_channel_visibility::Column::PageId.eq(page_id))
        .all(txn)
        .await?
        .into_iter()
        .map(|record| record.channel_slug)
        .collect::<Vec<_>>();
    Ok(is_visible_for_channel(&channel_slugs, channel_slug))
}

fn is_visible_for_channel(channel_slugs: &[String], channel_slug: Option<&str>) -> bool {
    if channel_slugs.is_empty() {
        return true;
    }
    let Some(channel_slug) = channel_slug
        .map(str::trim)
        .filter(|slug| !slug.is_empty())
        .map(str::to_ascii_lowercase)
    else {
        return false;
    };
    channel_slugs.iter().any(|allowed| allowed == &channel_slug)
}

async fn load_bound_artifact_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    page_id: Uuid,
    locale: &str,
) -> PagesResult<Option<PublishedLandingArtifact>> {
    let Some(body) = page_body::Entity::find()
        .filter(page_body::Column::PageId.eq(page_id))
        .filter(page_body::Column::Locale.eq(locale))
        .filter(page_body::Column::Format.eq(rustok_core::CONTENT_FORMAT_GRAPESJS))
        .one(txn)
        .await?
    else {
        return Ok(None);
    };
    let Some(binding) = page_published_landing_artifact::Entity::find_by_id(body.id)
        .one(txn)
        .await?
    else {
        return Ok(None);
    };
    let record = page_static_landing_artifact::Entity::find_by_id(binding.artifact_id)
        .filter(page_static_landing_artifact::Column::TenantId.eq(tenant_id))
        .filter(page_static_landing_artifact::Column::PageId.eq(page_id))
        .filter(page_static_landing_artifact::Column::Locale.eq(locale))
        .one(txn)
        .await?
        .ok_or_else(|| {
            PagesError::artifact_integrity(format!(
                "published landing binding for body `{}` references an invalid artifact",
                body.id
            ))
        })?;
    published_record(record).map(Some)
}

async fn find_artifact_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    page_id: Uuid,
    locale: &str,
    build_hash: &str,
) -> PagesResult<Option<page_static_landing_artifact::Model>> {
    Ok(page_static_landing_artifact::Entity::find()
        .filter(page_static_landing_artifact::Column::TenantId.eq(tenant_id))
        .filter(page_static_landing_artifact::Column::PageId.eq(page_id))
        .filter(page_static_landing_artifact::Column::Locale.eq(locale))
        .filter(page_static_landing_artifact::Column::BuildHash.eq(build_hash))
        .one(txn)
        .await?)
}

fn published_record(
    record: page_static_landing_artifact::Model,
) -> PagesResult<PublishedLandingArtifact> {
    verify_record(&record)?;
    Ok(PublishedLandingArtifact {
        page_id: record.page_id,
        locale: record.locale,
        build_hash: record.build_hash,
        artifact_hash: record.artifact_hash,
        document_html: record.document_html,
        css: record.css,
        content_hash: record.content_hash,
    })
}

fn verify_record(record: &page_static_landing_artifact::Model) -> PagesResult<()> {
    let identity: StaticLandingBuildIdentity =
        from_json(&record.identity, "landing build identity")?;
    let registry: ComponentRegistryManifest = from_json(&record.registry, "component registry")?;
    let head: PageHead = from_json(&record.head, "page head")?;
    let landing_sections: Vec<LandingSectionSnapshot> =
        from_json(&record.landing_sections, "landing section manifest")?;
    let page_index = usize::try_from(record.page_index)
        .map_err(|_| PagesError::artifact_integrity("stored landing page index is negative"))?;
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
            "stored static landing artifact metadata does not match its payload",
        ));
    }
    Ok(())
}

fn ensure_same_artifact(
    record: &page_static_landing_artifact::Model,
    compiled: &CompiledLandingArtifact,
) -> PagesResult<()> {
    if record.artifact_hash != compiled.artifact.artifact_hash
        || record.content_hash != compiled.page.content_hash
        || record.document_html != compiled.page.document_html
        || record.body_html != compiled.page.body_html
        || record.css != compiled.page.css
    {
        return Err(PagesError::artifact_integrity(format!(
            "static landing artifact collision for build hash `{}`",
            compiled.artifact.identity.build_hash
        )));
    }
    Ok(())
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

fn to_json(value: &impl serde::Serialize, label: &str) -> PagesResult<Value> {
    serde_json::to_value(value).map_err(|error| {
        PagesError::artifact_integrity(format!("unable to encode {label}: {error}"))
    })
}

fn from_json<T>(value: &Value, label: &str) -> PagesResult<T>
where
    T: serde::de::DeserializeOwned,
{
    serde_json::from_value(value.clone()).map_err(|error| {
        PagesError::artifact_integrity(format!("unable to decode stored {label}: {error}"))
    })
}

fn artifact_compile_error(error: impl std::fmt::Display) -> PagesError {
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

    #[test]
    fn compiler_produces_a_verified_single_page_artifact() {
        let content = serde_json::json!({
            "pages": [{
                "id": "home",
                "flyPageMeta": {
                    "title": "Home",
                    "description": "A stable landing page",
                    "slug": "home"
                },
                "component": {
                    "id": "root",
                    "type": "wrapper",
                    "components": [{
                        "id": "title",
                        "type": "heading",
                        "tagName": "h1",
                        "content": "Welcome"
                    }]
                }
            }]
        })
        .to_string();
        let compiled =
            PageBuilderArtifactService::compile_source("en", &content).expect("compiled artifact");
        assert_eq!(compiled.locale, "en");
        assert_eq!(compiled.artifact.pages.len(), 1);
        assert_eq!(compiled.page, compiled.artifact.pages[0]);
        compiled
            .artifact
            .verify_integrity()
            .expect("artifact integrity");
    }

    #[test]
    fn compiler_requires_exactly_one_fly_page() {
        let content = serde_json::json!({ "pages": [] }).to_string();
        assert!(PageBuilderArtifactService::compile_source("en", &content).is_err());
    }

    #[test]
    fn unrestricted_artifact_is_visible_without_a_channel() {
        assert!(is_visible_for_channel(&[], None));
    }

    #[test]
    fn restricted_artifact_requires_the_matching_channel() {
        let channels = vec!["web".to_string(), "mobile".to_string()];
        assert!(is_visible_for_channel(&channels, Some(" WEB ")));
        assert!(!is_visible_for_channel(&channels, Some("partner")));
        assert!(!is_visible_for_channel(&channels, None));
    }

    #[test]
    fn size_limit_rejects_oversized_document() {
        assert!(enforce_max("document HTML", 11, 10).is_err());
        assert!(enforce_max("document HTML", 10, 10).is_ok());
    }
}
