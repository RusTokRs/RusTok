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
  if (!existsSync(absolute)) {
    failures.push(`${relativePath}: required file is missing`);
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

const contractPath = "crates/rustok-forum/contracts/forum-mention-write-boundary.json";
const contract = JSON.parse(read(contractPath) || "{}");
const topicEntry = read(contract.source_entrypoints?.topic ?? "");
const replyEntry = read(contract.source_entrypoints?.reply ?? "");
const topicOwner = read(contract.owner_entrypoints?.topic_create?.owner ?? "");
const replyOwner = read(contract.owner_entrypoints?.reply_create?.owner ?? "");
const topicIntegration = read(contract.owner_entrypoints?.topic_create?.implementation ?? "");
const replyIntegration = read(contract.owner_entrypoints?.reply_update?.implementation ?? "");
const record = read("crates/rustok-forum/docs/forum-12b2-owner-write-integration.md");

requireText(topicEntry, 'include!("topic_core.rs");', "topic entrypoint must retain the established implementation");
requireText(topicEntry, 'include!("topic_relation_integration.rs");', "topic entrypoint must compose the relation hook");
requireText(replyEntry, 'include!("reply_core.rs");', "reply entrypoint must retain the established implementation");
requireText(replyEntry, 'include!("reply_relation_integration.rs");', "reply entrypoint must compose the relation hook");

for (const marker of ["create_with_relations", "update_with_relations"]) {
  requireText(topicOwner, marker, `topic owner must route ${marker}`);
}
requireText(replyOwner, "MentionRelationService", "reply owner must own relation composition");
requireText(replyOwner, "update_with_relations", "reply owner must route relation-aware edits");

requireOrder(
  topicIntegration,
  [
    ".prepare(",
    "let txn = self.db.begin().await?;",
    "forum_topic_translation::ActiveModel",
    ".persist_in_tx(&txn, prepared_relations)",
    "DomainEvent::ForumTopicCreated",
    "txn.commit().await?;"
  ],
  "topic create must prepare outside the transaction and persist after the source body"
);
requireOrder(
  topicIntegration,
  [
    "prepare_topic_relation_body_for_update",
    ".prepare(",
    "let txn = self.db.begin().await?;",
    "self.upsert_translation_in_tx(",
    ".persist_in_tx(&txn, prepared_relations)",
    "txn.commit().await?;"
  ],
  "topic edit must persist relations after the translation write"
);
requireOrder(
  replyOwner,
  [
    ".prepare(",
    "let txn = self.db.begin().await?;",
    "forum_reply_body::ActiveModel",
    ".persist_in_tx(&txn, prepared_relations)",
    "DomainEvent::ForumTopicReplied",
    "txn.commit().await?;"
  ],
  "reply create must persist relations after the body write"
);
requireOrder(
  replyIntegration,
  [
    ".prepare(",
    "let txn = self.db.begin().await?;",
    "self.upsert_body_in_tx(",
    ".persist_in_tx(&txn, prepared_relations)",
    "txn.commit().await?;"
  ],
  "reply edit must persist relations after the body write"
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
  "Maintainer verification was not executed"
]) {
  requireText(record, marker, `FORUM-12B2 implementation record is missing ${marker}`);
}

if (failures.length > 0) {
  console.error("forum mention integration verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("forum mention integration verification passed");
