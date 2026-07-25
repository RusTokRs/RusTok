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

function rejectText(source, marker, message) {
  if (source.includes(marker)) failures.push(message);
}

const contractPath =
  "crates/rustok-forum/contracts/forum-notification-inbox-open-authorization.json";
const contract = JSON.parse(read(contractPath) || "{}");
const inbox = read(contract.notifications_owner_file ?? "");
const surface = read(contract.notifications_surface_file ?? "");
const entities = read(contract.notifications_entity_file ?? "");
const docs = read(contract.notifications_live_contract ?? "");
const providerContract = read(contract.source_provider_contract ?? "");
const forumProvider = read(contract.forum_source_provider ?? "");
const proof = read(contract.sqlite_proof ?? "");
const upstream = JSON.parse(read(contract.upstream_contract ?? "") || "{}");
const plan = read(contract.canonical_plan ?? "");

if (contract.schema_version !== 1) {
  failures.push("forum notification inbox open contract must use schema_version=1");
}
if (contract.task !== "FORUM-20R" || contract.upstream_task !== "FORUM-20Q") {
  failures.push("forum notification inbox open contract must connect FORUM-20Q/R");
}
if (contract.verification?.execution_status !== "not_run_by_implementation_agent") {
  failures.push("inbox open authorization must not claim unexecuted evidence");
}

for (const delivered of [
  "owner_level_open_service",
  "exact_tenant_recipient_lookup",
  "cross_recipient_no_oracle",
  "validated_source_and_target_keys",
  "source_owned_authorization",
  "fresh_route_only",
  "stale_acl_fail_closed",
  "retryable_provider_failure_preserved",
  "read_state_unchanged",
  "delivery_attempts_unchanged",
  "public_crate_export",
  "sqlite_contract_proof",
  "live_notifications_docs",
]) {
  if (contract.composition?.[delivered] !== true) {
    failures.push(`forum notification inbox open contract must record ${delivered} as delivered`);
  }
}

for (const residual of [
  "bounded inbox listing API",
  "seen read and archive state mutations",
  "recipient privacy and blocking recheck at inbox open",
  "channel delivery enqueue and transports",
  "delivery-time target authorization",
  "host trust facts adapter",
  "host channel membership facts adapter",
  "initially non-public topic-created descriptor materialization",
  "search index SEO and deep-link migration",
  "PostgreSQL and cross-consumer runtime evidence",
]) {
  if (!contract.not_delivered?.includes(residual)) {
    failures.push(`forum notification inbox open contract must keep ${residual} explicitly open`);
  }
}

const deliveredSlices = [
  "FORUM-20H",
  "FORUM-20I",
  "FORUM-20J",
  "FORUM-20K",
  "FORUM-20L",
  "FORUM-20M",
  "FORUM-20N",
  "FORUM-20O",
  "FORUM-20P",
  "FORUM-20Q",
  "FORUM-20R",
];
const planSync = contract.canonical_plan_sync ?? {};
if (planSync.required_ledger_through !== "FORUM-20R") {
  failures.push("inbox open contract must require the canonical ledger through FORUM-20R");
}
if (JSON.stringify(planSync.required_delivered_sections) !== JSON.stringify(deliveredSlices)) {
  failures.push("inbox open contract must require FORUM-20H through FORUM-20R delivered sections");
}
if (planSync.status === "pending") {
  if (planSync.current_plan_through !== "FORUM-20G") {
    failures.push("pending canonical plan synchronization must identify FORUM-20G as current");
  }
  requireText(
    plan,
    "FORUM-20A-G provide",
    "pending canonical plan synchronization must remain grounded in FORUM-20A-G",
  );
  for (const slice of deliveredSlices) {
    rejectText(
      plan,
      `### Delivered in \`${slice}\``,
      `canonical plan now contains ${slice}; update canonical_plan_sync before claiming pending through G`,
    );
  }
} else if (planSync.status === "synchronized") {
  requireText(plan, "FORUM-20A-R provide", "synchronized canonical plan must advance through R");
  for (const slice of deliveredSlices) {
    requireText(plan, `### Delivered in \`${slice}\``, `canonical plan is missing ${slice}`);
  }
} else {
  failures.push("canonical_plan_sync.status must be pending or synchronized");
}

for (const marker of [
  "pub struct NotificationInboxOpenRequest",
  "pub tenant_id: Uuid",
  "pub recipient_id: Uuid",
  "pub notification_id: Uuid",
  "pub enum NotificationInboxOpenDecision",
  "Allowed { route: NotificationTargetRoute }",
  "Unavailable",
  "pub struct NotificationInboxOpenService",
  "db: DatabaseConnection",
  "registry: Arc<NotificationSourceRegistry>",
  "pub async fn authorize_open(",
  "notification::Entity::find_by_id(request.notification_id)",
  ".filter(notification::Column::TenantId.eq(request.tenant_id))",
  ".filter(notification::Column::RecipientId.eq(request.recipient_id))",
  "let Some(stored) = stored else",
  "return Ok(NotificationInboxOpenDecision::Unavailable)",
  "NotificationSourceSlug::new(stored.source_slug)",
  "NotificationSourceSlug::new(stored.target_owner)",
  "NotificationTargetKind::new(stored.target_kind)",
  "if target.id.is_nil()",
  ".registry",
  ".get(&source)",
  "AuthorizeNotificationTargetRequest {",
  "tenant_id: request.tenant_id",
  "recipient_id: request.recipient_id",
  "target,",
  ".map_err(NotificationError::from)?",
  "NotificationOpenAuthorization::Allowed { route }",
  "NotificationOpenAuthorization::Unavailable",
  "validate_request(&request)?",
]) {
  requireText(inbox, marker, `notification inbox open owner is missing ${marker}`);
}

const lookupIndex = inbox.indexOf("notification::Entity::find_by_id(request.notification_id)");
const missingIndex = inbox.indexOf("let Some(stored) = stored else");
const providerIndex = inbox.indexOf(".authorize_target_open(AuthorizeNotificationTargetRequest");
if (
  lookupIndex < 0 ||
  missingIndex < 0 ||
  providerIndex < 0 ||
  !(lookupIndex < missingIndex && missingIndex < providerIndex)
) {
  failures.push("exact ownership lookup and unavailable branch must precede the source provider call");
}

for (const forbidden of [
  "notification::ActiveModel",
  "ActiveValue::Set",
  "delivery_attempt::",
  "seen_at: Set(",
  "read_at: Set(",
  "archived_at: Set(",
  ".update(&self.db)",
  ".delete(&self.db)",
]) {
  rejectText(inbox, forbidden, `inbox open authorization must not mutate owner state through ${forbidden}`);
}

for (const marker of [
  "mod inbox;",
  "NotificationInboxOpenDecision",
  "NotificationInboxOpenRequest",
  "NotificationInboxOpenService",
]) {
  requireText(surface, marker, `notifications public surface is missing ${marker}`);
}
for (const marker of [
  "pub source_slug: String",
  "pub target_owner: String",
  "pub target_kind: String",
  "pub target_id: Uuid",
  "pub seen_at:",
  "pub read_at:",
  "pub archived_at:",
]) {
  requireText(entities, marker, `notification persistence identity is missing ${marker}`);
}
for (const marker of [
  "pub struct AuthorizeNotificationTargetRequest",
  "pub tenant_id: Uuid",
  "pub recipient_id: Uuid",
  "pub target: NotificationTargetRef",
  "async fn authorize_target_open(",
]) {
  requireText(providerContract, marker, `notification source contract is missing ${marker}`);
}
requireText(
  forumProvider,
  "async fn authorize_target_open(",
  "Forum source provider must implement open-time target authorization",
);

for (const marker of [
  "exact_recipient_gets_fresh_route_without_cross_recipient_oracle",
  "stale_target_and_retryable_owner_failure_remain_distinct",
  "invalid_stored_source_identity_fails_before_provider_invocation",
  "NotificationInboxOpenService::new",
  "NotificationInboxOpenDecision::Allowed",
  "NotificationInboxOpenDecision::Unavailable",
  "foreign and missing rows must not invoke a source provider",
  "NotificationProviderError::CapabilityUnavailable",
  "NotificationError::ProviderFailure { retryable: true }",
  "NotificationError::InvalidDescriptor",
]) {
  requireText(proof, marker, `inbox open SQLite proof is missing ${marker}`);
}

for (const marker of [
  "### Inbox open-time target authorization",
  "Missing, cross-tenant, and cross-recipient rows all",
  "It returns only the fresh owner-provided route or `Unavailable`.",
  "bounded inbox listing/read-state APIs and recipient privacy rechecks",
  "inbox_open_authorization_sqlite",
]) {
  requireText(docs, marker, `notifications live contract is missing ${marker}`);
}

if (
  upstream.schema_version !== 1 ||
  upstream.task !== "FORUM-20Q" ||
  upstream.upstream_task !== "FORUM-20P" ||
  upstream.downstream_task !== "FORUM-20R" ||
  upstream.composition?.notification_source_factory_consumption !== true
) {
  failures.push("FORUM-20R must remain linked to the FORUM-20Q host facts contract");
}

if (failures.length > 0) {
  console.error("Forum notification inbox open authorization verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("Forum notification inbox open authorization contract is source-ready.");
