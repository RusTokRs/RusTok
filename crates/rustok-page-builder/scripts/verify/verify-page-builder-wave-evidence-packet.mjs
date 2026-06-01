#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const repoRoot = path.resolve(__dirname, "..", "..", "..", "..");
const packetPath = path.join(
  repoRoot,
  "crates",
  "rustok-page-builder",
  "contracts",
  "evidence",
  "pages-wave0-dry-run-evidence.json",
);
const templatePath = path.join(
  repoRoot,
  "crates",
  "rustok-page-builder",
  "contracts",
  "page-builder-wave-evidence-template.json",
);

function fail(message) {
  console.error("[verify-page-builder-wave-evidence-packet] FAIL");
  console.error(`- ${message}`);
  process.exit(1);
}

function readJson(filePath) {
  if (!fs.existsSync(filePath)) {
    fail(`missing file: ${path.relative(repoRoot, filePath)}`);
  }
  return JSON.parse(fs.readFileSync(filePath, "utf8"));
}

function expectArrayIncludes(array, values, label) {
  if (!Array.isArray(array)) {
    fail(`${label} must be an array`);
  }
  for (const value of values) {
    if (!array.includes(value)) {
      fail(`${label} missing '${value}'`);
    }
  }
}

function expectObjectHas(object, keys, label) {
  if (!object || typeof object !== "object" || Array.isArray(object)) {
    fail(`${label} must be an object`);
  }
  for (const key of keys) {
    if (!(key in object)) {
      fail(`${label} missing '${key}'`);
    }
  }
}

function expectFlags(profile, expected) {
  for (const [key, value] of Object.entries(expected)) {
    if (profile.flags?.[key] !== value) {
      fail(
        `${profile.name}.flags.${key} expected '${value}', got '${profile.flags?.[key]}'`,
      );
    }
  }
}

const template = readJson(templatePath);
const packet = readJson(packetPath);
const requiredProfiles = template.required_profiles;

if (packet.schema_version !== template.schema_version) {
  fail(
    `schema_version mismatch: packet=${packet.schema_version}, template=${template.schema_version}`,
  );
}
if (packet.artifact !== "page_builder_wave_evidence_packet") {
  fail(`unexpected artifact: ${packet.artifact}`);
}
if (packet.mode !== "dry_run") {
  fail(`unexpected mode: ${packet.mode}`);
}
if (packet.module_slug !== "pages") {
  fail(`unexpected module_slug: ${packet.module_slug}`);
}

expectObjectHas(
  packet.metadata?.provider,
  template.required_sections.metadata.provider,
  "metadata.provider",
);
expectObjectHas(
  packet.metadata?.consumer,
  template.required_sections.metadata.consumer,
  "metadata.consumer",
);
expectObjectHas(
  packet.control_plane?.change_set,
  template.required_sections.control_plane.change_set,
  "control_plane.change_set",
);
expectObjectHas(
  packet.control_plane?.snapshots,
  template.required_sections.control_plane.snapshots,
  "control_plane.snapshots",
);
expectObjectHas(
  packet.observability?.metrics,
  template.required_sections.observability.metrics,
  "observability.metrics",
);
expectObjectHas(
  packet.observability?.traces,
  template.required_sections.observability.traces,
  "observability.traces",
);
expectObjectHas(
  packet.rollback,
  template.required_sections.rollback.required_fields,
  "rollback",
);
expectObjectHas(
  packet.approvals,
  template.required_sections.approvals,
  "approvals",
);

expectArrayIncludes(
  packet.metadata.consumer.fallback_matrix,
  requiredProfiles,
  "metadata.consumer.fallback_matrix",
);
expectArrayIncludes(
  packet.metadata.consumer.toggle_profiles,
  requiredProfiles,
  "metadata.consumer.toggle_profiles",
);

const profiles = packet.fallback?.profiles;
if (!Array.isArray(profiles)) {
  fail("fallback.profiles must be an array");
}
const byName = new Map(profiles.map((profile) => [profile.name, profile]));
expectArrayIncludes(
  [...byName.keys()],
  requiredProfiles,
  "fallback.profiles[].name",
);

const expectedProfileFlags = {
  all_on: {
    "builder.enabled": true,
    "builder.preview.enabled": true,
    "builder.properties.enabled": true,
    "builder.publish.enabled": true,
    "builder.legacy_bridge_readonly": true,
  },
  publish_off: {
    "builder.enabled": true,
    "builder.preview.enabled": true,
    "builder.properties.enabled": true,
    "builder.publish.enabled": false,
    "builder.legacy_bridge_readonly": true,
  },
  preview_off: {
    "builder.enabled": true,
    "builder.preview.enabled": false,
    "builder.properties.enabled": true,
    "builder.publish.enabled": false,
    "builder.legacy_bridge_readonly": true,
  },
  builder_off: {
    "builder.enabled": false,
    "builder.preview.enabled": false,
    "builder.properties.enabled": false,
    "builder.publish.enabled": false,
    "builder.legacy_bridge_readonly": true,
  },
};

for (const profileName of requiredProfiles) {
  const profile = byName.get(profileName);
  expectFlags(profile, expectedProfileFlags[profileName]);
  expectObjectHas(
    profile.smoke,
    template.required_sections.fallback.smoke_steps,
    `${profileName}.smoke`,
  );
  expectObjectHas(
    profile.read_guarantees,
    template.required_sections.fallback.read_guarantees,
    `${profileName}.read_guarantees`,
  );
  if (profile.decision !== "keep" && profile.decision !== "rollback") {
    fail(`${profileName}.decision must be keep|rollback`);
  }
}

if (
  byName.get("publish_off").smoke.publish_dry !== "typed_feature_disabled_error"
) {
  fail("publish_off.publish_dry must be typed_feature_disabled_error");
}
if (byName.get("builder_off").smoke.save_draft !== "readonly_fallback") {
  fail("builder_off.save_draft must be readonly_fallback");
}
if (packet.rollback.decision !== "keep") {
  fail(`rollback.decision expected keep, got ${packet.rollback.decision}`);
}
if ((packet.waivers ?? []).length !== 0) {
  fail("dry-run evidence packet must not carry waivers");
}

console.log("[verify-page-builder-wave-evidence-packet] PASS");
