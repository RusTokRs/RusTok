use std::{collections::BTreeMap, sync::Arc, time::Duration};

use async_trait::async_trait;
use rustok_api::{PortActor, PortContext, PortError, TenantLocale};
use rustok_tenant::{
    ReplaceTenantLocalePolicyRequest, TenantLocalePolicyEntry, TenantLocalePolicyPort,
    TenantLocalePolicyProjection,
};
use rustok_translation::{
    ReplaceRequiredTargetLocalesInput, TranslationError, TranslationPolicyService, migrations,
};
use sea_orm::{ConnectionTrait, Database, DatabaseConnection, DbBackend, Statement};
use sea_orm_migration::SchemaManager;
use tokio::sync::RwLock;
use uuid::Uuid;

#[derive(Default)]
struct FakeTenantLocalePolicies {
    projections: RwLock<BTreeMap<Uuid, TenantLocalePolicyProjection>>,
}

impl FakeTenantLocalePolicies {
    async fn insert(&self, projection: TenantLocalePolicyProjection) {
        self.projections
            .write()
            .await
            .insert(projection.tenant_id, projection);
    }

    async fn set_revision(&self, tenant_id: Uuid, revision: i64) {
        self.projections
            .write()
            .await
            .get_mut(&tenant_id)
            .unwrap()
            .revision = revision;
    }
}

#[async_trait]
impl TenantLocalePolicyPort for FakeTenantLocalePolicies {
    async fn read_locale_policy(
        &self,
        context: PortContext,
    ) -> Result<TenantLocalePolicyProjection, PortError> {
        let tenant_id = Uuid::parse_str(&context.tenant_id).unwrap();
        self.projections
            .read()
            .await
            .get(&tenant_id)
            .cloned()
            .ok_or_else(|| {
                PortError::not_found(
                    "translation.test_tenant_policy_missing",
                    "test tenant locale policy is missing",
                )
            })
    }

    async fn replace_locale_policy(
        &self,
        _context: PortContext,
        _request: ReplaceTenantLocalePolicyRequest,
    ) -> Result<TenantLocalePolicyProjection, PortError> {
        Err(PortError::unavailable(
            "translation.test_tenant_policy_write_unavailable",
            "test port is read-only",
        ))
    }
}

#[tokio::test]
async fn required_target_policy_is_revisioned_idempotent_and_tenant_scoped() {
    let tenant_a = Uuid::new_v4();
    let tenant_b = Uuid::new_v4();
    let (database, policies, service) = fixture(&[tenant_a, tenant_b]).await;
    policies.insert(locale_policy(tenant_a, 7)).await;
    policies.insert(locale_policy(tenant_b, 11)).await;

    let initial = service.read_policy(read_context(tenant_a)).await.unwrap();
    assert_eq!(initial.revision, 0);
    assert_eq!(initial.tenant_locale_policy_revision, 7);
    assert!(initial.required_target_locales.is_empty());

    let input = ReplaceRequiredTargetLocalesInput {
        expected_revision: 0,
        required_target_locales: vec![locale("fr"), locale("de")],
    };
    let created = service
        .replace_required_target_locales(write_context(tenant_a, "policy-create"), input.clone())
        .await
        .unwrap();
    assert_eq!(created.revision, 1);
    assert_eq!(
        created.required_target_locales,
        vec![locale("de"), locale("fr")]
    );

    let replay = service
        .replace_required_target_locales(write_context(tenant_a, "policy-create"), input)
        .await
        .unwrap();
    assert_eq!(replay, created);

    let other_tenant = service.read_policy(read_context(tenant_b)).await.unwrap();
    assert_eq!(other_tenant.revision, 0);
    assert!(other_tenant.required_target_locales.is_empty());

    let persisted_count: i64 = database
        .query_one_raw(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "SELECT COUNT(*) AS count FROM translation_policies WHERE tenant_id = ?",
            [tenant_a.into()],
        ))
        .await
        .unwrap()
        .unwrap()
        .try_get("", "count")
        .unwrap();
    assert_eq!(persisted_count, 1);
}

#[tokio::test]
async fn policy_rejects_disabled_duplicates_conflicts_and_stale_tenant_revision() {
    let tenant_id = Uuid::new_v4();
    let (_database, policies, service) = fixture(&[tenant_id]).await;
    policies.insert(locale_policy(tenant_id, 3)).await;

    let disabled = service
        .replace_required_target_locales(
            write_context(tenant_id, "policy-disabled"),
            ReplaceRequiredTargetLocalesInput {
                expected_revision: 0,
                required_target_locales: vec![locale("es")],
            },
        )
        .await
        .unwrap_err();
    assert!(matches!(
        disabled,
        TranslationError::RequiredTargetLocaleDisabled(locale) if locale == "es"
    ));

    let duplicate = service
        .replace_required_target_locales(
            write_context(tenant_id, "policy-duplicate"),
            ReplaceRequiredTargetLocalesInput {
                expected_revision: 0,
                required_target_locales: vec![locale("de"), locale("de")],
            },
        )
        .await
        .unwrap_err();
    assert!(matches!(
        duplicate,
        TranslationError::DuplicateRequiredTargetLocale
    ));

    let conflict = service
        .replace_required_target_locales(
            write_context(tenant_id, "policy-conflict"),
            ReplaceRequiredTargetLocalesInput {
                expected_revision: 4,
                required_target_locales: vec![locale("de")],
            },
        )
        .await
        .unwrap_err();
    assert!(matches!(
        conflict,
        TranslationError::TranslationPolicyConflict {
            expected: 4,
            actual: 0
        }
    ));

    service
        .replace_required_target_locales(
            write_context(tenant_id, "policy-valid"),
            ReplaceRequiredTargetLocalesInput {
                expected_revision: 0,
                required_target_locales: vec![locale("de")],
            },
        )
        .await
        .unwrap();
    let reused = service
        .replace_required_target_locales(
            write_context(tenant_id, "policy-valid"),
            ReplaceRequiredTargetLocalesInput {
                expected_revision: 0,
                required_target_locales: vec![locale("fr")],
            },
        )
        .await
        .unwrap_err();
    assert!(matches!(reused, TranslationError::IdempotencyConflict));

    policies.set_revision(tenant_id, 4).await;
    let stale = service.read_policy(read_context(tenant_id)).await.unwrap();
    assert_eq!(
        stale.freshness,
        rustok_translation::TranslationPolicyFreshness::Stale
    );
    assert_eq!(stale.revision, 1);
    assert!(stale.disabled_required_target_locales.is_empty());
}

async fn fixture(
    tenant_ids: &[Uuid],
) -> (
    DatabaseConnection,
    Arc<FakeTenantLocalePolicies>,
    TranslationPolicyService,
) {
    let database = Database::connect("sqlite::memory:").await.unwrap();
    database
        .execute_unprepared("PRAGMA foreign_keys = ON")
        .await
        .unwrap();
    database
        .execute_unprepared("CREATE TABLE tenants (id TEXT PRIMARY KEY NOT NULL)")
        .await
        .unwrap();
    let manager = SchemaManager::new(&database);
    for migration in migrations::migrations() {
        migration.up(&manager).await.unwrap();
    }
    for tenant_id in tenant_ids {
        database
            .execute_raw(Statement::from_sql_and_values(
                DbBackend::Sqlite,
                "INSERT INTO tenants (id) VALUES (?)",
                [(*tenant_id).into()],
            ))
            .await
            .unwrap();
    }
    let policies = Arc::new(FakeTenantLocalePolicies::default());
    let service = TranslationPolicyService::new(database.clone(), policies.clone());
    (database, policies, service)
}

fn locale_policy(tenant_id: Uuid, revision: i64) -> TenantLocalePolicyProjection {
    TenantLocalePolicyProjection {
        tenant_id,
        revision,
        default_locale: locale("en"),
        locales: ["en", "de", "fr"]
            .into_iter()
            .map(|value| TenantLocalePolicyEntry {
                locale: locale(value),
                name: value.to_string(),
                native_name: value.to_string(),
                is_default: value == "en",
                is_enabled: true,
                fallback_locale: (value != "en").then(|| locale("en")),
            })
            .collect(),
    }
}

fn read_context(tenant_id: Uuid) -> PortContext {
    PortContext::new(
        tenant_id.to_string(),
        PortActor::system(),
        "en",
        "translation-policy-read",
    )
    .with_deadline(Duration::from_secs(5))
}

fn write_context(tenant_id: Uuid, idempotency_key: &str) -> PortContext {
    PortContext::new(
        tenant_id.to_string(),
        PortActor::system(),
        "en",
        format!("translation-policy-{idempotency_key}"),
    )
    .with_idempotency_key(idempotency_key)
    .with_deadline(Duration::from_secs(5))
}

fn locale(value: &str) -> TenantLocale {
    TenantLocale::new(value).unwrap()
}
