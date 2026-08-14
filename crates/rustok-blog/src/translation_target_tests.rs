use std::{sync::Arc, time::Duration};

use rustok_api::{PortActor, PortContext, PortErrorKind, TenantLocale};
use rustok_core::{MigrationSource, SecurityContext, UserRole};
use rustok_outbox::{OutboxTransport, SysEvents, SysEventsMigration, TransactionalEventBus};
use rustok_taxonomy::TaxonomyModule;
use rustok_test_utils::db::setup_test_db;
use rustok_translation_targets::{
    FieldKey, ListTranslationResourcesRequest, ReadTranslationResourceRequest,
    TranslationFieldPatch, TranslationPatchRequest, TranslationTargetChangesRequest,
    TranslationTargetProgressRequest, TranslationTargetProvider,
};
use sea_orm::{DatabaseConnection, EntityTrait};
use sea_orm_migration::{MigrationTrait, SchemaManager};
use uuid::Uuid;

use crate::{
    BlogCategoryTranslationTargetProvider, BlogModule, CategoryService, CreateCategoryInput,
    UpdateCategoryInput,
};

async fn setup() -> (DatabaseConnection, Arc<CategoryService>) {
    let database = setup_test_db().await;
    let manager = SchemaManager::new(&database);
    SysEventsMigration
        .up(&manager)
        .await
        .expect("outbox migration should apply");
    for migration in TaxonomyModule.migrations() {
        migration
            .up(&manager)
            .await
            .expect("Taxonomy migration should apply");
    }
    for migration in BlogModule.migrations() {
        migration
            .up(&manager)
            .await
            .expect("Blog migration should apply");
    }
    let event_bus = TransactionalEventBus::new(Arc::new(OutboxTransport::new(database.clone())));
    let service = Arc::new(CategoryService::new(database.clone(), event_bus));
    (database, service)
}

#[tokio::test]
async fn translation_target_schema_supports_up_down_up() {
    let database = setup_test_db().await;
    let manager = SchemaManager::new(&database);
    SysEventsMigration
        .up(&manager)
        .await
        .expect("outbox migration should apply");
    for migration in TaxonomyModule.migrations() {
        migration
            .up(&manager)
            .await
            .expect("Taxonomy migration should apply");
    }
    let mut migrations = BlogModule.migrations();
    let translation_target_migration = migrations
        .pop()
        .expect("Blog translation target migration should be registered");

    for migration in migrations {
        migration
            .up(&manager)
            .await
            .expect("base Blog migration should apply");
    }
    translation_target_migration
        .up(&manager)
        .await
        .expect("Blog translation target migration should apply");
    translation_target_migration
        .down(&manager)
        .await
        .expect("Blog translation target migration should roll back");
    translation_target_migration
        .up(&manager)
        .await
        .expect("Blog translation target migration should reapply");

    let event_bus = TransactionalEventBus::new(Arc::new(OutboxTransport::new(database.clone())));
    let service = CategoryService::new(database, event_bus);
    service
        .create(
            Uuid::new_v4(),
            admin(),
            category_input("Migration proof", "migration-proof"),
        )
        .await
        .expect("reapplied translation target schema should accept revisioned categories");
}

fn admin() -> SecurityContext {
    SecurityContext::new(UserRole::Admin, Some(Uuid::new_v4()))
}

fn category_input(name: &str, slug: &str) -> CreateCategoryInput {
    CreateCategoryInput {
        locale: "en".to_string(),
        name: name.to_string(),
        slug: Some(slug.to_string()),
        description: Some("Category description".to_string()),
        parent_id: None,
        position: None,
        settings: serde_json::json!({}),
    }
}

fn read_context(tenant_id: Uuid) -> PortContext {
    PortContext::new(
        tenant_id.to_string(),
        PortActor::system(),
        "en",
        "blog-translation-read",
    )
    .with_deadline(Duration::from_secs(5))
}

fn field_patch(
    snapshot: &rustok_translation_targets::TranslationResourceSnapshot,
    key: &str,
    value: &str,
) -> TranslationFieldPatch {
    let field = snapshot
        .fields
        .iter()
        .find(|field| field.descriptor.key.as_str() == key)
        .expect("requested Blog category field should be exposed");
    TranslationFieldPatch {
        key: FieldKey::new(key).expect("static field key should be valid"),
        value: value.to_string(),
        expected_source_hash: field.source_hash.clone(),
    }
}

#[tokio::test]
async fn category_update_advances_exact_locale_and_owner_change_revisions() {
    let (database, service) = setup().await;
    let tenant_id = Uuid::new_v4();
    let category_id = service
        .create(tenant_id, admin(), category_input("Systems", "systems"))
        .await
        .expect("source Blog category should be created");

    let updated = service
        .update(
            tenant_id,
            category_id,
            admin(),
            UpdateCategoryInput {
                locale: "EN".to_string(),
                name: Some("Updated Systems".to_string()),
                slug: None,
                description: None,
                position: None,
                settings: None,
            },
        )
        .await
        .expect("Blog category update should advance revisioned owner state");
    assert_eq!(updated.locale, "en");
    assert_eq!(updated.name, "Updated Systems");
    assert_eq!(updated.slug, "updated-systems");
    assert_eq!(updated.position, 0);

    let provider = BlogCategoryTranslationTargetProvider::new(service);
    let snapshot = provider
        .read_resource(
            read_context(tenant_id),
            ReadTranslationResourceRequest {
                identity: rustok_translation_targets::TranslationResourceIdentity {
                    owner_slug: rustok_translation_targets::OwnerSlug::new("blog")
                        .expect("static owner slug should be valid"),
                    resource_kind: rustok_translation_targets::ResourceKind::new("category")
                        .expect("static resource kind should be valid"),
                    resource_id: rustok_translation_targets::ResourceId::new(
                        category_id.to_string(),
                    )
                    .expect("category UUID should be a valid resource id"),
                    subresource_id: None,
                },
                source_locale: TenantLocale::new("en").expect("source locale should be valid"),
                target_locale: TenantLocale::new("fr").expect("target locale should be valid"),
            },
        )
        .await
        .expect("updated source category should be readable through the owner target");
    assert_eq!(snapshot.summary.resource_revision.as_str(), "2");
    assert_eq!(snapshot.source_revision.as_str(), "2");

    let outbox_events = SysEvents::find()
        .all(&database)
        .await
        .expect("category update should write an outbox event");
    assert_eq!(outbox_events.len(), 1);
    assert_eq!(outbox_events[0].event_type, "index.reindex_requested");
}

#[tokio::test]
async fn translation_target_applies_replays_and_tracks_an_exact_category_locale() {
    let (database, service) = setup().await;
    let tenant_id = Uuid::new_v4();
    let category_id = service
        .create(tenant_id, admin(), category_input("Systems", "systems"))
        .await
        .expect("source Blog category should be created");
    let provider = BlogCategoryTranslationTargetProvider::new(service);
    let read_context = read_context(tenant_id);

    let source_changes = provider
        .read_changes(
            read_context.clone(),
            TranslationTargetChangesRequest {
                after: None,
                limit: 10,
            },
        )
        .await
        .expect("source Blog write should record a translation change");
    assert_eq!(source_changes.changes.len(), 1);
    let source_cursor = source_changes
        .next_cursor
        .expect("source change should return a resume cursor");

    let page = provider
        .list_resources(
            read_context.clone(),
            ListTranslationResourcesRequest {
                source_locale: TenantLocale::new("en").expect("source locale should be valid"),
                target_locale: TenantLocale::new("fr").expect("target locale should be valid"),
                cursor: None,
                limit: 10,
            },
        )
        .await
        .expect("exact source Blog category should be listed");
    assert_eq!(page.resources.len(), 1);
    assert_eq!(
        page.resources[0].identity.resource_id.as_str(),
        category_id.to_string()
    );

    let snapshot = provider
        .read_resource(
            read_context.clone(),
            ReadTranslationResourceRequest {
                identity: page.resources[0].identity.clone(),
                source_locale: TenantLocale::new("en").expect("source locale should be valid"),
                target_locale: TenantLocale::new("fr").expect("target locale should be valid"),
            },
        )
        .await
        .expect("exact source Blog category should be readable");
    assert!(snapshot.target_revision.is_none());
    assert_eq!(snapshot.rendered_fallback_locale, None);

    let patch = TranslationPatchRequest {
        identity: snapshot.summary.identity.clone(),
        source_locale: snapshot.source_locale.clone(),
        target_locale: snapshot.target_locale.clone(),
        expected_resource_revision: snapshot.summary.resource_revision.clone(),
        expected_source_revision: snapshot.source_revision.clone(),
        expected_target_revision: None,
        fields: vec![
            field_patch(&snapshot, "name", "Systemes"),
            field_patch(&snapshot, "slug", "systemes"),
            field_patch(&snapshot, "description", "Sujets logiciels systeme"),
        ],
        proposal_id: "blog-proposal-1".to_string(),
        approval_receipt_id: "blog-approval-1".to_string(),
    };
    assert!(
        provider
            .validate_patch(read_context.clone(), patch.clone())
            .await
            .expect("Blog patch validation should complete")
            .accepted
    );

    let apply_context = PortContext::new(
        tenant_id.to_string(),
        PortActor::system(),
        "en",
        "blog-translation-apply",
    )
    .with_idempotency_key("blog-translation-apply-1")
    .with_deadline(Duration::from_secs(5));
    let receipt = provider
        .apply_patch(apply_context.clone(), patch.clone())
        .await
        .expect("Blog patch should apply");
    assert_eq!(receipt.resource_revision.as_str(), "2");
    assert_eq!(receipt.target_revision.as_str(), "1");
    assert_eq!(
        receipt.applied_field_keys,
        vec![
            FieldKey::new("name").expect("static field key should be valid"),
            FieldKey::new("slug").expect("static field key should be valid"),
            FieldKey::new("description").expect("static field key should be valid"),
        ]
    );
    let replay = provider
        .apply_patch(apply_context.clone(), patch.clone())
        .await
        .expect("same Blog idempotency request should replay");
    assert_eq!(replay, receipt);

    let mut conflicting_patch = patch.clone();
    conflicting_patch.proposal_id = "blog-proposal-2".to_string();
    let conflict = provider
        .apply_patch(apply_context, conflicting_patch)
        .await
        .expect_err("same Blog idempotency key must reject a changed request");
    assert_eq!(conflict.kind, PortErrorKind::Conflict);
    assert_eq!(conflict.code, "outbox.operation_receipt_conflict");

    assert!(
        !provider
            .validate_patch(read_context.clone(), patch)
            .await
            .expect("stale Blog patch validation should complete")
            .accepted
    );

    let updated = provider
        .read_resource(
            read_context.clone(),
            ReadTranslationResourceRequest {
                identity: snapshot.summary.identity,
                source_locale: TenantLocale::new("en").expect("source locale should be valid"),
                target_locale: TenantLocale::new("fr").expect("target locale should be valid"),
            },
        )
        .await
        .expect("applied exact target should be readable");
    assert_eq!(
        updated
            .target_revision
            .as_ref()
            .map(|revision| revision.as_str()),
        Some("1")
    );
    for (key, expected) in [
        ("name", "Systemes"),
        ("slug", "systemes"),
        ("description", "Sujets logiciels systeme"),
    ] {
        assert_eq!(
            updated
                .fields
                .iter()
                .find(|field| field.descriptor.key.as_str() == key)
                .and_then(|field| field.exact_target_value.as_deref()),
            Some(expected)
        );
    }

    let applied_changes = provider
        .read_changes(
            read_context.clone(),
            TranslationTargetChangesRequest {
                after: Some(source_cursor),
                limit: 10,
            },
        )
        .await
        .expect("applied Blog patch should record a change");
    assert_eq!(applied_changes.changes.len(), 1);
    assert_eq!(
        applied_changes.changes[0].identity,
        page.resources[0].identity
    );
    assert_eq!(
        applied_changes.changes[0].resource_revision,
        receipt.resource_revision
    );

    let outbox_events = SysEvents::find()
        .all(&database)
        .await
        .expect("Blog translation apply should write an outbox event");
    assert_eq!(outbox_events.len(), 1);
    assert_eq!(outbox_events[0].event_type, "index.reindex_requested");

    let progress = provider
        .read_progress(
            read_context,
            TranslationTargetProgressRequest {
                source_locale: TenantLocale::new("en").expect("source locale should be valid"),
                target_locale: TenantLocale::new("fr").expect("target locale should be valid"),
            },
        )
        .await
        .expect("Blog progress should use exact target values");
    assert_eq!(progress.resources, 1);
    assert_eq!(progress.required_units, 2);
    assert_eq!(progress.exact_required_units, 2);
    assert_eq!(progress.optional_units, 1);
    assert_eq!(progress.exact_optional_units, 1);
    assert_eq!(progress.complete_resources, 1);
    assert!(progress.owner_change_cursor.is_some());

    let unauthorized = provider
        .read_resource(
            PortContext::new(
                tenant_id.to_string(),
                PortActor::user(Uuid::new_v4().to_string()),
                "en",
                "blog-translation-forbidden",
            )
            .with_deadline(Duration::from_secs(5)),
            ReadTranslationResourceRequest {
                identity: page.resources[0].identity.clone(),
                source_locale: TenantLocale::new("en").expect("source locale should be valid"),
                target_locale: TenantLocale::new("fr").expect("target locale should be valid"),
            },
        )
        .await
        .expect_err("unprivileged user should not read the Blog target");
    assert_eq!(unauthorized.kind, PortErrorKind::Forbidden);
}
