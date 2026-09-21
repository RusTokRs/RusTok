use std::sync::Arc;

use rustok_core::{MigrationSource, SecurityContext, UserRole};
use rustok_outbox::{OutboxTransport, SysEventsMigration, TransactionalEventBus};
use rustok_taxonomy::TaxonomyModule;
use rustok_test_utils::setup_test_db;
use sea_orm_migration::{MigrationTrait, SchemaManager};
use uuid::Uuid;

use crate::{CreateTagInput, TagService, UpdateTagInput};

async fn setup_schema() -> sea_orm::DatabaseConnection {
    let db = setup_test_db().await;
    let manager = SchemaManager::new(&db);

    SysEventsMigration
        .up(&manager)
        .await
        .expect("outbox migration should apply");
    for migration in TaxonomyModule.migrations() {
        migration
            .up(&manager)
            .await
            .expect("taxonomy migration should apply");
    }
    for migration in crate::migrations::migrations() {
        migration
            .up(&manager)
            .await
            .expect("blog migration should apply");
    }

    db
}

fn admin() -> SecurityContext {
    SecurityContext::new(UserRole::Admin, Some(Uuid::new_v4()))
}

#[tokio::test]
async fn module_owned_tag_update_does_not_require_english_translation() {
    let db = setup_schema().await;
    let service = TagService::new(db.clone());

    let tenant_id = Uuid::new_v4();
    let tag_id = service
        .create_tag(
            tenant_id,
            admin(),
            CreateTagInput {
                locale: "ru".to_string(),
                name: "Системы".to_string(),
                slug: Some("sistemy".to_string()),
            },
        )
        .await
        .expect("a non-English-only Blog tag should be creatable");

    let updated = service
        .update_tag(
            tenant_id,
            tag_id,
            admin(),
            UpdateTagInput {
                locale: "ru".to_string(),
                name: Some("Системные системы".to_string()),
                slug: Some("sistemnye-sistemy".to_string()),
            },
        )
        .await
        .expect("updating a non-English-only Blog tag must not probe en");

    assert_eq!(updated.id, tag_id);
    assert_eq!(updated.locale, "ru");
    assert_eq!(updated.name, "Системные системы");
    assert_eq!(updated.slug, "sistemnye-sistemy");
}

#[tokio::test]
async fn module_owned_tag_delete_does_not_require_english_translation() {
    let db = setup_schema().await;
    let service = TagService::new(db);

    let tenant_id = Uuid::new_v4();
    let tag_id = service
        .create_tag(
            tenant_id,
            admin(),
            CreateTagInput {
                locale: "ru".to_string(),
                name: "Системы".to_string(),
                slug: Some("sistemy".to_string()),
            },
        )
        .await
        .expect("a non-English-only Blog tag should be creatable");

    service
        .delete_tag(tenant_id, tag_id, admin())
        .await
        .expect("deleting a non-English-only Blog tag must not probe en");
}
