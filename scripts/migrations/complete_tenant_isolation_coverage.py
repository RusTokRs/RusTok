from pathlib import Path


def replace_once(path: Path, old: str, new: str, label: str) -> None:
    source = path.read_text()
    count = source.count(old)
    if count != 1:
        raise RuntimeError(f"{path}: {label}: expected 1 match, got {count}")
    path.write_text(source.replace(old, new, 1))


integration = Path("apps/server/tests/tenant_resolver_invariants_test.rs")
replace_once(
    integration,
    '''    let app = Router::new()
        .route("/tenant-probe", get(tenant_probe))
        .route_layer(middleware::from_fn_with_state(
''',
    '''    let app = Router::new()
        .route("/tenant-probe", get(tenant_probe))
        .route("/api/graphql", get(tenant_probe))
        .route("/storefront/products", get(tenant_probe))
        .route_layer(middleware::from_fn_with_state(
''',
    "add representative tenant-bound transports",
)
replace_once(
    integration,
    '''    (db, runtime_ctx, app)
}

async fn request_tenant_slug''',
    '''    (db, runtime_ctx, app)
}

async fn request_path(
    app: &Router,
    path: &str,
    tenant_header: Option<&str>,
) -> StatusCode {
    let mut builder = Request::builder().uri(path);
    if let Some(tenant_header) = tenant_header {
        builder = builder.header("X-Tenant-ID", tenant_header);
    }
    app.clone()
        .oneshot(builder.body(Body::empty()).expect("request"))
        .await
        .expect("tenant-bound request should complete")
        .status()
}

async fn request_tenant_slug''',
    "generic tenant-bound request helper",
)
replace_once(
    integration,
    '''#[tokio::test]
#[serial]
async fn header_resolution_resolves_active_tenant_context() {
''',
    '''#[tokio::test]
#[serial]
async fn tenant_bound_http_transports_reject_missing_tenant_assertion() {
    let settings = RustokSettings::default();
    let (_db, _runtime_ctx, app) = setup_tenant_router(settings).await;

    for path in ["/tenant-probe", "/api/graphql", "/storefront/products"] {
        assert_eq!(
            request_path(&app, path, None).await,
            StatusCode::BAD_REQUEST,
            "{path} must fail closed without tenant identity"
        );
    }
}

#[tokio::test]
#[serial]
async fn tenant_bound_http_transports_reject_attacker_controlled_identifier() {
    let settings = RustokSettings::default();
    let (_db, _runtime_ctx, app) = setup_tenant_router(settings).await;

    for path in ["/tenant-probe", "/api/graphql", "/storefront/products"] {
        assert_eq!(
            request_path(&app, path, Some("../../other-tenant")).await,
            StatusCode::BAD_REQUEST,
            "{path} must reject malformed tenant identity"
        );
    }
}

#[tokio::test]
#[serial]
async fn header_resolution_resolves_active_tenant_context() {
''',
    "negative HTTP transport tests",
)

graphql = Path("apps/server/src/controllers/graphql.rs")
replace_once(
    graphql,
    '''mod tests {
    use super::graphql_permissions;
    use rustok_api::{Permission, Resource};
''',
    '''mod tests {
    use super::graphql_permissions;
    use crate::{
        common::settings::RustokSettings,
        middleware::tenant,
        services::server_runtime_context::ServerRuntimeContext,
    };
    use rustok_api::{Permission, Resource};
    use rustok_cache::CacheService;
    use rustok_migrations::Migrator;
    use sea_orm::{ActiveModelTrait, Set};
    use serial_test::serial;
''',
    "GraphQL WS test imports",
)
replace_once(
    graphql,
    '''    #[test]
    fn manage_implies_graphql_read_and_list_without_widening_other_resources() {
''',
    '''    #[tokio::test]
    #[serial]
    async fn graphql_ws_tenant_handshake_fails_closed() {
        let db = rustok_test_utils::db::setup_test_db_with_migrations::<Migrator>().await;
        let runtime = ServerRuntimeContext::new(db.clone(), RustokSettings::default());
        tenant::init_tenant_cache_infrastructure(&runtime, &CacheService::from_url(None)).await;

        let malformed = match tenant::resolve_tenant_context_by_slug(&runtime, "../../other").await {
            Ok(_) => panic!("malformed WebSocket tenant slug must be rejected"),
            Err(error) => error,
        };
        assert_eq!(malformed.client_message(), "Invalid tenant identifier");

        let unknown = match tenant::resolve_tenant_context_by_slug(&runtime, "missing-ws-tenant").await {
            Ok(_) => panic!("unknown WebSocket tenant must be rejected"),
            Err(error) => error,
        };
        assert_eq!(unknown.client_message(), "Tenant not found");

        let now = chrono::Utc::now();
        crate::models::_entities::tenants::ActiveModel {
            id: Set(uuid::Uuid::new_v4()),
            name: Set("Disabled WS tenant".to_string()),
            slug: Set("disabled-ws-tenant".to_string()),
            domain: Set(None),
            settings: Set(serde_json::json!({})),
            default_locale: Set("en".to_string()),
            is_active: Set(false),
            created_at: Set(now.into()),
            updated_at: Set(now.into()),
        }
        .insert(&db)
        .await
        .expect("disabled tenant should insert");

        let disabled = match tenant::resolve_tenant_context_by_slug(&runtime, "disabled-ws-tenant").await {
            Ok(_) => panic!("disabled WebSocket tenant must be rejected"),
            Err(error) => error,
        };
        assert_eq!(disabled.client_message(), "Tenant is disabled");
    }

    #[test]
    fn manage_implies_graphql_read_and_list_without_widening_other_resources() {
''',
    "GraphQL WS fail-closed test",
)

verify = Path("scripts/verify/verify-tenant-resolution-architecture.mjs")
replace_once(
    verify,
    '''const graphql = fs.readFileSync("apps/server/src/controllers/graphql.rs", "utf8");
''',
    '''const graphql = fs.readFileSync("apps/server/src/controllers/graphql.rs", "utf8");
const telemetry = fs.readFileSync("crates/rustok-telemetry/src/metrics.rs", "utf8");
''',
    "telemetry gate input",
)
replace_once(
    verify,
    '''requireMatch(settings, /pub resolution: TenantResolutionMode/, "TenantSettings.resolution must use TenantResolutionMode");
forbidMatch(settings, /pub resolution: String/, "stringly typed tenant resolution is forbidden");
''',
    '''requireMatch(settings, /pub resolution: TenantResolutionMode/, "TenantSettings.resolution must use TenantResolutionMode");
requireMatch(settings, /pub enum TenantRuntimeProfile/, "tenant runtime profile must be typed");
requireMatch(settings, /pub profile: TenantRuntimeProfile/, "TenantSettings.profile must be explicit");
requireMatch(settings, /DevelopmentProfileForbiddenInProduction/, "development tenant profile must fail production validation");
forbidMatch(settings, /pub resolution: String/, "stringly typed tenant resolution is forbidden");
''',
    "profile architecture gates",
)
replace_once(
    verify,
    '''requireMatch(resolution, /resolve_explicit_slug/, "self-resolving transports must produce typed resolutions");
requireMatch(resolution, /SelfResolvingHandshake/, "explicit slug resolution must expose a typed source");
''',
    '''requireMatch(resolution, /resolve_explicit_slug/, "self-resolving transports must produce typed resolutions");
requireMatch(resolution, /SelfResolvingHandshake/, "explicit slug resolution must expose a typed source");
requireMatch(resolution, /TenantRuntimeProfile::SingleTenant/, "single-tenant behavior must be selected by the explicit profile");
forbidMatch(resolution, /if !settings\.tenant\.enabled/, "tenant enabled compatibility flag must not select runtime behavior directly");
''',
    "profile resolver gates",
)
replace_once(
    verify,
    '''requireMatch(runtime, /record_resolution_source\(resolution\.source\)/, "all tenant transports must emit telemetry from typed resolution results");
requireMatch(graphql, /tenant::resolve_tenant_context_by_slug/, "GraphQL WebSocket must use typed canonical tenant resolution");
''',
    '''requireMatch(runtime, /record_resolution_outcome/, "tenant resolution outcomes must use dedicated telemetry");
requireMatch(runtime, /record_tenant_resolution/, "tenant runtime must emit the dedicated tenant metric");
forbidMatch(runtime, /record_cache_operation\([\s\S]*tenant_resolution/, "tenant resolution must not use cache-operation telemetry");
requireMatch(telemetry, /rustok_tenant_resolutions_total/, "telemetry must register a dedicated tenant resolution counter");
requireMatch(telemetry, /pub fn record_tenant_resolution/, "telemetry must expose a tenant resolution recording helper");
requireMatch(graphql, /tenant::resolve_tenant_context_by_slug/, "GraphQL WebSocket must use typed canonical tenant resolution");
''',
    "dedicated telemetry gates",
)
replace_once(
    verify,
    '''forbidMatch(graphql, /models::tenants::Entity::find_by_slug/, "GraphQL WebSocket must not query tenants directly");
forbidMatch(facade, /pub use super::tenant_resolution/, "tenant resolution internals must not be public API");
''',
    '''forbidMatch(graphql, /models::tenants::Entity::find_by_slug/, "GraphQL WebSocket must not query tenants directly");
requireMatch(graphql, /graphql_ws_tenant_handshake_fails_closed/, "GraphQL WebSocket must have negative tenant handshake coverage");
requireMatch(integration, /tenant_bound_http_transports_reject_missing_tenant_assertion/, "tenant-bound HTTP surfaces must reject missing tenant identity");
requireMatch(integration, /tenant_bound_http_transports_reject_attacker_controlled_identifier/, "tenant-bound HTTP surfaces must reject malformed tenant identity");
requireMatch(integration, /\/api\/graphql/, "GraphQL HTTP tenant isolation must be covered");
requireMatch(integration, /\/storefront\/products/, "storefront tenant isolation must be covered");
forbidMatch(facade, /pub use super::tenant_resolution/, "tenant resolution internals must not be public API");
''',
    "negative transport coverage gates",
)

plan = Path("docs/verification/PLATFORM_HARDENING_IMPLEMENTATION_PLAN.md")
replace_once(
    plan,
    '''The plan was initially revalidated against `main` on 2026-07-17 at commit `9c3a5f1b443d7fc0fa1dae8ee9b09a29d2edfb67`. The progress ledger was refreshed on 2026-07-17 after the canonical tenant-resolution and transport-unification work through commit `cead00ec16522257b7b3d0689aaf14238a160558`.
''',
    '''The plan was initially revalidated against `main` on 2026-07-17 at commit `9c3a5f1b443d7fc0fa1dae8ee9b09a29d2edfb67`. The progress ledger was refreshed on 2026-07-17 after completing the typed tenant profile, dedicated resolution telemetry and cross-transport negative isolation coverage.
''',
    "ledger revision",
)
replace_once(
    plan,
    '''1. The enforced UI Content Security Policy still permits `unsafe-inline` and `unsafe-eval`; a strict report-only policy now exposes violations, but nonce/hash migration and enforcement remain required.
2. Negative tenant-isolation integration coverage is still incomplete across REST, GraphQL, WebSocket and storefront paths.
3. Browser E2E runs in a dedicated workflow, but repository branch protection has not yet been verified to require that workflow.
4. Five dependency waivers are now registered and time-bounded, but their exact reverse dependency paths and reachability evidence must be captured before the 2026-07-24 expiry.
5. Production JWT bootstrap policy now validates algorithm-specific key material, issuer, audience and HS256 secret quality; operational key rotation and emergency revocation remain separate production-readiness work.
''',
    '''1. The enforced UI Content Security Policy still permits `unsafe-inline` and `unsafe-eval`; a strict report-only policy now exposes violations, but nonce/hash migration and enforcement remain required.
2. Browser E2E runs in a dedicated workflow, but repository branch protection has not yet been verified to require that workflow.
3. Five dependency waivers are now registered and time-bounded, but their exact reverse dependency paths and reachability evidence must be captured before the 2026-07-24 expiry.
4. Production JWT bootstrap policy now validates algorithm-specific key material, issuer, audience and HS256 secret quality; operational key rotation and emergency revocation remain separate production-readiness work.
''',
    "close tenant open finding",
)
replace_once(
    plan,
    '''5. HTTP and GraphQL WebSocket use one cache-aware tenant read-port loader with typed errors; transport code no longer queries tenant persistence or reconstructs `TenantContext` independently.
6. Operator routes, self-resolving handshakes and the global read-only registry catalog are represented by one segment-safe route policy rather than duplicated bypass lists.
6. Subdomain tenant resolution requires at least one configured base domain at bootstrap.
''',
    '''5. HTTP and GraphQL WebSocket use one cache-aware tenant read-port loader with typed errors; transport code no longer queries tenant persistence or reconstructs `TenantContext` independently.
6. Operator routes, self-resolving handshakes and the global read-only registry catalog are represented by one segment-safe route policy rather than duplicated bypass lists.
7. Tenant runtime behavior is selected by an explicit `multi_tenant`, `single_tenant` or `development` profile; the development profile is forbidden in production.
8. Tenant resolution uses the dedicated `rustok_tenant_resolutions_total` metric with bounded transport, typed source and outcome labels rather than cache-operation telemetry.
9. Negative tenant isolation coverage rejects missing, malformed, unknown, conflicting and disabled tenant assertions across REST, GraphQL HTTP, GraphQL WebSocket and storefront paths.
10. Subdomain tenant resolution requires at least one configured base domain at bootstrap.
''',
    "closed tenant findings",
)
replace_once(
    plan,
    '''1. `HARD-107` Add required negative tenant-isolation integration tests.
2. Complete `HARD-101` with nonce/hash CSP and remove `unsafe-eval` from enforcement.
3. Complete `HARD-102` with violation collection, telemetry and an allowlist inventory.
4. Capture exact dependency paths and reachability evidence before advisory exceptions expire on 2026-07-24.
5. Add required negative tenant-isolation integration coverage for malformed and conflicting assertions across all transports.
6. Complete `HARD-105` with an explicitly named development/single-tenant profile contract.
7. Make `HARD-201` a required branch-protection check.
8. `HARD-204` API compatibility diff gates.
9. `HARD-205` Migration upgrade and rollback verification.
10. `HARD-206` Signed SemVer release workflow and artifacts.
11. `HARD-005` Protected main branch and merge policy.
12. `HARD-006` Benchmark claim evidence cleanup.
13. `HARD-202` Leptos admin/storefront browser smoke coverage.
14. `HARD-305` JWT/key rotation and emergency revocation runbooks.
15. `HARD-301` SLI/SLO definitions and dashboards.
16. `HARD-302` Worker backpressure and cancellation policy.
17. `HARD-307` Per-tenant resource quotas.
18. `HARD-304` Restore drills and disaster recovery evidence.
19. `HARD-306` Dependency degradation and chaos tests.
20. `HARD-406` Reproducible performance regression suite.
''',
    '''1. Complete `HARD-101` with nonce/hash CSP and remove `unsafe-eval` from enforcement.
2. Complete `HARD-102` with violation collection, telemetry and an allowlist inventory.
3. Capture exact dependency paths and reachability evidence before advisory exceptions expire on 2026-07-24.
4. Make `HARD-201` a required branch-protection check.
5. `HARD-204` API compatibility diff gates.
6. `HARD-205` Migration upgrade and rollback verification.
7. `HARD-206` Signed SemVer release workflow and artifacts.
8. `HARD-005` Protected main branch and merge policy.
9. `HARD-006` Benchmark claim evidence cleanup.
10. `HARD-202` Leptos admin/storefront browser smoke coverage.
11. `HARD-305` JWT/key rotation and emergency revocation runbooks.
12. `HARD-301` SLI/SLO definitions and dashboards.
13. `HARD-302` Worker backpressure and cancellation policy.
14. `HARD-307` Per-tenant resource quotas.
15. `HARD-304` Restore drills and disaster recovery evidence.
16. `HARD-306` Dependency degradation and chaos tests.
17. `HARD-406` Reproducible performance regression suite.
18. `HARD-108` Database-level tenant integrity checks for every tenant-owned relation.
19. `HARD-303` Structured audit logs for privileged and tenant-changing operations.
20. `HARD-308` Compliance evidence pack with threat model and data-flow diagrams.
''',
    "ordered backlog",
)
replace_once(
    plan,
    '''| `HARD-105` Default-tenant fallback restriction | In progress | Production restriction `47c8003`; usage telemetry `ce315be`; named profile contract remains |
| `HARD-106` Global catalog isolation review | Completed | Boundary test `f1ae6e1`; accepted decision `4d9cbb0`; wrapper parity `8965919` |
| `HARD-109` Clock anomaly handling | Implemented; runtime tests pending local execution | Durable generation `07ed2ab`; request/cache timestamps return errors; canonical loader `21ad3a99` |
| Canonical tenant context loading | Completed | Shared HTTP/GraphQL WebSocket read-port pipeline `21ad3a99`; negative-cache degradation and WS source telemetry `cead00ec` |
''',
    '''| `HARD-105` Default-tenant fallback restriction | Completed | Explicit runtime profiles, production development-profile ban and fallback/profile validation in this batch |
| `HARD-106` Global catalog isolation review | Completed | Boundary test `f1ae6e1`; accepted decision `4d9cbb0`; wrapper parity `8965919` |
| `HARD-107` Negative tenant isolation coverage | Completed | REST, GraphQL HTTP, GraphQL WebSocket and storefront fail-closed tests in this batch |
| `HARD-109` Clock anomaly handling | Completed | Durable generation `07ed2ab`; request/cache timestamps return errors; pre-epoch unit coverage |
| Canonical tenant context loading | Completed | Shared HTTP/GraphQL WebSocket read-port pipeline plus dedicated typed-source outcome telemetry |
''',
    "tenant ledger completion",
)
