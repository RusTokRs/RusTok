from pathlib import Path
import re


def replace_once(text: str, old: str, new: str, label: str) -> str:
    count = text.count(old)
    if count != 1:
        raise RuntimeError(f"{label}: expected 1 match, got {count}")
    return text.replace(old, new, 1)


def sub_once(text: str, pattern: str, replacement: str, label: str) -> str:
    updated, count = re.subn(pattern, replacement, text, count=1, flags=re.S)
    if count != 1:
        raise RuntimeError(f"{label}: expected 1 match, got {count}")
    return updated

facade_path = Path("apps/server/src/middleware/mod.rs")
facade = facade_path.read_text()
facade = sub_once(
    facade,
    r"pub mod tenant \{\n    pub use super::tenant_resolution::\{.*?\};\n    pub use super::tenant_runtime::\{resolve, TenantCacheInfrastructure, TenantCacheStats\};",
    """pub mod tenant {
    pub use super::tenant_runtime::{resolve, TenantCacheInfrastructure, TenantCacheStats};
    pub(crate) use super::tenant_runtime::{
        load_tenant_context_by_slug, TenantContextLoadError,
    };""",
    "narrow tenant facade",
)
facade_path.write_text(facade)

graphql_path = Path("apps/server/src/controllers/graphql.rs")
graphql = graphql_path.read_text()
graphql = replace_once(
    graphql,
    "use crate::graphql::AppSchema;",
    "use crate::graphql::AppSchema;\nuse crate::middleware::tenant;",
    "tenant loader import",
)
graphql = sub_once(
    graphql,
    r"    let tenant = crate::models::tenants::Entity::find_by_slug\(runtime_ctx\.db\(\), &tenant_slug\).*?    if !tenant\.is_enabled\(\) \{.*?    \}\n",
    """    let tenant_ctx = tenant::load_tenant_context_by_slug(&runtime_ctx, &tenant_slug)
        .await
        .map_err(|error| {
            tracing::warn!(tenant_slug, error = %error, "GraphQL WebSocket tenant resolution failed");
            async_graphql::Error::new(error.client_message())
        })?;
""",
    "canonical GraphQL WebSocket tenant loader",
)
graphql = sub_once(
    graphql,
    r"    let tenant_ctx = TenantContext \{.*?    \};\n",
    "",
    "remove duplicated TenantContext construction",
)
if graphql.count("tenant.id") != 2:
    raise RuntimeError(f"tenant id references: expected 2, got {graphql.count('tenant.id')}")
graphql = graphql.replace("tenant.id", "tenant_ctx.id")
graphql = replace_once(
    graphql,
    "Locale::parse(&tenant.default_locale)",
    "Locale::parse(&tenant_ctx.default_locale)",
    "tenant locale source",
)
graphql_path.write_text(graphql)

gate_path = Path("scripts/verify/verify-tenant-resolution-architecture.mjs")
gate = gate_path.read_text()
gate = replace_once(
    gate,
    'const integration = fs.readFileSync("apps/server/tests/tenant_resolver_invariants_test.rs", "utf8");',
    'const integration = fs.readFileSync("apps/server/tests/tenant_resolver_invariants_test.rs", "utf8");\nconst graphql = fs.readFileSync("apps/server/src/controllers/graphql.rs", "utf8");',
    "GraphQL gate input",
)
gate = replace_once(
    gate,
    'forbidMatch(integration, /tenant\\.resolution\\s*=\\s*"/, "integration tests must use typed tenant modes");',
    '''forbidMatch(integration, /tenant\\.resolution\\s*=\\s*"/, "integration tests must use typed tenant modes");
requireMatch(runtime, /pub\\(crate\\) async fn load_tenant_context\\(/, "HTTP tenant context loading must be canonical");
requireMatch(runtime, /pub\\(crate\\) async fn load_tenant_context_by_slug\\(/, "slug transports must use canonical tenant loading");
requireMatch(graphql, /tenant::load_tenant_context_by_slug/, "GraphQL WebSocket must use canonical tenant loading");
forbidMatch(graphql, /models::tenants::Entity::find_by_slug/, "GraphQL WebSocket must not query tenants directly");
forbidMatch(facade, /pub use super::tenant_resolution/, "tenant resolution internals must not be public API");
forbidMatch(runtime, /TenantResolutionSourceExtension/, "request metadata must not duplicate tenant resolution state");''',
    "transport architecture assertions",
)
gate_path.write_text(gate)

report = Path("TENANT_MIGRATION_FAILURE.md")
if report.exists():
    report.unlink()

adr_path = Path("DECISIONS/2026-04-03-request-trust-and-tenant-hardening.md")
if adr_path.exists():
    adr = adr_path.read_text()
    heading = "## Canonical tenant context loading"
    if heading not in adr:
        adr += """

## Canonical tenant context loading

HTTP middleware and self-resolving transports such as GraphQL WebSocket use the same cache-aware tenant read-port pipeline. Transport code may choose an identifier, but it must not query tenant persistence directly or reconstruct `TenantContext` independently. Resolution policy remains internal to the server crate; the public middleware facade exposes only the middleware and operational cache controls.
"""
        adr_path.write_text(adr)
