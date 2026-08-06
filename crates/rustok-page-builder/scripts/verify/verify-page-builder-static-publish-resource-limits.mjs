#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..", "..", "..");
const failures = [];
const files = {
  limits: "crates/rustok-page-builder/src/static_publish_resource_limits.rs",
  sanitization: "crates/rustok-page-builder/src/publish_sanitization.rs",
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
  "limits_are_positive_and_policy_hashed",
  "project_bytes_are_measured_from_the_prepared_project",
  "component_count_and_depth_use_the_current_pages_component_authority",
  "page_asset_and_style_counts_are_bounded",
  "limit_rejections_are_typed_and_path_bound",
  "resource_evidence_is_bound_into_the_v3_sanitization_hash",
  "resource_evidence_is_recomputed_during_integrity_verification",
  "legacy_v2_hash_and_integrity_remain_supported",
  "legacy_v2_does_not_gain_retroactive_resource_evidence",
  "reviewed_publish_calls_resource_validation_before_materialization",
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

for (const marker of [
  '"page_builder_static_publish_resource_limits_v1"',
  "max_project_bytes: 16 * 1024 * 1024",
  "max_pages: 128",
  "max_components: 50_000",
  "max_component_depth: 128",
  "max_assets: 4_096",
  "max_style_rules: 20_000",
  "serde_json::to_vec(&document.project)",
  "document.project.pages.len()",
  "document.project.assets.len()",
  "document.project.styles.len()",
  "component_observation(document)",
  "ComponentChildren::Nodes(children)",
  "depth.saturating_add(1)",
  '"landing_project_bytes_exceeded"',
  '"landing_page_count_exceeded"',
  '"landing_component_count_exceeded"',
  '"landing_component_depth_exceeded"',
  '"landing_asset_count_exceeded"',
  '"landing_style_rule_count_exceeded"',
  "limits_hash: limits.limits_hash()?",
  "first.verify_integrity()",
  "resource_limits_reject_excess_pages",
  "resource_limits_reject_excess_component_depth",
]) need(sources.limits, marker, "resource-limit source");

for (const marker of [
  '"page_builder_static_publish_sanitization_v3"',
  '"page_builder_static_publish_sanitization_v2"',
  '#[path = "static_publish_resource_limits.rs"]',
  "validate_static_publish_resource_limits(&document)?",
  "pub resource_limits: Option<PageBuilderStaticPublishResourceEvidence>",
  "resource evidence mismatch",
  "current sanitization hash is missing resource limits",
  "legacy sanitization hash must not include resource limits",
  "PAGE_BUILDER_STATIC_SANITIZATION_FORMAT,",
  "resource_limits.ok_or_else",
  "legacy_v2_sanitization_remains_verifiable",
]) need(sources.sanitization, marker, "sanitization source");

const policyIndex = sources.sanitization.indexOf("validate_static_publish_document(&document)?");
const resourceIndex = sources.sanitization.indexOf("validate_static_publish_resource_limits(&document)?");
const materializationBoundary = sources.sanitization.indexOf("serde_json::to_value(document.project)");
if (
  policyIndex < 0 ||
  resourceIndex < 0 ||
  materializationBoundary < 0 ||
  !(policyIndex < resourceIndex && resourceIndex < materializationBoundary)
) {
  failures.push("reviewed sanitization order must be policy -> resource limits -> sanitized project");
}

for (const marker of [
  "max_project_bytes: usize::MAX",
  "max_pages: usize::MAX",
  "max_components: usize::MAX",
  "max_component_depth: usize::MAX",
  "resource_limits: None,\n        sanitized_project",
]) forbid(sources.limits + sources.sanitization, marker, "fail-closed resource boundary");

for (const marker of [
  "source-ready / maintainer-validation-pending",
  "serialized prepared project: 16 MiB",
  "component nodes: 50,000",
  "page_builder_static_publish_sanitization_v3",
  "page_builder_static_publish_sanitization_v2",
  "do not receive retroactive rejection",
  "No raw runtime context",
  "intentionally not run",
]) need(sources.packet, marker, "resource-limit packet");

for (const marker of [
  "Reviewed publish resource limits",
  "static-publish-resource-limits-source-ready",
  "legacy sanitization v2 remains verifiable",
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
