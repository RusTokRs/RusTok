#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..", "..", "..");
const failures = [];
const files = {
  limits: "crates/rustok-page-builder/src/static_publish_resource_limits.rs",
  sanitization: "crates/rustok-page-builder/src/publish_sanitization.rs",
  staticLanding: "crates/rustok-page-builder/src/static_landing.rs",
  runtimeContract:
    "crates/rustok-page-builder/contracts/page-builder-publish-runtime-review.json",
  evidence:
    "crates/rustok-page-builder/contracts/evidence/page-builder-static-publish-resource-limits-source.json",
  packet: "crates/rustok-page-builder/docs/static-publish-resource-limits.md",
  actualization: "docs/modules/page-builder-parity-actualization-2026-08-05.md",
};

const absolute = (relativePath) => path.join(repoRoot, relativePath);
const read = (relativePath) => fs.readFileSync(absolute(relativePath), "utf8");
const need = (source, marker, label) => {
  if (!source.includes(marker)) failures.push(`${label}: missing ${marker}`);
};
const forbid = (source, marker, label) => {
  if (source.includes(marker)) failures.push(`${label}: forbidden ${marker}`);
};
const sliceBetween = (source, start, end, label) => {
  const startIndex = source.indexOf(start);
  if (startIndex < 0) {
    failures.push(`${label}: missing ${start}`);
    return "";
  }
  const endIndex = source.indexOf(end, startIndex + start.length);
  if (endIndex < 0) {
    failures.push(`${label}: missing ${end}`);
    return "";
  }
  return source.slice(startIndex, endIndex);
};
const requireOrdered = (source, markers, label) => {
  let previous = -1;
  for (const marker of markers) {
    const index = source.indexOf(marker, previous + 1);
    if (index < 0) {
      failures.push(`${label}: missing or out of order ${marker}`);
      return;
    }
    previous = index;
  }
};

for (const [label, relativePath] of Object.entries(files)) {
  if (!fs.existsSync(absolute(relativePath))) {
    failures.push(`${label}: missing ${relativePath}`);
    continue;
  }
  const stats = fs.lstatSync(absolute(relativePath));
  if (!stats.isFile() || stats.isSymbolicLink()) {
    failures.push(`${label}: ${relativePath} must be a regular non-symlink file`);
  }
}
if (failures.length > 0) {
  console.error("[verify-page-builder-static-publish-resource-limits] FAIL");
  failures.forEach((failure) => console.error(`- ${failure}`));
  process.exit(1);
}

const sources = Object.fromEntries(
  Object.entries(files).map(([label, relativePath]) => [label, read(relativePath)]),
);
const evidence = JSON.parse(sources.evidence);
const runtimeContract = JSON.parse(sources.runtimeContract);

if (evidence.format !== "page_builder_static_publish_resource_limits_source_v1") {
  failures.push("source evidence format drifted");
}
if (evidence.status !== "page_builder_static_publish_resource_limits_source_unvalidated") {
  failures.push("source evidence status drifted");
}
if (!Array.isArray(evidence.execution) || evidence.execution.length !== 0) {
  failures.push("source evidence execution must remain empty");
}
for (const [key, value] of Object.entries(evidence.validation ?? {})) {
  if (value !== false) failures.push(`source evidence validation.${key} must remain false`);
}

const expectedLimits = {
  max_project_bytes: 16 * 1024 * 1024,
  max_pages: 128,
  max_components: 50_000,
  max_component_depth: 128,
  max_assets: 4_096,
  max_style_rules: 20_000,
};
for (const [key, value] of Object.entries(expectedLimits)) {
  if (evidence.source_contract?.[key] !== value) {
    failures.push(`source_contract.${key} drifted`);
  }
}
for (const key of [
  "sanitization_hash_payload_unchanged",
  "limits_are_positive_and_policy_hashed",
  "project_bytes_use_a_bounded_streaming_counter",
  "component_count_and_depth_use_a_bounded_iterative_current_tree_scan",
  "page_asset_and_style_counts_are_bounded",
  "limit_rejections_are_typed_and_path_bound",
  "compiler_checks_limits_before_stable_id_and_recursive_policy_traversal",
  "compiler_rechecks_limits_after_stable_id_normalization",
  "compiler_rechecks_limits_on_exact_materialized_document",
  "resource_validation_runs_before_sanitized_project_hashing",
  "resource_validation_is_repeated_during_integrity_verification",
  "reviewed_publish_calls_resource_validation_before_materialization",
  "pages_sanitized_set_hash_contract_is_unchanged",
  "database_graphql_rest_event_and_artifact_schemas_are_unchanged",
  "anonymous_rendering_persistence_and_public_routes_are_unchanged",
]) {
  if (evidence.source_contract?.[key] !== true) {
    failures.push(`source_contract.${key} must be true`);
  }
}
for (const key of [
  "tests_run",
  "static_verifiers_run",
  "cargo_run",
  "formatting_run",
  "publish_or_materialization_run",
  "database_or_http_run",
  "workflows_or_ci_run",
]) {
  if (evidence.source_contract?.[key] !== false) {
    failures.push(`source_contract.${key} must remain false`);
  }
}

if (
  evidence.source_contract?.sanitization_format !==
    "page_builder_static_publish_sanitization_v2"
) {
  failures.push("source evidence sanitization format drifted");
}
if (
  runtimeContract.provider?.sanitization?.format !==
    "page_builder_static_publish_sanitization_v2" ||
  JSON.stringify(runtimeContract.provider?.sanitization?.hash_payload) !==
    JSON.stringify(["format", "policy_format", "policy_hash", "sanitized_project"])
) {
  failures.push("existing publish runtime sanitization identity contract drifted");
}

for (const marker of [
  '"page_builder_static_publish_resource_limits_v1"',
  "max_project_bytes: 16 * 1024 * 1024",
  "max_pages: 128",
  "max_components: 50_000",
  "max_component_depth: 128",
  "max_assets: 4_096",
  "max_style_rules: 20_000",
  "serde_json::to_writer(&mut counter, &document.project)",
  "BoundedByteCounter::new(maximum)",
  "document.project.pages.len()",
  "document.project.assets.len()",
  "document.project.styles.len()",
  "component_observation(document, &limits)",
  "ComponentChildren::Nodes(children)",
  "depth.saturating_add(1)",
  "component_count > limits.max_components",
  "max_component_depth > limits.max_component_depth",
  '"landing_project_bytes_exceeded"',
  '"landing_page_count_exceeded"',
  '"landing_component_count_exceeded"',
  '"landing_component_depth_exceeded"',
  '"landing_asset_count_exceeded"',
  '"landing_style_rule_count_exceeded"',
  "limits_hash: limits.limits_hash()?",
  "resource_limits_reject_excess_pages",
  "resource_limits_reject_excess_component_depth",
]) need(sources.limits, marker, "resource-limit source");

const boundedWriter = sliceBetween(
  sources.limits,
  "impl Write for BoundedByteCounter",
  "fn component_observation",
  "bounded byte counter write boundary",
);
requireOrdered(
  boundedWriter,
  [
    "let next = self.bytes.saturating_add(buffer.len());",
    "if next > self.maximum",
    "self.exceeded = true;",
    "return Err(io::Error::other(",
    '"static publish project byte limit exceeded"',
  ],
  "bounded byte counter rejection",
);

const stableEvidenceTest = sliceBetween(
  sources.limits,
  "fn resource_evidence_is_stable_and_policy_bound",
  "fn resource_limits_reject_excess_pages",
  "resource evidence integrity test",
);
requireOrdered(
  stableEvidenceTest,
  [
    "let first = validate_static_publish_resource_limits(&document)",
    "let second = validate_static_publish_resource_limits(&document)",
    "assert_eq!(first, second);",
    ".verify_integrity()",
  ],
  "resource evidence integrity verification",
);

for (const marker of [
  '"page_builder_static_publish_sanitization_v2"',
  "static_publish_resource_limits::{",
  "PageBuilderStaticPublishResourceLimitError",
  "validate_static_publish_resource_limits(&document)?",
  "sanitization_hash(&sanitized_project, &policy_format, &policy_hash)?",
  "result.verify_integrity()?",
  "sanitization_rejects_excess_global_resources",
]) need(sources.sanitization, marker, "sanitization source");
for (const marker of [
  "page_builder_static_publish_sanitization_v3",
  "pub resource_limits:",
  "resource_limits: Some(",
  '#[path = "static_publish_resource_limits.rs"]',
]) forbid(sources.sanitization, marker, "unchanged sanitization identity and compiler ownership");

const sanitizeFunction = sliceBetween(
  sources.sanitization,
  "pub fn sanitize_static_landing_project",
  "fn sanitization_hash",
  "reviewed sanitization function",
);
requireOrdered(
  sanitizeFunction,
  [
    "validate_static_publish_document(&document)?",
    "validate_static_publish_resource_limits(&document)?",
    "sanitization_hash(&sanitized_project, &policy_format, &policy_hash)?",
  ],
  "reviewed sanitization order",
);

const integrityFunction = sliceBetween(
  sources.sanitization,
  "pub fn verify_integrity",
  "#[derive(Debug, thiserror::Error)]",
  "sanitization integrity function",
);
if (
  integrityFunction.indexOf("validate_static_publish_document(&document)?") < 0 ||
  integrityFunction.indexOf("validate_static_publish_resource_limits(&document)?") < 0
) {
  failures.push("sanitization integrity must repeat policy and resource validation");
}

for (const marker of [
  '#[path = "static_publish_resource_limits.rs"]',
  "pub mod static_publish_resource_limits;",
  "PageBuilderStaticPublishResourceLimitError",
  "require_static_publish_resource_limits(&document)?",
  "require_static_publish_resource_limits(document)?",
  '"landing_static_publish_resource_limits_integrity"',
  "compiler_rechecks_materialized_resource_limits_before_recursive_policy",
]) need(sources.staticLanding, marker, "static landing compiler");

const prepareDocument = sliceBetween(
  sources.staticLanding,
  "pub(crate) fn prepare_document",
  "pub(crate) fn compile_prepared_document",
  "static landing prepare checkpoint",
);
requireOrdered(
  prepareDocument,
  [
    "let mut document = inspection.document().clone();",
    "require_static_publish_resource_limits(&document)?;",
    "document.ensure_stable_ids",
    "require_static_publish_resource_limits(&document)?;",
    "require_secure_resource_urls(&document)?;",
    "require_static_publish_policy(&document)?;",
  ],
  "static landing prepared policy order",
);

const compilePrepared = sliceBetween(
  sources.staticLanding,
  "pub(crate) fn compile_prepared_document",
  "pub(crate) fn render_policy",
  "exact materialized compiler checkpoint",
);
requireOrdered(
  compilePrepared,
  [
    "require_static_publish_resource_limits(document)?;",
    "require_secure_resource_urls(document)?;",
    "require_static_publish_policy(document)?;",
    "build_static_landing_artifact_with_renderer",
  ],
  "exact materialized policy order",
);

for (const marker of [
  "max_project_bytes: usize::MAX",
  "max_pages: usize::MAX",
  "max_components: usize::MAX",
  "max_component_depth: usize::MAX",
]) forbid(sources.limits, marker, "fail-closed resource boundary");

for (const marker of [
  "source-ready / maintainer-validation-pending",
  "serialized prepared project: 16 MiB",
  "component nodes: 50,000",
  "page_builder_static_publish_sanitization_v2",
  "Its SHA-256 payload remains exactly",
  "No new persisted DTO",
  "No raw runtime context",
  "intentionally not run",
]) need(sources.packet, marker, "resource-limit packet");

for (const marker of [
  "Reviewed publish resource limits",
  "static-publish-resource-limits-source-ready",
  "sanitization identity remains exactly `page_builder_static_publish_sanitization_v2`",
  "execution and rollout remain open",
]) need(sources.actualization, marker, "parity actualization");

if (failures.length > 0) {
  console.error("[verify-page-builder-static-publish-resource-limits] FAIL");
  failures.forEach((failure) => console.error(`- ${failure}`));
  process.exit(Math.min(failures.length, 255));
}
console.log(
  "[verify-page-builder-static-publish-resource-limits] PASS source_ready=true execution=pending",
);
