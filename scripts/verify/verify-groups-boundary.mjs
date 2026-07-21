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
  "crates/rustok-groups/src/graphql_governance.rs",
  "crates/rustok-groups/src/migrations/m20260721_000002_create_group_governance.rs",
  "crates/rustok-groups/src/migrations/m20260721_000003_enforce_group_language_agnostic_storage.rs",
  "crates/rustok-groups/admin/src/core.rs",
  "crates/rustok-groups/admin/src/model.rs",
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
  for (const forbiddenLocaleFallback of [
    "PLATFORM_FALLBACK_LOCALE",
    "build_locale_candidates",
    "rows.first()",
  ]) {
    if (service.includes(forbiddenLocaleFallback)) {
      failures.push(`Groups must not own locale fallback policy: ${forbiddenLocaleFallback}`);
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
  for (const languageAgnosticMarker of [
    "normalize_effective_locale",
    "normalize_language_agnostic_metadata",
    "translation::Column::Locale.eq(effective_locale.clone())",
    "title.chars().count() > 240",
    "value.chars().count() > 500",
    "object.contains_key(*key)",
  ]) {
    if (!service.includes(languageAgnosticMarker)) {
      failures.push(`Groups language-agnostic service contract is missing marker: ${languageAgnosticMarker}`);
    }
  }
}

if (fs.existsSync(path.join(root, "crates/rustok-groups/src/migrations/m20260721_000003_enforce_group_language_agnostic_storage.rs"))) {
  const migration = read(
    "crates/rustok-groups/src/migrations/m20260721_000003_enforce_group_language_agnostic_storage.rs",
  );
  for (const marker of [
    "ck_group_translations_locale_normalized",
    "ck_group_translations_presentation_shape",
    "ck_groups_metadata_language_agnostic",
    "ck_group_memberships_metadata_language_agnostic",
    "ck_group_feature_bindings_configuration_language_agnostic",
    "group_translations_language_agnostic_insert",
    "group_translations_language_agnostic_update",
    "groups_language_agnostic_metadata_insert",
    "groups_language_agnostic_metadata_update",
    "group_memberships_language_agnostic_metadata_insert",
    "group_memberships_language_agnostic_metadata_update",
    "group_feature_bindings_language_agnostic_configuration_insert",
    "group_feature_bindings_language_agnostic_configuration_update",
    "sqlite_locale_violation",
    "sqlite_language_agnostic_json_violation",
    "Irreversible by design",
  ]) {
    if (!migration.includes(marker)) {
      failures.push(`Groups language-agnostic migration is missing marker: ${marker}`);
    }
  }
}

if (fs.existsSync(path.join(root, "crates/rustok-groups/docs/README.md"))) {
  const contract = read("crates/rustok-groups/docs/README.md");
  for (const marker of [
    "host supplies the already-resolved effective locale",
    "never injects an English or arbitrary first-row fallback",
    "Catalog and search queries are scoped to that effective locale",
    "Unicode scalar values rather than UTF-8 bytes",
    "group_memberships.metadata",
    "group_feature_bindings.configuration",
    "reserved top-level presentation fields",
    "Nested provider-schema fields",
  ]) {
    if (!contract.includes(marker)) {
      failures.push(`Groups multilingual documentation is missing marker: ${marker}`);
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

if (fs.existsSync(path.join(root, "crates/rustok-groups/src/graphql_governance.rs"))) {
  const graphqlGovernance = read("crates/rustok-groups/src/graphql_governance.rs");
  for (const marker of [
    "MergedObject",
    "GroupsMutationRoot",
    "change_group_role",
    "transfer_group_ownership",
    "with_idempotency_key",
    "GroupGovernanceCommandPort",
  ]) {
    if (!graphqlGovernance.includes(marker)) {
      failures.push(`Groups governance GraphQL root is missing marker: ${marker}`);
    }
  }
}

if (fs.existsSync(path.join(root, "crates/rustok-groups/rustok-module.toml"))) {
  const manifest = read("crates/rustok-groups/rustok-module.toml");
  if (!manifest.includes('mutation = "graphql_governance::GroupsMutationRoot"')) {
    failures.push("Groups manifest must publish the merged governance mutation root");
  }
}

if (fs.existsSync(path.join(root, "crates/rustok-groups/admin/src/core.rs"))) {
  const core = read("crates/rustok-groups/admin/src/core.rs");
  for (const marker of [
    "prepare_change_group_role",
    "prepare_transfer_group_ownership",
    "Uuid::parse_str",
    "Uuid::new_v4",
    "GroupsAdminGovernanceInputError",
  ]) {
    if (!core.includes(marker)) {
      failures.push(`Groups admin governance core is missing marker: ${marker}`);
    }
  }
}

if (fs.existsSync(path.join(root, "crates/rustok-groups/admin/src/transport.rs"))) {
  const facade = read("crates/rustok-groups/admin/src/transport.rs");
  for (const marker of [
    "change_group_admin_role",
    "transfer_group_admin_ownership",
    "execute_selected_transport",
    "GROUPS_ADMIN_TRANSPORT_FALLBACK_POLICY",
  ]) {
    if (!facade.includes(marker)) {
      failures.push(`Groups admin governance facade is missing marker: ${marker}`);
    }
  }
}

if (fs.existsSync(path.join(root, "crates/rustok-groups/admin/src/transport/native_server_adapter.rs"))) {
  const nativeAdapter = read("crates/rustok-groups/admin/src/transport/native_server_adapter.rs");
  for (const marker of [
    "groups/admin/governance/change-role",
    "groups/admin/governance/transfer-ownership",
    "GroupGovernanceCommandPort",
    "with_idempotency_key",
  ]) {
    if (!nativeAdapter.includes(marker)) {
      failures.push(`Groups native governance adapter is missing marker: ${marker}`);
    }
  }
}

if (fs.existsSync(path.join(root, "crates/rustok-groups/admin/src/transport/graphql_adapter.rs"))) {
  const graphqlAdapter = read("crates/rustok-groups/admin/src/transport/graphql_adapter.rs");
  for (const marker of [
    "GroupsAdminChangeRole",
    "GroupsAdminTransferOwnership",
    "changeGroupRole",
    "transferGroupOwnership",
  ]) {
    if (!graphqlAdapter.includes(marker)) {
      failures.push(`Groups GraphQL governance adapter is missing marker: ${marker}`);
    }
  }
}

if (fs.existsSync(path.join(root, "crates/rustok-groups/admin/src/ui/leptos.rs"))) {
  const adminUi = read("crates/rustok-groups/admin/src/ui/leptos.rs");
  for (const marker of [
    "prepare_change_group_role",
    "prepare_transfer_group_ownership",
    "change_group_admin_role",
    "transfer_group_admin_ownership",
    "governance_success_message",
    "on_role_submit",
    "on_ownership_submit",
  ]) {
    if (!adminUi.includes(marker)) {
      failures.push(`Groups admin governance UI is missing marker: ${marker}`);
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
  const governanceProfile = registry?.transport_profiles?.find(
    (profile) => profile?.name === "embedded_governance_native",
  );
  for (const surface of ["rust_port", "graphql", "leptos_server_function"]) {
    if (!governanceProfile?.surfaces?.includes(surface)) {
      failures.push(`Groups governance transport profile is missing surface: ${surface}`);
    }
  }
  if (governanceProfile?.implicit_fallback !== false) {
    failures.push("Groups governance transport profile must reject implicit fallback");
  }
}

const governanceLocaleKeys = [
  "groups.admin.governance.title",
  "groups.admin.governance.body",
  "groups.admin.governance.groupId",
  "groups.admin.governance.targetUserId",
  "groups.admin.governance.newOwnerUserId",
  "groups.admin.governance.role",
  "groups.admin.governance.changeRole",
  "groups.admin.governance.transferOwnership",
  "groups.admin.governance.invalidGroupId",
  "groups.admin.governance.invalidTargetUserId",
  "groups.admin.governance.invalidNewOwnerUserId",
  "groups.admin.governance.roleChanged",
  "groups.admin.governance.ownershipTransferred",
];
for (const relative of [
  "crates/rustok-groups/admin/locales/en.json",
  "crates/rustok-groups/admin/locales/ru.json",
]) {
  if (fs.existsSync(path.join(root, relative))) {
    const messages = JSON.parse(read(relative));
    for (const key of governanceLocaleKeys) {
      if (typeof messages[key] !== "string" || messages[key].trim().length === 0) {
        failures.push(`Groups governance locale is missing key ${key}: ${relative}`);
      }
    }
  }
}

for (const relative of [
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

console.log("Groups FFA/FBA, privacy, governance, language-agnostic DB, multilingual, and ownership boundary checks passed.");
