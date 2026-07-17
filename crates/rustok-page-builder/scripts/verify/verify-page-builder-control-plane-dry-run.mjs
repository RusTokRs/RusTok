#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const repoRoot = path.resolve(__dirname, "..", "..", "..", "..");

function readJson(relativePath) {
  return JSON.parse(fs.readFileSync(path.join(repoRoot, relativePath), "utf8"));
}

function readText(relativePath) {
  return fs.readFileSync(path.join(repoRoot, relativePath), "utf8");
}

function fail(message) {
  console.error("[verify-page-builder-control-plane-dry-run] FAIL");
  console.error(`- ${message}`);
  process.exit(1);
}

function expect(condition, message) {
  if (!condition) fail(message);
}

const contract = readJson(
  "crates/rustok-page-builder/contracts/page-builder-control-plane-dry-run.json",
);
const registry = readJson("crates/rustok-page-builder/contracts/page-builder-fba-registry.json");
const wave0 = readJson(
  "crates/rustok-page-builder/contracts/evidence/pages-wave0-dry-run-evidence.json",
);
const wave1 = readJson(
  "crates/rustok-page-builder/contracts/evidence/pages-wave1-readiness-draft.json",
);
const rolloutSource = readText("crates/rustok-page-builder/src/rollout.rs");
const plan = readText("crates/rustok-page-builder/docs/implementation-plan.md");

const expectedProfiles = ["all_on", "publish_off", "preview_off", "builder_off"];
const expectedFlagKeys = [
  "builder.enabled",
  "builder.preview.enabled",
  "builder.properties.enabled",
  "builder.publish.enabled",
];

expect(contract.artifact === "page_builder_control_plane_dry_run_contract", "unexpected artifact");
expect(contract.builder_contract_version === registry.provider.builder_contract_version, "provider version drift");
expect(contract.consumer_min_version === registry.provider.consumer_min_version, "consumer min version drift");
expect(
  JSON.stringify(contract.change_set_contract.atomic_flag_keys) === JSON.stringify(expectedFlagKeys),
  "atomic flag key contract drift",
);
expect(
  JSON.stringify(contract.change_set_contract.required_profiles) === JSON.stringify(expectedProfiles),
  "required dry-run profiles drift",
);
expect(
  JSON.stringify(Object.keys(contract.profile_expectations)) === JSON.stringify(expectedProfiles),
  "profile expectation order drift",
);

for (const profile of expectedProfiles) {
  const expectation = contract.profile_expectations[profile];
  for (const key of expectedFlagKeys) {
    expect(typeof expectation[key] === "boolean", `${profile} missing boolean flag ${key}`);
  }
  expect(
    ["pass", "typed_feature_disabled_error"].includes(expectation.publish_dry),
    `${profile} has invalid publish_dry outcome`,
  );
}

for (const guarantee of contract.read_surface_guarantees) {
  for (const packet of [wave0, wave1]) {
    for (const profile of packet.fallback.profiles) {
      expect(profile.read_guarantees?.[guarantee] === true, `${packet.wave}/${profile.name} lacks ${guarantee}`);
    }
  }
}

for (const marker of Object.values(contract.source_markers)) {
  expect(
    rolloutSource.includes(marker) || plan.includes(marker) || JSON.stringify(wave1).includes(marker),
    `source marker not found: ${marker}`,
  );
}

expect(rolloutSource.includes("pub fn atomic_flag_keys()"), "runtime change-set must expose atomic flag keys");
expect(rolloutSource.includes("pub fn dry_run("), "runtime change-set must expose dry_run constructor");
expect(
  contract.change_set_contract.waiver_policy.owner_signoff_required === true &&
    contract.change_set_contract.waiver_policy.expiry_required === true &&
    contract.change_set_contract.waiver_policy.forbidden_for_wave1_promotion === true,
  "waiver policy must require signoff, expiry and block Wave 1 promotion",
);

console.log("[verify-page-builder-control-plane-dry-run] PASS");
