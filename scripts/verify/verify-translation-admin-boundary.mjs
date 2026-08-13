#!/usr/bin/env node

import { existsSync, readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = process.env.RUSTOK_VERIFY_REPO_ROOT
  ? path.resolve(process.env.RUSTOK_VERIFY_REPO_ROOT)
  : path.resolve(scriptDir, "../..");
const failures = [];

const files = {
  lib: "crates/rustok-translation/admin/src/lib.rs",
  core: "crates/rustok-translation/admin/src/core.rs",
  model: "crates/rustok-translation/admin/src/model.rs",
  transport: "crates/rustok-translation/admin/src/transport/mod.rs",
  native:
    "crates/rustok-translation/admin/src/transport/native_server_adapter.rs",
  graphql: "crates/rustok-translation/admin/src/transport/graphql_adapter.rs",
  leptos: "crates/rustok-translation/admin/src/ui/leptos.rs",
  manifest: "crates/rustok-translation/rustok-module.toml",
  adminCargo: "apps/admin/Cargo.toml",
  nextPackage: "apps/next-admin/packages/translation/src/index.tsx",
  nextApi: "apps/next-admin/packages/translation/src/api.ts",
  nextTypes: "apps/next-admin/packages/translation/src/types.ts",
  nextRegistry: "apps/next-admin/src/modules/index.ts",
  nextWrapper: "apps/next-admin/src/modules/translation-admin-client.tsx",
  nextLayout: "apps/next-admin/src/app/dashboard/translation/layout.tsx",
  nextRoute: "apps/next-admin/src/app/dashboard/translation/page.tsx",
  appRouter: "apps/server/src/services/app_router.rs",
  uiInput: "UI/leptos/src/input.rs",
  uiSelect: "UI/leptos/src/select.rs",
  uiTextarea: "UI/leptos/src/textarea.rs",
  uiLabel: "crates/leptos-ui/src/label.rs",
  leptosEn: "crates/rustok-translation/admin/locales/en.json",
  leptosRu: "crates/rustok-translation/admin/locales/ru.json",
  nextEn: "apps/next-admin/messages/en.json",
  nextRu: "apps/next-admin/messages/ru.json",
};

function absolute(relativePath) {
  return path.join(repoRoot, relativePath);
}

function fail(message) {
  failures.push(message);
}

function read(relativePath) {
  if (!existsSync(absolute(relativePath))) {
    fail(`${relativePath}: expected file`);
    return "";
  }
  return readFileSync(absolute(relativePath), "utf8");
}

function contains(text, marker, message) {
  const found =
    typeof marker === "string" ? text.includes(marker) : marker.test(text);
  if (!found) fail(message);
}

function excludes(text, marker, message) {
  const found =
    typeof marker === "string" ? text.includes(marker) : marker.test(text);
  if (found) fail(message);
}

function flatten(value, prefix = "") {
  if (value && typeof value === "object" && !Array.isArray(value)) {
    return Object.entries(value).flatMap(([key, child]) =>
      flatten(child, prefix ? `${prefix}.${key}` : key),
    );
  }
  return [prefix];
}

const source = Object.fromEntries(
  Object.entries(files).map(([name, relativePath]) => [
    name,
    read(relativePath),
  ]),
);

for (const marker of [
  "leptos::",
  "leptos_",
  "#[component]",
  "#[server]",
  "RwSignal",
  "LocalResource",
  "web_sys::",
]) {
  excludes(
    source.core,
    marker,
    `${files.core}: framework-neutral core contains ${marker}`,
  );
}
excludes(
  source.core,
  "#[allow(",
  `${files.core}: command construction must not suppress lint diagnostics`,
);

contains(
  source.lib,
  "pub use ui::leptos::TranslationAdmin;",
  `${files.lib}: crate root must export TranslationAdmin`,
);
contains(
  source.leptos,
  "use crate::transport;",
  `${files.leptos}: Leptos UI must call the transport facade`,
);
excludes(
  source.leptos,
  "graphql_adapter::",
  `${files.leptos}: Leptos UI must not select the raw GraphQL adapter`,
);
excludes(
  source.leptos,
  "native_server_adapter::",
  `${files.leptos}: Leptos UI must not select the raw native adapter`,
);
contains(
  source.leptos,
  "UiRouteContext",
  `${files.leptos}: Leptos UI must consume host route locale context`,
);
contains(
  source.leptos,
  "core::TAB_QUERY_KEY",
  `${files.leptos}: Leptos UI must use the core-owned tab query key`,
);
for (const marker of [
  'role="tablist"',
  'role="tab"',
  'aria-selected=if is_selected { "true" } else { "false" }',
  "aria-controls=panel_id",
  'role="tabpanel"',
  "aria-labelledby=tab_id",
  '"ArrowLeft" | "ArrowUp"',
  '"ArrowRight" | "ArrowDown"',
  '"Home" =>',
  '"End" =>',
]) {
  contains(
    source.leptos,
    marker,
    `${files.leptos}: missing accessible tab marker ${marker}`,
  );
}
if (/<Label(?![^>]*r#for=)/.test(source.leptos)) {
  fail(`${files.leptos}: every visible form label must identify its control`);
}
const translationFormControls =
  source.leptos.match(/<(?:Input|Textarea|Select)\b[\s\S]*?\/>/g) ?? [];
for (const control of translationFormControls) {
  const name = control.match(/\bname="([^"]+)"/)?.[1];
  if (name && !control.includes(`id="${name}"`)) {
    fail(
      `${files.leptos}: named form control ${name} must publish the matching explicit id`,
    );
  }
}
for (const [name, element] of [
  ["uiInput", "<input"],
  ["uiSelect", "<select"],
  ["uiTextarea", "<textarea"],
]) {
  contains(
    source[name],
    element,
    `${files[name]}: expected shared form control`,
  );
  contains(
    source[name],
    "id=id",
    `${files[name]}: named shared form controls must publish stable ids`,
  );
}
contains(
  source.uiLabel,
  "for=r#for",
  `${files.uiLabel}: shared labels must preserve explicit control association`,
);

contains(
  source.transport,
  "execute_selected_transport",
  `${files.transport}: transport facade must use selected-path execution`,
);
contains(
  source.transport,
  "without protocol fallback",
  `${files.transport}: transport facade must document no protocol fallback`,
);
contains(
  source.native,
  "#[server",
  `${files.native}: native adapter must expose a server function`,
);
contains(
  source.native,
  "HostRuntimeContext",
  `${files.native}: native adapter must use host runtime context`,
);
for (const runtimeEvidence of [
  "native_interchange_executes_authenticated_http_parity",
  "native_interchange_artifacts_execute_authenticated_http_parity",
  "native_policy_and_glossary_execute_authenticated_http_parity",
  "native_human_workflow_memory_and_progress_execute_http_parity",
  "native_qa_rejection_and_job_cancellation_execute_http_parity",
  "native_retry_and_apply_recovery_execute_http_parity",
  "native_inventory_and_provider_progress_execute_http_parity",
  "native_machine_operations_execute_authenticated_http_parity",
]) {
  contains(
    source.native,
    runtimeEvidence,
    `${files.native}: missing authenticated HTTP server-function evidence ${runtimeEvidence}`,
  );
}
contains(
  source.appRouter,
  "application_router_executes_authenticated_server_function",
  `${files.appRouter}: missing full application-router Translation runtime evidence`,
);
contains(
  source.appRouter,
  "compose_application_router(",
  `${files.appRouter}: Translation runtime evidence must execute the production router composition`,
);
contains(
  source.graphql,
  "rustok_graphql",
  `${files.graphql}: GraphQL adapter must use rustok-graphql`,
);
for (const marker of [
  "GraphqlMachineProposalOutcome",
  "MachineTranslationOperationStatus",
]) {
  contains(
    source.graphql,
    marker,
    `${files.graphql}: machine proposal generation must preserve the typed polling outcome (${marker})`,
  );
}
contains(
  source.native,
  "map_machine_proposal_outcome",
  `${files.native}: native adapter must preserve the typed machine polling outcome`,
);
for (const marker of [
  "MACHINE_PROPOSAL_OUTCOME_FIELDS",
  "MachineProposalOutcome",
  "MachineTranslationOperationStatus",
]) {
  contains(
    source.nextApi,
    marker,
    `${files.nextApi}: Next adapter must preserve the typed machine polling outcome (${marker})`,
  );
}

const rustOperations = [
  "ReadPolicy",
  "ReadMachineOperationStatus",
  "ListTargets",
  "ListGlossaries",
  "ReadGlossary",
  "ListMemoryEntries",
  "ReadMemoryEntry",
  "LookupMemory",
  "ReadJobProgress",
  "ReadReviewerQueue",
  "ReadReviewerWorkload",
  "ListWorkflowNotes",
  "ListInterchangeArtifacts",
  "ReadInterchangeArtifact",
  "ExportJob",
  "ReadProviderProgress",
  "ReadRequiredProviderProgress",
  "ReplacePolicy",
  "CreateWorkflowNote",
  "ResolveWorkflowNote",
  "CreateInterchangeExportArtifact",
  "StoreInterchangeImportArtifact",
  "ProcessInterchangeImportArtifact",
  "CreateGlossary",
  "UpdateGlossary",
  "ReplaceGlossaryTerms",
  "SetGlossaryActive",
  "SetMemoryRetention",
  "TombstoneMemoryEntry",
  "PurgeMemoryEntry",
  "CreateJob",
  "AddItem",
  "SaveProposal",
  "ImportItem",
  "EstimateMachineTranslation",
  "GenerateMachineProposal",
  "CancelMachineOperation",
  "RecoverMachineOperation",
  "SubmitProposal",
  "ApproveProposal",
  "ApplyProposal",
  "AssignItem",
  "UnassignItem",
  "CancelJob",
  "RetryItem",
  "RecoverApply",
  "RebuildJobProgress",
  "SyncProviderInventory",
  "RebuildProviderInventory",
];
for (const operation of rustOperations) {
  contains(
    source.model,
    operation,
    `${files.model}: missing typed operation ${operation}`,
  );
}

const sharedWorkbenchOperations = [
  "read_policy",
  "read_machine_operation_status",
  "list_targets",
  "list_glossaries",
  "read_glossary",
  "list_memory_entries",
  "read_memory_entry",
  "lookup_memory",
  "read_job_progress",
  "read_reviewer_queue",
  "read_reviewer_workload",
  "list_workflow_notes",
  "list_interchange_artifacts",
  "read_interchange_artifact",
  "replace_policy",
  "create_workflow_note",
  "resolve_workflow_note",
  "create_interchange_export_artifact",
  "store_interchange_import_artifact",
  "process_interchange_import_artifact",
  "create_glossary",
  "update_glossary",
  "replace_glossary_terms",
  "set_glossary_active",
  "set_memory_retention",
  "tombstone_memory_entry",
  "purge_memory_entry",
  "create_job",
  "export_job",
  "import_item",
  "rebuild_job_progress",
  "sync_inventory",
  "rebuild_inventory",
  "read_provider_progress",
  "read_required_progress",
  "add_item",
  "save_proposal",
  "estimate_machine_translation",
  "generate_machine_proposal",
  "cancel_machine_operation",
  "recover_machine_operation",
  "assign_item",
  "unassign_item",
  "cancel_job",
  "retry_item",
  "recover_apply",
  "submit_proposal",
  "approve_proposal",
  "apply_proposal",
];
for (const operation of sharedWorkbenchOperations) {
  contains(
    source.nextTypes,
    `'${operation}'`,
    `${files.nextTypes}: Next workbench is missing ${operation}`,
  );
  contains(
    source.nextApi,
    `case '${operation}'`,
    `${files.nextApi}: Next GraphQL adapter is missing ${operation}`,
  );
}

for (const marker of [
  "core::MachineProposalCommand::Estimate",
  "core::MachineProposalCommand::Generate",
  "core::read_machine_operation_status_operation",
  "core::cancel_machine_operation",
  "core::recover_machine_operation",
]) {
  contains(
    source.leptos,
    marker,
    `${files.leptos}: Workflow UI is missing machine control ${marker}`,
  );
}
for (const operation of [
  "estimate_machine_translation",
  "generate_machine_proposal",
  "read_machine_operation_status",
  "cancel_machine_operation",
  "recover_machine_operation",
]) {
  contains(
    source.nextPackage,
    `'${operation}'`,
    `${files.nextPackage}: Workflow UI is missing machine control ${operation}`,
  );
}
for (const marker of [
  "core::assign_item_operation",
  "core::unassign_item_operation",
  "core::cancel_job_operation",
  "core::retry_item_operation",
  "core::recover_apply_operation",
]) {
  contains(
    source.leptos,
    marker,
    `${files.leptos}: Workflow UI is missing recovery control ${marker}`,
  );
}
for (const marker of [
  "core::list_interchange_artifacts_operation",
  "core::read_interchange_artifact_operation",
  "core::create_interchange_export_artifact_operation",
  "core::store_interchange_import_artifact_operation",
  "core::process_interchange_import_artifact_operation",
]) {
  contains(
    source.leptos,
    marker,
    `${files.leptos}: Workflow UI is missing interchange artifact control ${marker}`,
  );
}
for (const operation of [
  "ListInterchangeArtifacts",
  "ReadInterchangeArtifact",
  "CreateInterchangeExportArtifact",
  "StoreInterchangeImportArtifact",
  "ProcessInterchangeImportArtifact",
]) {
  contains(
    source.native,
    `TranslationAdminOperation::${operation}`,
    `${files.native}: native adapter is missing interchange artifact operation ${operation}`,
  );
  contains(
    source.graphql,
    `TranslationAdminOperation::${operation}`,
    `${files.graphql}: GraphQL adapter is missing interchange artifact operation ${operation}`,
  );
}
for (const operation of [
  "assign_item",
  "unassign_item",
  "cancel_job",
  "retry_item",
  "recover_apply",
]) {
  contains(
    source.nextPackage,
    `'${operation}'`,
    `${files.nextPackage}: Workflow UI is missing recovery control ${operation}`,
  );
}

for (const tab of [
  "overview",
  "jobs",
  "glossaries",
  "memory",
  "inventory",
  "workflow",
]) {
  contains(
    source.core,
    `"${tab}"`,
    `${files.core}: Rust tab contract is missing ${tab}`,
  );
  contains(
    source.nextPackage,
    `'${tab}'`,
    `${files.nextPackage}: Next tab contract is missing ${tab}`,
  );
}

for (const marker of [
  'ui_classification = "admin_only"',
  'recommended_admin_surfaces = ["leptos-admin", "next-admin"]',
  "[provides.admin_ui]",
  'leptos_crate = "rustok-translation-admin"',
  'route_segment = "translation"',
  'next_package = "@rustok/translation-admin"',
  'leptos_locales_path = "admin/locales"',
]) {
  contains(
    source.manifest,
    marker,
    `${files.manifest}: missing manifest marker ${marker}`,
  );
}

for (const profile of ["csr", "hydrate", "ssr"]) {
  contains(
    source.adminCargo,
    `"rustok-translation-admin/${profile}"`,
    `${files.adminCargo}: missing Translation ${profile} feature wiring`,
  );
}
contains(
  source.adminCargo,
  'rustok-translation-admin = { path = "../../crates/rustok-translation/admin"',
  `${files.adminCargo}: missing Translation admin dependency`,
);
contains(
  source.nextRegistry,
  "packages/translation/src",
  `${files.nextRegistry}: Next module registry must import Translation package`,
);
contains(
  source.nextWrapper,
  "graphql={graphqlRequest}",
  `${files.nextWrapper}: host wrapper must inject the shared GraphQL executor`,
);
contains(
  source.nextRoute,
  "<TranslationAdminClient",
  `${files.nextRoute}: Next route must compose the package-owned workbench`,
);
contains(
  source.nextLayout,
  "<ModuleGuard",
  `${files.nextLayout}: Next route must enforce tenant module enablement`,
);
contains(
  source.nextLayout,
  "slug='translation'",
  `${files.nextLayout}: Next route must guard the Translation module slug`,
);
for (const marker of [
  "executeTranslationOperation",
  "TranslationOperation",
  "replaceTranslationPolicy",
  "createTranslationJob",
]) {
  excludes(
    source.nextRoute,
    marker,
    `${files.nextRoute}: host route must not own Translation operation ${marker}`,
  );
}
contains(
  source.nextPackage,
  "useTranslations('translation')",
  `${files.nextPackage}: Next package must consume host next-intl messages`,
);
contains(
  source.nextPackage,
  "searchParams.get('tab')",
  `${files.nextPackage}: Next package must keep tab selection in the URL`,
);
contains(
  source.nextPackage,
  "searchParams.get('glossary_id')",
  `${files.nextPackage}: Next package must keep glossary selection in the URL`,
);
contains(
  source.leptos,
  "core::GLOSSARY_ID_QUERY_KEY",
  `${files.leptos}: Leptos package must keep glossary selection in the URL`,
);
contains(
  source.nextPackage,
  "searchParams.get('memory_entry_id')",
  `${files.nextPackage}: Next package must keep memory selection in the URL`,
);
contains(
  source.leptos,
  "core::MEMORY_ENTRY_ID_QUERY_KEY",
  `${files.leptos}: Leptos package must keep memory selection in the URL`,
);
excludes(
  source.nextApi,
  /\bfetch\s*\(/,
  `${files.nextApi}: Next adapter must use the injected host GraphQL executor`,
);
for (const marker of [
  "document.cookie",
  "localStorage",
  "sessionStorage",
  "rustok-admin-locale",
]) {
  excludes(
    source.nextPackage,
    marker,
    `${files.nextPackage}: package-local locale fallback is forbidden (${marker})`,
  );
  excludes(
    source.leptos,
    marker,
    `${files.leptos}: package-local locale fallback is forbidden (${marker})`,
  );
}

try {
  const leptosEnKeys = flatten(JSON.parse(source.leptosEn)).sort();
  const leptosRuKeys = flatten(JSON.parse(source.leptosRu)).sort();
  const nextEn = JSON.parse(source.nextEn);
  const nextRu = JSON.parse(source.nextRu);
  const nextEnKeys = flatten(nextEn.translation, "translation").sort();
  const nextRuKeys = flatten(nextRu.translation, "translation").sort();
  const expected = JSON.stringify(leptosEnKeys);
  if (JSON.stringify(leptosRuKeys) !== expected) {
    fail("Translation Leptos locale bundles do not expose identical keys");
  }
  if (JSON.stringify(nextEnKeys) !== expected) {
    fail("Next English Translation keys differ from the Leptos contract");
  }
  if (JSON.stringify(nextRuKeys) !== expected) {
    fail("Next Russian Translation keys differ from the Leptos contract");
  }
} catch (error) {
  fail(`Translation locale bundle parsing failed: ${error.message}`);
}

const staleSupportPath = "crates/rustok-translation-admin-support";
if (existsSync(absolute(staleSupportPath))) {
  fail(`${staleSupportPath}: superseded support package must stay removed`);
}

if (failures.length > 0) {
  console.error("Translation admin boundary verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("Translation admin boundary verification passed");
