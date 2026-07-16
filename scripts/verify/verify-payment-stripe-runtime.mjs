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
    failures.push(`${relativePath}: file is missing`);
    return "";
  }
  return readFileSync(absolute, "utf8");
}

function requireMarker(source, marker, message) {
  if (!source.includes(marker)) failures.push(message);
}

function forbidMarker(source, marker, message) {
  if (source.includes(marker)) failures.push(message);
}

const cargoPath = "apps/server/Cargo.toml";
const moduleRegistryPath = "apps/server/src/services/mod.rs";
const runtimePath = "apps/server/src/services/payment_provider_runtime.rs";
const attachmentPath = "apps/server/src/services/commerce_provider_runtime.rs";
const planPath = "crates/rustok-commerce/docs/implementation-plan.md";
const packagePath = "package.json";

const cargo = read(cargoPath);
const moduleRegistry = read(moduleRegistryPath);
const runtime = read(runtimePath);
const attachment = read(attachmentPath);
const plan = read(planPath);
const packageJson = read(packagePath);

for (const marker of [
  'payment-stripe = ["mod-payment", "rustok-payment/stripe", "dep:rustok-secrets"]',
  'rustok-secrets   = { workspace = true, optional = true }',
]) {
  requireMarker(cargo, marker, `${cargoPath}: missing ${marker}`);
}
requireMarker(
  moduleRegistry,
  'pub mod payment_provider_runtime;',
  `${moduleRegistryPath}: payment provider runtime module is not registered`,
);
for (const marker of [
  "RUSTOK_STRIPE_TENANT_CREDENTIALS_JSON",
  "StripeTenantCredentialRefs",
  "SecretRef",
  "SecretResolverRegistry",
  "validate_reference_for_tenant",
  "resolve_for_tenant",
  "reference_owners",
  "register_external",
  "PaymentProviderHealth::Ready",
  "StaticStripeCredentialProvider",
]) {
  if (marker === "StaticStripeCredentialProvider") {
    forbidMarker(runtime, marker, `${runtimePath}: static Stripe credentials are forbidden in server composition`);
  } else {
    requireMarker(runtime, marker, `${runtimePath}: missing ${marker}`);
  }
}
for (const marker of [
  "RUSTOK_STRIPE_SECRET_KEY",
  "RUSTOK_STRIPE_WEBHOOK_SECRET",
  "secret_key: String",
  "webhook_secret: String",
]) {
  forbidMarker(runtime, marker, `${runtimePath}: raw credential configuration is forbidden (${marker})`);
}
requireMarker(
  attachment,
  "build_payment_provider_registry",
  `${attachmentPath}: transports must attach the deployment-composed payment registry`,
);
requireMarker(
  attachment,
  "server.shared_insert(registry.clone())",
  `${attachmentPath}: composed registry must be shared across transports`,
);
requireMarker(
  plan,
  "verify-payment-stripe-runtime.mjs",
  `${planPath}: Stripe runtime verifier must be listed`,
);
requireMarker(
  packageJson,
  '"verify:payment:stripe-runtime"',
  `${packagePath}: Stripe runtime verifier script is not registered`,
);
requireMarker(
  packageJson,
  "npm run verify:payment:stripe-runtime",
  `${packagePath}: aggregate verification must run Stripe runtime guard`,
);

if (failures.length > 0) {
  console.error("payment Stripe runtime verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("payment Stripe runtime verification passed");
