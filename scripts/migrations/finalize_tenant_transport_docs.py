from pathlib import Path
import runpy


def replace_once(path: Path, old: str, new: str, label: str) -> None:
    source = path.read_text()
    count = source.count(old)
    if count != 1:
        raise RuntimeError(f"{label}: expected 1 match, got {count}")
    path.write_text(source.replace(old, new, 1))


facade = Path("apps/server/src/middleware/mod.rs")
replace_once(
    facade,
    '''mod tenant_resolution;
#[path = "tenant.rs"]
mod tenant_runtime;''',
    '''mod tenant_resolution;
mod tenant_route_policy;
#[path = "tenant.rs"]
mod tenant_runtime;''',
    "register route-policy module",
)
replace_once(
    facade,
    '''    pub(crate) use super::tenant_runtime::{load_tenant_context_by_slug, TenantContextLoadError};''',
    '''    pub(crate) use super::tenant_runtime::{
        resolve_tenant_context_by_slug, TenantContextLoadError,
    };''',
    "export typed explicit slug pipeline",
)

graphql = Path("apps/server/src/controllers/graphql.rs")
replace_once(
    graphql,
    '''    let tenant_ctx = tenant::load_tenant_context_by_slug(&runtime_ctx, &tenant_slug)
        .await''',
    '''    let tenant_ctx = tenant::resolve_tenant_context_by_slug(&runtime_ctx, &tenant_slug)
        .await''',
    "route GraphQL WebSocket through typed resolution",
)
replace_once(
    graphql,
    '''    rustok_telemetry::metrics::record_cache_operation(
        "tenant_resolution",
        "resolve",
        "graphql_ws_payload",
    );

''',
    "",
    "remove manual GraphQL source metric",
)

gate = Path("scripts/verify/verify-tenant-resolution-architecture.mjs")
replace_once(
    gate,
    '''const resolution = fs.readFileSync("apps/server/src/middleware/tenant_resolution.rs", "utf8");
const integration''',
    '''const resolution = fs.readFileSync("apps/server/src/middleware/tenant_resolution.rs", "utf8");
const routePolicy = fs.readFileSync("apps/server/src/middleware/tenant_route_policy.rs", "utf8");
const integration''',
    "load route-policy source",
)
replace_once(
    gate,
    '''requireMatch(resolution, /SelfResolvingHandshake/, "self-resolving handshakes must be explicit in route policy");
requireMatch(resolution, /path_is_or_descendant/, "global routes must use segment-safe matching");''',
    '''requireMatch(routePolicy, /enum TenantRouteScope/, "route scopes must have a dedicated typed policy");
requireMatch(routePolicy, /SelfResolvingHandshake/, "self-resolving handshakes must be explicit in route policy");
requireMatch(routePolicy, /path_is_or_descendant/, "global routes must use segment-safe matching");
forbidMatch(resolution, /enum TenantRouteScope|fn tenant_route_scope/, "resolver must not own route classification");
requireMatch(resolution, /resolve_explicit_slug/, "self-resolving transports must produce typed resolutions");
requireMatch(resolution, /SelfResolvingHandshake/, "explicit slug resolution must expose a typed source");''',
    "separate route policy assertions",
)
replace_once(
    gate,
    '''requireMatch(runtime, /pub\(crate\) async fn load_tenant_context_by_slug\(/, "slug transports must use canonical tenant loading");
requireMatch(graphql, /tenant::load_tenant_context_by_slug/, "GraphQL WebSocket must use canonical tenant loading");
requireMatch(graphql, /graphql_ws_payload/, "GraphQL WebSocket tenant resolution must emit source telemetry");''',
    '''requireMatch(runtime, /pub\(crate\) async fn resolve_tenant_context_by_slug\(/, "slug transports must use typed canonical resolution");
requireMatch(runtime, /record_resolution_source\(resolution\.source\)/, "all tenant transports must emit telemetry from typed resolution results");
requireMatch(graphql, /tenant::resolve_tenant_context_by_slug/, "GraphQL WebSocket must use typed canonical tenant resolution");
forbidMatch(graphql, /graphql_ws_payload/, "GraphQL WebSocket must not invent a manual resolution source");''',
    "enforce typed GraphQL pipeline",
)

decision = Path("DECISIONS/2026-07-17-typed-tenant-resolution.md")
replace_once(
    decision,
    '''`middleware/tenant_resolution.rs` is the single owner of:

- request route classification;
- tenant identifier extraction and validation;
- resolution source classification;
- typed resolution failures.''',
    '''`middleware/tenant_resolution.rs` is the single owner of tenant identifier extraction, validation, source classification and typed resolution failures. `middleware/tenant_route_policy.rs` separately owns route-scope classification so transport exposure cannot become resolver logic.''',
    "separate resolver and route-policy decision",
)
replace_once(
    decision,
    '''The runtime middleware consumes `TenantResolution` and records telemetry from the actual result. It does not predict fallback behavior and contains no catch-all branch. When both the configured tenant header and `X-Tenant-Slug` are supplied, the slug is treated as a correlated assertion and must match the tenant loaded by the primary identifier.''',
    '''Every tenant-bound transport consumes a `TenantResolution` and records telemetry from its typed source. HTTP derives the resolution from the request; self-resolving handshakes create an explicit typed slug resolution before using the same cache-aware context loader. No transport invents source labels or predicts fallback behavior. When both the configured tenant header and `X-Tenant-Slug` are supplied, the slug is treated as a correlated assertion and must match the tenant loaded by the primary identifier.''',
    "document typed transport resolution",
)

ledger = Path("scripts/migrations/update_tenant_hardening_ledger.py")
if ledger.exists():
    runpy.run_path(str(ledger))
    ledger.unlink()
