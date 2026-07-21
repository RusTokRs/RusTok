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
  "crates/rustok-groups/src/domain.rs",
  "crates/rustok-groups/src/ports.rs",
  "crates/rustok-groups/src/service.rs",
  "crates/rustok-groups/src/governance.rs",
  "crates/rustok-groups/src/governance_entities.rs",
  "crates/rustok-groups/src/migrations/m20260721_000002_create_group_governance.rs",
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
  for (const privacyMarker of [
    "GroupAction::ViewSummary",
    "GroupVisibility::Closed.as_str()",
    "include_private_content",
    "return Err(GroupsError::NotFound)",
  ]) {
    if (!service.includes(privacyMarker)) {
      failures.push(`Groups closed/secret privacy split is missing marker: ${privacyMarker}`);
    }
  }
}

if (fs.existsSync(path.join(root, "crates/rustok-groups/src/domain.rs"))) {
  const domain = read("crates/rustok-groups/src/domain.rs");
  if (!domain.includes('ViewSummary => "view_summary"')) {
    failures.push("Groups domain must separate shell access from private content access");
  }
}

if (fs.existsSync(path.join(root, "crates/rustok-groups/src/governance.rs"))) {
  const governance = read("crates/rustok-groups/src/governance.rs");
  for (const marker of [
    "GroupGovernanceCommandPort",
    "PortCallPolicy::write()",
    "command_receipt",
    "audit_entry",
    "transfer_group_ownership",
  ]) {
    if (!governance.includes(marker)) {
      failures.push(`Groups governance boundary is missing marker: ${marker}`);
    }
  }
}

if (fs.existsSync(path.join(root, "crates/rustok-groups/contracts/groups-fba-registry.json"))) {
  const registry = JSON.parse(read("crates/rustok-groups/contracts/groups-fba-registry.json"));
  if (registry?.privacy?.default_on_provider_unavailable !== "deny_private_content") {
    failures.push("Groups FBA privacy fallback must fail closed");
  }
  if (registry?.privacy?.closed_group_discovery !== "summary_shell") {
    failures.push("Closed groups must publish only a discoverable summary shell");
  }
  if (registry?.privacy?.closed_group_private_content !== "active_membership_or_platform_manage") {
    failures.push("Closed group private content must remain membership-gated");
  }
  if (registry?.privacy?.secret_group_direct_read !== "not_found_without_membership_or_platform_manage") {
    failures.push("Secret group direct reads must preserve non-disclosure semantics");
  }
  if (registry?.feature_provider?.implicit_fallback !== false) {
    failures.push("Groups feature transport must not use implicit fallback");
  }
  const governancePort = registry?.provider?.ports?.find(
    (port) => port?.name === "GroupGovernanceCommandPort",
  );
  if (!governancePort?.transactional_receipt || !governancePort?.transactional_audit) {
    failures.push("Groups governance port must declare transactional receipt and audit");
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

console.log("Groups FFA/FBA, privacy, governance, multilingual, and ownership boundary checks passed.");
