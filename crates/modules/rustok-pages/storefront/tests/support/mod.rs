//! Published-landing fixture shared by the `ssr` storefront tests.
//!
//! The storefront renders only the landing artifact a reviewed publish bound to the page, so a
//! test that needs page content seeds that binding. The rows are written directly: these tests
//! exercise the storefront read path, not the reviewed publish command that owns the writes.

use std::error::Error;

use chrono::Utc;
use rustok_page_builder::PAGE_BUILDER_DOCUMENT_FORMAT;
use rustok_page_builder::static_landing::StaticLandingCompiler;
use rustok_pages::entities::{
    page_body, page_published_landing_artifact, page_static_landing_artifact,
};
use sea_orm::{ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, Set};
use serde_json::json;
use uuid::Uuid;

const LANDING_LOCALE: &str = "en";

/// Binds a freshly compiled `en` landing whose markup contains `marker` to the page, replacing
/// the landing bound before. The page must already have an `en` translation.
pub async fn publish_landing(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    page_id: Uuid,
    marker: &str,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    let project = json!({
        "pages": [{
            "id": "home",
            "flyPageMeta": {
                "title": marker,
                "description": "Storefront landing fixture",
                "slug": "home"
            },
            "component": {
                "id": "root",
                "type": "wrapper",
                "components": [{
                    "id": "heading",
                    "type": "heading",
                    "tagName": "h1",
                    "content": marker
                }]
            }
        }]
    });
    let artifact = StaticLandingCompiler::default().compile_publish(&project)?;
    artifact.verify_integrity()?;
    let rendered = artifact
        .pages
        .first()
        .cloned()
        .ok_or_else(|| std::io::Error::other("compiled landing artifact has no page"))?;
    let now: sea_orm::prelude::DateTimeWithTimeZone = Utc::now().into();

    let existing_body = page_body::Entity::find()
        .filter(page_body::Column::TenantId.eq(tenant_id))
        .filter(page_body::Column::PageId.eq(page_id))
        .filter(page_body::Column::Locale.eq(LANDING_LOCALE))
        .one(db)
        .await?;
    let body_id = match existing_body {
        Some(body) => {
            let body_id = body.id;
            let mut active: page_body::ActiveModel = body.into();
            active.content = Set(serde_json::to_string(&project)?);
            active.format = Set(PAGE_BUILDER_DOCUMENT_FORMAT.to_string());
            active.updated_at = Set(now);
            active.update(db).await?;
            body_id
        }
        None => {
            let body_id = Uuid::new_v4();
            page_body::ActiveModel {
                id: Set(body_id),
                tenant_id: Set(tenant_id),
                page_id: Set(page_id),
                locale: Set(LANDING_LOCALE.to_string()),
                content: Set(serde_json::to_string(&project)?),
                format: Set(PAGE_BUILDER_DOCUMENT_FORMAT.to_string()),
                updated_at: Set(now),
            }
            .insert(db)
            .await?;
            body_id
        }
    };

    let artifact_id = Uuid::new_v4();
    page_static_landing_artifact::ActiveModel {
        id: Set(artifact_id),
        tenant_id: Set(tenant_id),
        page_id: Set(page_id),
        locale: Set(LANDING_LOCALE.to_string()),
        source_hash: Set(artifact.identity.source_hash.clone()),
        build_hash: Set(artifact.identity.build_hash.clone()),
        artifact_hash: Set(artifact.artifact_hash.clone()),
        materialization_hash: Set(None),
        materialization_identity: Set(None),
        runtime_snapshots: Set(None),
        renderer_id: Set(artifact.identity.renderer.id.clone()),
        renderer_release: Set(artifact.identity.renderer.release.clone()),
        identity: Set(serde_json::to_value(&artifact.identity)?),
        registry: Set(serde_json::to_value(&artifact.registry)?),
        page_index: Set(i32::try_from(rendered.page_index)?),
        fly_page_id: Set(rendered.page_id.clone()),
        slug: Set(rendered.slug.clone()),
        head: Set(serde_json::to_value(&rendered.head)?),
        document_html: Set(rendered.document_html.clone()),
        body_html: Set(rendered.body_html.clone()),
        css: Set(rendered.css.clone()),
        content_hash: Set(rendered.content_hash.clone()),
        landing_sections: Set(serde_json::to_value(&rendered.landing_sections)?),
        instance_key: Set("canonical".to_string()),
        created_at: Set(now),
    }
    .insert(db)
    .await?;

    page_published_landing_artifact::Entity::delete_by_id(body_id)
        .exec(db)
        .await?;
    page_published_landing_artifact::ActiveModel {
        page_body_id: Set(body_id),
        tenant_id: Set(tenant_id),
        page_id: Set(page_id),
        locale: Set(LANDING_LOCALE.to_string()),
        artifact_id: Set(artifact_id),
        published_at: Set(now),
    }
    .insert(db)
    .await?;
    Ok(())
}
