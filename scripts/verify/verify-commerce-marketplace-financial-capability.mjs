#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const here = path.dirname(fileURLToPath(import.meta.url));
const root = path.resolve(here, "../..");

function read(relativePath) {
  return fs.readFileSync(path.join(root, relativePath), "utf8");
}

function fail(message) {
  console.error(`commerce marketplace-financial capability guard failed: ${message}`);
  process.exitCode = 1;
}

function requireMatch(source, pattern, message) {
  if (!pattern.test(source)) fail(message);
}

function forbidMatch(source, pattern, message) {
  if (pattern.test(source)) fail(message);
}

function featureBody(source, feature) {
  const escaped = feature.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
  const match = source.match(new RegExp(`^${escaped}\\s*=\\s*\\[([^\\]]*)\\]`, "m"));
  if (!match) {
    fail(`missing Cargo feature ${feature}`);
    return "";
  }
  return match[1];
}

const commerceCargo = read("crates/rustok-commerce/Cargo.toml");
const commerceFeature = featureBody(commerceCargo, "marketplace-financial");
for (const dependency of [
  "dep:rustok-marketplace",
  "dep:rustok-marketplace-allocation",
  "dep:rustok-marketplace-commission",
  "dep:rustok-marketplace-ledger",
]) {
  if (!commerceFeature.includes(dependency)) {
    fail(`marketplace-financial must enable ${dependency}`);
  }
}
for (const dependency of [
  "rustok-marketplace",
  "rustok-marketplace-allocation",
  "rustok-marketplace-commission",
  "rustok-marketplace-ledger",
]) {
  requireMatch(
    commerceCargo,
    new RegExp(`^${dependency}\\s*=\\s*\\{[^}]*optional\\s*=\\s*true[^}]*\\}`, "m"),
    `${dependency} must stay optional in rustok-commerce`,
  );
}
requireMatch(
  commerceCargo,
  /^default\s*=\s*\[\]\s*$/m,
  "rustok-commerce default feature set must stay marketplace-free",
);

const distributionCargo = read("crates/rustok-distribution/Cargo.toml");
const distributionBase = featureBody(distributionCargo, "mod-commerce");
if (/marketplace/i.test(distributionBase)) {
  fail("rustok-distribution/mod-commerce must not enable marketplace owners");
}
const distributionCapability = featureBody(
  distributionCargo,
  "commerce-marketplace-financial",
);
for (const dependency of [
  "mod-commerce",
  "mod-marketplace_allocation",
  "mod-marketplace_commission",
  "mod-marketplace_ledger",
  "rustok-commerce/marketplace-financial",
]) {
  if (!distributionCapability.includes(dependency)) {
    fail(`distribution capability must include ${dependency}`);
  }
}

const serverCargo = read("apps/server/Cargo.toml");
const serverBase = featureBody(serverCargo, "mod-commerce");
if (/marketplace/i.test(serverBase)) {
  fail("server mod-commerce must not enable marketplace owners");
}
const serverCapability = featureBody(serverCargo, "commerce-marketplace-financial");
for (const dependency of [
  "mod-commerce",
  "mod-marketplace_allocation",
  "mod-marketplace_commission",
  "mod-marketplace_ledger",
  "rustok-commerce/marketplace-financial",
  "rustok-distribution/commerce-marketplace-financial",
]) {
  if (!serverCapability.includes(dependency)) {
    fail(`server capability must include ${dependency}`);
  }
}

const commerceLib = read("crates/rustok-commerce/src/lib.rs");
forbidMatch(
  commerceLib,
  /fn\s+register_runtime_extensions\s*\(/,
  "base CommerceModule must not hard-require MarketplaceFinancialRuntime during runtime extension registration",
);
requireMatch(
  commerceLib,
  /#\[cfg\(feature = "marketplace-financial"\)\][\s\S]*MarketplaceFinancialRuntime/,
  "marketplace financial listener/runtime use must be feature-gated in CommerceModule",
);

const migrations = read("crates/rustok-commerce/src/migrations/mod.rs");
for (const migration of [
  "m20260721_000001_create_checkout_marketplace_economics_checkpoints",
  "m20260721_000002_create_marketplace_financial_operations",
  "m20260721_000003_create_marketplace_paid_event_inbox",
  "m20260721_000004_create_marketplace_reversal_event_inbox",
  "m20260721_000005_enforce_marketplace_reversal_event_mysql_integrity",
  "m20260721_000006_create_marketplace_reversal_adaptation_failures",
]) {
  requireMatch(
    migrations,
    new RegExp(`#\\[cfg\\(feature = "marketplace-financial"\\)\\]\\nmod ${migration};`),
    `${migration} must be excluded from base Commerce migrations`,
  );
}

const serviceModules = read("crates/rustok-commerce/src/services/mod.rs");
for (const moduleName of [
  "checkout_marketplace_allocation",
  "checkout_marketplace_commission",
  "checkout_marketplace_economics",
  "marketplace_financial_runtime",
  "marketplace_paid_event_inbox",
  "marketplace_paid_order_financial",
  "marketplace_reversal_event_inbox",
]) {
  requireMatch(
    serviceModules,
    new RegExp(`#\\[cfg\\(feature = "marketplace-financial"\\)\\][\\s\\S]{0,100}(?:mod|pub use) ${moduleName}`),
    `${moduleName} must be feature-gated`,
  );
}

for (const [relativePath, marker] of [
  ["crates/rustok-commerce/src/controllers/mod.rs", "marketplace_financial"],
  ["crates/rustok-commerce/src/graphql/mod.rs", "marketplace_financial"],
  ["crates/rustok-commerce/src/graphql_runtime.rs", "marketplace_financial_runtime"],
  ["crates/rustok-commerce/src/openapi.rs", "openapi_marketplace_financial.rs"],
]) {
  const source = read(relativePath);
  requireMatch(
    source,
    /#\[cfg\(feature = "marketplace-financial"\)\]/,
    `${relativePath} must contain marketplace-financial gating for ${marker}`,
  );
}

const pipeline = read(
  "crates/rustok-commerce/src/services/checkout_stage_pipeline_owner_ports.rs",
);
requireMatch(
  pipeline,
  /#\[cfg\(not\(feature = "marketplace-financial"\)\)\][\s\S]*marketplace checkout lines require the `marketplace-financial` Commerce capability/,
  "base staged checkout must fail closed for marketplace lines before capture",
);

const storefront = read("crates/rustok-commerce/src/storefront_staged_checkout_runtime.rs");
requireMatch(
  storefront,
  /#\[cfg\(feature = "marketplace-financial"\)\][\s\S]*MarketplaceAllocationService[\s\S]*MarketplaceCommissionService[\s\S]*MarketplaceLedgerService/,
  "mounted marketplace owner construction must only exist inside the explicit capability",
);

const providerRuntime = read("apps/server/src/services/commerce_provider_runtime.rs");
requireMatch(
  providerRuntime,
  /#\[cfg\(feature = "commerce-marketplace-financial"\)\][\s\S]*MarketplaceFinancialRuntime/,
  "server MarketplaceFinancialRuntime composition must be capability-gated",
);

const dispatcher = read("apps/server/src/services/module_event_dispatcher.rs");
requireMatch(
  dispatcher,
  /#\[cfg\(feature = "commerce-marketplace-financial"\)\]\s*\n\s*spawn_marketplace_financial_worker_if_enabled/,
  "marketplace financial worker startup must be capability-gated",
);

const serverServices = read("apps/server/src/services/mod.rs");
requireMatch(
  serverServices,
  /#\[cfg\(feature = "commerce-marketplace-financial"\)\]\s*\n\s*pub mod marketplace_financial_worker;/,
  "marketplace financial worker module must be capability-gated",
);

if (!process.exitCode) {
  console.log("commerce marketplace-financial capability guard: source contract OK");
}
