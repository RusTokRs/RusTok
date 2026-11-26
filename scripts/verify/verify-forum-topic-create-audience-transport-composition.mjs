#!/usr/bin/env node
import { containsMarker } from "./lib/forum-source-verification.mjs";

import { existsSync, readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { assertCanonicalLedger } from "./lib/forum-source-verification.mjs";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = process.env.RUSTOK_VERIFY_REPO_ROOT
  ? path.resolve(process.env.RUSTOK_VERIFY_REPO_ROOT)
  : path.resolve(scriptDir, "../..");
const failures = [];

function read(relativePath) {
  const absolute = path.join(repoRoot, relativePath);
  if (!existsSync(absolute)) {
    failures.push(`${relativePath}: required file is missing`);
    return "";
  }
  return readFileSync(absolute, "utf8");
}

function requireText(source, marker, message) {
  if (!containsMarker(source, marker)) failures.push(message);
}

function rejectText(source, marker, message) {
  if (source.includes(marker)) failures.push(message);
}

function between(source, start, end, label) {
  const from = source.indexOf(start);
  const to = source.indexOf(end, from + start.length);
  if (from < 0 || to < 0 || to <= from) {
    failures.push(`${label}: bounded section is missing`);
    return "";
  }
  return source.slice(from, to);
}

const contractPath =
  "crates/modules/rustok-forum/contracts/forum-topic-create-audience-transport-composition.json";
const contract = JSON.parse(read(contractPath) || "{}");
const transport = read(contract.transport_context_file ?? "");
const graphqlRuntime = read(contract.graphql_runtime_file ?? "");
const graphqlModule = read(contract.graphql_module_file ?? "");
const graphqlLegacy = read(contract.graphql_legacy_mutation_file ?? "");
const graphqlTypes = read(contract.graphql_types_file ?? "");
const graphqlCommand = read(contract.graphql_command_mutation_file ?? "");
const httpRuntime = read(contract.http_runtime_file ?? "");
requireText(httpRuntime, "post(content_commands::create_topic)", "REST must mount the canonical topic create command");
const httpCommand = read(contract.http_command_create_file ?? "");
const manifest = read(contract.module_manifest ?? "");
const hostPublication = read(contract.host_runtime_publication_file ?? "");
const hostGraphql = read(contract.host_graphql_composition_file ?? "");
const hostHttp = read(contract.host_http_composition_file ?? "");
const adapter = read(contract.group_facts_adapter_file ?? "");
const owner = read(contract.owner_enforcement_file ?? "");
const note = read(contract.owner_note ?? "");
const crateApi = read(contract.crate_api ?? "");
const plan = read(contract.canonical_plan ?? "");

if (
  contract.schema_version !== 1 ||
  contract.task !== "FORUM-20AS" ||
  contract.upstream_task !== "FORUM-20AR"
) {
  failures.push("topic-create transport contract must identify FORUM-20AS after FORUM-20AR");
}
if (contract.verification?.execution_status !== "not_run_by_implementation_agent") {
  failures.push("topic-create transport contract must not claim unexecuted evidence");
}
for (const key of [
  "shared_exact_transport_context_builder",
  "auth_tenant_request_identity_validation",
  "dto_identity_forbidden",
  "read_deadline_semantics",
  "permission_claim_forwarding",
  "resolved_route_channel_forwarding",
  "graphql_manifest_runtime_factory",
  "graphql_legacy_create_composed",
  "graphql_command_create_composed",
  "rest_create_composed",
  "host_extension_facts_consumed",
  "groups_feature_profile_supported",
  "missing_provider_fail_closed",
  "local_policy_compatibility_preserved",
  "owner_authorization_before_writes_preserved",
]) {
  if (contract.composition?.[key] !== true) {
    failures.push(`topic-create transport contract must record ${key}`);
  }
}
for (const key of [
  "migration_changed",
  "topic_create_dto_changed",
  "forum_groups_dependency_added",
  "trust_facts_adapter_added",
  "channel_membership_facts_adapter_added",
]) {
  if (contract.composition?.[key] !== false) {
    failures.push(`topic-create transport contract must keep ${key} false`);
  }
}

for (const marker of [
  "enum ForumTopicCreateTransport",
  "Graphql",
  "Rest",
  "topic_create_audience_port_context",
  "auth.tenant_id != tenant_id",
  "request.tenant_id != tenant_id",
  "request.user_id != Some(auth.user_id)",
  "auth.port_actor()",
  "with_deadline(FORUM_TOPIC_CREATE_FACTS_DEADLINE)",
  "context.with_claim(permission.to_string())",
  "request.channel_slug.as_deref()",
  "context.with_channel(channel_slug.to_string())",
  "forum-{}-topic-create-{}-{}",
]) {
  requireText(transport, marker, `shared topic-create transport context is missing ${marker}`);
}
for (const marker of [
  "exact_rest_context_uses_authenticated_identity_deadline_claims_and_channel",
  "graphql_context_without_http_request_uses_authenticated_tenant_and_fallback_locale",
  "mismatched_auth_request_tenant_or_user_fails_before_owner_facts",
  "context.deadline_ms, Some(5_000)",
  "context.channel.as_deref(), Some(\"members\")",
]) {
  requireText(transport, marker, `topic-create transport inline contract test is missing ${marker}`);
}

for (const marker of [
  "pub struct ForumGraphqlRuntimeData",
  "audience_facts: Option<SharedForumAudienceFactsPort>",
  "pub fn attach_schema_data(",
  "inputs.shared_get::<SharedForumAudienceFactsPort>()",
  "TopicService::with_audience_facts",
  "TopicService::new",
  "schema_factory_consumes_host_published_audience_facts_without_db_discovery",
  "schema_factory_preserves_optional_provider_absence",
]) {
  requireText(graphqlRuntime, marker, `Forum GraphQL runtime data is missing ${marker}`);
}
for (const marker of [
  "mod runtime_data;",
  "attach_schema_data",
  "ForumGraphqlRuntimeData",
]) {
  requireText(graphqlModule, marker, `Forum GraphQL module is missing ${marker}`);
}
requireText(
  manifest,
  'runtime_data_factory = "graphql::attach_schema_data"',
  "Forum manifest must declare the GraphQL runtime-data factory",
);

for (const [source, label, commandMarker] of [
  [graphqlLegacy, "legacy GraphQL topic-create", ".create_with_audience_context("],
  [graphqlCommand, "command GraphQL topic-create", ".create_command_with_audience_context("],
]) {
  for (const marker of [
    "ForumGraphqlRuntimeData",
    "topic_create_audience_port_context(",
    "ForumTopicCreateTransport::Graphql",
    "ctx.data_opt::<rustok_api::RequestContext>()",
    ".topic_service(db.clone(), event_bus.clone())",
    commandMarker,
  ]) {
    requireText(source, marker, `${label} is missing ${marker}`);
  }
}

for (const marker of [
  "audience_facts: Option<SharedForumAudienceFactsPort>",
  "runtime.shared_get::<SharedForumAudienceFactsPort>()",
  "fn topic_service(&self) -> crate::TopicService",
  "TopicService::with_audience_facts",
  "TopicService::new",
]) {
  requireText(httpRuntime, marker, `Forum HTTP runtime is missing ${marker}`);
}
for (const [source, label, commandMarker] of [
  [httpCommand, "command REST topic-create", ".create_command_with_audience_context("],
]) {
  for (const marker of [
    "request_context: RequestContext",
    ".topic_service()",
    "topic_create_audience_port_context(",
    "ForumTopicCreateTransport::Rest",
    commandMarker,
  ]) {
    requireText(source, marker, `${label} is missing ${marker}`);
  }
}

for (const marker of [
  "ServerForumAudienceGroupFactsPort::shared(",
  "extensions.insert(audience_facts)",
]) {
  requireText(hostPublication, marker, `server audience facts publication is missing ${marker}`);
}
requireText(
  hostGraphql,
  "runtime_extensions.apply_to_host_runtime(host_runtime)",
  "GraphQL host must transfer runtime extension values into GraphqlRuntimeInputs",
);
requireText(
  hostHttp,
  ".apply_to_host_runtime(runtime_ctx)",
  "HTTP module router host must transfer runtime extension values",
);
for (const marker of [
  "impl ForumAudienceFactsPort for ServerForumAudienceGroupFactsPort",
  "GroupMembershipEnforcementService::new(db)",
  "context.require_policy(PortCallPolicy::read())",
]) {
  requireText(adapter, marker, `server Groups facts adapter is missing ${marker}`);
}

const createHelper = between(
  owner,
  "async fn create_command_with_optional_audience_context(",
  "pub async fn resolve_canonical_topic(",
  "topic-create owner helper",
);
requireText(createHelper, ".create_with_audience_authorization(", "topic transport must delegate transactional audience admission");
requireText(createHelper, "&self.create_audience", "topic transport must retain the configured audience owner");
rejectText(createHelper, ".create_command(", "topic transport must not bypass configured admission");

const legacyInput = between(
  graphqlTypes,
  "pub struct CreateForumTopicInput",
  "pub struct UpdateForumTopicInput",
  "legacy GraphQL topic-create input",
);
const commandInput = between(
  graphqlCommand,
  "pub struct CreateForumTopicWithQuotesInput",
  "pub struct UpdateForumTopicWithQuotesInput",
  "command GraphQL topic-create input",
);
for (const [input, label] of [
  [legacyInput, "legacy GraphQL topic-create input"],
  [commandInput, "command GraphQL topic-create input"],
]) {
  for (const forbidden of ["user_id", "actor_id", "recipient_id", "request_tenant_id"]) {
    rejectText(input, forbidden, `${label} must not accept ${forbidden}`);
  }
}

for (const marker of ["### Selected hierarchy for create admission", "Rust tests remain uncompiled/unexecuted", "configured audience owner even when `PortContext` is absent"]) {
  requireText(note, marker, `create transport documentation is missing ${marker}`);
}
for (const marker of [
  "ForumGraphqlRuntimeData",
  "graphql::attach_schema_data",
  "GraphQL and REST topic-create",
]) {
  requireText(crateApi, marker, `Forum CRATE_API is missing ${marker}`);
}
assertCanonicalLedger({
  failures,
  plan,
  slices: ["FORUM-20AS"],
  markers: ["consume the existing feature-guarded Groups facts publication for both transports"],
  label: "canonical Forum plan",
});
const forum20DeliveryTail = between(
  plan,
  "### Delivered in `FORUM-20AR`",
  "### Compatibility and degraded mode",
  "FORUM-20AR/AS delivery tail",
);
requireText(
  forum20DeliveryTail,
  "### Delivered in `FORUM-20AS`",
  "FORUM-20AS delivered section must follow FORUM-20AR before compatibility",
);

if (failures.length > 0) {
  console.error("Forum topic-create audience transport composition verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}
console.log("Forum topic-create audience transport composition contract is source-ready.");
