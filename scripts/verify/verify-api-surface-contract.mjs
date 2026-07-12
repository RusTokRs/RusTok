#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../..");
const failures = [];

function read(relativePath) {
  const fullPath = path.join(root, relativePath);
  if (!fs.existsSync(fullPath)) {
    failures.push(`missing ${relativePath}`);
    return "";
  }
  return fs.readFileSync(fullPath, "utf8");
}

function requireContains(relativePath, marker, description) {
  if (!read(relativePath).includes(marker)) failures.push(description);
}

function requireNotContains(relativePath, marker, description) {
  if (read(relativePath).includes(marker)) failures.push(description);
}

const axumHostContracts = [
  ["apps/server/src/host.rs", "pub async fn run", "server host exposes the Axum entrypoint"],
  ["apps/server/src/services/server_bootstrap.rs", "bootstrap_application_router", "server owns explicit router bootstrap"],
  ["apps/server/src/services/app_router.rs", "Router", "server composes an Axum router"],
  ["apps/server/src/services/server_runtime_context.rs", "pub struct ServerRuntimeContext", "server owns typed runtime state"],
  ["apps/server/src/services/server_runtime_context.rs", "pub fn shared_get<T>", "server runtime exposes typed shared handles"],
  ["crates/rustok-api/src/runtime.rs", "pub struct HostRuntimeContext", "API owns the host-neutral runtime context"],
  ["crates/rustok-api/src/runtime.rs", "pub fn with_shared_value<T>", "host runtime accepts typed values"],
  ["crates/rustok-web/src/lib.rs", "json_response", "rustok-web owns response formatting"],
  ["crates/rustok-runtime/src/lib.rs", "RuntimeComposition", "runtime composition is owned by rustok-runtime"],
  ["crates/rustok-cli/src/main.rs", "run_with_environment", "CLI owns standalone command execution"],
];

for (const [relativePath, marker, description] of axumHostContracts) {
  requireContains(relativePath, marker, description);
}

for (const relativePath of [
  "crates/alloy/Cargo.toml",
  "crates/rustok-ai/Cargo.toml",
  "crates/rustok-blog/Cargo.toml",
  "crates/rustok-commerce/Cargo.toml",
  "crates/rustok-content-orchestration/Cargo.toml",
  "crates/rustok-forum/Cargo.toml",
  "crates/rustok-outbox/Cargo.toml",
  "crates/rustok-pages/Cargo.toml",
]) {
  requireNotContains(relativePath, "apps/server", `${relativePath} must not depend on the composition host`);
}

for (const [relativePath, marker, description] of [
  ["crates/alloy/src/controllers/mod.rs", "AlloyHttpRuntime", "Alloy HTTP owns narrow runtime state"],
  ["crates/rustok-commerce/src/controllers/mod.rs", "CommerceHttpRuntime", "commerce HTTP owns narrow runtime state"],
  ["crates/rustok-blog/src/controllers/mod.rs", "axum_router", "blog exposes an Axum router"],
  ["crates/rustok-pages/src/controllers/mod.rs", "axum_router", "pages exposes an Axum router"],
  ["crates/rustok-ai/src/service/types.rs", "AiHostRuntime", "AI owns a typed runtime contract"],
  ["crates/rustok-content-orchestration/src/lib.rs", "build_content_orchestration_service", "content orchestration exposes typed construction"],
]) {
  requireContains(relativePath, marker, description);
}

for (const relativePath of [
  "apps/server/src/services/email.rs",
  "apps/server/src/services/event_bus.rs",
  "apps/server/src/services/graphql_schema.rs",
  "apps/server/src/services/release_backend.rs",
  "apps/server/src/services/runtime_guardrails.rs",
]) {
  requireContains(relativePath, "ServerRuntimeContext", `${relativePath} consumes typed server runtime state`);
}

if (failures.length > 0) {
  console.error("API surface contract verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("API surface contract verification passed");
