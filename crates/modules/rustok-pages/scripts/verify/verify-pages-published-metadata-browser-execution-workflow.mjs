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
  workflow: ".github/workflows/pages-published-metadata-browser-evidence.yml",
  sourceExecution:
    "crates/rustok-pages/contracts/evidence/pages-consumer-properties-source-execution.json",
  sourceWorkflow: ".github/workflows/pages-consumer-properties-source-evidence.yml",
  actualization:
    "docs/modules/pages-published-metadata-browser-evidence-harness-actualization-2026-08-13.md",
};
const verifier =
  "crates/rustok-pages/scripts/verify/verify-pages-published-metadata-browser-execution-workflow.mjs";

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
const requireOrderedMarkers = (source, markers, label) => {
  let previous = -1;
  for (const marker of markers) {
    const index = source.indexOf(marker, previous + 1);
    if (index < 0) {
      failures.push(`${label}: missing or out of order at ${marker}`);
      return;
    }
    previous = index;
  }
};

for (const relativePath of [...Object.values(files), verifier]) {
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
  console.error("[verify-pages-published-metadata-browser-execution-workflow] FAIL");
  failures.forEach((failure) => console.error(`- ${failure}`));
  process.exit(1);
}

const contract = JSON.parse(read(files.contract));
const surfaceEvidence = JSON.parse(read(files.surfaceEvidence));
const sourceExecution = JSON.parse(read(files.sourceExecution));
const workflow = read(files.workflow);
const sourceWorkflow = read(files.sourceWorkflow);
const actualization = read(files.actualization);

requireValue(
  contract.schema_version === 1 &&
    contract.module === "pages" &&
    contract.packet === "published_metadata_surface_browser_evidence" &&
    contract.status === "source_ready_maintainer_execution_pending",
  "browser execution contract identity drifted",
);
requireValue(
  contract.workflow === files.workflow && contract.workflow_verifier === verifier,
  "browser execution workflow binding drifted",
);
requireValue(
  contract.workflow_execution?.trigger === "workflow_dispatch" &&
    contract.workflow_execution?.required_ref === "refs/heads/main" &&
    contract.workflow_execution?.source_commit_must_equal_dispatch_sha === true &&
    contract.workflow_execution?.review_confirmation_input ===
      "reviewed_deployment_identity" &&
    contract.workflow_execution?.protected_environment ===
      "pages-published-metadata-browser-evidence" &&
    contract.workflow_execution?.protected_environment_requires_main_only_and_reviewers === true &&
    contract.workflow_execution?.storage_state_secret ===
      "RUSTOK_PAGES_PUBLISHED_METADATA_EDITOR_STORAGE_STATE_B64" &&
    contract.workflow_execution?.raw_external_inputs_masked === true &&
    contract.workflow_execution?.artifact_contains_only_bounded_packet === true &&
    contract.workflow_execution?.artifact_retention_days === 90 &&
    contract.workflow_execution?.workflow_mutates_repository === false,
  "browser workflow execution controls drifted",
);
requireValue(
  contract.workflow_execution?.run_index_status?.context ===
      "pages-published-metadata-browser-evidence-index" &&
    contract.workflow_execution?.run_index_status?.ref === "refs/heads/main" &&
    contract.workflow_execution?.run_index_status?.permission === "statuses: write" &&
    contract.workflow_execution?.run_index_status?.target === "github_actions_run_url" &&
    contract.workflow_execution?.run_index_status?.repository_content_mutation === false &&
    contract.workflow_execution?.run_index_status?.deployment_provenance_attestation === false &&
    contract.workflow_execution?.run_index_status?.consumer_properties_admission === false,
  "browser workflow run index boundary drifted",
);
for (const required of [files.workflow, verifier]) {
  requireValue(
    Array.isArray(contract.required_source_files) &&
      contract.required_source_files.includes(required),
    `browser contract required_source_files missing ${required}`,
  );
}
requireValue(
  surfaceEvidence.browser_execution?.contract === files.contract &&
    surfaceEvidence.browser_execution?.runner === contract.runner &&
    surfaceEvidence.browser_execution?.state ===
      "source_ready_maintainer_execution_pending",
  "published metadata surface/browser contract linkage drifted",
);

const verifierCommand = `node ${verifier}`;
for (const required of [files.workflow, verifier]) {
  requireValue(
    Array.isArray(sourceExecution.required_source_files) &&
      sourceExecution.required_source_files.includes(required),
    `consumer source execution required_source_files missing ${required}`,
  );
}
requireValue(
  sourceExecution.source_packets?.published_browser_execution?.workflow === files.workflow,
  "consumer source execution does not bind the browser workflow source",
);

for (const marker of [
  `- \"${files.workflow}\"`,
  `- \"${verifier}\"`,
  `run: ${verifierCommand}`,
]) {
  requireText(sourceWorkflow, marker, "consumer source evidence workflow");
}

for (const marker of [
  "name: Pages Published Metadata Browser Evidence",
  "workflow_dispatch:",
  "source_commit:",
  "deployment_digest:",
  "published_url:",
  "draft_url:",
  "archived_url:",
  "missing_url:",
  "reviewed_deployment_identity:",
  "permissions:",
  "contents: read",
  "statuses: write",
  "environment: pages-published-metadata-browser-evidence",
  'test "$GITHUB_REF" = "refs/heads/main"',
  'test "$SOURCE_COMMIT" = "$GITHUB_SHA"',
  'test "$REVIEWED_DEPLOYMENT_IDENTITY" = "true"',
  "persist-credentials: false",
  "verify-pages-published-metadata-browser-evidence-harness.mjs",
  "npm ci --prefix apps/next-admin --no-audit --no-fund",
  "playwright install --with-deps chromium",
  "::add-mask::%s",
  "RUSTOK_PAGES_PUBLISHED_METADATA_EDITOR_STORAGE_STATE_B64",
  "base64 --decode",
  "RUSTOK_PAGES_PUBLISHED_METADATA_BROWSER_SOURCE_COMMIT",
  "RUSTOK_PAGES_PUBLISHED_METADATA_DEPLOYMENT_DIGEST",
  "RUSTOK_PAGES_PUBLISHED_METADATA_PUBLISHED_URL",
  "RUSTOK_PAGES_PUBLISHED_METADATA_DRAFT_URL",
  "RUSTOK_PAGES_PUBLISHED_METADATA_ARCHIVED_URL",
  "RUSTOK_PAGES_PUBLISHED_METADATA_MISSING_URL",
  "playwright.pages-published-metadata.config.ts",
  "pages_published_metadata_browser_execution_v1",
  "browser_execution_passed_consumer_properties_admission_pending",
  "packet.retained_secrets !== false",
  "packet.metadata_values_retained !== false",
  "packet.browser_execution_only !== true",
  "packet.consumer_properties_admission_pending !== true",
  "actions/upload-artifact@v7",
  "target/pages-published-metadata-browser-evidence.json",
  "retention-days: 90",
  "name: Published Metadata Browser Evidence Gate",
  "Publish reviewed browser evidence run index",
  "if: github.ref == 'refs/heads/main'",
  "pages-published-metadata-browser-evidence-index",
  'target_url="https://github.com/${GITHUB_REPOSITORY}/actions/runs/${GITHUB_RUN_ID}"',
  '"https://api.github.com/repos/${GITHUB_REPOSITORY}/statuses/${GITHUB_SHA}"',
  'test "$BROWSER_RESULT" = "success"',
  'rm -f -- "$RUSTOK_PAGES_PUBLISHED_METADATA_EDITOR_STORAGE_STATE"',
]) {
  requireText(workflow, marker, "published metadata browser workflow");
}
for (const forbidden of [
  "push:",
  "pull_request:",
  "pull_request_target:",
  "workflow_call:",
  "contents: write",
  "pull-requests: write",
  "persist-credentials: true",
  "git push",
  "git commit",
  "gh pr",
  "playwright-report",
  "test-results",
]) {
  forbidText(workflow, forbidden, "published metadata browser workflow");
}

requireOrderedMarkers(
  workflow,
  [
    "Require main dispatch and reviewed deployment identity",
    "Checkout exact reviewed source",
    "Verify retained browser harness source",
    "Mask reviewed external inputs",
    "Materialize protected editor storage state",
    "Execute retained published metadata browser packet",
    "Verify bounded success packet",
    "Archive bounded browser evidence",
  ],
  "browser workflow fail-closed ordering",
);
requireOrderedMarkers(
  workflow,
  [
    "Verify bounded success packet",
    "Archive bounded browser evidence",
  ],
  "bounded packet must be verified before archive",
);
requireOrderedMarkers(
  workflow,
  [
    "Archive bounded browser evidence",
    "Publish reviewed browser evidence run index",
    "Require browser evidence success",
  ],
  "browser run index gate ordering",
);

for (const marker of [
  "dispatch-only workflow",
  "pages-published-metadata-browser-evidence",
  "RUSTOK_PAGES_PUBLISHED_METADATA_EDITOR_STORAGE_STATE_B64",
  "does not establish deployment provenance",
  "does not change `executed_evidence`",
  "reviewed `source_commit` input must equal the dispatch `GITHUB_SHA`",
  "only uploaded artifact is the bounded JSON packet",
]) {
  requireText(actualization, marker, "published metadata browser actualization");
}

if (failures.length > 0) {
  console.error("[verify-pages-published-metadata-browser-execution-workflow] FAIL");
  failures.forEach((failure) => console.error(`- ${failure}`));
  process.exit(Math.min(failures.length, 255));
}

console.log(
  "[verify-pages-published-metadata-browser-execution-workflow] PASS dispatch_only=true reviewed_deployment_required=true protected_storage_state=true run_index=ready browser_execution=pending",
);
