import fs from "node:fs";
import path from "node:path";

const root = process.cwd();
const required = [
  "crates/rustok-groups/Cargo.toml",
  "crates/rustok-groups/rustok-module.toml",
  "crates/rustok-groups/README.md",
  "crates/rustok-groups/docs/README.md",
  "crates/rustok-groups/docs/implementation-plan.md",
  "crates/rustok-groups/contracts/groups-fba-registry.json",
  "crates/rustok-groups/src/ports.rs",
  "crates/rustok-groups/src/service.rs",
  "crates/rustok-groups/admin/src/core.rs",
  "crates/rustok-groups/admin/src/transport.rs",
  "crates/rustok-groups/admin/src/transport/native_server_adapter.rs",
  "crates/rustok-groups/admin/src/transport/graphql_adapter.rs",
  "crates/rustok-groups/admin/src/ui/leptos.rs",
  "crates/rustok-groups/storefront/src/core.rs",
  "crates/rustok-groups/storefront/src/transport.rs",
  "crates/rustok-groups/storefront/src/transport/native_server_adapter.rs",
  "crates/rustok-groups/storefront/src/transport/graphql_adapter.rs",
  "crates/rustok-groups/storefront/src/ui/leptos.rs",
  "crates/rustok-groups/admin/locales/en.json",
  "crates/rustok-groups/admin/locales/ru.json",
  "crates/rustok-groups/storefront/locales/en.json",
  "crates/rustok-groups/storefront/locales/ru.json",
];

const failures = [];
const read = (relative) => fs.readFileSync(path.join(root, relative), "utf8");

for (const relative of required) {
  if (!fs.existsSync(path.join(root, relative))) {
    failures.push(`missing required Groups artifact: ${relative}`);
  }
}

for (const relative of [
  "crates/rustok-groups/admin/src/core.rs",
  "crates/rustok-groups/storefront/src/core.rs",
]) {
  if (fs.existsSync(path.join(root, relative)) && /use\s+leptos|leptos::/.test(read(relative))) {
    failures.push(`FFA core must remain framework-neutral: ${relative}`);
  }
}

for (const relative of [
  "crates/rustok-groups/admin/src/ui/leptos.rs",
  "crates/rustok-groups/storefront/src/ui/leptos.rs",
]) {
  if (fs.existsSync(path.join(root, relative))) {
    const content = read(relative);
    if (!content.includes("crate::transport")) {
      failures.push(`Leptos UI must consume the transport facade: ${relative}`);
    }
    if (/graphql_adapter|native_server_adapter/.test(content)) {
      failures.push(`Leptos UI must not consume raw adapters: ${relative}`);
    }
  }
}

if (fs.existsSync(path.join(root, "crates/rustok-groups/src/service.rs"))) {
  const service = read("crates/rustok-groups/src/service.rs");
  for (const forbidden of [
    "rustok_forum::entities",
    "rustok_blog::entities",
    "rustok_pages::entities",
    "rustok_product::entities",
    "rustok_marketplace_listing::entities",
  ]) {
    if (service.includes(forbidden)) {
      failures.push(`Groups must not import foreign persistence: ${forbidden}`);
    }
  }
  if (!service.includes("PortCallPolicy::read()") || !service.includes("PortCallPolicy::write()")) {
    failures.push("Groups ports must enforce read/write call policies");
  }
}

if (fs.existsSync(path.join(root, "crates/rustok-groups/contracts/groups-fba-registry.json"))) {
  const registry = JSON.parse(read("crates/rustok-groups/contracts/groups-fba-registry.json"));
  if (registry?.privacy?.default_on_provider_unavailable !== "deny_private_content") {
    failures.push("Groups FBA privacy fallback must fail closed");
  }
  if (registry?.feature_provider?.implicit_fallback !== false) {
    failures.push("Groups feature transport must not use implicit fallback");
  }
}

for (const relative of [
  "crates/rustok-groups/admin/locales/en.json",
  "crates/rustok-groups/admin/locales/ru.json",
  "crates/rustok-groups/storefront/locales/en.json",
  "crates/rustok-groups/storefront/locales/ru.json",
]) {
  if (fs.existsSync(path.join(root, relative))) {
    JSON.parse(read(relative));
  }
}

if (failures.length > 0) {
  console.error("Groups boundary verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("Groups FFA/FBA, multilingual, and ownership boundary checks passed.");
