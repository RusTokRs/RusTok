use std::sync::Arc;

use chrono::Utc;
use rustok_core::{MigrationSource, SecurityContext};
use rustok_outbox::{OutboxTransport, SysEventsMigration, TransactionalEventBus};
use rustok_pages::entities::{page_route_alias, page_route_history_import, page_route_publication};
use rustok_pages::{
    CreatePageInput, ImportPageRouteHistoryInput, PAGE_ROUTE_HISTORY_IMPORT_CONFLICT,
    PageRouteDisposition, PageRouteHistoryImportItem, PageRouteHistoryImportService,
    PageRouteService, PageService, PageTranslationInput, PagesError, PagesModule,
};
use rustok_test_utils::db::setup_test_db;
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, DatabaseConnection, EntityTrait,
    PaginatorTrait, QueryFilter,
};
use sea_orm_migration::{MigrationTrait, SchemaManager};
use uuid::Uuid;

async fn setup() -> (
    DatabaseConnection,
    PageService,
    PageRouteHistoryImportService,
    Uuid,
) {
    let db = setup_test_db().await;
    let manager = SchemaManager::new(&db);
    SysEventsMigration
        .up(&manager)
        .await
        .expect("outbox migrations should apply");
    for migration in PagesModule.migrations() {
        migration
            .up(&manager)
            .await
            .expect("Pages migrations should apply");
    }
    let event_bus = TransactionalEventBus::new(Arc::new(OutboxTransport::new(db.clone())));
    let service = PageService::new(db.clone(), event_bus);
    let importer = PageRouteHistoryImportService::new(db.clone());
    (db, service, importer, Uuid::new_v4())
}

fn translation(title: &str, slug: &str) -> PageTranslationInput {
    PageTranslationInput {
        locale: "en".to_string(),
        title: title.to_string(),
        slug: Some(slug.to_string()),
        meta_title: None,
        meta_description: None,
    }
}

async fn create_draft(
    service: &PageService,
    tenant_id: Uuid,
    title: &str,
    slug: &str,
) -> rustok_pages::PageResponse {
    service
        .create(
            tenant_id,
            SecurityContext::system(),
            CreatePageInput {
                translations: vec![translation(title, slug)],
                template: Some("default".to_string()),
                body: None,
                channel_slugs: None,
                publish: false,
            },
        )
        .await
        .expect("page draft should be created")
}

fn import_input(
    source: &str,
    items: impl IntoIterator<Item = (&'static str, Uuid, &'static str)>,
) -> ImportPageRouteHistoryInput {
    ImportPageRouteHistoryInput {
        source: source.to_string(),
        items: items
            .into_iter()
            .map(
                |(source_record_id, page_id, slug)| PageRouteHistoryImportItem {
                    source_record_id: source_record_id.to_string(),
                    page_id,
                    locale: "en".to_string(),
                    slug: slug.to_string(),
                },
            )
            .collect(),
    }
}

#[tokio::test]
async fn existing_page_import_is_replay_safe_and_becomes_gone_after_delete() {
    let (db, service, importer, tenant_id) = setup().await;
    let draft = create_draft(&service, tenant_id, "Legacy", "legacy").await;

    let imported = importer
        .import_public_routes(
            tenant_id,
            SecurityContext::system(),
            import_input("legacy-export", [("page-legacy-en", draft.id, "legacy")]),
        )
        .await
        .expect("operator-proven public route should import");
    assert_eq!(imported.processed_item_count, 1);
    assert_eq!(imported.inserted_receipt_count, 1);
    assert_eq!(imported.replayed_receipt_count, 0);
    assert_eq!(imported.inserted_snapshot_count, 1);
    assert_eq!(imported.inserted_gone_alias_count, 0);

    let replay = importer
        .import_public_routes(
            tenant_id,
            SecurityContext::system(),
            import_input("LEGACY-EXPORT", [("page-legacy-en", draft.id, "legacy")]),
        )
        .await
        .expect("exact normalized provenance replay should verify");
    assert_eq!(replay.inserted_receipt_count, 0);
    assert_eq!(replay.replayed_receipt_count, 1);
    assert_eq!(replay.inserted_snapshot_count, 0);
    assert_eq!(replay.inserted_gone_alias_count, 0);

    service
        .delete(tenant_id, SecurityContext::system(), draft.id)
        .await
        .expect("retained imported route should tombstone in the delete transaction");

    let route = PageRouteService::new(db.clone())
        .resolve(tenant_id, "en", "legacy")
        .await
        .expect("deleted imported route should resolve as gone");
    assert_eq!(route.disposition, PageRouteDisposition::Gone);
    assert_eq!(route.requested_page_id, Some(draft.id));

    let duplicate = service
        .create(
            tenant_id,
            SecurityContext::system(),
            CreatePageInput {
                translations: vec![translation("Replacement", "legacy")],
                template: Some("default".to_string()),
                body: None,
                channel_slugs: None,
                publish: false,
            },
        )
        .await
        .expect_err("imported public claim must remain reserved");
    assert!(matches!(duplicate, PagesError::DuplicateSlug { .. }));

    assert_eq!(
        page_route_history_import::Entity::find()
            .filter(page_route_history_import::Column::TenantId.eq(tenant_id))
            .count(&db)
            .await
            .expect("import receipts should be queryable"),
        1
    );
}

#[tokio::test]
async fn deleted_page_import_preserves_redirects_and_rejects_provenance_drift() {
    let (db, _service, importer, tenant_id) = setup().await;
    let deleted_page_id = Uuid::new_v4();

    page_route_alias::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(tenant_id),
        page_id: Set(deleted_page_id),
        locale: Set("en".to_string()),
        slug: Set("about".to_string()),
        disposition: Set("redirect".to_string()),
        target_page_id: Set(Some(deleted_page_id)),
        target_locale: Set(Some("en".to_string())),
        reason: Set("Published page slug changed".to_string()),
        created_at: Set(Utc::now().into()),
    }
    .insert(&db)
    .await
    .expect("historical redirect should be seeded");

    let imported = importer
        .import_public_routes(
            tenant_id,
            SecurityContext::system(),
            import_input(
                "archive-ledger",
                [
                    ("deleted-about-en", deleted_page_id, "about"),
                    ("deleted-company-en", deleted_page_id, "company"),
                ],
            ),
        )
        .await
        .expect("redirect plus terminal route should import atomically");
    assert_eq!(imported.inserted_receipt_count, 2);
    assert_eq!(imported.inserted_snapshot_count, 2);
    assert_eq!(imported.inserted_gone_alias_count, 1);

    let routes = PageRouteService::new(db.clone());
    for slug in ["about", "company"] {
        let route = routes
            .resolve(tenant_id, "en", slug)
            .await
            .expect("every imported deleted route should resolve as gone");
        assert_eq!(route.disposition, PageRouteDisposition::Gone);
        assert_eq!(route.requested_page_id, Some(deleted_page_id));
    }

    let replay = importer
        .import_public_routes(
            tenant_id,
            SecurityContext::system(),
            import_input(
                "archive-ledger",
                [
                    ("deleted-about-en", deleted_page_id, "about"),
                    ("deleted-company-en", deleted_page_id, "company"),
                ],
            ),
        )
        .await
        .expect("exact deleted-page import should replay");
    assert_eq!(replay.inserted_receipt_count, 0);
    assert_eq!(replay.replayed_receipt_count, 2);
    assert_eq!(replay.inserted_snapshot_count, 0);
    assert_eq!(replay.inserted_gone_alias_count, 0);

    let drift = importer
        .import_public_routes(
            tenant_id,
            SecurityContext::system(),
            import_input(
                "archive-ledger",
                [("deleted-company-en", deleted_page_id, "different")],
            ),
        )
        .await
        .expect_err("a provenance key must not be rebound to another route");
    match drift {
        PagesError::Rich(error) => assert_eq!(
            error.error_code.as_deref(),
            Some(PAGE_ROUTE_HISTORY_IMPORT_CONFLICT)
        ),
        other => panic!("unexpected import drift error: {other}"),
    }

    assert_eq!(
        page_route_publication::Entity::find()
            .filter(page_route_publication::Column::TenantId.eq(tenant_id))
            .filter(page_route_publication::Column::PageId.eq(deleted_page_id))
            .count(&db)
            .await
            .expect("imported snapshots should be queryable"),
        2
    );
}

#[tokio::test]
async fn redirect_only_deleted_page_import_fails_closed_and_rolls_back() {
    let (db, _service, importer, tenant_id) = setup().await;
    let deleted_page_id = Uuid::new_v4();

    page_route_alias::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(tenant_id),
        page_id: Set(deleted_page_id),
        locale: Set("en".to_string()),
        slug: Set("historical".to_string()),
        disposition: Set("redirect".to_string()),
        target_page_id: Set(Some(deleted_page_id)),
        target_locale: Set(Some("en".to_string())),
        reason: Set("Published page slug changed".to_string()),
        created_at: Set(Utc::now().into()),
    }
    .insert(&db)
    .await
    .expect("historical redirect should be seeded");

    let error = importer
        .import_public_routes(
            tenant_id,
            SecurityContext::system(),
            import_input(
                "incomplete-ledger",
                [("redirect-only", deleted_page_id, "historical")],
            ),
        )
        .await
        .expect_err("missing page import without a terminal route must fail closed");
    assert!(matches!(error, PagesError::Rich(_)));

    assert_eq!(
        page_route_history_import::Entity::find()
            .filter(page_route_history_import::Column::TenantId.eq(tenant_id))
            .count(&db)
            .await
            .expect("failed import receipts should not commit"),
        0
    );
    assert_eq!(
        page_route_publication::Entity::find()
            .filter(page_route_publication::Column::TenantId.eq(tenant_id))
            .count(&db)
            .await
            .expect("failed import snapshots should not commit"),
        0
    );
}
