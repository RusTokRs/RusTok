import fs from "node:fs";
import path from "node:path";

const root = process.cwd();
const failures = [];
const read = (relative) => fs.readFileSync(path.join(root, relative), "utf8");
const exists = (relative) => fs.existsSync(path.join(root, relative));

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
  "crates/rustok-groups/src/localization.rs",
  "crates/rustok-groups/src/invitations.rs",
  "crates/rustok-groups/src/targeted_invitations.rs",
  "crates/rustok-groups/src/applications.rs",
  "crates/rustok-groups/src/applications_legacy.rs",
  "crates/rustok-groups/src/applications_cas.rs",
  "crates/rustok-groups/src/policy_history.rs",
  "crates/rustok-groups/src/governance.rs",
  "crates/rustok-groups/src/graphql_applications.rs",
  "crates/rustok-groups/src/graphql_policy_history.rs",
  "crates/rustok-groups/src/graphql_application_cas.rs",
  "crates/rustok-groups/src/migrations/m20260722_000007_create_group_membership_policy_revisions.rs",
  "crates/rustok-groups/admin/src/core.rs",
  "crates/rustok-groups/admin/src/application_core.rs",
  "crates/rustok-groups/admin/src/transport.rs",
  "crates/rustok-groups/admin/src/ui/root.rs",
  "crates/rustok-groups/admin/src/ui/applications.rs",
  "crates/rustok-groups/admin/src/ui/policy_editor.rs",
  "crates/rustok-groups/storefront/src/core.rs",
  "crates/rustok-groups/storefront/src/application_core.rs",
  "crates/rustok-groups/storefront/src/transport.rs",
  "crates/rustok-groups/storefront/src/ui/leptos.rs",
  "crates/rustok-groups/storefront/src/ui/application.rs",
  "scripts/verify/verify-groups-localization-boundary.mjs",
  "scripts/verify/verify-groups-invitations-boundary.mjs",
  "scripts/verify/verify-groups-targeted-invitation-delivery.mjs",
  "scripts/verify/verify-groups-membership-applications.mjs",
  "scripts/verify/verify-groups-membership-policy-revisions.mjs",
  "scripts/verify/verify-groups-application-policy-cas.mjs",
];

for (const relative of required) {
  if (!exists(relative)) failures.push(`missing required Groups artifact: ${relative}`);
}

for (const corePath of [
  "crates/rustok-groups/admin/src/core.rs",
  "crates/rustok-groups/admin/src/application_core.rs",
  "crates/rustok-groups/storefront/src/core.rs",
  "crates/rustok-groups/storefront/src/application_core.rs",
]) {
  if (exists(corePath) && /use\s+leptos|leptos::/.test(read(corePath))) {
    failures.push(`FFA core must remain framework-neutral: ${corePath}`);
  }
}

for (const uiPath of [
  "crates/rustok-groups/admin/src/ui/leptos.rs",
  "crates/rustok-groups/admin/src/ui/localization.rs",
  "crates/rustok-groups/admin/src/ui/invitations.rs",
  "crates/rustok-groups/admin/src/ui/applications.rs",
  "crates/rustok-groups/admin/src/ui/policy_editor.rs",
  "crates/rustok-groups/storefront/src/ui/leptos.rs",
  "crates/rustok-groups/storefront/src/ui/invitation_acceptance.rs",
  "crates/rustok-groups/storefront/src/ui/application.rs",
]) {
  if (!exists(uiPath)) continue;
  const ui = read(uiPath);
  if (!ui.includes("crate::transport")) {
    failures.push(`Leptos UI must consume the Groups transport facade: ${uiPath}`);
  }
  if (/graphql_(?:applications|invitations|policy_history|policy_locale)?_?adapter|native_(?:applications|invitations|policy_history|policy_locale)?_?adapter|native_server_adapter/.test(ui)) {
    failures.push(`Leptos UI must not import raw transport adapters: ${uiPath}`);
  }
}

const serviceFiles = [
  "crates/rustok-groups/src/service.rs",
  "crates/rustok-groups/src/localization.rs",
  "crates/rustok-groups/src/invitations.rs",
  "crates/rustok-groups/src/targeted_invitations.rs",
  "crates/rustok-groups/src/applications_legacy.rs",
  "crates/rustok-groups/src/applications_cas.rs",
  "crates/rustok-groups/src/policy_history.rs",
  "crates/rustok-groups/src/governance.rs",
];
for (const servicePath of serviceFiles) {
  if (!exists(servicePath)) continue;
  const service = read(servicePath);
  for (const forbidden of [
    "rustok_forum::entities",
    "rustok_blog::entities",
    "rustok_pages::entities",
    "rustok_product::entities",
    "rustok_marketplace_listing::entities",
  ]) {
    if (service.includes(forbidden)) {
      failures.push(`Groups owner service must not import foreign persistence: ${servicePath} -> ${forbidden}`);
    }
  }
}

if (exists("crates/rustok-groups/src/service.rs")) {
  const service = read("crates/rustok-groups/src/service.rs");
  for (const marker of [
    "GroupAction::ViewSummary",
    "GroupVisibility::Closed.as_str()",
    "include_private_content",
    "PortCallPolicy::read()",
    "PortCallPolicy::write()",
    "translation::Column::Locale.eq(effective_locale.clone())",
  ]) {
    if (!service.includes(marker)) failures.push(`Groups core service is missing marker: ${marker}`);
  }
  for (const forbidden of ["PLATFORM_FALLBACK_LOCALE", "build_locale_candidates", "rows.first()"] ) {
    if (service.includes(forbidden)) failures.push(`Groups must not own locale fallback policy: ${forbidden}`);
  }
}

if (exists("crates/rustok-groups/src/policy_history.rs")) {
  const history = read("crates/rustok-groups/src/policy_history.rs");
  for (const marker of [
    "GroupApplicationPolicyHistoryReadPort",
    "GroupApplicationPolicyHistoryService",
    "PortCallPolicy::read()",
    "GroupApplicationReadPort::list_group_membership_applications",
  ]) {
    if (!history.includes(marker)) failures.push(`Groups policy history boundary is missing marker: ${marker}`);
  }
}

if (exists("crates/rustok-groups/src/applications_cas.rs")) {
  const cas = read("crates/rustok-groups/src/applications_cas.rs");
  for (const marker of [
    "GroupApplicationCasCommandPort",
    "GROUP_APPLICATION_POLICY_CHANGED_CODE",
    "ensure_policy_update_precondition",
    "ensure_loaded_policy_precondition",
  ]) {
    if (!cas.includes(marker)) failures.push(`Groups application CAS boundary is missing marker: ${marker}`);
  }
}

if (exists("crates/rustok-groups/rustok-module.toml")) {
  const manifest = read("crates/rustok-groups/rustok-module.toml");
  for (const marker of [
    'query = "graphql_application_cas::GroupsQueryRoot"',
    'mutation = "graphql_application_cas::GroupsMutationRoot"',
    'subpath = "applications"',
    'subpath = "invitations"',
  ]) {
    if (!manifest.includes(marker)) failures.push(`Groups manifest is missing final composition marker: ${marker}`);
  }
}

if (exists("crates/rustok-groups/src/graphql_application_cas.rs")) {
  const graphql = read("crates/rustok-groups/src/graphql_application_cas.rs");
  for (const marker of [
    "GroupsBaseQueryRoot",
    "GroupsBaseMutationRoot",
    "GroupsApplicationCasMutation",
    "upsert_group_application_policy_if_current",
    "submit_group_membership_application_if_current",
  ]) {
    if (!graphql.includes(marker)) failures.push(`Groups final GraphQL root is missing marker: ${marker}`);
  }
}

if (exists("crates/rustok-groups/src/ports.rs")) {
  const ports = read("crates/rustok-groups/src/ports.rs");
  for (const marker of [
    "GroupSummaryReadPort",
    "GroupMembershipReadPort",
    "GroupAccessReadPort",
    "GroupLocalizationReadPort",
    "GroupInvitationReadPort",
    "GroupTargetedInvitationCommandPort",
    "GroupApplicationReadPort",
    "GroupApplicationPolicyHistoryReadPort",
    "GroupApplicationCasCommandPort",
    "GroupApplicationCommandPort",
    "GroupGovernanceCommandPort",
    'private_content_fallback: "deny"',
    "implicit_transport_fallback: false",
  ]) {
    if (!ports.includes(marker)) failures.push(`Groups capability descriptor is missing marker: ${marker}`);
  }
}

for (const facadePath of [
  "crates/rustok-groups/admin/src/transport.rs",
  "crates/rustok-groups/storefront/src/transport.rs",
]) {
  if (!exists(facadePath)) continue;
  const facade = read(facadePath);
  if (!facade.includes("execute_selected_transport")) {
    failures.push(`Groups facade must use the selected transport executor: ${facadePath}`);
  }
  if (!facade.includes('"never falls back"')) {
    failures.push(`Groups facade must declare no implicit fallback: ${facadePath}`);
  }
}

if (exists("crates/rustok-groups/contracts/groups-fba-registry.json")) {
  const registry = JSON.parse(read("crates/rustok-groups/contracts/groups-fba-registry.json"));
  if (registry?.status !== "in_progress") failures.push("Groups FBA registry must remain in_progress until runtime evidence exists");
  if (registry?.privacy?.default_on_provider_unavailable !== "deny_private_content") failures.push("Groups privacy fallback must fail closed");
  if (registry?.privacy?.secret_group_direct_read !== "not_found_without_membership_or_platform_manage") failures.push("Secret group direct reads must preserve non-disclosure");
  if (registry?.feature_provider?.implicit_fallback !== false) failures.push("Feature providers must not use implicit fallback");
  if (registry?.membership_applications?.module_local_fallback !== false) failures.push("Application policy must not own locale fallback");
  if (registry?.membership_applications?.transport_fallback !== "never") failures.push("Application transport must never fall back implicitly");
  if (registry?.membership_applications?.policy_revision_history !== "implemented_source") failures.push("Policy revision history must remain source-only before runtime evidence");
  if (registry?.membership_applications?.atomic_expected_revision_guard !== "implemented_source") failures.push("Atomic expected-revision guard must be source-complete");
  if (registry?.evidence?.membership_application_policy_cas !== null) failures.push("Unexecuted application CAS evidence must remain null");
}

for (const localePath of [
  "crates/rustok-groups/admin/locales/en.json",
  "crates/rustok-groups/admin/locales/ru.json",
  "crates/rustok-groups/storefront/locales/en.json",
  "crates/rustok-groups/storefront/locales/ru.json",
]) {
  if (exists(localePath)) JSON.parse(read(localePath));
}

if (failures.length > 0) {
  console.error("Groups aggregate FFA/FBA boundary verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("Groups aggregate ownership, privacy, exact-locale, FFA/FBA, application CAS/history, invitation, governance, and no-fallback boundary checks passed.");
