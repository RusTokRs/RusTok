#!/usr/bin/env node
// Social Graph owner-local receipt-cleanup CLI guardrails.

import { existsSync, readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = process.env.RUSTOK_VERIFY_REPO_ROOT
  ? path.resolve(process.env.RUSTOK_VERIFY_REPO_ROOT)
  : path.resolve(scriptDir, "../..");
const failures = [];

const paths = {
  rootCargo: "Cargo.toml",
  manifest: "crates/rustok-social-graph/rustok-module.toml",
  cargo: "crates/rustok-social-graph-cli/Cargo.toml",
  source: "crates/rustok-social-graph-cli/src/lib.rs",
  registryCargo: "crates/rustok-cli-registry/Cargo.toml",
  generated: "crates/rustok-cli-registry/src/generated.rs",
  docs: "crates/rustok-social-graph/docs/receipt-cleanup-cli.md",
};

function read(relativePath) {
  const absolutePath = path.join(repoRoot, relativePath);
  if (!existsSync(absolutePath)) {
    failures.push(`${relativePath}: expected file`);
    return "";
  }
  return readFileSync(absolutePath, "utf8");
}

function requireText(content, value, label) {
  if (!content.includes(value)) failures.push(`${label}: missing ${value}`);
}

function forbidText(content, value, label) {
  if (content.includes(value)) failures.push(`${label}: forbidden ${value}`);
}

const rootCargo = read(paths.rootCargo);
const manifest = read(paths.manifest);
const cargo = read(paths.cargo);
const source = read(paths.source);
const registryCargo = read(paths.registryCargo);
const generated = read(paths.generated);
const docs = read(paths.docs);

requireText(
  rootCargo,
  'rustok-social-graph-cli = { path = "crates/rustok-social-graph-cli" }',
  "workspace CLI dependency",
);
requireText(rootCargo, 'rustok-social-graph = { path = "crates/rustok-social-graph" }', "workspace owner dependency");
requireText(manifest, "[provides.cli]", "module CLI declaration");
requireText(manifest, 'namespace = "social_graph"', "module CLI namespace");
requireText(
  manifest,
  'factory = "rustok_social_graph_cli::command_provider"',
  "module CLI factory",
);
requireText(cargo, 'name = "rustok-social-graph-cli"', "owner CLI crate");
requireText(cargo, "rustok-social-graph.workspace = true", "owner crate dependency");
requireText(registryCargo, "rustok-social-graph-cli.workspace = true", "selected registry dependency");
requireText(generated, "rustok_social_graph_cli::command_provider(runtime)", "generated provider wiring");

for (const marker of [
  '"social_graph"',
  '"receipt-cleanup"',
  ".with_dry_run()",
  'required_u32(options, "retention_days")',
  "ChronoDuration::try_days",
  "retention_duration(u32::MAX).is_err()",
  "DEFAULT_CLEANUP_LIMIT: u32 = 100",
  "MAX_SOCIAL_GRAPH_RECEIPT_CLEANUP_BATCH",
  "SocialGraphReceiptMaintenanceService::new",
  "SocialGraphReceiptMaintenancePort::cleanup_completed_receipts",
  "PortActor::system()",
  ".with_deadline(CLEANUP_DEADLINE)",
  ".with_idempotency_key(format!(",
  "matched_receipts",
  "deleted_receipts",
  "oldest_retained_completed_at_unix_seconds",
]) {
  requireText(source, marker, "receipt-cleanup CLI contract");
}

for (const forbidden of [
  "social_graph_command_receipts",
  "request_json",
  "response_json",
  "ChronoDuration::days(",
  'unwrap_or("30")',
  "DEFAULT_RETENTION",
  "tokio::spawn",
]) {
  forbidText(source, forbidden, "owner CLI boundary");
}

for (const marker of [
  "`--retention-days` is mandatory",
  "no deployment retention default",
  "no scheduler or automatic cleanup",
  "Application rollback keeps the receipt table and rows",
]) {
  requireText(docs, marker, "receipt-cleanup operating contract");
}

if (failures.length > 0) {
  console.error("Social Graph receipt-cleanup CLI verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("Social Graph receipt-cleanup CLI verification passed");
