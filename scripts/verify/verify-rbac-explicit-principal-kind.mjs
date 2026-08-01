#!/usr/bin/env node

import { readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const configuredRoot = process.env.RUSTOK_VERIFY_REPO_ROOT?.trim();
const root = configuredRoot
  ? path.resolve(configuredRoot)
  : path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../..");
const read = (relativePath) => readFileSync(path.join(root, relativePath), "utf8");
const failures = [];
const requireText = (source, value, label) => {
  if (!source.includes(value)) failures.push(`${label}: missing ${value}`);
};
const forbidText = (source, value, label) => {
  if (source.includes(value)) failures.push(`${label}: forbidden ${value}`);
};

const files = {
  kind: "crates/rustok-api/src/context/principal_kind.rs",
  auth: "crates/rustok-api/src/context/auth.rs",
  context: "crates/rustok-api/src/context/mod.rs",
  api: "crates/rustok-api/src/lib.rs",
  middleware: "apps/server/src/middleware/auth_context.rs",
  graphqlHost: "apps/server/src/controllers/graphql.rs",
  owner: "crates/rustok-rbac/src/control_plane.rs",
  graphqlPolicy: "crates/rustok-rbac/src/graphql/control_plane.rs",
  graphqlQuery: "crates/rustok-rbac/src/graphql/query.rs",
  graphqlMutation: "crates/rustok-rbac/src/graphql/mutation.rs",
  rest: "apps/server/src/controllers/artifact_permissions.rs",
  native: "crates/rustok-rbac/admin/src/transport/native_server_adapter.rs",
  plan: "crates/rustok-rbac/docs/implementation-plan.md",
  master: "docs/verification/PLATFORM_VERIFICATION_PLAN.md",
};

const sources = Object.fromEntries(
  Object.entries(files).map(([name, relativePath]) => [name, read(relativePath)]),
);

for (const marker of [
  "pub enum AuthPrincipalKind",
  "DirectUser",
  "DelegatedUser",
  "Service",
  "pub fn from_authenticated_facts",
  '"direct" if client_id.is_none() && !session_id.is_nil()',
  '"authorization_code" if client_id.is_some() && session_id.is_nil()',
  '"client_credentials" if client_id.is_some() && session_id.is_nil()',
  "authenticated_facts_classify_fail_closed",
]) requireText(sources.kind, marker, `${files.kind}: host-neutral kind`);

for (const marker of [
  "pub struct AuthPrincipalContext",
  "pub kind: AuthPrincipalKind",
  "pub struct AuthPrincipalContextExtension",
  "impl<S> FromRequestParts<S> for AuthPrincipalContext",
  "Authenticated principal kind is unavailable",
]) requireText(sources.auth, marker, `${files.auth}: typed request context`);

requireText(sources.context, "mod principal_kind;", `${files.context}: module`);
requireText(
  sources.context,
  "pub use principal_kind::AuthPrincipalKind;",
  `${files.context}: host-neutral export`,
);
requireText(
  sources.api,
  "AuthPrincipalKind, ChannelContext",
  `${files.api}: unconditional re-export`,
);

for (const marker of [
  "AuthPrincipalKind::from_authenticated_facts(",
  "AuthPrincipalContextExtension(",
  "AuthPrincipalContext::new(principal_kind)",
  'code = "auth.principal_kind_invalid"',
  "Authenticated principal classification is invalid",
]) requireText(sources.middleware, marker, `${files.middleware}: HTTP/native construction`);

for (const marker of [
  "fn graphql_principal_kind(current_user: &CurrentUser)",
  "AuthPrincipalKind::from_authenticated_facts(",
  ".data(AuthPrincipalContext::new(principal_kind))",
  "data.insert(AuthPrincipalContext::new(principal_kind));",
  'code = "graphql.auth_principal_kind_invalid"',
  'code = "graphql_ws.auth_principal_kind_invalid"',
]) requireText(sources.graphqlHost, marker, `${files.graphqlHost}: GraphQL construction`);

for (const marker of [
  "pub struct RbacControlPlanePrincipal",
  "pub principal_kind: AuthPrincipalKind",
  "!principal.principal_kind.is_direct_user()",
  "principal.tenant_id != tenant_id",
  "delegated_and_service_principals_are_denied_even_with_management_permission",
]) requireText(sources.owner, marker, `${files.owner}: owner policy`);

for (const forbidden of [
  "principal.client_id",
  "principal.grant_type",
  "principal.session_id",
]) forbidText(sources.owner, forbidden, `${files.owner}: inferred authority`);

for (const [name, marker] of [
  ["graphqlPolicy", "principal_kind: principal_context.kind"],
  ["graphqlQuery", "data::<AuthPrincipalContext>()"],
  ["graphqlMutation", "data::<AuthPrincipalContext>()"],
  ["rest", "principal_context: AuthPrincipalContext"],
  ["native", "leptos_axum::extract::<AuthPrincipalContext>()"],
]) requireText(sources[name], marker, `${files[name]}: typed consumer`);

for (const [name, source] of [
  ["graphqlPolicy", sources.graphqlPolicy],
  ["rest", sources.rest],
  ["native", sources.native],
]) {
  for (const forbidden of [
    "session_id: auth.session_id",
    "client_id: auth.client_id",
    "grant_type: &auth.grant_type",
  ]) forbidText(source, forbidden, `${files[name]}: transport inference`);
}

for (const marker of [
  "### P1. Explicit actor-kind contract",
  "[x] Add an explicit actor/principal kind",
  "[x] Preserve fail-closed compatibility",
  "[x] Add boundary tests proving",
  "Status: `in_progress`",
  "verify-rbac-explicit-principal-kind.mjs",
]) requireText(sources.plan, marker, `${files.plan}: implementation handoff`);

for (const marker of [
  "Current item: `core/rbac`",
  "Next item: `core/rbac`",
  "`core/rbac` — `crates/rustok-rbac` — in_progress",
  "explicit typed principal kind",
]) requireText(sources.master, marker, `${files.master}: active cursor`);

if (failures.length > 0) {
  console.error("RBAC explicit principal-kind verification failed:");
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  "✔ RBAC control-plane admission consumes one explicit typed principal kind across HTTP, GraphQL, WebSocket, REST, and native adapters",
);
