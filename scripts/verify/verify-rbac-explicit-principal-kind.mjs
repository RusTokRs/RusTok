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
const normalizeWhitespace = (value) => value.replace(/\s+/g, " ").trim();
const requireNormalizedText = (source, value, label) => {
  if (!normalizeWhitespace(source).includes(normalizeWhitespace(value))) {
    failures.push(`${label}: missing ${value}`);
  }
};
const forbidText = (source, value, label) => {
  if (source.includes(value)) failures.push(`${label}: forbidden ${value}`);
};

const files = {
  kind: "crates/rustok-api/src/context/principal_kind.rs",
  auth: "crates/rustok-api/src/context/auth.rs",
  context: "crates/rustok-api/src/context/mod.rs",
  api: "crates/rustok-api/src/lib.rs",
  resolver: "apps/server/src/extractors/auth/mod.rs",
  resolverTests: "apps/server/src/extractors/auth/tests.rs",
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
  "pub principal_kind: AuthPrincipalKind",
  "fn classify_access_token_claims(",
  "AuthPrincipalKind::from_authenticated_facts(",
  "let principal_kind = classify_access_token_claims(&claims)?;",
  "principal_kind,",
]) requireText(sources.resolver, marker, `${files.resolver}: single classification source`);

for (const marker of [
  "token_claim_classifier_returns_explicit_principal_kinds",
  "AuthPrincipalKind::DirectUser",
  "AuthPrincipalKind::DelegatedUser",
  "AuthPrincipalKind::Service",
  "principal_kind: AuthPrincipalKind::Service",
]) requireText(sources.resolverTests, marker, `${files.resolverTests}: resolver regression`);

requireText(
  sources.middleware,
  "AuthPrincipalContextExtension(",
  `${files.middleware}: HTTP/native propagation`,
);
requireNormalizedText(
  sources.middleware,
  "AuthPrincipalContext::new(current_user.principal_kind)",
  `${files.middleware}: HTTP/native propagation`,
);

for (const marker of [
  "AuthPrincipalContext::new(current_user.principal_kind)",
  "request = request.data(auth_ctx).data(principal_context);",
  "data.insert(principal_context);",
]) requireText(sources.graphqlHost, marker, `${files.graphqlHost}: GraphQL propagation`);

for (const [name, source] of [
  ["middleware", sources.middleware],
  ["graphqlHost", sources.graphqlHost],
]) {
  for (const forbidden of [
    "AuthPrincipalKind::from_authenticated_facts(",
    "fn graphql_principal_kind(",
    "auth.principal_kind_invalid",
    "graphql.auth_principal_kind_invalid",
    "graphql_ws.auth_principal_kind_invalid",
  ]) forbidText(source, forbidden, `${files[name]}: duplicate classification`);
}

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
  "### P0 — exact-head verification",
  "Run formatting, Events/RBAC/Admin/server compilation, focused tests, verifiers",
  "### P1 — operator parity and lifecycle",
  "Status: `in_progress`",
  "verify-rbac-explicit-principal-kind.mjs",
]) requireNormalizedText(sources.plan, marker, `${files.plan}: implementation handoff`);

for (const marker of [
  "Current item: `core/rbac`",
  "Next item: `core/rbac`",
  "`core/rbac` remains `in_progress`",
  "incident/live negative transport evidence",
]) requireNormalizedText(sources.master, marker, `${files.master}: active cursor`);

if (failures.length > 0) {
  console.error("RBAC explicit principal-kind verification failed:");
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  "✔ authentication classifies one explicit principal kind and every RBAC control-plane transport consumes it without metadata fallback",
);
