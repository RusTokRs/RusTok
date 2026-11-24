use std::{
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};

use async_trait::async_trait;
use rustok_api::{PortActor, PortContext, PortError, PortErrorKind, TenantLocale};
use rustok_core::{MigrationSource, SecurityContext, UserRole};
use rustok_outbox::SysEventsMigration;
use rustok_test_utils::db::setup_test_db;
use rustok_translation_targets::{
    FieldKey, ListTranslationResourcesRequest, OpaqueCursor, ReadTranslationResourceRequest,
    TranslationFieldPatch, TranslationPatchRequest, TranslationResourceLifecycle,
    TranslationTargetChangesRequest, TranslationTargetProgressRequest, TranslationTargetProvider,
};
use sea_orm::{
    ConnectionTrait, DatabaseConnection, DatabaseTransaction, DbBackend, EntityTrait, Statement,
    TransactionTrait,
};
use sea_orm_migration::{MigrationTrait, SchemaManager};
use uuid::Uuid;

use crate::{
    CreateTaxonomyTermInput, ModuleTermCreateInput, TaxonomyModule,
    TaxonomyModuleTermTranslationOwner, TaxonomyModuleTermTranslationOwnerRegistry,
    TaxonomyScopeType, TaxonomyService, TaxonomyTermKind, TaxonomyTranslationTargetProvider,
    UpdateTaxonomyTermInput,
};

#[derive(Clone)]
struct TestModuleTranslationOwner {
    authorize_calls: Arc<AtomicUsize>,
    apply_calls: Arc<AtomicUsize>,
}

#[async_trait]
impl TaxonomyModuleTermTranslationOwner for TestModuleTranslationOwner {
    fn module_slug(&self) -> &str {
        "blog"
    }

    fn authorize(
        &self,
        _context: &PortContext,
        _tenant_id: uuid::Uuid,
        _kind: TaxonomyTermKind,
        _term_id: uuid::Uuid,
        _action: rustok_api::Action,
    ) -> Result<(), PortError> {
        self.authorize_calls.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }

    async fn on_translation_applied_in_tx(
        &self,
        _transaction: &DatabaseTransaction,
        _context: &PortContext,
        _tenant_id: uuid::Uuid,
        _kind: TaxonomyTermKind,
        _term_id: uuid::Uuid,
    ) -> Result<(), PortError> {
        self.apply_calls.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
}

async fn setup() -> (DatabaseConnection, Arc<TaxonomyService>) {
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
    let service = Arc::new(TaxonomyService::new(database.clone()));
    (database, service)
}

#[tokio::test]
async fn journal_migration_preserves_history_and_replays_legacy_checkpoints() {
    let database = setup_test_db().await;
    let manager = SchemaManager::new(&database);
    let mut migrations = TaxonomyModule.migrations();
    let journal_migration = migrations
        .pop()
        .expect("Taxonomy journal ordering migration should be registered");

    for migration in migrations {
        migration
            .up(&manager)
            .await
            .expect("base Taxonomy migration should apply");
    }
    let tenant_id = Uuid::new_v4();
    let term_id = Uuid::new_v4();
    let legacy_checkpoint = Uuid::from_u128(2);
    // Old IDs sort the deletion before creation, even with the same timestamp.
    for (id, revision, operation, lifecycle) in [
        (legacy_checkpoint, 1_i64, "upsert", "active"),
        (Uuid::from_u128(1), 2_i64, "delete", "deleted"),
    ] {
        database
            .execute_raw(Statement::from_sql_and_values(
                DbBackend::Sqlite,
                "INSERT INTO taxonomy_translation_changes
                    (id, tenant_id, term_id, locale, resource_revision, target_revision,
                     operation, lifecycle, created_at)
                 VALUES (?, ?, ?, 'en', ?, 1, ?, ?, '2026-11-24T00:00:00Z')",
                [
                    id.into(),
                    tenant_id.into(),
                    term_id.into(),
                    revision.into(),
                    operation.into(),
                    lifecycle.into(),
                ],
            ))
            .await
            .unwrap();
    }
    journal_migration
        .up(&manager)
        .await
        .expect("Taxonomy journal ordering migration should apply");
    journal_migration
        .down(&manager)
        .await
        .expect("Taxonomy journal ordering migration should roll back");
    journal_migration
        .up(&manager)
        .await
        .expect("Taxonomy journal ordering migration should reapply");

    let service = Arc::new(TaxonomyService::new(database));
    service
        .create_term(
            tenant_id,
            admin(),
            CreateTaxonomyTermInput {
                kind: TaxonomyTermKind::Tag,
                scope_type: TaxonomyScopeType::Global,
                scope_value: None,
                locale: "en".to_string(),
                name: "Migration proof".to_string(),
                slug: Some("migration-proof".to_string()),
                canonical_key: None,
                description: None,
                aliases: Vec::new(),
            },
        )
        .await
        .expect("reapplied translation target schema should accept revisioned terms");

    let provider = TaxonomyTranslationTargetProvider::new(service);
    let page = provider
        .read_changes(
            read_context(tenant_id),
            TranslationTargetChangesRequest {
                after: Some(OpaqueCursor::new(legacy_checkpoint.to_string()).unwrap()),
                limit: 3,
            },
        )
        .await
        .unwrap();
    assert_eq!(page.changes.len(), 3);
    assert_eq!(
        page.changes[0].identity.resource_id.as_str(),
        term_id.to_string()
    );
    assert_eq!(
        page.changes[0].lifecycle,
        TranslationResourceLifecycle::Active
    );
    assert_eq!(
        page.changes[1].identity.resource_id,
        page.changes[0].identity.resource_id
    );
    assert_eq!(
        page.changes[1].lifecycle,
        TranslationResourceLifecycle::Deleted
    );
    assert_eq!(page.next_cursor.unwrap().as_str(), "position:3");
}

fn admin() -> SecurityContext {
    SecurityContext::new(UserRole::Admin, Some(Uuid::new_v4()))
}

fn read_context(tenant_id: Uuid) -> PortContext {
    PortContext::new(
        tenant_id.to_string(),
        PortActor::system(),
        "en",
        "taxonomy-translation-read",
    )
    .with_deadline(Duration::from_secs(5))
}

#[tokio::test]
async fn inventory_requires_maintenance_grants_and_preserves_owner_scope_and_tombstones() {
    let (database, service) = setup().await;
    let tenant_id = Uuid::new_v4();
    let other_tenant_id = Uuid::new_v4();
    let txn = database.begin().await.unwrap();
    let mut ids = Vec::new();
    for (tenant, name) in [
        (tenant_id, "Blog Tag"),
        (tenant_id, "Deleted Tag"),
        (other_tenant_id, "Another Tenant"),
    ] {
        ids.push(
            service
                .create_module_term_in_tx(
                    &txn,
                    tenant,
                    TaxonomyTermKind::Tag,
                    "blog",
                    ModuleTermCreateInput {
                        locale: "de".into(),
                        name: name.into(),
                        slug: None,
                        canonical_key: None,
                    },
                )
                .await
                .unwrap(),
        );
    }
    crate::delete_module_term_in_tx(
        &txn,
        tenant_id,
        ids[1],
        &admin(),
        TaxonomyTermKind::Tag,
        "blog",
    )
    .await
    .unwrap();
    txn.commit().await.unwrap();

    let provider = TaxonomyTranslationTargetProvider::new(service);
    let changes_request = TranslationTargetChangesRequest {
        after: None,
        limit: 10,
    };
    let baseline_request = ListTranslationResourcesRequest {
        source_locale: TenantLocale::new("en").unwrap(),
        target_locale: TenantLocale::new("de").unwrap(),
        cursor: None,
        limit: 10,
    };
    let admin = PortContext::new(
        tenant_id.to_string(),
        PortActor::user(Uuid::new_v4().to_string()),
        "en",
        "taxonomy-inventory",
    )
    .with_role("admin")
    .with_deadline(Duration::from_secs(5));
    for insufficient in [
        admin
            .clone()
            .with_claim("taxonomy:read")
            .with_claim("translations:manage"),
        admin
            .clone()
            .with_claim("taxonomy:manage")
            .with_claim("translations:update"),
    ] {
        let error = provider
            .read_changes(insufficient.clone(), changes_request.clone())
            .await
            .unwrap_err();
        assert_eq!(error.kind, PortErrorKind::Forbidden);
        let error = provider
            .list_inventory_resources(insufficient, baseline_request.clone())
            .await
            .unwrap_err();
        assert_eq!(error.kind, PortErrorKind::Forbidden);
    }

    let maintainer = admin
        .with_claim("taxonomy:manage")
        .with_claim("translations:manage");
    let changes = provider
        .read_changes(maintainer.clone(), changes_request)
        .await
        .unwrap();
    assert_eq!(changes.changes.len(), 3);
    let deleted = &changes.changes[2];
    assert_eq!(deleted.lifecycle, TranslationResourceLifecycle::Deleted);
    assert_eq!(deleted.identity.resource_id.as_str(), ids[1].to_string());
    assert_eq!(changes.next_cursor.unwrap().as_str(), "position:3");
    let baseline = provider
        .list_inventory_resources(maintainer.clone(), baseline_request.clone())
        .await
        .unwrap();
    assert_eq!(baseline.changes.len(), 1);
    assert_eq!(
        baseline.changes[0].identity.resource_id.as_str(),
        ids[0].to_string()
    );
    let discovery = provider
        .list_resources(
            maintainer,
            ListTranslationResourcesRequest {
                source_locale: baseline_request.target_locale,
                target_locale: baseline_request.source_locale,
                ..baseline_request
            },
        )
        .await
        .unwrap();
    assert!(discovery.resources.is_empty());
}

#[tokio::test]
async fn resource_pagination_does_not_expose_a_hidden_term_in_the_cursor() {
    let (database, service) = setup().await;
    let tenant_id = Uuid::new_v4();
    let txn = database.begin().await.unwrap();
    let ids = [Uuid::from_u128(1), Uuid::from_u128(2), Uuid::from_u128(3)];
    for (id, scope, name) in [
        (ids[0], "forum", "Hidden"),
        (ids[1], "blog", "Visible"),
        (ids[2], "blog", "More"),
    ] {
        crate::sync_module_category_copy_in_tx(
            &txn,
            tenant_id,
            crate::SyncModuleCategoryInput {
                category_id: id,
                module_scope: scope.into(),
                canonical_key: name.into(),
                locale: "en".into(),
                name: name.into(),
                slug: name.into(),
                description: None,
                parent_id: None,
                position: 0,
                aliases: Vec::new(),
                icon_key: None,
                color: None,
            },
        )
        .await
        .unwrap();
    }
    txn.commit().await.unwrap();
    let mut owners = TaxonomyModuleTermTranslationOwnerRegistry::default();
    owners
        .register(TestModuleTranslationOwner {
            authorize_calls: Arc::new(AtomicUsize::new(0)),
            apply_calls: Arc::new(AtomicUsize::new(0)),
        })
        .unwrap();
    let provider = TaxonomyTranslationTargetProvider::with_owner_registry(service, owners);
    let request = ListTranslationResourcesRequest {
        source_locale: TenantLocale::new("en").unwrap(),
        target_locale: TenantLocale::new("de").unwrap(),
        cursor: None,
        limit: 1,
    };
    let first_page = provider
        .list_resources(read_context(tenant_id), request.clone())
        .await
        .unwrap();
    assert_eq!(first_page.resources.len(), 1);
    let cursor = first_page.next_cursor.unwrap();
    assert_eq!(cursor.as_str(), ids[1].to_string());
    assert_eq!(
        first_page.resources[0].identity.resource_id.as_str(),
        cursor.as_str()
    );
    let second_page = provider
        .list_resources(
            read_context(tenant_id),
            ListTranslationResourcesRequest {
                cursor: Some(cursor),
                ..request
            },
        )
        .await
        .unwrap();
    assert_eq!(second_page.resources.len(), 1);
    assert_eq!(
        second_page.resources[0].identity.resource_id.as_str(),
        ids[2].to_string()
    );
    assert!(second_page.next_cursor.is_none());
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
        .expect("requested Taxonomy field should be exposed");
    TranslationFieldPatch {
        key: FieldKey::new(key)
            .expect("static field key should be valid: invariant validated at startup"),
        value: value.to_string(),
        expected_source_hash: field.source_hash.clone(),
    }
}

#[tokio::test]
async fn translation_target_applies_replays_and_tracks_an_exact_term_locale() {
    let (database, service) = setup().await;
    let tenant_id = Uuid::new_v4();
    let term_id = service
        .create_term(
            tenant_id,
            admin(),
            CreateTaxonomyTermInput {
                kind: TaxonomyTermKind::Tag,
                scope_type: TaxonomyScopeType::Global,
                scope_value: None,
                locale: "en".to_string(),
                name: "Systems".to_string(),
                slug: Some("systems".to_string()),
                canonical_key: None,
                description: Some("System software topics".to_string()),
                aliases: Vec::new(),
            },
        )
        .await
        .expect("source taxonomy term should be created");
    let provider = TaxonomyTranslationTargetProvider::new(service);
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
        .expect("source Taxonomy write should record a translation change");
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
        .expect("exact source taxonomy term should be listed");
    assert_eq!(page.resources.len(), 1);
    assert_eq!(
        page.resources[0].identity.resource_id.as_str(),
        term_id.to_string()
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
        .expect("exact source taxonomy term should be readable");
    assert!(snapshot.target_revision.is_none());

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
        proposal_id: "taxonomy-proposal-1".to_string(),
        approval_receipt_id: "taxonomy-approval-1".to_string(),
    };
    assert!(
        provider
            .validate_patch(read_context.clone(), patch.clone())
            .await
            .expect("Taxonomy patch validation should complete")
            .accepted
    );

    let apply_context = PortContext::new(
        tenant_id.to_string(),
        PortActor::system(),
        "en",
        "taxonomy-translation-apply",
    )
    .with_idempotency_key("taxonomy-translation-apply-1")
    .with_deadline(Duration::from_secs(5));
    let receipt = provider
        .apply_patch(apply_context.clone(), patch.clone())
        .await
        .expect("Taxonomy patch should apply");
    assert_eq!(receipt.resource_revision.as_str(), "2");
    assert_eq!(receipt.target_revision.as_str(), "1");
    assert_eq!(
        receipt.applied_field_keys,
        vec![
            FieldKey::new("name")
                .expect("static field key should be valid: invariant validated at startup"),
            FieldKey::new("slug")
                .expect("static field key should be valid: invariant validated at startup"),
            FieldKey::new("description")
                .expect("static field key should be valid: invariant validated at startup"),
        ]
    );
    let replay = provider
        .apply_patch(apply_context.clone(), patch.clone())
        .await
        .expect("same Taxonomy idempotency request should replay");
    assert_eq!(replay, receipt);

    let outbox_events = rustok_outbox::SysEvents::find()
        .all(&database)
        .await
        .expect("global Taxonomy Tag translation should enqueue a Search reindex event");
    assert_eq!(outbox_events.len(), 1);
    assert_eq!(outbox_events[0].event_type, "index.reindex_requested");
    assert_eq!(
        outbox_events[0].payload["event"]["data"]["target_type"],
        "search"
    );
    assert!(outbox_events[0].payload["event"]["data"]["target_id"].is_null());

    let mut conflicting_patch = patch;
    conflicting_patch.proposal_id = "taxonomy-proposal-2".to_string();
    let conflict = provider
        .apply_patch(apply_context, conflicting_patch)
        .await
        .expect_err("same Taxonomy idempotency key must reject a changed request");
    assert_eq!(conflict.kind, PortErrorKind::Conflict);
    assert_eq!(conflict.code, "outbox.operation_receipt_conflict");

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
        .expect("applied Taxonomy patch should record a change");
    assert_eq!(applied_changes.changes.len(), 1);
    assert_eq!(
        applied_changes.changes[0].identity,
        page.resources[0].identity
    );
    assert_eq!(
        applied_changes.changes[0].resource_revision,
        receipt.resource_revision
    );

    let progress = provider
        .read_progress(
            read_context,
            TranslationTargetProgressRequest {
                source_locale: TenantLocale::new("en").expect("source locale should be valid"),
                target_locale: TenantLocale::new("fr").expect("target locale should be valid"),
            },
        )
        .await
        .expect("Taxonomy progress should use exact target values");
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
                "taxonomy-translation-forbidden",
            )
            .with_deadline(Duration::from_secs(5)),
            ReadTranslationResourceRequest {
                identity: page.resources[0].identity.clone(),
                source_locale: TenantLocale::new("en").expect("source locale should be valid"),
                target_locale: TenantLocale::new("fr").expect("target locale should be valid"),
            },
        )
        .await
        .expect_err("unprivileged user should not read the Taxonomy target");
    assert_eq!(unauthorized.kind, PortErrorKind::Forbidden);
}

#[tokio::test]
async fn global_tag_update_and_delete_enqueue_search_reindex() {
    let (database, service) = setup().await;
    let tenant_id = Uuid::new_v4();
    let term_id = service
        .create_term(
            tenant_id,
            admin(),
            CreateTaxonomyTermInput {
                kind: TaxonomyTermKind::Tag,
                scope_type: TaxonomyScopeType::Global,
                scope_value: None,
                locale: "en".to_string(),
                name: "Systems".to_string(),
                slug: Some("systems".to_string()),
                canonical_key: None,
                description: None,
                aliases: Vec::new(),
            },
        )
        .await
        .expect("global Tag should be created");

    service
        .update_term(
            tenant_id,
            term_id,
            admin(),
            UpdateTaxonomyTermInput {
                locale: "en".to_string(),
                name: Some("Systems Updated".to_string()),
                slug: None,
                description: None,
                aliases: None,
            },
        )
        .await
        .expect("global Tag update should succeed");

    let events_after_update = rustok_outbox::SysEvents::find()
        .all(&database)
        .await
        .expect("Search reindex event should be durable after global Tag update");
    assert_eq!(events_after_update.len(), 1);
    assert_eq!(events_after_update[0].event_type, "index.reindex_requested");
    assert_eq!(
        events_after_update[0].payload["event"]["data"]["target_type"],
        "search"
    );
    assert!(events_after_update[0].payload["event"]["data"]["target_id"].is_null());

    service
        .delete_term(tenant_id, term_id, admin())
        .await
        .expect("global Tag delete should succeed");

    let events_after_delete = rustok_outbox::SysEvents::find()
        .all(&database)
        .await
        .expect("Search reindex event should be durable after global Tag delete");
    assert_eq!(events_after_delete.len(), 2);
    assert!(
        events_after_delete
            .iter()
            .all(|event| event.event_type == "index.reindex_requested")
    );
}

#[tokio::test]
async fn global_tag_reindex_outbox_failure_rolls_back_taxonomy_update() {
    let (database, service) = setup().await;
    let tenant_id = Uuid::new_v4();
    let term_id = service
        .create_term(
            tenant_id,
            admin(),
            CreateTaxonomyTermInput {
                kind: TaxonomyTermKind::Tag,
                scope_type: TaxonomyScopeType::Global,
                scope_value: None,
                locale: "en".to_string(),
                name: "Systems".to_string(),
                slug: Some("systems".to_string()),
                canonical_key: None,
                description: None,
                aliases: Vec::new(),
            },
        )
        .await
        .expect("global Tag should be created");

    database
        .execute_unprepared("DROP TABLE sys_events")
        .await
        .expect("test must be able to disable the transactional outbox");

    let error = service
        .update_term(
            tenant_id,
            term_id,
            admin(),
            UpdateTaxonomyTermInput {
                locale: "en".to_string(),
                name: Some("Must Roll Back".to_string()),
                slug: None,
                description: None,
                aliases: None,
            },
        )
        .await
        .expect_err("missing Search reindex outbox must abort the Taxonomy update");
    assert!(matches!(error, crate::TaxonomyError::Internal(_)));

    let persisted = service
        .get_term(tenant_id, admin(), term_id, "en", None)
        .await
        .expect("rolled-back Taxonomy term should remain readable");
    assert_eq!(persisted.name, "Systems");
}

#[tokio::test]
async fn module_owned_translation_requires_owner_and_runs_owner_side_effect_hook() {
    let (database, service) = setup().await;
    let tenant_id = Uuid::new_v4();

    let transaction = database.begin().await.expect("transaction should start");
    let term_id = service
        .create_module_term_in_tx(
            &transaction,
            tenant_id,
            TaxonomyTermKind::Tag,
            "blog",
            ModuleTermCreateInput {
                locale: "en".to_string(),
                name: "Systems".to_string(),
                slug: Some("systems".to_string()),
                canonical_key: None,
            },
        )
        .await
        .expect("module term should be created");
    transaction
        .commit()
        .await
        .expect("transaction should commit");

    let identity = rustok_translation_targets::TranslationResourceIdentity {
        owner_slug: rustok_translation_targets::OwnerSlug::new("taxonomy").unwrap(),
        resource_kind: rustok_translation_targets::ResourceKind::new("term").unwrap(),
        resource_id: rustok_translation_targets::ResourceId::new(term_id.to_string()).unwrap(),
        subresource_id: None,
    };

    let taxonomy_reader = PortContext::new(
        tenant_id.to_string(),
        PortActor::user(Uuid::new_v4().to_string()),
        "en",
        "taxonomy-only-read",
    )
    .with_claim("taxonomy:read")
    .with_role("manager")
    .with_deadline(Duration::from_secs(5));

    let provider = TaxonomyTranslationTargetProvider::new(service.clone());
    let list = provider
        .list_resources(
            taxonomy_reader.clone(),
            ListTranslationResourcesRequest {
                source_locale: TenantLocale::new("en").unwrap(),
                target_locale: TenantLocale::new("fr").unwrap(),
                cursor: None,
                limit: 10,
            },
        )
        .await
        .expect("translation discovery should succeed");
    assert!(list.resources.is_empty());

    let denied = provider
        .read_resource(
            taxonomy_reader.clone(),
            ReadTranslationResourceRequest {
                identity: identity.clone(),
                source_locale: TenantLocale::new("en").unwrap(),
                target_locale: TenantLocale::new("fr").unwrap(),
            },
        )
        .await
        .expect_err("module-owned term must not bypass its owner");
    assert_eq!(denied.kind, PortErrorKind::Forbidden);

    let authorize_calls = Arc::new(AtomicUsize::new(0));
    let apply_calls = Arc::new(AtomicUsize::new(0));
    let mut owners = TaxonomyModuleTermTranslationOwnerRegistry::default();
    owners
        .register(TestModuleTranslationOwner {
            authorize_calls: authorize_calls.clone(),
            apply_calls: apply_calls.clone(),
        })
        .expect("test owner should register");
    let provider = TaxonomyTranslationTargetProvider::with_owner_registry(service, owners);

    let list = provider
        .list_resources(
            read_context(tenant_id),
            ListTranslationResourcesRequest {
                source_locale: TenantLocale::new("en").unwrap(),
                target_locale: TenantLocale::new("fr").unwrap(),
                cursor: None,
                limit: 10,
            },
        )
        .await
        .expect("registered owner should authorize discovery");
    assert_eq!(list.resources.len(), 1);
    assert!(authorize_calls.load(Ordering::SeqCst) >= 1);

    let snapshot = provider
        .read_resource(
            read_context(tenant_id),
            ReadTranslationResourceRequest {
                identity,
                source_locale: TenantLocale::new("en").unwrap(),
                target_locale: TenantLocale::new("fr").unwrap(),
            },
        )
        .await
        .expect("registered owner should authorize reads");

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
        ],
        proposal_id: "module-owner-proposal".to_string(),
        approval_receipt_id: "module-owner-approval".to_string(),
    };
    provider
        .apply_patch(
            PortContext::new(
                tenant_id.to_string(),
                PortActor::system(),
                "en",
                "module-owner-apply",
            )
            .with_idempotency_key("module-owner-apply-1")
            .with_deadline(Duration::from_secs(5)),
            patch,
        )
        .await
        .expect("registered owner should authorize apply");

    assert_eq!(apply_calls.load(Ordering::SeqCst), 1);

    let outbox_events = rustok_outbox::SysEvents::find()
        .all(&database)
        .await
        .expect("module-owned Tag translation should not emit a global Search rebuild");
    assert!(
        outbox_events.is_empty(),
        "module-owned Taxonomy changes must not over-invalidate the tenant Search scope"
    );
}

#[tokio::test]
async fn exact_category_translation_retains_and_reserves_the_previous_route() {
    let (db, service) = setup().await;
    let tenant_id = Uuid::new_v4();
    let category_id = Uuid::new_v4();
    let txn = db.begin().await.unwrap();
    for (locale, name, slug) in [("en", "Notes", "notes"), ("fr", "Carnets", "carnets")] {
        crate::sync_module_category_copy_in_tx(
            &txn,
            tenant_id,
            crate::SyncModuleCategoryInput {
                category_id,
                module_scope: "blog".into(),
                canonical_key: format!("blog-category-{category_id}"),
                locale: locale.into(),
                name: name.into(),
                slug: slug.into(),
                aliases: Vec::new(),
                description: None,
                parent_id: None,
                position: 0,
                icon_key: None,
                color: None,
            },
        )
        .await
        .unwrap();
    }
    txn.commit().await.unwrap();

    let mut owners = TaxonomyModuleTermTranslationOwnerRegistry::default();
    owners
        .register(TestModuleTranslationOwner {
            authorize_calls: Arc::default(),
            apply_calls: Arc::default(),
        })
        .unwrap();
    let provider = TaxonomyTranslationTargetProvider::with_owner_registry(service.clone(), owners);
    let request = ReadTranslationResourceRequest {
        identity: rustok_translation_targets::TranslationResourceIdentity {
            owner_slug: rustok_translation_targets::OwnerSlug::new("taxonomy").unwrap(),
            resource_kind: rustok_translation_targets::ResourceKind::new("term").unwrap(),
            resource_id: rustok_translation_targets::ResourceId::new(category_id.to_string())
                .unwrap(),
            subresource_id: None,
        },
        source_locale: TenantLocale::new("en").unwrap(),
        target_locale: TenantLocale::new("fr").unwrap(),
    };
    for (slug, is_reserved) in [("journal", false), ("carnets", true)] {
        let snapshot = provider
            .read_resource(read_context(tenant_id), request.clone())
            .await
            .unwrap();
        let patch = TranslationPatchRequest {
            identity: snapshot.summary.identity.clone(),
            source_locale: snapshot.source_locale.clone(),
            target_locale: snapshot.target_locale.clone(),
            expected_resource_revision: snapshot.summary.resource_revision.clone(),
            expected_source_revision: snapshot.source_revision.clone(),
            expected_target_revision: snapshot.target_revision.clone(),
            fields: vec![field_patch(&snapshot, "slug", slug)],
            proposal_id: format!("category-slug-{slug}"),
            approval_receipt_id: format!("category-slug-approved-{slug}"),
        };
        let result = provider
            .apply_patch(
                read_context(tenant_id).with_idempotency_key(format!("category-slug-{slug}")),
                patch,
            )
            .await;
        if is_reserved {
            assert_eq!(result.unwrap_err().kind, PortErrorKind::Conflict);
        } else {
            result.unwrap();
        }
    }

    for (slug, is_alias) in [("journal", false), ("carnets", true)] {
        let route = service
            .resolve_term_route_for_module(
                tenant_id,
                TaxonomyTermKind::Category,
                "blog",
                "fr",
                None,
                slug,
            )
            .await
            .unwrap()
            .unwrap();
        assert_eq!(route.term_id, category_id);
        assert_eq!(route.matched_locale, "fr");
        assert_eq!(route.alias_id.is_some(), is_alias);
    }
}
