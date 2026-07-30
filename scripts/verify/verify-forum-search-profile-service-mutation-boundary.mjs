#!/usr/bin/env node

import { existsSync, readFileSync, readdirSync, statSync } from "node:fs";
import path from "node:path";

const root = process.env.RUSTOK_VERIFY_REPO_ROOT
  ? path.resolve(process.env.RUSTOK_VERIFY_REPO_ROOT)
  : path.resolve(".");
const failures = [];

const contractPath =
  "crates/rustok-forum/contracts/forum-search-profile-service-mutation-boundary.json";
const notePath =
  "crates/rustok-forum/docs/forum-23a8-profile-service-mutation-boundary.md";
const servicesPath = "crates/rustok-profiles/src/services.rs";
const graphqlMutationPath = "crates/rustok-profiles/src/graphql/mutation.rs";
const cliBackfillPath = "crates/rustok-profiles/cli/src/lib.rs";

const directMethods = [
  "upsert_profile",
  "update_profile_handle",
  "update_profile_content",
  "update_profile_locale",
  "update_profile_visibility",
  "update_profile_media",
  "backfill_profile",
];
const safeRuntimeMarkers = [
  "upsert_profile_with_event(",
  "update_profile_handle_with_event(",
  "update_profile_content_with_event(",
  "update_profile_locale_with_event(",
  "update_profile_visibility_with_event(",
  "update_profile_media_with_event(",
  "backfill_profile_with_event(",
];
const skippedDirectoryNames = new Set([
  ".git",
  "target",
  "node_modules",
  "vendor",
  "output",
  "tests",
  "test",
  "fixtures",
  "examples",
  "benches",
]);

function read(relativePath) {
  const target = path.join(root, relativePath);
  if (!existsSync(target)) {
    failures.push(`${relativePath}: expected file is missing`);
    return "";
  }
  return readFileSync(target, "utf8");
}

function requireMarker(source, marker, label) {
  if (!source.includes(marker)) failures.push(`${label}: missing ${marker}`);
}

function rejectMarker(source, marker, label) {
  if (source.includes(marker)) failures.push(`${label}: forbidden ${marker}`);
}

function isTestLikeFile(relativePath) {
  const normalized = relativePath.split(path.sep).join("/");
  const segments = normalized.split("/");
  if (segments.some((segment) => skippedDirectoryNames.has(segment))) return true;
  const basename = path.posix.basename(normalized);
  return /(?:^|_)(?:test|tests)\.rs$/.test(basename);
}

function collectRustFiles(relativeRoot, output = []) {
  const absoluteRoot = path.join(root, relativeRoot);
  if (!existsSync(absoluteRoot)) return output;
  for (const entry of readdirSync(absoluteRoot)) {
    if (skippedDirectoryNames.has(entry)) continue;
    const relativePath = path.join(relativeRoot, entry);
    const absolutePath = path.join(root, relativePath);
    const stat = statSync(absolutePath);
    if (stat.isDirectory()) {
      collectRustFiles(relativePath, output);
    } else if (entry.endsWith(".rs") && !isTestLikeFile(relativePath)) {
      output.push(relativePath.split(path.sep).join("/"));
    }
  }
  return output;
}

function lineForOffset(source, offset) {
  return source.slice(0, offset).split("\n").length;
}

function findDirectCalls(source, method) {
  const patterns = [
    new RegExp(`\\.${method}\\s*\\(`, "g"),
    new RegExp(`\\bProfileService\\s*::\\s*${method}\\s*\\(`, "g"),
  ];
  const matches = [];
  for (const pattern of patterns) {
    for (const match of source.matchAll(pattern)) {
      matches.push({ offset: match.index ?? 0, text: match[0] });
    }
  }
  return matches;
}

const services = read(servicesPath);
const graphqlMutation = read(graphqlMutationPath);
const cliBackfill = read(cliBackfillPath);
const note = read(notePath);
let contract = null;
try {
  contract = JSON.parse(read(contractPath));
} catch (error) {
  failures.push(`${contractPath}: invalid JSON: ${error.message}`);
}

for (const method of directMethods) {
  requireMarker(services, `pub async fn ${method}(`, servicesPath);
}
for (const marker of safeRuntimeMarkers.slice(0, 6)) {
  requireMarker(graphqlMutation, marker, graphqlMutationPath);
}
requireMarker(cliBackfill, safeRuntimeMarkers[6], cliBackfillPath);
for (const method of directMethods) {
  rejectMarker(graphqlMutation, `.${method}(`, graphqlMutationPath);
  rejectMarker(cliBackfill, `.${method}(`, cliBackfillPath);
}

const productionRustFiles = [
  ...collectRustFiles("apps"),
  ...collectRustFiles("crates"),
];
for (const relativePath of productionRustFiles) {
  if (relativePath === servicesPath) continue;
  const source = readFileSync(path.join(root, relativePath), "utf8");
  for (const method of directMethods) {
    for (const match of findDirectCalls(source, method)) {
      failures.push(
        `${relativePath}:${lineForOffset(source, match.offset)}: direct non-event ProfileService call ${match.text.trim()}`,
      );
    }
  }
}

for (const marker of [
  "FORUM-23A8",
  "source-level production call-site gate",
  "does not make the methods compile-time private",
  "GraphQL self-service",
  "CLI backfill",
  "Not run by the implementation agent",
]) {
  requireMarker(note, marker, notePath);
}

if (contract) {
  if (contract.task !== "FORUM-23A8") failures.push(`${contractPath}: unexpected task`);
  if (contract.status !== "source_complete_execution_pending") {
    failures.push(`${contractPath}: unexpected status`);
  }
  if (contract.owner_definition_path !== servicesPath) {
    failures.push(`${contractPath}: unexpected owner definition path`);
  }
  if (contract.source_gate_verifier !== path.relative(root, import.meta.filename ?? "")) {
    // import.meta.filename is not available on every supported Node release, so the
    // canonical path is checked explicitly below instead of relying on this branch.
  }
  if (
    contract.source_gate_verifier !==
    "scripts/verify/verify-forum-search-profile-service-mutation-boundary.mjs"
  ) {
    failures.push(`${contractPath}: unexpected source gate verifier`);
  }
  if (JSON.stringify(contract.direct_non_event_methods) !== JSON.stringify(directMethods)) {
    failures.push(`${contractPath}: direct method inventory drift`);
  }
  for (const key of [
    "repository_production_call_sites_are_forbidden",
    "graphql_self_service_uses_transactional_helpers",
    "cli_backfill_uses_transactional_helper",
    "tests_may_use_direct_service_methods",
  ]) {
    if (contract.source_boundary?.[key] !== true) {
      failures.push(`${contractPath}: source boundary ${key} drift`);
    }
  }
  for (const key of [
    "direct_methods_are_compile_time_private",
    "external_downstream_repositories_are_scanned",
    "runtime_verification_was_executed",
  ]) {
    if (contract.non_claims?.[key] !== true) {
      failures.push(`${contractPath}: non-claim ${key} drift`);
    }
  }
  if (contract.verification?.execution_status !== "not_run_by_implementation_agent") {
    failures.push(`${contractPath}: execution status drift`);
  }
}

if (failures.length > 0) {
  console.error("Profiles direct mutation boundary verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("Profiles direct mutation boundary verification passed.");
