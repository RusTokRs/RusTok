use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter};
use uuid::Uuid;

use rustok_core::SecurityContext;
use rustok_outbox::TransactionalEventBus;
use rustok_pages::dto::{CreatePageInput, PageBodyInput, PageTranslationInput};
use rustok_pages::services::PageService;

use crate::model::PageStarter;

/// Imports landing pages declared in the blueprint.
pub async fn import_pages(
    db: &DatabaseConnection,
    event_bus: &TransactionalEventBus,
    tenant_id: Uuid,
    security: &SecurityContext,
    pages: &[PageStarter],
    locale: &str,
) -> Result<(usize, usize), Box<dyn std::error::Error + Send + Sync>> {
    let service = PageService::new(db.clone(), event_bus.clone());
    let mut created = 0;
    let mut skipped = 0;

    for page in pages {
        // Idempotency: check if page translation with this slug already exists for this tenant
        let existing = rustok_pages::entities::page_translation::Entity::find()
            .filter(rustok_pages::entities::page_translation::Column::TenantId.eq(tenant_id))
            .filter(rustok_pages::entities::page_translation::Column::Slug.eq(&page.slug))
            .one(db)
            .await?;

        if existing.is_some() {
            tracing::info!(slug = %page.slug, "Page already exists, skipping");
            skipped += 1;
            continue;
        }

        let input = CreatePageInput {
            template: page.template.clone(),
            publish: false, // draft with Fly/GrapesJS document
            translations: vec![PageTranslationInput {
                locale: locale.to_string(),
                title: page.title.clone(),
                slug: Some(page.slug.clone()),
                meta_title: Some(page.title.clone()),
                meta_description: None,
            }],
            body: Some(PageBodyInput {
                locale: locale.to_string(),
                document: page.body.document.clone(),
            }),
            channel_slugs: page.channels.clone(),
        };

        service.create(tenant_id, security.clone(), input).await?;
        created += 1;
    }

    Ok((created, skipped))
}
