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
    "crates/modules/rustok-pages/contracts/evidence/pages-published-metadata-browser-execution-contract.json",
  surfaceEvidence:
    "crates/modules/rustok-pages/contracts/evidence/pages-published-metadata-surface-source.json",
  revisionEvidence:
    "crates/modules/rustok-pages/contracts/evidence/pages-metadata-revision-isolation-source.json",
  consumerContract:
    "crates/modules/rustok-page-builder/contracts/page-builder-consumer-properties.json",
  surface: "crates/modules/rustok-pages/admin/src/standalone_metadata.rs",
  panel: "crates/modules/rustok-page-builder/admin/src/editor/consumer_properties.rs",
  config: "apps/next-admin/playwright.pages-published-metadata.config.ts",
  setup: "apps/next-admin/tests/pages-published-metadata/global-setup.ts",
  runner: "apps/next-admin/tests/pages-published-metadata/browser-evidence.spec.ts",
  plan: "docs/modules/pages-page-builder-parity-continuation-plan.md",
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
const normalizedConfig = config.replaceAll("'", '"');
const normalizedRunner = runner.replaceAll("'", '"');
const compactRunner = normalizedRunner.replace(/\s+/gu, " ");

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

, marker, "browser harness actualization");
}

if (failures.length > 0) {
  console.error("[verify-pages-published-metadata-browser-evidence-harness] FAIL");
  failures.forEach((failure) => console.error(`- ${failure}`));
  process.exit(Math.min(failures.length, 255));
}

console.log(
  "[verify-pages-published-metadata-browser-evidence-harness] PASS source_ready=true maintainer_browser_execution=pending consumer_properties_admission=pending",
);
