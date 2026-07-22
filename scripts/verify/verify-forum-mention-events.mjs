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

function reject(source, pattern, message) {
  if (pattern.test(source)) failures.push(message);
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

const contract = JSON.parse(
  read("crates/rustok-forum/contracts/forum-mention-write-boundary.json") || "{}",
);
const eventFamily = read(contract.event_contract?.family ?? "");
const eventRegistry = read(contract.event_contract?.payload_registry ?? "");
const eventCrate = read("crates/rustok-events/src/lib.rs");
const relationService = read("crates/rustok-forum/src/services/mention_relation.rs");
const migration = read(contract.owner_journal?.migration ?? "");
const readService = read(contract.owner_read?.service ?? "");
const readDto = read(contract.owner_read?.dto ?? "");
const serviceRegistry = read("crates/rustok-forum/src/services/mod.rs");
const crateRoot = read("crates/rustok-forum/src/lib.rs");

for (const eventType of contract.event_contract?.event_types ?? []) {
  requireText(eventFamily, eventType, `event family is missing ${eventType}`);
  requireText(migration, eventType, `journal migration is missing ${eventType}`);
}
for (const marker of [
  "pub enum ForumMentionEvent",
  "UserMentionAdded",
  "AudienceMentionAdded",
  "schema_version(&self) -> u16",
  "impl EventContract for ForumMentionEvent",
  "impl ValidateEvent for ForumMentionEvent",
  "mentioned_user_id",
  "source_revision_id",
]) {
  requireText(eventFamily, marker, `typed Forum mention contract is missing ${marker}`);
}
reject(
  eventFamily,
  /email|phone|contact|address|handle_snapshot/,
  "Forum mention event contract must contain target identity, not contact or handle data",
);
for (const marker of [
  "ContractEventPayload::ForumMention",
  "Self::ForumMention(event)",
]) {
  requireText(eventRegistry, marker, `sealed payload registry is missing ${marker}`);
}
requireText(eventCrate, "forum_mention_event_schema", "event schema registry must include Forum mentions");

for (const marker of [
  "publish_contract_in_tx_with_envelope_id",
  "forum_domain_event::ActiveModel",
  "event_id: Set(event_id)",
  "publish_added_target_events_in_tx",
]) {
  requireText(relationService, marker, `owner publisher is missing ${marker}`);
}
requireOrder(
  relationService,
  [
    "replayed: true",
    "let result = MentionRelationSyncResult",
    "publish_added_target_events_in_tx",
  ],
  "identical replay must return before added-target event publication",
);
reject(
  relationService,
  /rustok_notifications|NotificationService|notification_source/,
  "Forum relation persistence must not call Notifications synchronously",
);

for (const marker of [
  "DatabaseBackend::Postgres",
  "DatabaseBackend::Sqlite",
  "chk_forum_domain_events_event_type",
  "forum_domain_events_next",
  "forum_domain_events_immutable_update",
  "forum_domain_events_immutable_delete",
]) {
  requireText(migration, marker, `journal migration is missing ${marker}`);
}

for (const marker of [
  "pub struct ForumRelationReadService",
  "pub async fn get(",
  "Action::Read",
  "RelationRevisionUnavailable",
  "MAX_MENTIONS_PER_REVISION",
  "MAX_QUOTES_PER_REVISION",
  "order_by_desc",
]) {
  requireText(readService, marker, `bounded relation read is missing ${marker}`);
}
for (const marker of [
  "ForumRelationSnapshotQuery",
  "ForumRelationSnapshotResponse",
  "user_ids",
  "audiences",
  "quotes",
]) {
  requireText(readDto, marker, `relation read DTO is missing ${marker}`);
}
for (const forbidden of contract.owner_read?.forbidden_fields ?? []) {
  if (readDto.includes(forbidden)) {
    failures.push(`relation read DTO must not expose ${forbidden}`);
  }
}
requireText(serviceRegistry, "ForumRelationReadService", "relation read service must be exported");
requireText(crateRoot, "ForumRelationReadService", "crate root must export relation read service");

for (const root of contract.transport_roots ?? []) {
  for (const relative of collectRustFiles(path.join(repoRoot, root))) {
    const source = read(path.join(root, relative).replaceAll(path.sep, "/"));
    for (const symbol of contract.forbidden_transport_symbols ?? []) {
      if (source.includes(symbol)) {
        failures.push(`${root}/${relative}: transport must not access private symbol ${symbol}`);
      }
    }
  }
}

if (failures.length > 0) {
  console.error("forum mention event verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("forum mention event verification passed");
