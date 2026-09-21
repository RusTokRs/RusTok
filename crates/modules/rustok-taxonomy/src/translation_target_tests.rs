use std::{sync::{Arc, atomic::{AtomicUsize, Ordering}}, time::Duration};

use async_trait::async_trait;
use rustok_api::{PortActor, PortContext, PortError, PortErrorKind, TenantLocale};
use rustok_core::{MigrationSource, SecurityContext, UserRole};
use rustok_outbox::SysEventsMigration;
use rustok_test_utils::db::setup_test_db;
use rustok_translation_targets::{
    FieldKey, ListTranslationResourcesRequest, ReadTranslationResourceRequest,
    TranslationFieldPatch, TranslationPatchRequest, TranslationTargetChangesRequest,
    TranslationTargetProgressRequest, TranslationTargetProvider,
};
use sea_orm::{DatabaseConnection, DatabaseTransaction, TransactionTrait};
use sea_orm_migration::{MigrationTrait, SchemaManager};
use uuid::Uuid;

use crate::{
    CreateTaxonomyTermInput, ModuleTermCreateInput, TaxonomyModule,
    TaxonomyModuleTermTranslationOwner, TaxonomyModuleTermTranslationOwnerRegistry,
    TaxonomyScopeType, TaxonomyService, TaxonomyTermKind, TaxonomyTranslationTargetProvider,
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
async fn translation_target_schema_supports_up_down_up() {
    let database = setup_test_db().await;
    let manager = SchemaManager::new(&database);
    let mut migrations = TaxonomyModule.migrations();
    let translation_target_migration = migrations
        .pop()
        .expect("Taxonomy translation target migration should be registered");

    for migration in migrations {
        migration
            .up(&manager)
            .await
            .expect("base Taxonomy migration should apply");
    }
    translation_target_migration
        .up(&manager)
        .await
        .expect("Taxonomy translation target migration should apply");
    translation_target_migration
        .down(&manager)
        .await
        .expect("Taxonomy translation target migration should roll back");
    translation_target_migration
        .up(&manager)
        .await
        .expect("Taxonomy translation target migration should reapply");

    let service = TaxonomyService::new(database);
    service
        .create_term(
            Uuid::new_v4(),
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
        key: FieldKey::new(key).expect("static field key should be valid"),
        value: value.to_string(),
        expected_source_hash: field.source_hash.clone(),
    }
}

#[tokio::test]
async fn translation_target_applies_replays_and_tracks_an_exact_term_locale() {
    let (_database, service) = setup().await;
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
            FieldKey::new("name").expect("static field key should be valid"),
            FieldKey::new("slug").expect("static field key should be valid"),
            FieldKey::new("description").expect("static field key should be valid"),
        ]
    );
    let replay = provider
        .apply_patch(apply_context.clone(), patch.clone())
        .await
        .expect("same Taxonomy idempotency request should replay");
    assert_eq!(replay, receipt);

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
            },
        )
        .await
        .expect("module term should be created");
    transaction.commit().await.expect("transaction should commit");

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
}
