#!/usr/bin/env node

import { existsSync, readFileSync } from "node:fs";
import path from "node:path";

const repoRoot = process.env.RUSTOK_VERIFY_REPO_ROOT
  ? path.resolve(process.env.RUSTOK_VERIFY_REPO_ROOT)
  : path.resolve(".");
const failures = [];

function repoPath(relativePath) {
  return path.join(repoRoot, relativePath);
}

function read(relativePath) {
  const target = repoPath(relativePath);
  if (!existsSync(target)) {
    failures.push(`${relativePath}: expected file is missing`);
    return "";
  }
  return readFileSync(target, "utf8");
}

function json(relativePath) {
  try {
    return JSON.parse(read(relativePath));
  } catch (error) {
    failures.push(`${relativePath}: invalid JSON: ${error.message}`);
    return null;
  }
}

function requireMarker(source, marker, label) {
  if (!source.includes(marker)) failures.push(`${label}: missing ${marker}`);
}

function requireNoMarker(source, marker, label) {
  if (source.includes(marker)) failures.push(`${label}: forbidden ${marker}`);
}

function sameSet(actual, expected) {
  return [...actual].sort().join("|") === [...expected].sort().join("|");
}

const registryPath = "crates/rustok-comments/contracts/comments-fba-registry.json";
const evidencePath =
  "crates/rustok-comments/contracts/evidence/comments-contract-test-static-matrix.json";
const sharedPolicyPath = "crates/rustok-api/src/ports.rs";
const providerPath = "crates/rustok-comments/src/ports.rs";
const publicReadPath = "crates/rustok-comments/src/public_read.rs";
const dtoPath = "crates/rustok-comments/src/dto.rs";
const richtextPath = "crates/rustok-comments/src/richtext.rs";
const planPath = "crates/rustok-comments/docs/implementation-plan.md";
const packagePath = "package.json";
const verifierPath = "scripts/verify/verify-comments-port-boundary.mjs";
const selfTestPath = "scripts/verify/verify-comments-port-boundary.test.mjs";

const expectedOperations = [
  "create_comment",
  "get_comment",
  "list_comments_for_target",
  "list_public_comments_for_target",
  "update_comment",
  "set_comment_status",
  "delete_comment",
];
const writeOperations = [
  "create_comment",
  "update_comment",
  "set_comment_status",
  "delete_comment",
];
const readOperations = [
  "get_comment",
  "list_comments_for_target",
  "list_public_comments_for_target",
];

const registry = json(registryPath);
const evidence = json(evidencePath);
const packageJson = json(packagePath);
const sharedPolicy = read(sharedPolicyPath);
const provider = read(providerPath);
const publicRead = read(publicReadPath);
const dto = read(dtoPath);
const richtext = read(richtextPath);
const plan = read(planPath);

if (evidence) {
  if (evidence.schema_version !== 2) failures.push(`${evidencePath}: schema_version drift`);
  if (
    evidence.module !== "comments" ||
    evidence.surface !== "comments_thread_port_boundary" ||
    evidence.role !== "provider"
  ) failures.push(`${evidencePath}: identity drift`);
  if (evidence.status !== "source_verified_no_compile") failures.push(`${evidencePath}: status drift`);
  if (evidence.compile_policy !== "not_run_by_request" || evidence.runtime_status !== "pending") {
    failures.push(`${evidencePath}: execution policy drift`);
  }
  const source = evidence.source_contract ?? {};
  for (const [key, expected] of Object.entries({
    shared_port_policy: sharedPolicyPath,
    provider_port: providerPath,
    public_projection: publicReadPath,
    dto: dtoPath,
    richtext: richtextPath,
    provider_registry: registryPath,
  })) {
    if (source[key] !== expected) failures.push(`${evidencePath}: ${key} drift`);
  }
  if (!sameSet(evidence.profiles?.source_verified ?? [], ["in_process"])) {
    failures.push(`${evidencePath}: source-verified profile drift`);
  }
  if (!sameSet(evidence.profiles?.pending ?? [], ["remote_adapter_placeholder"])) {
    failures.push(`${evidencePath}: pending profile drift`);
  }
  if (!sameSet((evidence.cases ?? []).map((entry) => entry.operation), expectedOperations)) {
    failures.push(`${evidencePath}: operation set drift`);
  }
  for (const entry of evidence.cases ?? []) {
    if (entry.runtime_evidence !== "pending") {
      failures.push(`${evidencePath}: ${entry.operation} runtime status drift`);
    }
  }
  if (
    evidence.fallback_smoke?.status !== "planned" ||
    evidence.fallback_smoke?.runtime_evidence !== "pending"
  ) failures.push(`${evidencePath}: fallback status drift`);
}

if (registry) {
  if (registry.schema_version !== 4) failures.push(`${registryPath}: schema_version drift`);
  if (
    registry.module !== "comments" ||
    registry.role !== "provider" ||
    registry.contract_version !== "comments.thread.v1"
  ) failures.push(`${registryPath}: identity drift`);
  const port = registry.ports?.find((entry) => entry.name === "CommentsThreadPort");
  if (!port || !sameSet(port.operations ?? [], expectedOperations)) {
    failures.push(`${registryPath}: port operation drift`);
  }
  if (!sameSet(port?.write_operations ?? [], writeOperations)) {
    failures.push(`${registryPath}: write operation drift`);
  }
  if (!sameSet(port?.read_operations ?? [], readOperations)) {
    failures.push(`${registryPath}: read operation drift`);
  }
  const contractTests = registry.contract_tests ?? {};
  if (
    contractTests.status !== "source_verified_no_compile" ||
    contractTests.runtime_status !== "pending" ||
    contractTests.runner !== verifierPath
  ) failures.push(`${registryPath}: contract-test status/runner drift`);
  if (!sameSet(contractTests.source_profiles ?? [], ["in_process"])) {
    failures.push(`${registryPath}: source profile drift`);
  }
  if (!sameSet(contractTests.pending_profiles ?? [], ["remote_adapter_placeholder"])) {
    failures.push(`${registryPath}: pending profile drift`);
  }
  if (!sameSet((contractTests.cases ?? []).map((entry) => entry.operation), expectedOperations)) {
    failures.push(`${registryPath}: contract-test operation drift`);
  }
  const gate = registry.verification_chain?.source_gates?.comments_port_boundary;
  if (
    gate?.package_script !== "verify:comments:port-boundary" ||
    gate?.test_package_script !== "test:verify:comments:port-boundary" ||
    gate?.verifier !== verifierPath ||
    gate?.self_test !== selfTestPath ||
    gate?.evidence !== evidencePath
  ) failures.push(`${registryPath}: source gate drift`);
}

if (
  packageJson?.scripts?.["verify:comments:port-boundary"] !== `node ${verifierPath}` ||
  packageJson?.scripts?.["test:verify:comments:port-boundary"] !== `node ${selfTestPath}`
) failures.push(`${packagePath}: comments port leaf command drift`);

for (const marker of [
  "pub const fn read() -> Self",
  "requires_deadline: true",
  "requires_idempotency_key: false",
  "pub const fn write() -> Self",
  "requires_idempotency_key: true",
  "self.require_write_semantics()",
  "self.require_read_semantics()",
]) requireMarker(sharedPolicy, marker, sharedPolicyPath);

for (const marker of [
  "pub trait CommentsThreadPort: Send + Sync",
  "struct InProcessCommentsThreadProvider",
  "pub fn in_process_comments_thread_port(",
  "CommentsService::with_event_bus(db.clone(), event_bus)",
  "impl CommentsThreadPort for InProcessCommentsThreadProvider",
  "fn parse_tenant_id(context: &PortContext)",
  "fn comments_error_to_port_error(error: CommentsError)",
  "CommentsError::Database(source)",
  "PortError::unavailable(\"comments.database\"",
  "CommentsError::EventPublication(message)",
  "PortErrorKind::NotFound",
  "PortErrorKind::Conflict",
  "PortErrorKind::Forbidden",
  "PortError::validation(\"comments.validation\"",
]) requireMarker(provider, marker, providerPath);

for (const operation of expectedOperations) {
  requireMarker(provider, `async fn ${operation}(`, `${providerPath}:${operation}`);
}

const implStart = provider.indexOf("impl CommentsThreadPort for InProcessCommentsThreadProvider");
if (implStart === -1) {
  failures.push(`${providerPath}: missing in-process implementation`);
} else {
  const implementation = provider.slice(implStart);
  for (const operation of writeOperations) {
    const start = implementation.indexOf(`async fn ${operation}(`);
    const next = implementation.indexOf("\n    async fn ", start + 1);
    const body = implementation.slice(start, next === -1 ? implementation.length : next);
    requireMarker(body, "context.require_policy(PortCallPolicy::write())?", `${providerPath}:${operation}`);
    requireMarker(body, ".map_err(comments_error_to_port_error)", `${providerPath}:${operation}`);
  }
  for (const operation of readOperations) {
    const start = implementation.indexOf(`async fn ${operation}(`);
    const next = implementation.indexOf("\n    async fn ", start + 1);
    const body = implementation.slice(start, next === -1 ? implementation.length : next);
    requireMarker(body, "context.require_policy(PortCallPolicy::read())?", `${providerPath}:${operation}`);
    requireMarker(body, ".map_err(comments_error_to_port_error)", `${providerPath}:${operation}`);
  }
}

requireMarker(
  provider,
  "crate::public_read::list_public_comments_for_target(",
  `${providerPath}:public projection`,
);
requireNoMarker(
  provider,
  "list_public_comments_for_target(\n        &self,\n        context: PortContext,\n        target_type: String,\n        target_id: Uuid,\n        filter: ListCommentsFilter,\n        fallback_locale: Option<String>,\n    ) -> Result<(Vec<CommentListItem>, u64), PortError> {\n        self.list_comments_for_target",
  `${providerPath}:public projection`,
);

for (const marker of [
  "comment_thread::Column::TenantId.eq(tenant_id)",
  "comment_thread::Column::TargetType.eq(target_type)",
  "comment_thread::Column::TargetId.eq(target_id)",
  "comment::Column::TenantId.eq(tenant_id)",
  "comment::Column::DeletedAt.is_null()",
  "comment::Column::Status.eq(CommentStatus::Approved)",
  "filter.per_page.clamp(1, MAX_PUBLIC_COMMENTS_PER_PAGE)",
  "project_comment_body(&resolved.body)?",
  "projection.plain_text.chars().take(200)",
]) requireMarker(publicRead, marker, publicReadPath);

for (const marker of [
  "pub body: RichTextDocument",
  "pub body: Option<RichTextDocument>",
  "pub body: RichTextView",
  "pub body_text: String",
  "pub body_preview: String",
]) requireMarker(dto, marker, dtoPath);
requireNoMarker(dto, "body_format", dtoPath);
requireNoMarker(dto, "content_json", dtoPath);

for (const marker of [
  "serialize_comment_body(document: RichTextDocument)",
  "validate_and_normalize(document, RichTextProfile::Comment)",
  "project_comment_body(raw: &str)",
  "parse_json(raw, RichTextProfile::Comment)",
  "plain_text(&document, RichTextProfile::Comment)",
  "project(&document, RichTextProfile::Comment)",
]) requireMarker(richtext, marker, richtextPath);

for (const marker of [
  "comments-contract-test-static-matrix.json",
  "verify:comments:port-boundary",
  "test:verify:comments:port-boundary",
  "source_verified_no_compile",
  "remote adapter remains pending",
]) requireMarker(plan, marker, planPath);

if (failures.length > 0) {
  console.error("Comments provider port boundary verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("Comments provider port source boundary is consistent");
