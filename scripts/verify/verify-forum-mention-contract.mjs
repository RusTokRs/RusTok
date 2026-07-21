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
  const absolute = path.join(repoRoot, relativePath);
  if (!existsSync(absolute)) {
    failures.push(`${relativePath}: required file is missing`);
    return "";
  }
  return readFileSync(absolute, "utf8");
}

function requireText(source, marker, message) {
  if (!source.includes(marker)) failures.push(message);
}

function reject(source, pattern, message) {
  if (pattern.test(source)) failures.push(message);
}

const contractPath = "crates/rustok-forum/src/mentions.rs";
const errorPath = "crates/rustok-forum/src/error.rs";
const libPath = "crates/rustok-forum/src/lib.rs";
const testPath = "crates/rustok-forum/tests/mention_contract.rs";
const planPath = "crates/rustok-forum/docs/implementation-plan.md";
const crateApiPath = "crates/rustok-forum/CRATE_API.md";

const contract = read(contractPath);
const error = read(errorPath);
const lib = read(libPath);
const tests = read(testPath);
const plan = read(planPath);
const crateApi = read(crateApiPath);

for (const marker of [
  "FORUM_MAX_MENTION_TARGETS_PER_REVISION: usize = 32",
  "FORUM_MAX_QUOTE_REFERENCES_PER_REVISION: usize = 32",
  "use serde::Serialize;",
  "normalize_content_format",
  "validate_and_sanitize_rt_json",
  "ProfileService::normalize_handle",
  "ProfilesReader",
  "ProfileVisibility::Public | ProfileVisibility::Authenticated",
  "ForumRevisionIdentity",
  "ForumQuoteReference",
  "resolved: ForumResolvedMentions",
  "diff_forum_mentions",
  "Forum mention replay changed an existing revision projection",
  "source: current.source().clone()",
  "pub fn event_candidates(&self)",
  "pub fn added_users(&self)",
]) {
  requireText(contract, marker, `${contractPath}: missing required contract marker ${marker}`);
}

requireText(
  error,
  "MentionTargetUnavailable",
  `${errorPath}: safe mention target error is missing`,
);
requireText(
  error,
  '"FORUM_MENTION_TARGET_UNAVAILABLE"',
  `${errorPath}: stable mention target code is missing`,
);
requireText(lib, "pub mod mentions;", `${libPath}: mention module is not public`);
requireText(lib, "pub use mentions::*;", `${libPath}: mention contracts are not exported`);

for (const marker of [
  "markdown_extraction_ignores_code_escaping_and_email_addresses",
  "rt_json_extraction_ignores_code_blocks_and_code_marks",
  "profile_resolution_is_tenant_scoped_and_fail_closed",
  "manual candidate construction must enforce audience permission",
  "revision_diff_emits_only_new_targets_and_replay_is_immutable",
  "identical revision replay must be idempotent",
  "quote_references_are_revision_bound_deduplicated_and_non_recursive",
]) {
  requireText(tests, marker, `${testPath}: missing contract coverage ${marker}`);
}

reject(
  contract,
  /rustok_notifications|NotificationSource|notification_source|publish_in_tx|DomainEvent/,
  `${contractPath}: FORUM-12A must not call notification or event delivery`,
);
reject(
  contract,
  /rustok_profiles::entities|ProfileService::new|profile::Entity/,
  `${contractPath}: mention resolution must use the public ProfilesReader boundary`,
);
reject(
  contract,
  /sea_orm|ActiveModel|Entity::|\.insert\(|\.update\(|\.delete\(/,
  `${contractPath}: FORUM-12A must not add persistence before the owner migration slice`,
);
reject(
  contract,
  /\bDeserialize\b/,
  `${contractPath}: bounded mention contracts must not bypass constructors through Deserialize`,
);
reject(
  contract,
  /\bpub\s+(?:added|removed|unchanged)_(?:users|audiences)\s*:/,
  `${contractPath}: mention diff collections must remain immutable`,
);

requireText(plan, "Delivered in `FORUM-12A`", `${planPath}: FORUM-12A is not recorded`);
requireText(
  plan,
  "remains `in_progress`",
  `${planPath}: remaining FORUM-12 scope is not explicit`,
);
requireText(
  crateApi,
  "ForumMentionRevisionProjection",
  `${crateApiPath}: mention revision contract is not documented`,
);
requireText(
  crateApi,
  "ForumQuoteReference",
  `${crateApiPath}: quote revision contract is not documented`,
);

if (failures.length > 0) {
  console.error("forum mention contract verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("forum mention contract verification passed");
