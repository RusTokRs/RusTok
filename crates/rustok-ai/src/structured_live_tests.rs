use std::{collections::BTreeMap, sync::Arc, time::Duration};

use rustok_api::{PortActor, PortContext, manifest_hash::hash_manifest};
use rustok_core::ModuleRegistry;
use rustok_outbox::{OutboxTransport, TransactionalEventBus};
use rustok_secrets::{EnvResolver, SecretAccessPolicy, SecretResolverRegistry};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use serde_json::json;
use uuid::Uuid;

use crate::{
    AiHostRuntime, AiProviderConfig, AiProviderTarget, AiProviderTargetCatalog,
    AiStructuredTaskCatalog, AiStructuredTaskDescriptor, AiStructuredTaskPort,
    AiStructuredTaskRequest, AiStructuredTaskStatus, AiTaskDataClassification,
    ProviderEgressPolicy, ProviderTargetId,
    accounting::{BudgetPolicy, ProviderPolicy, StructuredAccounting},
    entities::ai_structured_budgets,
    structured_result::StructuredResultKeyring,
    structured_runtime::DurableAiStructuredTaskPort,
    structured_test_support,
};

const LIVE_CONFIG_ENV: &str = "RUSTOK_AI_LIVE_STRUCTURED_PROVIDER_CONFIG_JSON";
const LIVE_TASK_SLUG: &str = "structured_live_probe";

/// Deployment-only structured-runtime probe. It is ignored by default because
/// it makes a billable provider call and resolves only explicitly prefixed env
/// keys. It runs through the durable ledger, routing, accounting, JSON Schema,
/// encrypted result, and restart-replay path rather than calling an engine
/// directly.
#[tokio::test]
#[ignore = "requires deployment-owned RUSTOK_AI_LIVE_STRUCTURED_PROVIDER_CONFIG_JSON and provider credentials"]
async fn executes_declared_live_provider_through_durable_structured_runtime() {
    let raw = std::env::var(LIVE_CONFIG_ENV)
        .unwrap_or_else(|_| panic!("set {LIVE_CONFIG_ENV} to one AiProviderConfig JSON object"));
    let mut config: AiProviderConfig = serde_json::from_str(&raw).unwrap_or_else(|error| {
        panic!("{LIVE_CONFIG_ENV} must be valid AiProviderConfig JSON: {error}")
    });
    assert!(
        config.credential_refs.values().all(|reference| {
            reference.resolver == "env" && reference.key.starts_with("RUSTOK_AI_LIVE_")
        }),
        "live structured credentials must use the env resolver and RUSTOK_AI_LIVE_ keys"
    );

    let database = structured_test_support::database().await;
    let tenant_id = Uuid::new_v4();
    let provider_profile_id = Uuid::new_v4();
    config.tenant_id = tenant_id;
    structured_test_support::insert_tenant(&database, tenant_id).await;

    let target_id =
        ProviderTargetId::new("live_structured_probe").expect("live structured provider target id");
    let egress_policy = live_egress_policy(&config);
    let provider_targets = AiProviderTargetCatalog::new_with_egress_policy(
        vec![AiProviderTarget {
            id: target_id.clone(),
            provider_slug: config.provider_slug.clone(),
            display_name: "Live structured provider".to_string(),
            auth: config.target_auth,
            settings: config.settings.clone(),
        }],
        &egress_policy,
    )
    .expect("live structured provider target");
    structured_test_support::insert_live_provider_profile(
        &database,
        tenant_id,
        provider_profile_id,
        &target_id,
        &config,
    )
    .await;
    structured_test_support::insert_task_profile_for(
        &database,
        tenant_id,
        LIVE_TASK_SLUG,
        &[provider_profile_id],
    )
    .await;

    let accounting = StructuredAccounting::new(database.clone());
    accounting
        .put_budget(BudgetPolicy {
            tenant_id,
            currency_code: "USD".to_string(),
            limit_minor_units: 1_000_000,
            max_concurrent: 1,
        })
        .await
        .expect("live structured budget policy");
    accounting
        .put_provider_policy(ProviderPolicy {
            tenant_id,
            provider_profile_id,
            allowed_classifications: vec![AiTaskDataClassification::TenantPrivate],
            currency_code: "USD".to_string(),
            input_cost_per_million_minor: 1_000_000,
            output_cost_per_million_minor: 2_000_000,
            max_concurrent: 1,
            is_active: true,
        })
        .await
        .expect("live structured provider policy");

    let output_schema = json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "type": "object",
        "additionalProperties": false,
        "required": ["probe"],
        "properties": {"probe": {"type": "string", "enum": ["ok"]}}
    });
    let descriptor = AiStructuredTaskDescriptor {
        owner: "ai-live-evidence".to_string(),
        task_slug: LIVE_TASK_SLUG.to_string(),
        prompt_policy_digest: "a".repeat(64),
        input_schema_digest: "b".repeat(64),
        output_schema_digest: hash_manifest(&output_schema).expect("live output schema digest"),
        system_prompt: "Return only a JSON object whose probe field is exactly ok.".to_string(),
        allowed_classifications: vec![AiTaskDataClassification::TenantPrivate],
        max_input_bytes: 4096,
        max_output_bytes: 4096,
        max_attempts: 1,
    };
    let catalog = AiStructuredTaskCatalog::default();
    catalog
        .register(descriptor.clone())
        .expect("live structured task descriptor");
    let request = AiStructuredTaskRequest {
        owner: descriptor.owner,
        task_slug: descriptor.task_slug,
        prompt_policy_digest: descriptor.prompt_policy_digest,
        input_schema_digest: descriptor.input_schema_digest,
        input: json!({"instruction": "Return the exact probe value requested by policy."}),
        output_schema,
        classification: AiTaskDataClassification::TenantPrivate,
        evidence: BTreeMap::from([(
            "probe".to_string(),
            "deployment_live_structured_runtime".to_string(),
        )]),
        limits: crate::AiStructuredTaskLimits {
            max_output_bytes: 4096,
            max_attempts: 1,
        },
    };
    let context = PortContext::new(
        tenant_id.to_string(),
        PortActor::service("ai-live-evidence"),
        "en",
        "ai-live-structured-runtime",
    )
    .with_idempotency_key(format!("live-structured-{}", Uuid::new_v4()))
    .with_deadline(Duration::from_secs(90));
    let secrets = SecretResolverRegistry::builder()
        .resolver(
            "env",
            EnvResolver,
            SecretAccessPolicy::Prefix(vec!["RUSTOK_AI_LIVE_".to_string()]),
        )
        .build();
    let keyring = StructuredResultKeyring::for_test(
        "live-v1",
        Duration::from_secs(300),
        BTreeMap::from([("live-v1".to_string(), [19_u8; 32])]),
    );
    let first_port = DurableAiStructuredTaskPort::new(
        live_runtime(
            database.clone(),
            secrets.clone(),
            egress_policy.clone(),
            provider_targets.clone(),
        ),
        catalog.clone(),
        keyring.clone(),
    );
    let completed = first_port
        .execute(context.clone(), request.clone())
        .await
        .expect("live structured execution");
    assert_eq!(completed.status, AiStructuredTaskStatus::Completed);
    assert_eq!(
        completed
            .output
            .as_ref()
            .and_then(|value| value.get("probe")),
        Some(&json!("ok"))
    );
    assert_eq!(completed.attempts.len(), 1);
    assert_eq!(
        completed.attempts[0].status,
        AiStructuredTaskStatus::Completed
    );
    let usage = completed.usage.as_ref().expect("live structured usage");
    assert!(usage.total_tokens > 0);
    let committed_before_restart = budget_committed(&database, tenant_id).await;
    assert_eq!(committed_before_restart, usage.cost_minor_units);

    let restarted_port = DurableAiStructuredTaskPort::new(
        live_runtime(database.clone(), secrets, egress_policy, provider_targets),
        catalog,
        keyring,
    );
    let replayed = restarted_port
        .execute(context, request)
        .await
        .expect("live structured restart replay");
    assert_eq!(replayed.execution_id, completed.execution_id);
    assert_eq!(replayed.output, completed.output);
    assert_eq!(replayed.attempts, completed.attempts);
    assert_eq!(
        budget_committed(&database, tenant_id).await,
        committed_before_restart
    );
}

fn live_egress_policy(config: &AiProviderConfig) -> ProviderEgressPolicy {
    let mut allowed_origins = config
        .settings
        .values()
        .filter_map(serde_json::Value::as_str)
        .filter_map(|value| url::Url::parse(value).ok())
        .filter_map(|url| url.host_str().map(str::to_ascii_lowercase))
        .collect::<Vec<_>>();
    allowed_origins.sort();
    allowed_origins.dedup();
    ProviderEgressPolicy {
        allowed_origins,
        allow_local_origins: true,
    }
}

fn live_runtime(
    database: sea_orm::DatabaseConnection,
    secrets: SecretResolverRegistry,
    egress_policy: ProviderEgressPolicy,
    provider_targets: AiProviderTargetCatalog,
) -> AiHostRuntime {
    AiHostRuntime::new(
        database.clone(),
        TransactionalEventBus::new(Arc::new(OutboxTransport::new(database))),
        ModuleRegistry::new(),
        secrets,
        egress_policy,
        provider_targets,
    )
}

async fn budget_committed(database: &sea_orm::DatabaseConnection, tenant_id: Uuid) -> u64 {
    u64::try_from(
        ai_structured_budgets::Entity::find()
            .filter(ai_structured_budgets::Column::TenantId.eq(tenant_id))
            .one(database)
            .await
            .expect("live structured budget query")
            .expect("live structured budget")
            .committed_minor_units,
    )
    .expect("live structured committed cost")
}
