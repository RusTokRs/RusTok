use std::{sync::Arc, time::Duration};

use async_trait::async_trait;
use rustok_api::{PortActor, PortContext, PortErrorKind, TenantLocale};
use rustok_blog::{
    BlogCategoryDeleteCleanupPort, BlogError, BlogModule, CategoryService, CreateCategoryInput,
    CreateTagInput, ListTagsFilter, TagService, UpdateCategoryInput, UpdateTagInput,
};
use rustok_core::{MigrationSource, SecurityContext, UserRole};
use rustok_outbox::{OutboxTransport, SysEventsMigration, TransactionalEventBus};
use rustok_taxonomy::{
    ModuleTermCreateInput, TaxonomyModule, TaxonomyModuleTermTranslationOwnerRegistry,
    TaxonomyService, TaxonomyTermKind, TaxonomyTranslationTargetProvider,
};
use rustok_translation_targets::{
    ListTranslationResourcesRequest, OwnerSlug, ReadTranslationResourceRequest, ResourceId,
    ResourceKind, TranslationFieldPatch, TranslationPatchRequest, TranslationResourceIdentity,
    TranslationResourceSnapshot, TranslationTargetProvider,
};
use sea_orm::{
    ConnectOptions, ConnectionTrait, Database, DatabaseConnection, DatabaseTransaction, DbBackend,
    Statement, TransactionTrait,
};
use sea_orm_migration::{MigrationTrait, SchemaManager};
use uuid::Uuid;

use super::{in_process_blog_taxonomy_port, register_blog_translation_owner};

async fn setup() -> DatabaseConnection {
    let mut options = ConnectOptions::new("sqlite::memory:");
    options.max_connections(1).sqlx_logging(false);
    let db = Database::connect(options).await.unwrap();
    db.execute_unprepared(
        "CREATE TABLE users (id TEXT NOT NULL PRIMARY KEY, tenant_id TEXT NOT NULL, UNIQUE (tenant_id, id))",
    )
    .await
    .unwrap();
    let manager = SchemaManager::new(&db);
    SysEventsMigration.up(&manager).await.unwrap();
    for migration in TaxonomyModule
        .migrations()
        .into_iter()
        .chain(BlogModule.migrations())
    {
        migration.up(&manager).await.unwrap();
    }
    db
}

fn admin() -> SecurityContext {
    SecurityContext::new(UserRole::Admin, Some(Uuid::new_v4()))
}

fn translation_context(tenant_id: Uuid, grants: &[&str]) -> PortContext {
    let context = PortContext::new(
        tenant_id.to_string(),
        PortActor::user(Uuid::new_v4().to_string()),
        "en",
        "blog-translation-test",
    )
    .with_role("admin")
    .with_deadline(Duration::from_secs(5));
    grants
        .iter()
        .fold(context, |context, grant| context.with_claim(*grant))
}

fn translation_read(term_id: Uuid, target: &str) -> ReadTranslationResourceRequest {
    ReadTranslationResourceRequest {
        identity: TranslationResourceIdentity {
            owner_slug: OwnerSlug::new("taxonomy").unwrap(),
            resource_kind: ResourceKind::new("term").unwrap(),
            resource_id: ResourceId::new(term_id.to_string()).unwrap(),
            subresource_id: None,
        },
        source_locale: TenantLocale::new("en").unwrap(),
        target_locale: TenantLocale::new(target).unwrap(),
    }
}

fn translation_patch(snapshot: &TranslationResourceSnapshot) -> TranslationPatchRequest {
    TranslationPatchRequest {
        identity: snapshot.summary.identity.clone(),
        source_locale: snapshot.source_locale.clone(),
        target_locale: snapshot.target_locale.clone(),
        expected_resource_revision: snapshot.summary.resource_revision.clone(),
        expected_source_revision: snapshot.source_revision.clone(),
        expected_target_revision: snapshot.target_revision.clone(),
        fields: snapshot
            .fields
            .iter()
            .filter_map(|field| {
                let value = match field.descriptor.key.as_str() {
                    "name" => "Nachrichten",
                    "slug" => "nachrichten",
                    _ => return None,
                };
                Some(TranslationFieldPatch {
                    key: field.descriptor.key.clone(),
                    value: value.into(),
                    expected_source_hash: field.source_hash.clone(),
                })
            })
            .collect(),
        proposal_id: "blog-translation".into(),
        approval_receipt_id: "approved-blog-translation".into(),
    }
}

async fn category_revision_and_events(db: &DatabaseConnection, category: Uuid) -> (i64, i64) {
    let row = db.query_one_raw(Statement::from_sql_and_values(
        DbBackend::Sqlite,
        "SELECT revision, (SELECT COUNT(*) FROM sys_events) AS events FROM blog_categories WHERE id = ?",
        [category.into()],
    )).await.unwrap().unwrap();
    (
        row.try_get("", "revision").unwrap(),
        row.try_get("", "events").unwrap(),
    )
}

#[tokio::test]
async fn blog_translation_uses_owner_permissions_limits_and_atomic_effects() {
    let db = setup().await;
    let tenant_id = Uuid::new_v4();
    db.execute_unprepared(
        "CREATE TABLE tenant_modules (tenant_id TEXT NOT NULL, module_slug TEXT NOT NULL, enabled BOOLEAN NOT NULL)",
    ).await.unwrap();
    db.execute_raw(Statement::from_sql_and_values(
        DbBackend::Sqlite,
        "INSERT INTO tenant_modules (tenant_id, module_slug, enabled) VALUES (?, 'blog', 1)",
        [tenant_id.into()],
    ))
    .await
    .unwrap();
    let port = in_process_blog_taxonomy_port(db.clone());
    let bus = TransactionalEventBus::new(Arc::new(OutboxTransport::new(db.clone())));
    let categories = CategoryService::new(db.clone(), bus).with_taxonomy_port(port.clone());
    let category_id = categories
        .create(
            tenant_id,
            admin(),
            CreateCategoryInput {
                locale: "en".into(),
                name: "News".into(),
                slug: None,
                description: None,
                parent_id: None,
                position: None,
                settings: serde_json::json!({"visible": true}),
            },
        )
        .await
        .unwrap();
    let tags = TagService::new(db.clone()).with_taxonomy_port(port);
    let tag_id = tags
        .create_tag(
            tenant_id,
            admin(),
            CreateTagInput {
                locale: "en".into(),
                name: "News tag".into(),
                slug: None,
            },
        )
        .await
        .unwrap();
    let mut registry = TaxonomyModuleTermTranslationOwnerRegistry::default();
    register_blog_translation_owner(&mut registry, db.clone()).unwrap();
    let provider = TaxonomyTranslationTargetProvider::with_owner_registry(
        Arc::new(TaxonomyService::new(db.clone())),
        registry,
    );
    let category_reader =
        translation_context(tenant_id, &["taxonomy:read", "blog_categories:read"]);
    let page = provider
        .list_resources(
            category_reader.clone(),
            ListTranslationResourcesRequest {
                source_locale: TenantLocale::new("en").unwrap(),
                target_locale: TenantLocale::new("de").unwrap(),
                cursor: None,
                limit: 10,
            },
        )
        .await
        .unwrap();
    assert_eq!(page.resources.len(), 1);
    assert_eq!(
        page.resources[0].identity.resource_id.as_str(),
        category_id.to_string()
    );
    let tag_reader = translation_context(tenant_id, &["taxonomy:read", "tags:read"]);
    let tag = provider
        .read_resource(tag_reader, translation_read(tag_id, "de"))
        .await
        .unwrap();
    assert_eq!(tag.fields.len(), 2);
    assert!(
        tag.fields
            .iter()
            .all(|field| field.descriptor.max_characters == Some(100))
    );
    assert_eq!(
        provider
            .read_resource(category_reader.clone(), translation_read(tag_id, "de"))
            .await
            .unwrap_err()
            .kind,
        PortErrorKind::Forbidden
    );
    let tag_editor = translation_context(tenant_id, &["taxonomy:update", "tags:update"])
        .with_idempotency_key("blog-tag-translation");
    let tag_patch = translation_patch(&tag);
    let mut invalid_tag_patch = tag_patch.clone();
    invalid_tag_patch
        .fields
        .iter_mut()
        .find(|field| field.key.as_str() == "name")
        .unwrap()
        .value = "x".repeat(101);
    assert!(
        !provider
            .validate_patch(tag_editor.clone(), invalid_tag_patch)
            .await
            .unwrap()
            .accepted
    );
    provider.apply_patch(tag_editor, tag_patch).await.unwrap();
    let translated_tag = tags
        .get_tag(tenant_id, admin(), tag_id, "de")
        .await
        .unwrap();
    assert_eq!(translated_tag.name, "Nachrichten");
    assert_eq!(translated_tag.effective_locale, "de");
    let tag_read = translation_context(tenant_id, &["taxonomy:read", "tags:read"]);
    let installed_tag = provider
        .read_resource(tag_read, translation_read(tag_id, "de"))
        .await
        .unwrap();
    let mut rename = translation_patch(&installed_tag);
    rename
        .fields
        .iter_mut()
        .find(|field| field.key.as_str() == "slug")
        .unwrap()
        .value = "aktuelle-nachrichten".into();
    provider
        .apply_patch(
            translation_context(tenant_id, &["taxonomy:update", "tags:update"])
                .with_idempotency_key("blog-tag-translation-rename"),
            rename,
        )
        .await
        .unwrap();
    let old_route = TaxonomyService::new(db.clone())
        .resolve_term_route_for_module(
            tenant_id,
            TaxonomyTermKind::Tag,
            "blog",
            "de",
            None,
            "nachrichten",
        )
        .await
        .unwrap()
        .unwrap();
    assert_eq!(old_route.term_id, tag_id);
    assert!(old_route.alias_id.is_some());

    let category = provider
        .read_resource(category_reader.clone(), translation_read(category_id, "de"))
        .await
        .unwrap();
    let description = category
        .fields
        .iter()
        .find(|field| field.descriptor.key.as_str() == "description")
        .unwrap();
    assert_eq!(description.descriptor.max_characters, Some(1_000));
    let patch = translation_patch(&category);
    let mut long_description = patch.clone();
    long_description.fields.push(TranslationFieldPatch {
        key: description.descriptor.key.clone(),
        value: "x".repeat(1_001),
        expected_source_hash: description.source_hash.clone(),
    });
    let editor = translation_context(tenant_id, &["taxonomy:update", "blog_categories:update"])
        .with_idempotency_key("blog-category-translation");
    let validation = provider
        .validate_patch(editor.clone(), long_description)
        .await
        .unwrap();
    assert!(!validation.accepted);
    assert!(
        validation
            .issues
            .iter()
            .any(|issue| issue.code == "field_too_long")
    );
    assert_eq!(
        provider
            .apply_patch(
                translation_context(tenant_id, &["taxonomy:update"])
                    .with_idempotency_key("blog-category-translation"),
                patch.clone(),
            )
            .await
            .unwrap_err()
            .kind,
        PortErrorKind::Forbidden,
    );
    let (revision, events) = category_revision_and_events(&db, category_id).await;
    let receipt = provider
        .apply_patch(editor.clone(), patch.clone())
        .await
        .unwrap();
    assert_eq!(provider.apply_patch(editor, patch).await.unwrap(), receipt);
    assert_eq!(
        category_revision_and_events(&db, category_id).await,
        (revision + 1, events + 1)
    );
    let translated = categories
        .get(tenant_id, admin(), category_id, "de")
        .await
        .unwrap();
    assert_eq!(translated.name, "Nachrichten");
    assert_eq!(translated.settings, serde_json::json!({"visible": true}));

    let request = translation_read(category_id, "fr");
    let next = provider
        .read_resource(category_reader.clone(), request.clone())
        .await
        .unwrap();
    db.execute_unprepared(
        "CREATE TRIGGER fail_blog_reindex BEFORE INSERT ON sys_events
        BEGIN SELECT RAISE(ABORT, 'fixture rejected event'); END",
    )
    .await
    .unwrap();
    let failing_editor =
        translation_context(tenant_id, &["taxonomy:update", "blog_categories:update"])
            .with_idempotency_key("rollback-blog-translation");
    assert!(
        provider
            .apply_patch(failing_editor, translation_patch(&next))
            .await
            .is_err()
    );
    assert_eq!(
        provider
            .read_resource(category_reader.clone(), request.clone())
            .await
            .unwrap(),
        next
    );
    assert_eq!(
        category_revision_and_events(&db, category_id).await,
        (revision + 1, events + 1)
    );
    db.execute_unprepared("UPDATE tenant_modules SET enabled = 0")
        .await
        .unwrap();
    assert_eq!(
        provider
            .read_resource(category_reader, request)
            .await
            .unwrap_err()
            .kind,
        PortErrorKind::Forbidden
    );
}

struct RejectCleanup;

#[async_trait]
impl BlogCategoryDeleteCleanupPort for RejectCleanup {
    async fn cleanup_in_tx(
        &self,
        _txn: &DatabaseTransaction,
        _tenant_id: Uuid,
        _category_id: Uuid,
    ) -> Result<(), BlogError> {
        Err(BlogError::conflict("attached data refused cleanup"))
    }
}

#[tokio::test]
async fn category_copy_and_delete_use_taxonomy_and_preserve_typed_cleanup_failure() {
    let db = setup().await;
    let tenant_id = Uuid::new_v4();
    let port = in_process_blog_taxonomy_port(db.clone());
    let bus = TransactionalEventBus::new(Arc::new(OutboxTransport::new(db.clone())));
    let service = CategoryService::new(db.clone(), bus)
        .with_taxonomy_port(port.clone())
        .with_category_delete_cleanup(Arc::new(RejectCleanup));
    let category_id = service
        .create(
            tenant_id,
            admin(),
            CreateCategoryInput {
                locale: "en".into(),
                name: "World".into(),
                slug: Some("world".into()),
                description: None,
                parent_id: None,
                position: Some(0),
                settings: serde_json::json!({}),
            },
        )
        .await
        .unwrap();
    let category = service
        .update(
            tenant_id,
            category_id,
            admin(),
            UpdateCategoryInput {
                locale: "en".into(),
                name: Some("News".into()),
                slug: Some("news".into()),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    assert_eq!(category.slug, "news");
    let taxonomy = TaxonomyService::new(db.clone());
    let historical = taxonomy
        .resolve_term_route_for_module(
            tenant_id,
            TaxonomyTermKind::Category,
            "blog",
            "en",
            None,
            "world",
        )
        .await
        .unwrap()
        .unwrap();
    assert_eq!(historical.term_id, category_id);
    assert!(historical.alias_id.is_some());

    let error = service
        .delete(tenant_id, category_id, admin())
        .await
        .unwrap_err();
    assert!(
        matches!(error, BlogError::Conflict(message) if message == "attached data refused cleanup")
    );
    let category = service
        .get(tenant_id, admin(), category_id, "en")
        .await
        .unwrap();
    assert_eq!(category.slug, "news");

    let service = service
        .with_category_delete_cleanup(Arc::new(rustok_blog::NoopBlogCategoryDeleteCleanupPort));
    service
        .delete(tenant_id, category_id, admin())
        .await
        .unwrap();
    let result = taxonomy
        .resolve_term_route_for_module(
            tenant_id,
            TaxonomyTermKind::Category,
            "blog",
            "en",
            None,
            "world",
        )
        .await
        .unwrap();
    assert!(result.is_none());
    assert!(matches!(
        service.get(tenant_id, admin(), category_id, "en").await,
        Err(BlogError::CategoryNotFound(_))
    ));
}

#[tokio::test]
async fn tag_mutations_use_owner_scope_and_caller_transaction() {
    let db = setup().await;
    let tenant_id = Uuid::new_v4();
    let port = in_process_blog_taxonomy_port(db.clone());
    let tags = TagService::new(db.clone()).with_taxonomy_port(port.clone());
    let id = tags
        .create_tag(
            tenant_id,
            admin(),
            CreateTagInput {
                locale: "en".into(),
                name: "Notes".into(),
                slug: None,
            },
        )
        .await
        .unwrap();
    let taxonomy = TaxonomyService::new(db.clone());
    let txn = db.begin().await.unwrap();
    let other_term = taxonomy
        .create_module_term_in_tx(
            &txn,
            tenant_id,
            TaxonomyTermKind::Tag,
            "product",
            ModuleTermCreateInput {
                locale: "en".into(),
                name: "Different".into(),
                slug: None,
                canonical_key: None,
            },
        )
        .await
        .unwrap();
    assert_eq!(
        port.find_all_valid_tag_ids((&txn).into(), tenant_id)
            .await
            .unwrap(),
        [id].into_iter().collect()
    );
    assert!(
        !port
            .load_tags_by_ids((&txn).into(), tenant_id, &[other_term], "en", None)
            .await
            .unwrap()
            .contains_key(&other_term)
    );
    assert!(
        port.find_all_valid_tag_ids((&txn).into(), Uuid::new_v4())
            .await
            .unwrap()
            .is_empty()
    );
    assert!(matches!(
        port.ensure_module_tags_in_tx(&txn, tenant_id, "en", &["Absent".into()], false)
            .await,
        Err(BlogError::Forbidden(_))
    ));
    let created = port
        .ensure_module_tags_in_tx(&txn, tenant_id, "en", &["Temporary".into()], true)
        .await
        .unwrap()[0];
    assert_eq!(
        port.resolve_tag_id_for_module((&txn).into(), tenant_id, "temporary", "en", None)
            .await
            .unwrap(),
        Some(created)
    );
    txn.rollback().await.unwrap();

    let updated = tags
        .update_tag(
            tenant_id,
            id,
            admin(),
            UpdateTagInput {
                locale: "en".into(),
                name: Some("Field notes".into()),
                slug: None,
            },
        )
        .await
        .unwrap();
    assert_eq!(updated.slug, "field-notes");
    let (items, count) = tags
        .list_tags(
            tenant_id,
            admin(),
            ListTagsFilter {
                locale: Some("en".into()),
                page: 1,
                per_page: 10,
            },
        )
        .await
        .unwrap();
    assert_eq!(count, 1);
    assert_eq!(items[0].id, id);
    tags.delete_tag(tenant_id, id, admin()).await.unwrap();
    assert!(
        port.find_all_valid_tag_ids((&db).into(), tenant_id)
            .await
            .unwrap()
            .is_empty()
    );
}
