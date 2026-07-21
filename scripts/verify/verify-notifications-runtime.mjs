#!/usr/bin/env node

import { existsSync, readFileSync, readdirSync, statSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = process.env.RUSTOK_VERIFY_REPO_ROOT
  ? path.resolve(process.env.RUSTOK_VERIFY_REPO_ROOT)
  : path.resolve(scriptDir, "../..");
const failures = [];

function absolute(relativePath) {
  return path.join(repoRoot, relativePath);
}

function read(relativePath) {
  const filePath = absolute(relativePath);
  if (!existsSync(filePath)) {
    failures.push(`${relativePath}: required file is missing`);
    return "";
  }
  return readFileSync(filePath, "utf8");
}

function requireText(source, marker, message) {
  if (!source.includes(marker)) failures.push(message);
}

function reject(source, pattern, message) {
  if (pattern.test(source)) failures.push(message);
}

function collectRustFiles(relativeRoot) {
  const root = absolute(relativeRoot);
  if (!existsSync(root)) return [];
  const files = [];
  function walk(directory) {
    for (const entry of readdirSync(directory)) {
      const child = path.join(directory, entry);
      if (statSync(child).isDirectory()) walk(child);
      else if (entry.endsWith(".rs")) files.push(child);
    }
  }
  walk(root);
  return files;
}

const modulesPath = "modules.toml";
const distributionCargoPath = "crates/rustok-distribution/Cargo.toml";
const distributionLibPath = "crates/rustok-distribution/src/lib.rs";
const serverCargoPath = "apps/server/Cargo.toml";
const serverRuntimePath = "apps/server/src/services/module_event_dispatcher.rs";
const adminCargoPath = "apps/admin/Cargo.toml";
const storefrontCargoPath = "apps/storefront/Cargo.toml";
const apiCargoPath = "crates/rustok-notifications-api/Cargo.toml";
const providerPath = "crates/rustok-notifications-api/src/provider.rs";
const keysPath = "crates/rustok-notifications-api/src/keys.rs";
const forumCargoPath = "crates/rustok-forum/Cargo.toml";
const forumLibPath = "crates/rustok-forum/src/lib.rs";
const forumSourcePath = "crates/rustok-forum/src/notification_source.rs";
const runtimeTestPath = "crates/rustok-forum/tests/notification_source_sqlite.rs";
const canonicalPlanPath = "crates/rustok-forum/docs/implementation-plan.md";

const modules = read(modulesPath);
const distributionCargo = read(distributionCargoPath);
const distributionLib = read(distributionLibPath);
const serverCargo = read(serverCargoPath);
const serverRuntime = read(serverRuntimePath);
const adminCargo = read(adminCargoPath);
const storefrontCargo = read(storefrontCargoPath);
const apiCargo = read(apiCargoPath);
const provider = read(providerPath);
const keys = read(keysPath);
const forumCargo = read(forumCargoPath);
const forumLib = read(forumLibPath);
const forumSource = read(forumSourcePath);
const runtimeTest = read(runtimeTestPath);
const canonicalPlan = read(canonicalPlanPath);

requireText(modules, 'notifications = { crate = "rustok-notifications"', `${modulesPath}: optional notifications module is not composed`);
const defaultEnabled = modules.match(/default_enabled\s*=\s*\[([^\]]*)\]/s)?.[1] ?? "";
if (/notifications/.test(defaultEnabled)) {
  failures.push(`${modulesPath}: notifications must remain tenant-disabled by default`);
}

for (const [source, marker, message] of [
  [distributionCargo, 'mod-notifications = ["dep:rustok-notifications"]', `${distributionCargoPath}: distribution feature is missing`],
  [distributionLib, 'registry.register(rustok_notifications::NotificationsModule)', `${distributionLibPath}: owner module is not registered`],
  [serverCargo, '"mod-notifications"', `${serverCargoPath}: server default composition omits notifications`],
  [serverCargo, 'mod-notifications = ["dep:rustok-notifications"', `${serverCargoPath}: server feature does not own the notifications dependency`],
  [serverRuntime, 'materialize_notification_source_registry', `${serverRuntimePath}: deferred providers are not materialized`],
  [serverRuntime, 'apply_to_host_runtime', `${serverRuntimePath}: provider factories do not receive neutral host context`],
  [adminCargo, 'rustok-notifications-admin', `${adminCargoPath}: notifications admin package is not composed`],
  [storefrontCargo, 'rustok-notifications-storefront', `${storefrontCargoPath}: notifications storefront package is not composed`],
  [apiCargo, 'dep:rustok-api', `${apiCargoPath}: server contract cannot use HostRuntimeContext`],
]) {
  requireText(source, marker, message);
}

for (const marker of [
  "trait NotificationSourceProviderFactory",
  "NotificationSourceFactoryRegistry",
  "register_notification_source_provider_factory",
  "materialize_notification_source_registry",
  "FactorySourceMismatch",
  "FactoryBuild",
]) {
  requireText(provider, marker, `${providerPath}: missing deferred provider invariant ${marker}`);
}
reject(provider, /DatabaseConnection|sea_orm::|forum_domain_event/, `${providerPath}: neutral factory contract exposes source persistence`);
requireText(keys, "safe_route_query", `${keysPath}: target route query policy is not bounded`);
reject(keys, /url::Url|reqwest/, `${keysPath}: safe internal route validation must not become an external URL resolver`);

const forumProductionCargo = forumCargo.split("[dev-dependencies]")[0];
requireText(forumProductionCargo, "rustok-notifications-api.workspace = true", `${forumCargoPath}: Forum must depend on the neutral API`);
reject(forumProductionCargo, /rustok-notifications\s*\.(workspace|path)|rustok-notifications\s*=\s*\{/, `${forumCargoPath}: Forum production code depends on the notifications owner`);
requireText(forumLib, "register_notification_source_provider_factory", `${forumLibPath}: Forum does not publish its source factory`);

for (const marker of [
  "forum.topic.created",
  "forum_domain_event::Entity::find()",
  "SequenceNo.eq(sequence_no)",
  "TenantId.eq(event.tenant_id())",
  "TopicStatus::Open",
  "forum_topic_channel_access::Entity::find()",
  "NotifyNewTopics.eq(true)",
  "ForumSubscriptionLevel::Muted",
  "request.bounded_limit()",
  "limit((limit + 1) as u64)",
  "NotificationAudienceCursor::new",
  "NotificationOpenAuthorization::Unavailable",
  "/modules/forum?category={}&topic={}",
  "Internal { retryable: true }",
]) {
  requireText(forumSource, marker, `${forumSourcePath}: missing source-provider invariant ${marker}`);
}
reject(forumSource, /rustok_notifications::/, `${forumSourcePath}: Forum imports the notifications owner`);
reject(forumSource, /email_address|phone_number|smtp|rendered_html/i, `${forumSourcePath}: Forum source leaks channel/contact/rendered data`);

for (const marker of [
  "notifications owner is absent",
  "NotificationsModule",
  "materialize_notification_source_registry",
  "limit: 1",
  "cross-tenant authorization should fail closed",
  "DROP TABLE forum_domain_events",
  "retryable: true",
]) {
  requireText(runtimeTest, marker, `${runtimeTestPath}: missing executable profile evidence ${marker}`);
}

for (const filePath of collectRustFiles("crates/rustok-forum/src/services")) {
  const source = readFileSync(filePath, "utf8");
  if (/rustok_notifications(?!(?:_api))/.test(source)) {
    failures.push(`${path.relative(repoRoot, filePath)}: Forum command service calls the notifications owner synchronously`);
  }
}

requireText(canonicalPlan, "Delivered in `NOTIFY-00B`", `${canonicalPlanPath}: NOTIFY-00B delivery is not recorded`);

if (failures.length > 0) {
  console.error("notifications runtime verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("notifications runtime verification passed");
