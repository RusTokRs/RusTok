import fs from "node:fs";
import path from "node:path";

const root = process.cwd();
const read = (relative) => fs.readFileSync(path.join(root, relative), "utf8");
const exists = (relative) => fs.existsSync(path.join(root, relative));
const failures = [];

const required = [
  "crates/rustok-groups/src/application_entities.rs",
  "crates/rustok-groups/src/applications.rs",
  "crates/rustok-groups/src/graphql_applications.rs",
  "crates/rustok-groups/src/graphql_policy_history.rs",
  "crates/rustok-groups/src/migrations/m20260722_000006_create_group_membership_applications.rs",
  "crates/rustok-groups/admin/src/application_core.rs",
  "crates/rustok-groups/admin/src/application_model.rs",
  "crates/rustok-groups/admin/src/transport/native_applications_adapter.rs",
  "crates/rustok-groups/admin/src/transport/graphql_applications_adapter.rs",
  "crates/rustok-groups/admin/src/ui/applications.rs",
  "crates/rustok-groups/storefront/src/application_core.rs",
  "crates/rustok-groups/storefront/src/application_model.rs",
  "crates/rustok-groups/storefront/src/transport/native_applications_adapter.rs",
  "crates/rustok-groups/storefront/src/transport/graphql_applications_adapter.rs",
  "crates/rustok-groups/storefront/src/ui/application.rs",
];

for (const relative of required) {
  if (!exists(relative)) failures.push(`missing membership application artifact: ${relative}`);
}

const migrationPath = "crates/rustok-groups/src/migrations/m20260722_000006_create_group_membership_applications.rs";
if (exists(migrationPath)) {
  const migration = read(migrationPath);
  for (const marker of [
    "group_membership_policies",
    "group_membership_policy_translations",
    "group_membership_applications",
    "ux_group_membership_policies_tenant_group",
    "ux_group_membership_policy_translations_tenant_policy_locale",
    "ux_group_membership_applications_tenant_group_user",
    "policy_snapshot",
    "acknowledged_rule_keys",
    "status IN ('pending', 'approved', 'rejected', 'cancelled')",
  ]) {
    if (!migration.includes(marker)) failures.push(`membership application migration is missing marker: ${marker}`);
  }
}

const servicePath = "crates/rustok-groups/src/applications.rs";
if (exists(servicePath)) {
  const service = read(servicePath);
  for (const marker of [
    "GroupApplicationReadPort",
    "GroupApplicationCommandPort",
    "PortCallPolicy::read()",
    "PortCallPolicy::write()",
    "normalize_locale_tag",
    "GroupJoinPolicy::Request",
    "GroupVisibility::Secret",
    "policy_snapshot",
    "validate_submission",
    "command_receipt",
    "audit_entry",
    "increment_group_membership_version",
    "exclusive_lock",
    "group.membership_application_submitted",
    "group.membership_application_approved",
    "group.membership_application_rejected",
  ]) {
    if (!service.includes(marker)) failures.push(`membership application owner service is missing marker: ${marker}`);
  }
  for (const forbidden of [
    "rustok_profiles::",
    "rustok_notifications::",
    "rustok_forum::",
    "rustok_blog::",
    "PLATFORM_FALLBACK_LOCALE",
    "rows.first()",
  ]) {
    if (service.includes(forbidden)) failures.push(`membership application owner service crosses a forbidden boundary: ${forbidden}`);
  }
}

const graphqlPath = "crates/rustok-groups/src/graphql_applications.rs";
if (exists(graphqlPath)) {
  const graphql = read(graphqlPath);
  for (const marker of [
    "MergedObject",
    "group_application_policy",
    "group_membership_applications",
    "upsert_group_application_policy",
    "submit_group_membership_application",
    "review_group_membership_application",
    "GroupApplicationCommandPort",
    "GroupApplicationReadPort",
    "with_idempotency_key",
  ]) {
    if (!graphql.includes(marker)) failures.push(`membership application GraphQL surface is missing marker: ${marker}`);
  }
}

const manifestPath = "crates/rustok-groups/rustok-module.toml";
if (exists(manifestPath)) {
  const manifest = read(manifestPath);
  for (const marker of [
    'query = "graphql_policy_history::GroupsQueryRoot"',
    'mutation = "graphql_policy_history::GroupsMutationRoot"',
    'subpath = "applications"',
  ]) {
    if (!manifest.includes(marker)) failures.push(`Groups manifest is missing application composition marker: ${marker}`);
  }
}

for (const corePath of [
  "crates/rustok-groups/admin/src/application_core.rs",
  "crates/rustok-groups/storefront/src/application_core.rs",
]) {
  if (exists(corePath) && /use\s+leptos|leptos::/.test(read(corePath))) {
    failures.push(`membership application FFA core must remain framework-neutral: ${corePath}`);
  }
}

for (const uiPath of [
  "crates/rustok-groups/admin/src/ui/applications.rs",
  "crates/rustok-groups/storefront/src/ui/application.rs",
]) {
  if (!exists(uiPath)) continue;
  const ui = read(uiPath);
  if (!ui.includes("crate::transport")) failures.push(`membership application UI must consume the transport facade: ${uiPath}`);
  if (/graphql_applications_adapter|native_applications_adapter/.test(ui)) {
    failures.push(`membership application UI must not import raw adapters: ${uiPath}`);
  }
}

for (const facadePath of [
  "crates/rustok-groups/admin/src/transport.rs",
  "crates/rustok-groups/storefront/src/transport.rs",
]) {
  if (!exists(facadePath)) continue;
  const facade = read(facadePath);
  for (const marker of ["execute_selected_transport", "never falls back"]) {
    if (!facade.includes(marker)) failures.push(`membership application facade is missing marker ${marker}: ${facadePath}`);
  }
}

const registryPath = "crates/rustok-groups/contracts/groups-fba-registry.json";
if (exists(registryPath)) {
  const registry = JSON.parse(read(registryPath));
  const readPort = registry?.provider?.ports?.find((port) => port?.name === "GroupApplicationReadPort");
  const commandPort = registry?.provider?.ports?.find((port) => port?.name === "GroupApplicationCommandPort");
  if (!readPort?.exact_locale_only) failures.push("GroupApplicationReadPort must declare exact-locale selection");
  if (!commandPort?.transactional_receipt || !commandPort?.transactional_audit || !commandPort?.transactional_membership) {
    failures.push("GroupApplicationCommandPort must declare transactional receipt, audit, and membership state");
  }
  if (registry?.membership_applications?.module_local_fallback !== false) {
    failures.push("membership application policy must not own locale fallback");
  }
  if (registry?.membership_applications?.transport_fallback !== "never") {
    failures.push("membership application transport must never fall back implicitly");
  }
  for (const evidenceKey of [
    "membership_application_transport_parity",
    "membership_application_concurrency",
    "membership_application_policy_revision",
    "membership_application_bulk_review",
  ]) {
    if (registry?.evidence?.[evidenceKey] !== null) {
      failures.push(`unexecuted membership application evidence must remain null: ${evidenceKey}`);
    }
  }
}

for (const localePath of [
  "crates/rustok-groups/admin/locales/en.json",
  "crates/rustok-groups/admin/locales/ru.json",
  "crates/rustok-groups/storefront/locales/en.json",
  "crates/rustok-groups/storefront/locales/ru.json",
]) {
  if (!exists(localePath)) continue;
  const messages = JSON.parse(read(localePath));
  const prefix = localePath.includes("admin/") ? "groups.admin.applications." : "groups.storefront.application.";
  if (!Object.keys(messages).some((key) => key.startsWith(prefix))) {
    failures.push(`membership application locale namespace is missing: ${localePath}`);
  }
}

if (failures.length > 0) {
  console.error("Groups membership application boundary verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("Groups membership application owner, FBA, FFA, exact-locale, snapshot, and no-fallback boundary checks passed.");
