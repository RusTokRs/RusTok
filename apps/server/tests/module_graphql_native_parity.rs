use async_graphql::{EmptySubscription, Schema};
use rustok_core::ModuleRegistry;
use rustok_server::graphql::mutations::RootMutation;
use rustok_server::graphql::queries::RootQuery;
use sea_orm::{ConnectOptions, Database};
use uuid::Uuid;

async fn module_schema_contract() -> String {
    let url = format!(
        "sqlite:file:module_transition_schema_{}?mode=memory&cache=shared",
        Uuid::new_v4()
    );
    let mut options = ConnectOptions::new(url);
    options
        .max_connections(1)
        .min_connections(1)
        .sqlx_logging(false);
    let db = Database::connect(options).await.expect("test database");
    let schema = Schema::build(RootQuery, RootMutation, EmptySubscription)
        .data(db)
        .data(ModuleRegistry::new())
        .finish();
    schema.sdl()
}

#[tokio::test]
async fn transition_graphql_contract_matches_the_owner_command_shape() {
    let contract = module_schema_contract().await;

    assert!(contract.contains("moduleTransitionCheckpoint(operationId: UUID!)"));
    assert!(contract.contains("activeModuleTransitions"));
    assert!(contract.contains("moduleRetentionHolds"));
    assert!(contract.contains("revision: Int!"));
    assert!(contract.contains("finalizeModuleTransition"));
    assert!(contract.contains("expectedRevision: Int!"));
    assert!(contract.contains("idempotencyKey: UUID!"));
    assert!(!contract.contains("triggerModuleRecovery"));
}

#[tokio::test]
async fn effective_policy_graphql_contract_is_the_redacted_owner_projection() {
    let contract = module_schema_contract().await;

    assert!(contract.contains("moduleEffectivePolicy: ModuleEffectivePolicy!"));
    assert!(contract.contains("type ModuleEffectivePolicy {"));
    assert!(contract.contains("policyRevision: String!"));
    assert!(contract.contains("type ModuleEffectivePolicyDecision {"));
    assert!(contract.contains("denialReasons: [ModuleEffectivePolicyDenialReason!]!"));
    assert!(contract.contains("type ModuleEffectivePolicyDenialReason {"));
    assert!(contract.contains("moduleSlug: String"));
    assert!(!contract.contains("ModuleEffectivePolicyFact"));
}

#[tokio::test]
async fn installed_modules_graphql_contract_is_the_browser_safe_projection() {
    let contract = module_schema_contract().await;
    let installed_module_block = contract
        .split_once("type InstalledModule {")
        .expect("InstalledModule type is present in GraphQL SDL")
        .1
        .split_once("\n}")
        .expect("InstalledModule type closes in GraphQL SDL")
        .0;
    let fields = installed_module_block
        .lines()
        .filter_map(|line| line.trim().split_once(':').map(|(name, _)| name.trim()))
        .collect::<Vec<_>>();

    assert_eq!(
        fields,
        [
            "slug",
            "source",
            "crateName",
            "version",
            "required",
            "dependencies",
        ]
    );
    assert!(!installed_module_block.contains("git:"));
    assert!(!installed_module_block.contains("rev:"));
    assert!(!installed_module_block.contains("path:"));
}

#[tokio::test]
async fn tenant_modules_graphql_contract_is_the_owner_static_lifecycle_projection() {
    let contract = module_schema_contract().await;
    let tenant_module_block = contract
        .split_once("type TenantModule {")
        .expect("TenantModule type is present in GraphQL SDL")
        .1
        .split_once("\n}")
        .expect("TenantModule type closes in GraphQL SDL")
        .0;
    let fields = tenant_module_block
        .lines()
        .filter_map(|line| line.trim().split_once(':').map(|(name, _)| name.trim()))
        .collect::<Vec<_>>();

    assert_eq!(fields, ["moduleSlug", "enabled", "settings", "revision"]);
    assert!(contract.contains("tenantModules(limit: Int): [TenantModule!]!"));
    assert!(!tenant_module_block.contains("activeIdempotencyKey"));
}

#[tokio::test]
async fn module_registry_graphql_contract_is_the_canonical_static_projection() {
    let contract = module_schema_contract().await;
    let module_registry_block = contract
        .split_once("type ModuleRegistryItem {")
        .expect("ModuleRegistryItem type is present in GraphQL SDL")
        .1
        .split_once("\n}")
        .expect("ModuleRegistryItem type closes in GraphQL SDL")
        .0;
    let fields = module_registry_block
        .lines()
        .filter_map(|line| line.trim().split_once(':').map(|(name, _)| name.trim()))
        .collect::<Vec<_>>();

    assert_eq!(
        fields,
        [
            "moduleSlug",
            "name",
            "description",
            "version",
            "kind",
            "enabled",
            "lifecycleRevision",
            "dependencies",
            "ownership",
            "trustLevel",
            "hasAdminUi",
            "hasStorefrontUi",
            "uiClassification",
            "recommendedAdminSurfaces",
            "showcaseAdminSurfaces",
        ]
    );
    assert!(contract.contains("moduleRegistry(limit: Int): [ModuleRegistryItem!]!"));
    assert!(!module_registry_block.contains("settingsSchema"));
}

#[tokio::test]
async fn marketplace_graphql_contract_uses_the_browser_safe_catalog_projection() {
    let contract = module_schema_contract().await;
    let marketplace_block = contract
        .split_once("type MarketplaceModule {")
        .expect("MarketplaceModule type is present in GraphQL SDL")
        .1
        .split_once("\n}")
        .expect("MarketplaceModule type closes in GraphQL SDL")
        .0;
    let fields = marketplace_block
        .lines()
        .filter_map(|line| line.trim().split_once(':').map(|(name, _)| name.trim()))
        .collect::<Vec<_>>();

    assert_eq!(
        fields,
        [
            "slug",
            "name",
            "latestVersion",
            "description",
            "source",
            "kind",
            "category",
            "tags",
            "iconUrl",
            "bannerUrl",
            "screenshots",
            "crateName",
            "dependencies",
            "ownership",
            "trustLevel",
            "rustokMinVersion",
            "rustokMaxVersion",
            "publisher",
            "checksumSha256",
            "signaturePresent",
            "versions",
            "hasAdminUi",
            "hasStorefrontUi",
            "uiClassification",
            "registryLifecycle",
            "compatible",
            "recommendedAdminSurfaces",
            "showcaseAdminSurfaces",
            "settingsSchema",
            "installed",
            "installedVersion",
            "updateAvailable",
        ]
    );

    let lifecycle_block = contract
        .split_once("type RegistryModuleLifecycle {")
        .expect("RegistryModuleLifecycle type is present in GraphQL SDL")
        .1
        .split_once("\n}")
        .expect("RegistryModuleLifecycle type closes in GraphQL SDL")
        .0;
    assert!(lifecycle_block.contains("ownerBinding: RegistryOwnerLifecycle"));
    assert!(contract.contains("owner: String!"));
    assert!(contract.contains("requestedBy: String!"));
    assert!(contract.contains("actor: String!"));
    let event_payload_block = contract
        .split_once("type RegistryGovernanceEventPayloadLifecycle {")
        .expect("RegistryGovernanceEventPayloadLifecycle type is present in GraphQL SDL")
        .1
        .split_once("\n}")
        .expect("RegistryGovernanceEventPayloadLifecycle type closes in GraphQL SDL")
        .0;
    assert!(event_payload_block.contains("automatedChecks: [RegistryAutomatedCheckLifecycle!]!"));
    let automated_check_block = contract
        .split_once("type RegistryAutomatedCheckLifecycle {")
        .expect("RegistryAutomatedCheckLifecycle type is present in GraphQL SDL")
        .1
        .split_once("\n}")
        .expect("RegistryAutomatedCheckLifecycle type closes in GraphQL SDL")
        .0;
    assert!(automated_check_block.contains("key: String!"));
    assert!(automated_check_block.contains("status: String!"));
    assert!(automated_check_block.contains("detail: String"));
    assert!(!contract.contains("type RegistryPrincipal {"));
}
