#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = path.resolve(
  path.dirname(fileURLToPath(import.meta.url)),
  "..",
  "..",
  "..",
  "..",
);
const failures = [];
const files = {
  contract:
    "crates/rustok-pages/contracts/evidence/pages-published-metadata-browser-execution-contract.json",
  surfaceEvidence:
    "crates/rustok-pages/contracts/evidence/pages-published-metadata-surface-source.json",
  revisionEvidence:
    "crates/rustok-pages/contracts/evidence/pages-metadata-revision-isolation-source.json",
  consumerContract:
    "crates/rustok-page-builder/contracts/page-builder-consumer-properties.json",
  surface: "crates/rustok-pages/admin/src/standalone_metadata.rs",
  panel: "crates/rustok-page-builder/admin/src/editor/consumer_properties.rs",
  config: "apps/next-admin/playwright.pages-published-metadata.config.ts",
  setup: "apps/next-admin/tests/pages-published-metadata/global-setup.ts",
  runner: "apps/next-admin/tests/pages-published-metadata/browser-evidence.spec.ts",
  plan: "docs/modules/pages-page-builder-parity-continuation-plan.md",
  actualization:
    "docs/modules/pages-published-metadata-browser-evidence-harness-actualization-2026-08-13.md",
};

const absolute = (relativePath) => path.join(repoRoot, relativePath);
const read = (relativePath) => fs.readFileSync(absolute(relativePath), "utf8");
const requireValue = (condition, message) => {
  if (!condition) failures.push(message);
};
const requireText = (source, marker, label) => {
  if (!source.includes(marker)) failures.push(`${label}: missing ${marker}`);
};
const forbidText = (source, marker, label) => {
  if (source.includes(marker)) failures.push(`${label}: forbidden ${marker}`);
};

for (const relativePath of Object.values(files)) {
  if (!fs.existsSync(absolute(relativePath))) {
    failures.push(`${relativePath}: missing`);
    continue;
  }
  const stats = fs.lstatSync(absolute(relativePath));
  if (!stats.isFile() || stats.isSymbolicLink()) {
    failures.push(`${relativePath}: must be a regular non-symlink file`);
  }
}
if (failures.length > 0) {
  console.error("[verify-pages-published-metadata-browser-evidence-harness] FAIL");
  failures.forEach((failure) => console.error(`- ${failure}`));
  process.exit(1);
}

const contract = JSON.parse(read(files.contract));
const surfaceEvidence = JSON.parse(read(files.surfaceEvidence));
const revisionEvidence = JSON.parse(read(files.revisionEvidence));
const consumerContract = JSON.parse(read(files.consumerContract));
const surface = read(files.surface);
const panel = read(files.panel);
const config = read(files.config);
const setup = read(files.setup);
const runner = read(files.runner);
const plan = read(files.plan);
const actualization = read(files.actualization);

requireValue(contract.schema_version === 1, "browser contract schema version drifted");
requireValue(contract.module === "pages", "browser contract module drifted");
requireValue(
  contract.packet === "published_metadata_surface_browser_evidence",
  "browser contract packet identity drifted",
);
requireValue(
  contract.status === "source_ready_maintainer_execution_pending",
  "browser contract must remain source-only before execution",
);
requireValue(
  contract.runner === files.runner &&
    contract.config === files.config &&
    contract.global_setup === files.setup,
  "browser contract runner/config/setup binding drifted",
);
requireValue(
  contract.output?.environment ===
      "RUSTOK_PAGES_PUBLISHED_METADATA_BROWSER_EVIDENCE_OUTPUT" &&
    contract.output?.default_path ===
      "target/pages-published-metadata-browser-evidence.json" &&
    contract.output?.format === "pages_published_metadata_browser_execution_v1" &&
    contract.output?.status ===
      "browser_execution_passed_consumer_properties_admission_pending",
  "browser output contract drifted",
);
requireValue(
  JSON.stringify(contract.profiles) ===
    JSON.stringify(["published", "draft", "archived", "missing"]),
  "browser profile matrix drifted",
);

for (const key of [
  "source_commit",
  "deployment_digest",
  "editor_storage_state",
  "published_url",
  "draft_url",
  "archived_url",
  "missing_url",
]) {
  requireValue(
    typeof contract.environment?.[key] === "string" &&
      contract.environment[key].startsWith("RUSTOK_PAGES_PUBLISHED_METADATA_"),
    `browser environment.${key} drifted`,
  );
}

requireValue(
  contract.deployment_identity?.source_commit_verified_against_checkout_head === true &&
    contract.deployment_identity?.deployment_digest_is_maintainer_supplied_reviewed_identity === true &&
    contract.deployment_identity?.browser_independent_digest_to_deployment_attestation === false &&
    contract.deployment_identity?.deployment_provenance_must_be_verified_outside_this_browser_packet === true,
  "deployment identity boundary drifted",
);

for (const required of [
  files.contract,
  files.config,
  files.setup,
  files.runner,
  files.surface,
  files.panel,
  files.consumerContract,
  files.surfaceEvidence,
  files.revisionEvidence,
]) {
  requireValue(
    Array.isArray(contract.required_source_files) &&
      contract.required_source_files.includes(required),
    `required_source_files missing ${required}`,
  );
}

for (const retained of [
  "exact source commit",
  "maintainer-supplied reviewed immutable deployment RepoDigest",
  "source-file SHA-256 hashes",
  "storage-state file SHA-256 hash and byte size",
  "profile URL SHA-256 hashes",
  "bounded boolean/count observations",
]) {
  requireValue(
    contract.retained_data?.includes(retained),
    `retained data contract missing ${retained}`,
  );
}
for (const forbidden of [
  "raw profile URLs",
  "cookies",
  "authorization headers",
  "storage-state contents",
  "raw DOM or HTML",
  "metadata field values",
  "tenant or actor identifiers",
]) {
  requireValue(
    contract.forbidden_retained_data?.includes(forbidden),
    `forbidden retained data contract missing ${forbidden}`,
  );
}
for (const claim of [
  "browser execution",
  "browser-independent digest-to-deployment attestation",
  "metadata persistence mutation",
  "consumer_properties_contract executed evidence",
  "Pages FFA promotion",
  "Page Builder FBA promotion",
]) {
  requireValue(contract.not_claimed?.includes(claim), `not_claimed missing ${claim}`);
}

requireValue(
  surfaceEvidence.status === "pages_published_metadata_surface_source_unvalidated" &&
    surfaceEvidence.source_contract?.browser_execution_contract_linked === true &&
    surfaceEvidence.source_contract?.browser_execution_required_for_executed_evidence === true,
  "published metadata source/browser binding drifted",
);
requireValue(
  surfaceEvidence.browser_execution?.state ===
      "source_ready_maintainer_execution_pending" &&
    surfaceEvidence.browser_execution?.contract === files.contract &&
    surfaceEvidence.browser_execution?.runner === files.runner &&
    surfaceEvidence.browser_execution?.verifier ===
      "crates/rustok-pages/scripts/verify/verify-pages-published-metadata-browser-evidence-harness.mjs",
  "published metadata browser registration drifted",
);
requireValue(
  Array.isArray(surfaceEvidence.execution) && surfaceEvidence.execution.length === 0 &&
    surfaceEvidence.validation?.browser_run === false,
  "published metadata source must not claim browser execution",
);
requireValue(
  revisionEvidence.status === "pages_metadata_revision_isolation_source_unvalidated" &&
    revisionEvidence.source_contract?.dirty_fly_state_mutated_by_metadata_port === false &&
    revisionEvidence.source_contract?.metadata_revision_advances_independently === true,
  "linked metadata revision/isolation source drifted",
);
requireValue(
  consumerContract.status === "metadata_surface_cutover_complete" &&
    consumerContract.executed_evidence === "pending" &&
    consumerContract.pages_consumer?.published_surface?.state === "source_connected",
  "consumer-properties execution boundary drifted",
);

for (const marker of [
  'data-pages-published-metadata-surface="registered"',
  'data-pages-published-metadata-admission="published-only"',
  'data-pages-fly-canvas-mounted="false"',
  'data-pages-document-authoring="false"',
  'data-pages-metadata-runtime="registered"',
  'data-pages-metadata-persistence="owner-port"',
  "<ConsumerPropertiesPanel",
  "PublishedMetadataSurfaceAdmission::Hidden",
  "PublishedMetadataSurfaceAdmission::Registered",
  'page.status.eq_ignore_ascii_case("published")',
]) {
  requireText(surface, marker, "published metadata surface");
}
for (const forbidden of [
  "PagesBuilderFacade",
  "PageBuilderAdminHostContext",
  "patch_page_metadata(",
  "save_page_document",
]) {
  forbidText(surface.split("#[cfg(test)]")[0], forbidden, "published metadata surface");
}
for (const marker of [
  'data-fly-consumer-properties="ready"',
  "data-fly-consumer-property-editor=property_editor_id",
  'format!("fly-consumer-property-{}", field.id)',
  '"Save properties"',
]) {
  requireText(panel, marker, "registered consumer properties panel");
}

for (const marker of [
  'testDir: "./tests/pages-published-metadata"',
  'globalSetup: "./tests/pages-published-metadata/global-setup.ts"',
  "fullyParallel: false",
  "forbidOnly: true",
  "retries: 0",
  "workers: 1",
  'trace: "off"',
  'screenshot: "off"',
  'video: "off"',
  'name: "pages-published-metadata-chromium"',
]) {
  requireText(config, marker, "Playwright config");
}
for (const marker of [
  "pages-published-metadata-browser-execution-contract.json",
  "browser evidence output must remain inside repository target/",
  "rmSync(resolveOutput(contract), { force: true })",
]) {
  requireText(setup, marker, "browser global setup");
}

for (const marker of [
  "execFileSync(\"git\", [\"rev-parse\", \"HEAD\"]",
  "sourceCommit !== head",
  "required_source_files",
  "sourceHashes()",
  "deployment digest must be an immutable image RepoDigest",
  "credential-free HTTP(S) URL without a fragment",
  "browser evidence output must remain inside repository target/",
  'surfaceSelector = "[data-pages-published-metadata-surface=\'registered\']"',
  'panelSelector = "[data-fly-consumer-properties=\'ready\']"',
  'expect(contract.profiles).toEqual(["published", "draft", "archived", "missing"])',
  '"data-pages-published-metadata-admission"',
  '"data-pages-fly-canvas-mounted"',
  '"data-pages-document-authoring"',
  '"data-pages-metadata-runtime"',
  '"data-pages-metadata-persistence"',
  '"rustok.pages.metadata.editor"',
  '"#fly-consumer-property-title"',
  '"#fly-consumer-property-slug"',
  'name: "Save properties"',
  'await assertHiddenProfile(browser, "draft")',
  'await assertHiddenProfile(browser, "archived")',
  'await assertHiddenProfile(browser, "missing")',
  "retained_secrets: false",
  "metadata_values_retained: false",
  "consumer_properties_admission_pending: true",
]) {
  requireText(runner, marker, "browser runner");
}
for (const forbidden of [
  "storageState: await",
  "page.content()",
  "localStorage",
  "sessionStorage",
  "document.cookie",
  "authorization",
  "Save properties\" }).click",
]) {
  forbidText(runner, forbidden, "browser runner retention/mutation boundary");
}

for (const marker of [
  "`source-ready` means code, contracts, build source or retained harness source exists.",
  "Pages and Page Builder remain one vertical pipeline with explicit owners:",
  "Pages owns persistence, lifecycle, immutable bindings",
  "Pages admin owns the optional same-origin authoring launch control",
  "Page Builder/Fly owns the reviewed document, sanitizer, runtime materialization",
  "No build, workflow, Docker, HTTP or browser execution is claimed by source inspection.",
]) {
  requireText(plan, marker, "parity continuation plan");
}
for (const marker of [
  "source-ready / maintainer-browser-execution-pending / consumer-properties-admission-pending",
  "pages_published_metadata_browser_execution_v1",
  "four reviewed Pages admin profile URLs",
  "does not click `Save properties`",
  "No browser execution is claimed by this source slice",
]) {
  requireText(actualization, marker, "browser harness actualization");
}

if (failures.length > 0) {
  console.error("[verify-pages-published-metadata-browser-evidence-harness] FAIL");
  failures.forEach((failure) => console.error(`- ${failure}`));
  process.exit(Math.min(failures.length, 255));
}

console.log(
  "[verify-pages-published-metadata-browser-evidence-harness] PASS source_ready=true maintainer_browser_execution=pending consumer_properties_admission=pending",
);
