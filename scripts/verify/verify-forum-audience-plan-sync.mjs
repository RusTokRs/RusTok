#!/usr/bin/env node

import { existsSync, readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = process.env.RUSTOK_VERIFY_REPO_ROOT
  ? path.resolve(process.env.RUSTOK_VERIFY_REPO_ROOT)
  : path.resolve(scriptDir, "../..");
const failures = [];

function read(relativePath) {
  const absolute = path.join(repoRoot, relativePath ?? "");
  if (!relativePath || !existsSync(absolute)) {
    failures.push(`${relativePath || "<missing path>"}: required file is missing`);
    return "";
  }
  return readFileSync(absolute, "utf8");
}

function requireText(source, marker, message) {
  if (!source.includes(marker)) failures.push(message);
}

function rejectText(source, marker, message) {
  if (source.includes(marker)) failures.push(message);
}

const contractPath =
  "crates/rustok-forum/contracts/forum-audience-plan-sync.json";
const contract = JSON.parse(read(contractPath) || "{}");
const plan = read(contract.canonical_plan);
const crateApi = read(contract.crate_api);
const ownerNote = read(contract.owner_note);
const upstream = read(contract.upstream_contract);
const taskNotes = (contract.delivered_task_notes ?? []).map((file) => ({
  file,
  source: read(file),
}));
const taskContracts = (contract.delivered_task_contracts ?? []).map((file) => ({
  file,
  source: read(file),
}));

if (
  contract.schema_version !== 1 ||
  contract.task !== "FORUM-20BA" ||
  contract.upstream_task !== "FORUM-20AZ" ||
  contract.required_ledger_through !== "FORUM-20BA"
) {
  failures.push("FORUM-20BA synchronization contract identity is invalid");
}

if (taskNotes.length !== 5 || taskContracts.length !== 5) {
  failures.push("FORUM-20BA must bind exactly five delivered task notes and contracts");
}

for (const marker of [
  "FORUM-20A-AZ",
  "### Delivered in `FORUM-20AV`",
  "### Delivered in `FORUM-20AW`",
  "### Delivered in `FORUM-20AX`",
  "### Delivered in `FORUM-20AY`",
  "### Delivered in `FORUM-20AZ`",
  "### Delivered in `FORUM-20BA`",
  "verify-forum-audience-plan-sync.mjs",
  "verify-forum-reply-create-audience-enforcement.mjs",
  "verify-forum-reply-create-audience-transport-composition.mjs",
  "verify-forum-topic-reply-create-audience-policy.mjs",
  "verify-forum-moderation-audience-policy.mjs",
  "verify-forum-moderation-audience-transport-composition.mjs",
]) {
  requireText(plan, marker, `canonical Forum plan is missing ${marker}`);
}

for (const stale of [
  "reply-create command-time enforcement and optional\n    topic-local narrowing, then add moderation audiences",
  "enforce the inherited category reply-create audience before every public reply owner write",
  "add moderation audience policy persistence and enforcement plus owner write commands",
  "FORUM-20C-AU read-composition",
]) {
  rejectText(plan, stale, `canonical Forum plan retains stale scope: ${stale}`);
}

for (const marker of [
  "ForumCategoryReplyCreateAudiencePolicyService",
  "ForumTopicReplyCreateAudiencePolicyService",
  "ForumCategoryModerationAudiencePolicyService",
  "FORUM-20AZ",
]) {
  requireText(crateApi, marker, `CRATE_API is missing delivered audience marker ${marker}`);
}

for (const marker of [
  "FORUM-20AV",
  "FORUM-20AW",
  "FORUM-20AX",
  "FORUM-20AY",
  "FORUM-20AZ",
  "ForumUserTrustAudienceFactsPort",
  "No new moderation route is claimed",
  "not run by\nthe implementation agent",
]) {
  requireText(ownerNote, marker, `FORUM-20BA owner note is missing ${marker}`);
}

for (const { file, source } of taskNotes) {
  requireText(
    source,
    "Resolved by `FORUM-20BA`",
    `${file}: canonical plan debt is not marked resolved by FORUM-20BA`,
  );
  rejectText(source, "Forum trust remains unavailable", `${file}: stale unavailable trust claim`);
  rejectText(source, "Trust remains blocked on `FORUM-26`", `${file}: stale blocked trust claim`);
  rejectText(
    source,
    "A later safe repository-local edit must advance",
    `${file}: stale future plan-sync instruction`,
  );
}

for (const { file, source } of taskContracts) {
  requireText(source, '"task": "FORUM-20', `${file}: task identity is missing`);
}

for (const marker of [
  '"downstream_plan_sync_task": "FORUM-20BA"',
  '"downstream_plan_sync_contract": "crates/rustok-forum/contracts/forum-audience-plan-sync.json"',
]) {
  requireText(upstream, marker, `FORUM-20AZ handoff is missing ${marker}`);
}

for (const [key, expected] of [
  ["canonical_ledger_advanced_through_forum_20az", true],
  ["documentation_sync_recorded_as_forum_20ba", true],
  ["reply_create_enforcement_removed_from_remaining_scope", true],
  ["topic_local_reply_narrowing_removed_from_remaining_scope", true],
  ["category_moderation_audience_removed_from_remaining_scope", true],
  ["authoritative_forum_trust_status_corrected", true],
  ["owner_note_plan_debt_marked_resolved", true],
  ["latest_handoff_updated", true],
  ["runtime_behavior_changed", false],
  ["migration_changed", false],
  ["dependency_changed", false],
  ["public_contract_changed", false],
]) {
  if (contract.synchronization?.[key] !== expected) {
    failures.push(`FORUM-20BA synchronization.${key} must be ${expected}`);
  }
}

if (contract.verification?.execution_status !== "not_run_by_implementation_agent") {
  failures.push("FORUM-20BA must not claim maintainer runtime execution");
}

if (failures.length > 0) {
  console.error("Forum audience plan synchronization verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("Forum audience plan synchronization is source-ready.");
