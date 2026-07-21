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

function collectFiles(root, relative = "") {
  const directory = path.join(root, relative);
  if (!existsSync(directory)) return [];
  const files = [];
  for (const entry of readdirSync(directory)) {
    if ([".git", "node_modules", "target"].includes(entry)) continue;
    const child = path.join(relative, entry);
    const childAbsolute = path.join(root, child);
    if (statSync(childAbsolute).isDirectory()) files.push(...collectFiles(root, child));
    else files.push(child.replaceAll(path.sep, "/"));
  }
  return files;
}

const apiLibPath = "crates/rustok-notifications-api/src/lib.rs";
const keysPath = "crates/rustok-notifications-api/src/keys.rs";
const modelPath = "crates/rustok-notifications-api/src/model.rs";
const providerPath = "crates/rustok-notifications-api/src/provider.rs";
const ownerLibPath = "crates/rustok-notifications/src/lib.rs";
const ownerServicePath = "crates/rustok-notifications/src/service.rs";
const manifestPath = "crates/rustok-notifications/rustok-module.toml";
const adminCorePath = "crates/rustok-notifications/admin/src/core.rs";
const adminTransportPath = "crates/rustok-notifications/admin/src/transport.rs";
const adminUiPath = "crates/rustok-notifications/admin/src/ui/leptos.rs";
const storefrontCorePath = "crates/rustok-notifications/storefront/src/core.rs";
const storefrontTransportPath = "crates/rustok-notifications/storefront/src/transport.rs";
const storefrontUiPath = "crates/rustok-notifications/storefront/src/ui/leptos.rs";
const canonicalPlanPath = "crates/rustok-forum/docs/implementation-plan.md";

const apiLib = read(apiLibPath);
const keys = read(keysPath);
const model = read(modelPath);
const provider = read(providerPath);
const ownerLib = read(ownerLibPath);
const ownerService = read(ownerServicePath);
const manifest = read(manifestPath);
const adminCore = read(adminCorePath);
const adminTransport = read(adminTransportPath);
const adminUi = read(adminUiPath);
const storefrontCore = read(storefrontCorePath);
const storefrontTransport = read(storefrontTransportPath);
const storefrontUi = read(storefrontUiPath);
const canonicalPlan = read(canonicalPlanPath);

for (const marker of [
  "NotificationSourceSlug",
  "NotificationTypeKey",
  "NotificationTemplateKey",
  "NotificationTargetRoute",
]) {
  requireText(apiLib, marker, `${apiLibPath}: missing public ${marker} contract`);
}

for (const marker of [
  "SOURCE_SLUG_MAX_BYTES",
  "SEMANTIC_KEY_MAX_BYTES",
  "AUDIENCE_CURSOR_MAX_BYTES",
  "TARGET_ROUTE_MAX_BYTES",
  "InvalidRoute",
]) {
  requireText(keys, marker, `${keysPath}: missing bounded key invariant ${marker}`);
}

for (const marker of [
  "MAX_NOTIFICATION_TEMPLATE_FIELDS",
  "MAX_NOTIFICATION_TEMPLATE_DATA_BYTES",
  "MAX_NOTIFICATION_AUDIENCE_PAGE_SIZE",
  "DuplicateAudienceRecipient",
  "InvalidSourceRevision",
  "impl<'de> Deserialize<'de> for NotificationSourceEventRef",
  "impl<'de> Deserialize<'de> for NotificationAudiencePage",
  "NotificationOpenAuthorization",
]) {
  requireText(model, marker, `${modelPath}: missing bounded semantic invariant ${marker}`);
}

for (const marker of [
  "trait NotificationSourceProvider",
  "describe_event",
  "resolve_audience",
  "authorize_target_open",
  "NotificationSourceRegistry",
  "register_notification_source_provider",
  "notification_source_registry_from_extensions",
  "DuplicateSource",
]) {
  requireText(provider, marker, `${providerPath}: missing source registry contract ${marker}`);
}

requireText(ownerLib, "ensure_notification_source_registry", `${ownerLibPath}: module does not initialize the neutral source registry`);
requireText(ownerLib, "&[\"outbox\"]", `${ownerLibPath}: notifications owner must declare the outbox dependency`);
requireText(ownerService, "unwrap_or_else(|| Arc::new(NotificationSourceRegistry::default()))", `${ownerServicePath}: missing-source degraded mode must remain an empty registry`);
requireText(manifest, 'slug = "notifications"', `${manifestPath}: notifications module slug is missing`);
requireText(manifest, 'leptos_crate = "rustok-notifications-admin"', `${manifestPath}: admin package is not owner-declared`);
requireText(manifest, 'leptos_crate = "rustok-notifications-storefront"', `${manifestPath}: storefront package is not owner-declared`);

requireText(adminCore, "NotificationsAdminStatus", `${adminCorePath}: admin bootstrap model is missing`);
requireText(adminTransport, "NotificationsAdminStatus::foundation()", `${adminTransportPath}: admin transport must return explicit foundation state`);
requireText(adminUi, "NotificationsAdmin", `${adminUiPath}: admin owner component is missing`);
requireText(storefrontCore, "unread_count: None", `${storefrontCorePath}: storefront must not invent an unread count`);
requireText(storefrontTransport, "NotificationStorefrontState::foundation()", `${storefrontTransportPath}: storefront transport must expose explicit degraded state`);
requireText(storefrontUi, "NotificationsView", `${storefrontUiPath}: storefront owner component is missing`);
requireText(canonicalPlan, "Delivered in `NOTIFY-00A`", `${canonicalPlanPath}: NOTIFY-00A delivery is not recorded`);

const notificationSources = collectFiles(absolute("crates/rustok-notifications-api"))
  .concat(collectFiles(absolute("crates/rustok-notifications")))
  .filter((file) => file.endsWith(".rs"))
  .map((file) => readFileSync(path.join(repoRoot, file.startsWith("crates/") ? file : `crates/rustok-notifications/${file}`), "utf8"))
  .join("\n");

reject(notificationSources, /rustok_email|smtp|phone_number|email_address|rendered_html/i, "notification foundation must not own channel SDKs, contact data, or rendered HTML");
reject(provider, /DatabaseConnection|sea_orm::|entities::/, `${providerPath}: neutral source provider contract must not expose persistence`);
reject(model, /serde_json::Value|HashMap<String,\s*serde_json::Value>/, `${modelPath}: semantic descriptor must not accept arbitrary JSON`);
reject(adminTransport, /localStorage|gloo_storage|reqwest|DatabaseConnection/, `${adminTransportPath}: bootstrap admin must not create shadow state or direct backend access`);
reject(storefrontTransport, /localStorage|gloo_storage|Some\s*\(\s*[1-9]/, `${storefrontTransportPath}: bootstrap storefront must not create shadow unread state`);

for (const relativePath of collectFiles(repoRoot)) {
  if (!relativePath.endsWith(".rs") && !relativePath.endsWith("Cargo.toml")) continue;
  if (relativePath.startsWith("crates/rustok-notifications/")) continue;
  if (relativePath.startsWith("crates/rustok-notifications-api/")) continue;
  const source = readFileSync(absolute(relativePath), "utf8");
  if (/rustok[_-]notifications(?!(?:[_-]api))/.test(source)) {
    failures.push(`${relativePath}: producer/consumer imports the notifications owner instead of the neutral API contract`);
  }
}

if (failures.length > 0) {
  console.error("notifications foundation verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("notifications foundation verification passed");
