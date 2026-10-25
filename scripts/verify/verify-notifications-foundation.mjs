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
    if (entry.startsWith(".") || entry.startsWith("target") || entry === "node_modules") continue;
    const child = path.join(relative, entry);
    const childAbsolute = path.join(root, child);
    if (statSync(childAbsolute).isDirectory()) files.push(...collectFiles(root, child));
    else files.push(child.replaceAll(path.sep, "/"));
  }
  return files;
}

const apiLibPath = "crates/modules/rustok-notifications/src/lib.rs";
const keysPath = "crates/modules/rustok-notifications/src/keys.rs";
const modelPath = "crates/modules/rustok-notifications/src/model.rs";
const providerPath = "crates/modules/rustok-notifications/src/provider.rs";
const ownerLibPath = "crates/modules/rustok-notifications/src/lib.rs";
const ownerModulePath = "crates/modules/rustok-notifications/src/module.rs";
const ownerServicePath = "crates/modules/rustok-notifications/src/service.rs";
const manifestPath = "crates/modules/rustok-notifications/rustok-module.toml";
const adminCorePath = "crates/modules/rustok-notifications/admin/src/core.rs";
const adminTransportPath = "crates/modules/rustok-notifications/admin/src/transport.rs";
const adminUiPath = "crates/modules/rustok-notifications/admin/src/ui/leptos.rs";
const storefrontCorePath = "crates/modules/rustok-notifications/storefront/src/core.rs";
const storefrontTransportPath = "crates/modules/rustok-notifications/storefront/src/transport.rs";
const storefrontUiPath = "crates/modules/rustok-notifications/storefront/src/ui/leptos.rs";
const canonicalPlanPath = existsSync(absolute("crates/modules/rustok-forum/docs/archive/implementation-plan-2026-08-06.snapshot"))
  ? "crates/modules/rustok-forum/docs/archive/implementation-plan-2026-08-06.snapshot"
  : "crates/modules/rustok-forum/docs/implementation-plan.md";

const apiLib = read(apiLibPath);
const keys = read(keysPath);
const model = read(modelPath);
const provider = read(providerPath);
const ownerLib = read(ownerLibPath);
const ownerModule = read(ownerModulePath);
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
  'segment != "." && segment != ".."',
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
  "pub fn source_revision(&self)",
  "pub fn recipients(&self)",
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
requireText(ownerModule, "&[\"outbox\"]", `${ownerModulePath}: notifications owner must declare the outbox dependency`);
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

const notificationSources = [
  apiLib,
  keys,
  model,
  provider,
  ownerLib,
  ownerService,
  adminCore,
  adminTransport,
  adminUi,
  storefrontCore,
  storefrontTransport,
  storefrontUi,
].join("\n");

reject(notificationSources, /rustok_email|smtp|phone_number|email_address|rendered_html/i, "notification foundation must not own channel SDKs, contact data, or rendered HTML");
reject(provider, /DatabaseConnection|sea_orm::|entities::/, `${providerPath}: neutral source provider contract must not expose persistence`);
reject(model, /serde_json::Value|HashMap<String,\s*serde_json::Value>/, `${modelPath}: semantic descriptor must not accept arbitrary JSON`);
reject(model, /pub\s+source_revision\s*:/, `${modelPath}: source revision must remain constructor-validated and private`);
reject(model, /pub\s+recipients\s*:/, `${modelPath}: audience recipients must remain constructor-validated and private`);
reject(adminTransport, /localStorage|gloo_storage|reqwest|DatabaseConnection/, `${adminTransportPath}: bootstrap admin must not create shadow state or direct backend access`);
reject(storefrontTransport, /localStorage|gloo_storage|Some\s*\(\s*[1-9]/, `${storefrontTransportPath}: bootstrap storefront must not create shadow unread state`);

for (const relativePath of collectFiles(repoRoot)) {
  if (relativePath === "Cargo.toml") continue;
  if (!relativePath.endsWith(".rs") && !relativePath.endsWith("Cargo.toml")) continue;
  if (relativePath.startsWith("crates/modules/rustok-notifications/")) continue;
  if (relativePath.startsWith("crates/utils/rustok-distribution/")) continue;
  if (relativePath.startsWith("crates/utils/rustok-migrations/")) continue;
  if (relativePath.startsWith("apps/server/")) continue;
  if (relativePath.startsWith("apps/admin/")) continue;
  if (relativePath.startsWith("apps/storefront/")) continue;
  const source = readFileSync(absolute(relativePath), "utf8");
  if (relativePath.endsWith("Cargo.toml")) {
    const prodSource = source.split("[dev-dependencies]")[0];
    if (/rustok-notifications\s*=\s*\{[^}]*"server"/.test(prodSource)) {
      failures.push(`${relativePath}: producer enables notifications server feature`);
    }
  } else if (/rustok_notifications::(?:entities|candidate|fanout|inbox|worker|service|migrations|Notification[s]?Service)/.test(source)) {
    failures.push(`${relativePath}: producer imports the notifications owner persistence instead of the neutral API contract`);
  }
}

if (failures.length > 0) {
  console.error("notifications foundation verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("notifications foundation verification passed");
