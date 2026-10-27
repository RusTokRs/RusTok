#!/usr/bin/env node

import { existsSync, readFileSync, readdirSync, statSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = process.env.RUSTOK_VERIFY_REPO_ROOT
  ? path.resolve(process.env.RUSTOK_VERIFY_REPO_ROOT)
  : path.resolve(scriptDir, "../..");
const failures = [];

function read(relativePath) {
  const absolute = path.join(repoRoot, relativePath);
  if (!relativePath || !existsSync(absolute)) {
    failures.push(`${relativePath || "<missing path>"}: required file is missing`);
    return "";
  }
  return readFileSync(absolute, "utf8");
}

function requireText(source, marker, message) {
  if (!source.includes(marker)) failures.push(message);
}

function requireOrder(source, markers, message) {
  let previous = -1;
  for (const marker of markers) {
    const index = source.indexOf(marker, previous + 1);
    if (index < 0 || index <= previous) {
      failures.push(`${message}: missing or out-of-order marker ${marker}`);
      return;
    }
    previous = index;
  }
}

function section(source, marker, description) {
  const index = source.indexOf(marker);
  if (index < 0) {
    failures.push(`${description}: missing method marker ${marker}`);
    return "";
  }
  return source.slice(index);
}

// A write path is identified by the block that persists prepared relations, not by the name the
// owner happened to give the method: the invariant is the order, and the method is free to be
// renamed, split into an audited entrypoint plus an implementation, or inlined.
function relationWriteBlock(source, distinguishing, description) {
  const blocks = [];
  const pattern = /async fn ([a-z0-9_]+)/g;
  let match;
  while ((match = pattern.exec(source)) !== null) {
    blocks.push({ name: match[1], start: match.index });
  }
  for (let index = 0; index < blocks.length; index += 1) {
    const end = index + 1 < blocks.length ? blocks[index + 1].start : source.length;
    const body = source.slice(blocks[index].start, end);
    if (
      body.includes(".persist_in_tx(&txn, prepared_relations)") &&
      body.includes(distinguishing)
    ) {
      return body;
    }
  }
  failures.push(
    `${description}: no owner method persists prepared relations together with ${distinguishing}`,
  );
  return "";
}

function collectRustFiles(root, relative = "") {
  const absolute = path.join(root, relative);
  if (!existsSync(absolute)) return [];
  const files = [];
  for (const entry of readdirSync(absolute)) {
    const childRelative = path.join(relative, entry);
    const childAbsolute = path.join(root, childRelative);
    const stat = statSync(childAbsolute);
    if (stat.isDirectory()) files.push(...collectRustFiles(root, childRelative));
    else if (entry.endsWith(".rs")) files.push(childRelative.replaceAll(path.sep, "/"));
  }
  return files;
}

const contractPath = "crates/modules/rustok-forum/contracts/forum-mention-write-boundary.json";
const contract = JSON.parse(read(contractPath) || "{}");
const topicEntry = read(contract.source_entrypoints?.topic ?? "");
const replyEntry = read(contract.source_entrypoints?.reply ?? "");
const topicOwner = read(contract.owner_entrypoints?.topic_create?.owner ?? "");
const replyOwner = read(contract.owner_entrypoints?.reply_create?.owner ?? "");
const mentionSource = read("crates/modules/rustok-forum/src/mentions.rs");
const profilesReader = read("crates/modules/rustok-profiles/src/reader.rs");
const profileServices = read("crates/modules/rustok-profiles/src/services.rs");
const mentionService = read("crates/modules/rustok-forum/src/services/mention_relation.rs");
const b2Record = read("crates/modules/rustok-forum/docs/forum-12b2-owner-write-integration.md");

requireText(profilesReader, "find_profile_records_by_handles", "ProfilesReader must expose bounded batch handle lookup");
requireText(profileServices, "pub const MAX_PROFILE_HANDLE_BATCH: usize = 64", "Profiles owner must expose a hard batch bound");
requireText(profileServices, "if handles.len() > MAX_PROFILE_HANDLE_BATCH", "Profiles batch lookup must reject over-limit requests");
requireText(profileServices, "pub async fn find_profile_records_by_handles", "Profiles owner must implement bounded batch handle lookup");
requireText(mentionSource, "find_profile_records_by_handles(", "Forum mention resolution must use bounded Profiles handle lookup");
for (const forbidden of [
  ".get_profile_by_handle(",
  "for handle in candidates.handles",
]) {
  if (mentionSource.includes(forbidden)) {
    failures.push(`crates/modules/rustok-forum/src/mentions.rs: mention resolution must not reintroduce per-handle Profiles reads: ${forbidden}`);
  }
}
// The owner entrypoints and their delegates are declared by the contract; the guard reads them from
// there so a renamed method keeps the delegation honest instead of pinning one spelling.
for (const [entryKey, entry] of Object.entries(contract.owner_entrypoints ?? {})) {
  const ownerSource = read(entry.owner ?? "");
  requireText(
    ownerSource,
    entry.delegate,
    `${entryKey}: owner must delegate relation-aware work to ${entry.delegate}`,
  );
}
// The reply owner holds the relation service as a field, so composition is proven by the field being
// driven for both prepare and persist rather than by the type name appearing in this file.
if (!/self\s*\.relations\s*\n?\s*\.prepare\(/.test(replyOwner)) {
  failures.push("reply owner must prepare relation composition through its own relation service");
}

const topicCreate = relationWriteBlock(
  topicEntry,
  "forum_topic_translation::ActiveModel",
  "topic create",
);
requireOrder(
  topicCreate,
  [
    ".prepare(",
    "let txn = self.db.begin().await?;",
    "forum_topic_translation::ActiveModel",
    ".persist_in_tx(&txn, prepared_relations)",
    "DomainEvent::ForumTopicCreated",
    "txn.commit().await?;",
  ],
  "topic create must prepare outside the transaction and persist after the source body",
);

const topicUpdate = relationWriteBlock(
  topicEntry,
  "self.upsert_translation_in_tx(",
  "topic edit",
);
requireOrder(
  topicUpdate,
  [
    "prepare_topic_relation_body_for_update",
    ".prepare(",
    "let txn = self.db.begin().await?;",
    "self.upsert_translation_in_tx(",
    ".persist_in_tx(&txn, prepared_relations)",
    "txn.commit().await?;",
  ],
  "topic edit must persist relations after the translation write",
);

const replyCreate = relationWriteBlock(
  replyOwner,
  "forum_reply_body::ActiveModel",
  "reply create",
);
requireOrder(
  replyCreate,
  [
    ".prepare(",
    "let txn = self.db.begin().await?;",
    "forum_reply_body::ActiveModel",
    ".persist_in_tx(&txn, prepared_relations)",
    "DomainEvent::ForumTopicReplied",
    "txn.commit().await?;",
  ],
  "reply create must persist relations after the body write",
);

const replyUpdate = relationWriteBlock(replyEntry, "self.upsert_body_in_tx(", "reply edit");
requireOrder(
  replyUpdate,
  [
    ".prepare(",
    "let txn = self.db.begin().await?;",
    "self.upsert_body_in_tx(",
    ".persist_in_tx(&txn, prepared_relations)",
    "txn.commit().await?;",
  ],
  "reply edit must persist relations after the body write",
);

for (const marker of [
  "publish_added_target_events_in_tx",
  "publish_contract_in_tx_with_envelope_id",
  "forum_domain_event::ActiveModel",
]) {
  requireText(mentionService, marker, `mention persistence must compose FORUM-12C marker ${marker}`);
}
requireOrder(
  mentionService,
  [
    "let result = MentionRelationSyncResult",
    "publish_added_target_events_in_tx",
    "Ok(result)",
  ],
  "mention events must be derived from the persisted added-target result",
);

for (const root of contract.transport_roots ?? []) {
  for (const relative of collectRustFiles(path.join(repoRoot, root))) {
    const source = read(path.join(root, relative).replaceAll(path.sep, "/"));
    for (const symbol of contract.forbidden_transport_symbols ?? []) {
      if (source.includes(symbol)) {
        failures.push(`${root}/${relative}: transport must not access private relation symbol ${symbol}`);
      }
    }
  }
}

for (const marker of [
  "FORUM-12B2",
  "prepare the projection",
  "persist_in_tx",
  "Quote command DTOs are intentionally unchanged",
  "Maintainer verification was not executed",
]) {
  requireText(b2Record, marker, `FORUM-12B2 implementation record is missing ${marker}`);
}

if (failures.length > 0) {
  console.error("forum mention integration verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("forum mention integration verification passed");
