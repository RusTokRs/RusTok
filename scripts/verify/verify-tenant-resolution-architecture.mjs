#!/usr/bin/env node
import fs from "node:fs";

const settings = fs.readFileSync("apps/server/src/common/settings.rs", "utf8");
const runtime = fs.readFileSync("apps/server/src/middleware/tenant.rs", "utf8");
const facade = fs.readFileSync("apps/server/src/middleware/mod.rs", "utf8");
const resolution = fs.readFileSync("apps/server/src/middleware/tenant_resolution.rs", "utf8");
const routePolicy = fs.readFileSync("apps/server/src/middleware/tenant_route_policy.rs", "utf8");
const integration = fs.readFileSync("apps/server/tests/tenant_resolver_invariants_test.rs", "utf8");
const graphql = fs.readFileSync("apps/server/src/controllers/graphql.rs", "utf8");

const failures = [];
const requireMatch = (source, pattern, message) => {
  if (!pattern.test(source)) failures.push(message);
};
const forbidMatch = (source, pattern, message) => {
  if (pattern.test(source)) failures.push(message);
};

requireMatch(settings, /pub enum TenantResolutionMode/, "tenant resolution mode must be a typed enum");
requireMatch(settings, /pub resolution: TenantResolutionMode/, "TenantSettings.resolution must use TenantResolutionMode");
forbidMatch(settings, /pub resolution: String/, "stringly typed tenant resolution is forbidden");
requireMatch(resolution, /match settings\.tenant\.resolution\s*\{/, "canonical resolver must exhaustively match the typed mode");
forbidMatch(resolution, /_\s*=>/, "canonical tenant resolution must not contain catch-all branches");
requireMatch(resolution, /pub\(crate\) enum TenantResolutionSource/, "resolution source must be typed and crate-private");
requireMatch(routePolicy, /enum TenantRouteScope/, "route scopes must have a dedicated typed policy");
requireMatch(routePolicy, /SelfResolvingHandshake/, "self-resolving handshakes must be explicit in route policy");
requireMatch(routePolicy, /path_is_or_descendant/, "global routes must use segment-safe matching");
forbidMatch(resolution, /enum TenantRouteScope|fn tenant_route_scope/, "resolver must not own route classification");
requireMatch(resolution, /resolve_explicit_slug/, "self-resolving transports must produce typed resolutions");
requireMatch(resolution, /SelfResolvingHandshake/, "explicit slug resolution must expose a typed source");
requireMatch(resolution, /asserted_slug/, "dual tenant headers must be correlated against the resolved tenant");
forbidMatch(runtime, /fn should_bypass_tenant_resolution/, "route policy must not be duplicated in tenant runtime");
forbidMatch(runtime, /fn resolve_identifier/, "identifier policy must live in tenant_resolution.rs");
forbidMatch(runtime, /unwrap_or_default\(\)/, "tenant cache timestamps must not mask clock failures");
forbidMatch(facade, /tenant_legacy/, "legacy tenant alias is forbidden");
forbidMatch(facade, /validate_request_tenant_policy/, "facade must not duplicate tenant policy");
forbidMatch(facade, /default_tenant_fallback_will_be_used/, "facade must not predict fallback usage");
forbidMatch(integration, /tenant\.resolution\s*=\s*"/, "integration tests must use typed tenant modes");
requireMatch(runtime, /pub\(crate\) async fn load_tenant_context\(/, "HTTP tenant context loading must be canonical");
requireMatch(runtime, /pub\(crate\) async fn resolve_tenant_context_by_slug\(/, "slug transports must use typed canonical resolution");
requireMatch(runtime, /record_resolution_source\(resolution\.source\)/, "all tenant transports must emit telemetry from typed resolution results");
requireMatch(graphql, /tenant::resolve_tenant_context_by_slug/, "GraphQL WebSocket must use typed canonical tenant resolution");
forbidMatch(graphql, /graphql_ws_payload/, "GraphQL WebSocket must not invent a manual resolution source");
forbidMatch(graphql, /models::tenants::Entity::find_by_slug/, "GraphQL WebSocket must not query tenants directly");
forbidMatch(facade, /pub use super::tenant_resolution/, "tenant resolution internals must not be public API");
forbidMatch(runtime, /TenantResolutionSourceExtension/, "request metadata must not duplicate tenant resolution state");

if (failures.length) {
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(1);
}
console.log("✔ typed tenant resolution architecture verified");
