use std::{
    collections::BTreeSet,
    sync::{Arc, Mutex},
    time::Duration,
};

use async_trait::async_trait;
use chrono::Utc;
use rustok_api::{PortActor, PortContext, PortError, TenantLocale};
use rustok_tenant::{
    ReplaceTenantLocalePolicyRequest, TenantLocalePolicyEntry, TenantLocalePolicyPort,
    TenantLocalePolicyProjection,
};
use rustok_translation::{
    ProviderProjectionFreshness, ReplaceRequiredTargetLocalesInput, TranslationError,
    TranslationPolicyService, TranslationProgressService, entities::provider_checkpoint,
    migrations,
};
use rustok_translation_targets::{
    ListTranslationResourcesRequest, OpaqueCursor, OwnerSlug, ReadTranslationResourceRequest,
    ResourceKind, TranslationApplicationReceipt, TranslationPatchRequest,
    TranslationPatchValidation, TranslationResourcePage, TranslationResourceSnapshot,
    TranslationTargetCapability, TranslationTargetProgressFacts, TranslationTargetProgressRequest,
    TranslationTargetProvider, TranslationTargetProviderDescriptor, TranslationTargetRegistry,
};
use sea_orm::{
    ActiveModelTrait, ConnectionTrait, Database, DatabaseConnection, DbBackend, Set, Statement,
};
use sea_orm_migration::SchemaManager;
use uuid::Uuid;

struct ProgressProvider {
    facts: Arc<Mutex<TranslationTargetProgressFacts>>,
    aggregate_capability: bool,
    provider_error: bool,
}

struct TestTenantLocalePolicies;

#[async_trait]
impl TenantLocalePolicyPort for TestTenantLocalePolicies {
    async fn read_locale_policy(
        &self,
        context: PortContext,
    ) -> Result<TenantLocalePolicyProjection, PortError> {
        Ok(TenantLocalePolicyProjection {
            tenant_id: Uuid::parse_str(&context.tenant_id).unwrap(),
            revision: 7,
            default_locale: TenantLocale::new("en").unwrap(),
            locales: ["en", "de", "fr"]
                .into_iter()
                .map(|locale| TenantLocalePolicyEntry {
                    locale: TenantLocale::new(locale).unwrap(),
                    name: locale.to_string(),
                    native_name: locale.to_string(),
                    is_default: locale == "en",
                    is_enabled: true,
                    fallback_locale: (locale != "en").then(|| TenantLocale::new("en").unwrap()),
                })
                .collect(),
        })
    }

    async fn replace_locale_policy(
        &self,
        _context: PortContext,
        _request: ReplaceTenantLocalePolicyRequest,
    ) -> Result<TenantLocalePolicyProjection, PortError> {
        Err(unavailable())
    }
}

fn tenant_locale_policies() -> Arc<dyn TenantLocalePolicyPort> {
    Arc::new(TestTenantLocalePolicies)
}

#[async_trait]
impl TranslationTargetProvider for ProgressProvider {
    fn descriptor(&self) -> TranslationTargetProviderDescriptor {
        TranslationTargetProviderDescriptor {
            owner_slug: OwnerSlug::new("media").unwrap(),
            resource_kind: ResourceKind::new("asset").unwrap(),
            display_name: "Media asset metadata".to_string(),
            capabilities: if self.aggregate_capability {
                BTreeSet::from([TranslationTargetCapability::AggregateProgress])
            } else {
                BTreeSet::from([TranslationTargetCapability::ListResources])
            },
            read_permission_floor: BTreeSet::from(["media:read".to_string()]),
            apply_permission_floor: BTreeSet::from(["media:update".to_string()]),
        }
    }

    async fn list_resources(
        &self,
        _context: PortContext,
        _request: ListTranslationResourcesRequest,
    ) -> Result<TranslationResourcePage, PortError> {
        Err(unavailable())
    }

    async fn read_resource(
        &self,
        _context: PortContext,
        _request: ReadTranslationResourceRequest,
    ) -> Result<TranslationResourceSnapshot, PortError> {
        Err(unavailable())
    }

    async fn validate_patch(
        &self,
        _context: PortContext,
        _request: TranslationPatchRequest,
    ) -> Result<TranslationPatchValidation, PortError> {
        Err(unavailable())
    }

    async fn apply_patch(
        &self,
        _context: PortContext,
        _request: TranslationPatchRequest,
    ) -> Result<TranslationApplicationReceipt, PortError> {
        Err(unavailable())
    }

    async fn read_progress(
        &self,
        _context: PortContext,
        _request: TranslationTargetProgressRequest,
    ) -> Result<TranslationTargetProgressFacts, PortError> {
        if self.provider_error {
            return Err(PortError::unavailable(
                "media.translation_progress_unavailable",
                "provider progress is unavailable",
            ));
        }
        Ok(self.facts.lock().unwrap().clone())
    }
}

fn unavailable() -> PortError {
    PortError::unavailable("translation.test_unavailable", "not used by this fixture")
}

async fn database_fixture() -> (DatabaseConnection, Uuid) {
    let database = Database::connect("sqlite::memory:").await.unwrap();
    database
        .execute_unprepared("CREATE TABLE tenants (id TEXT PRIMARY KEY NOT NULL)")
        .await
        .unwrap();
    let manager = SchemaManager::new(&database);
    for migration in migrations::migrations() {
        migration.up(&manager).await.unwrap();
    }
    let tenant_id = Uuid::new_v4();
    database
        .execute_raw(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "INSERT INTO tenants (id) VALUES (?)",
            [tenant_id.into()],
        ))
        .await
        .unwrap();
    (database, tenant_id)
}

fn context(tenant_id: Uuid) -> PortContext {
    PortContext::new(
        tenant_id.to_string(),
        PortActor::system(),
        "en",
        "translation-provider-progress",
    )
    .with_deadline(Duration::from_secs(5))
}

fn write_context(tenant_id: Uuid, idempotency_key: &str) -> PortContext {
    context(tenant_id).with_idempotency_key(idempotency_key)
}

fn valid_facts(cursor: &str) -> TranslationTargetProgressFacts {
    TranslationTargetProgressFacts {
        required_units: 4,
        exact_required_units: 3,
        optional_units: 2,
        exact_optional_units: 1,
        resources: 3,
        complete_resources: 2,
        owner_change_cursor: Some(OpaqueCursor::new(cursor).unwrap()),
    }
}

#[tokio::test]
async fn provider_progress_reports_unknown_current_and_behind_without_comparing_opaque_distance() {
    let (database, tenant_id) = database_fixture().await;
    let facts = Arc::new(Mutex::new(valid_facts("owner-1")));
    let mut registry = TranslationTargetRegistry::default();
    registry
        .register(ProgressProvider {
            facts: facts.clone(),
            aggregate_capability: true,
            provider_error: false,
        })
        .unwrap();
    let service = TranslationProgressService::new(
        database.clone(),
        Arc::new(registry),
        tenant_locale_policies(),
    );
    let owner_slug = OwnerSlug::new("media").unwrap();
    let resource_kind = ResourceKind::new("asset").unwrap();
    let source_locale = TenantLocale::new("en").unwrap();
    let target_locale = TenantLocale::new("de").unwrap();

    let unknown = service
        .read_provider_progress(
            context(tenant_id),
            owner_slug.clone(),
            resource_kind.clone(),
            source_locale.clone(),
            target_locale.clone(),
        )
        .await
        .unwrap();
    assert_eq!(unknown.freshness, ProviderProjectionFreshness::Unknown);
    assert!(unknown.projected_cursor.is_none());
    assert!(unknown.checkpoint_revision.is_none());

    provider_checkpoint::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(tenant_id),
        owner_slug: Set(owner_slug.as_str().to_string()),
        resource_kind: Set(resource_kind.as_str().to_string()),
        cursor: Set(Some("owner-1".to_string())),
        revision: Set(7),
        updated_at: Set(Utc::now().fixed_offset()),
    }
    .insert(&database)
    .await
    .unwrap();
    let current = service
        .read_provider_progress(
            context(tenant_id),
            owner_slug.clone(),
            resource_kind.clone(),
            source_locale.clone(),
            target_locale.clone(),
        )
        .await
        .unwrap();
    assert_eq!(current.freshness, ProviderProjectionFreshness::Current);
    assert_eq!(current.checkpoint_revision, Some(7));
    assert_eq!(
        current.projected_cursor.as_ref().map(OpaqueCursor::as_str),
        Some("owner-1")
    );

    *facts.lock().unwrap() = valid_facts("owner-2");
    let behind = service
        .read_provider_progress(
            context(tenant_id),
            owner_slug.clone(),
            resource_kind.clone(),
            source_locale.clone(),
            target_locale.clone(),
        )
        .await
        .unwrap();
    assert_eq!(behind.freshness, ProviderProjectionFreshness::Behind);
    assert_eq!(
        behind
            .facts
            .owner_change_cursor
            .as_ref()
            .map(OpaqueCursor::as_str),
        Some("owner-2")
    );

    let foreign_tenant = Uuid::new_v4();
    database
        .execute_raw(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "INSERT INTO tenants (id) VALUES (?)",
            [foreign_tenant.into()],
        ))
        .await
        .unwrap();
    let isolated = service
        .read_provider_progress(
            context(foreign_tenant),
            owner_slug,
            resource_kind,
            source_locale,
            target_locale,
        )
        .await
        .unwrap();
    assert_eq!(isolated.freshness, ProviderProjectionFreshness::Unknown);
    assert!(isolated.projected_cursor.is_none());

    let mut no_change_facts = valid_facts("owner-2");
    no_change_facts.owner_change_cursor = None;
    *facts.lock().unwrap() = no_change_facts;
    provider_checkpoint::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(foreign_tenant),
        owner_slug: Set("media".to_string()),
        resource_kind: Set("asset".to_string()),
        cursor: Set(None),
        revision: Set(1),
        updated_at: Set(Utc::now().fixed_offset()),
    }
    .insert(&database)
    .await
    .unwrap();
    let empty_current = service
        .read_provider_progress(
            context(foreign_tenant),
            OwnerSlug::new("media").unwrap(),
            ResourceKind::new("asset").unwrap(),
            TenantLocale::new("en").unwrap(),
            TenantLocale::new("de").unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(
        empty_current.freshness,
        ProviderProjectionFreshness::Current
    );
}

#[tokio::test]
async fn required_policy_drives_cross_locale_provider_denominators_and_freshness() {
    let (database, tenant_id) = database_fixture().await;
    let tenant_locale_policies = tenant_locale_policies();
    let policy_service =
        TranslationPolicyService::new(database.clone(), Arc::clone(&tenant_locale_policies));
    policy_service
        .replace_required_target_locales(
            write_context(tenant_id, "required-progress-policy"),
            ReplaceRequiredTargetLocalesInput {
                expected_revision: 0,
                required_target_locales: vec![
                    TenantLocale::new("fr").unwrap(),
                    TenantLocale::new("en").unwrap(),
                    TenantLocale::new("de").unwrap(),
                ],
            },
        )
        .await
        .unwrap();

    let facts = Arc::new(Mutex::new(valid_facts("owner-1")));
    let mut registry = TranslationTargetRegistry::default();
    registry
        .register(ProgressProvider {
            facts: facts.clone(),
            aggregate_capability: true,
            provider_error: false,
        })
        .unwrap();
    provider_checkpoint::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(tenant_id),
        owner_slug: Set("media".to_string()),
        resource_kind: Set("asset".to_string()),
        cursor: Set(Some("owner-1".to_string())),
        revision: Set(4),
        updated_at: Set(Utc::now().fixed_offset()),
    }
    .insert(&database)
    .await
    .unwrap();
    let service =
        TranslationProgressService::new(database, Arc::new(registry), tenant_locale_policies);

    let current = service
        .read_required_provider_progress(
            context(tenant_id),
            OwnerSlug::new("media").unwrap(),
            ResourceKind::new("asset").unwrap(),
            TenantLocale::new("en").unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(
        current.required_target_locales,
        vec![
            TenantLocale::new("de").unwrap(),
            TenantLocale::new("fr").unwrap()
        ]
    );
    assert_eq!(current.translation_policy_revision, 1);
    assert_eq!(current.tenant_locale_policy_revision, 7);
    assert_eq!(current.required_units, 8);
    assert_eq!(current.exact_required_units, 6);
    assert_eq!(current.optional_units, 4);
    assert_eq!(current.exact_optional_units, 2);
    assert_eq!(current.resource_locale_pairs, 6);
    assert_eq!(current.complete_resource_locale_pairs, 4);
    assert_eq!(current.targets.len(), 2);
    assert_eq!(current.freshness, ProviderProjectionFreshness::Current);

    *facts.lock().unwrap() = valid_facts("owner-2");
    let behind = service
        .read_required_provider_progress(
            context(tenant_id),
            OwnerSlug::new("media").unwrap(),
            ResourceKind::new("asset").unwrap(),
            TenantLocale::new("en").unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(behind.freshness, ProviderProjectionFreshness::Behind);
}

#[tokio::test]
async fn provider_progress_rejects_invalid_facts_and_missing_capability() {
    let (database, tenant_id) = database_fixture().await;
    let invalid_facts = Arc::new(Mutex::new(TranslationTargetProgressFacts {
        required_units: 1,
        exact_required_units: 2,
        optional_units: 0,
        exact_optional_units: 0,
        resources: 1,
        complete_resources: 1,
        owner_change_cursor: None,
    }));
    let mut invalid_registry = TranslationTargetRegistry::default();
    invalid_registry
        .register(ProgressProvider {
            facts: invalid_facts,
            aggregate_capability: true,
            provider_error: false,
        })
        .unwrap();
    let service = TranslationProgressService::new(
        database.clone(),
        Arc::new(invalid_registry),
        tenant_locale_policies(),
    );
    let error = service
        .read_provider_progress(
            context(tenant_id),
            OwnerSlug::new("media").unwrap(),
            ResourceKind::new("asset").unwrap(),
            TenantLocale::new("en").unwrap(),
            TenantLocale::new("de").unwrap(),
        )
        .await
        .unwrap_err();
    assert!(matches!(
        error,
        TranslationError::InvalidProviderProgress(_)
    ));

    let mut unsupported_registry = TranslationTargetRegistry::default();
    unsupported_registry
        .register(ProgressProvider {
            facts: Arc::new(Mutex::new(valid_facts("owner-1"))),
            aggregate_capability: false,
            provider_error: false,
        })
        .unwrap();
    let service = TranslationProgressService::new(
        database,
        Arc::new(unsupported_registry),
        tenant_locale_policies(),
    );
    let error = service
        .read_provider_progress(
            context(tenant_id),
            OwnerSlug::new("media").unwrap(),
            ResourceKind::new("asset").unwrap(),
            TenantLocale::new("en").unwrap(),
            TenantLocale::new("de").unwrap(),
        )
        .await
        .unwrap_err();
    assert!(matches!(
        error,
        TranslationError::AggregateProgressUnavailable
    ));
}

#[tokio::test]
async fn provider_progress_propagates_provider_failure_and_rejects_corrupt_checkpoint() {
    let (database, tenant_id) = database_fixture().await;
    let mut failed_registry = TranslationTargetRegistry::default();
    failed_registry
        .register(ProgressProvider {
            facts: Arc::new(Mutex::new(valid_facts("owner-1"))),
            aggregate_capability: true,
            provider_error: true,
        })
        .unwrap();
    let service = TranslationProgressService::new(
        database.clone(),
        Arc::new(failed_registry),
        tenant_locale_policies(),
    );
    let error = service
        .read_provider_progress(
            context(tenant_id),
            OwnerSlug::new("media").unwrap(),
            ResourceKind::new("asset").unwrap(),
            TenantLocale::new("en").unwrap(),
            TenantLocale::new("de").unwrap(),
        )
        .await
        .unwrap_err();
    assert!(matches!(
        error,
        TranslationError::Provider {
            code,
            retryable: true,
            ..
        } if code == "media.translation_progress_unavailable"
    ));

    provider_checkpoint::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(tenant_id),
        owner_slug: Set("media".to_string()),
        resource_kind: Set("asset".to_string()),
        cursor: Set(Some(" ".to_string())),
        revision: Set(1),
        updated_at: Set(Utc::now().fixed_offset()),
    }
    .insert(&database)
    .await
    .unwrap();
    let mut valid_registry = TranslationTargetRegistry::default();
    valid_registry
        .register(ProgressProvider {
            facts: Arc::new(Mutex::new(valid_facts("owner-1"))),
            aggregate_capability: true,
            provider_error: false,
        })
        .unwrap();
    let service = TranslationProgressService::new(
        database,
        Arc::new(valid_registry),
        tenant_locale_policies(),
    );
    let error = service
        .read_provider_progress(
            context(tenant_id),
            OwnerSlug::new("media").unwrap(),
            ResourceKind::new("asset").unwrap(),
            TenantLocale::new("en").unwrap(),
            TenantLocale::new("de").unwrap(),
        )
        .await
        .unwrap_err();
    assert!(matches!(
        error,
        TranslationError::InvalidProviderCheckpoint(_)
    ));
}
