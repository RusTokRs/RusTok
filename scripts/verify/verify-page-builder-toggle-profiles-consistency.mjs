#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const repoRoot = path.resolve(__dirname, "..", "..");
const pagesManifest = path.join(repoRoot, "crates", "rustok-pages", "rustok-module.toml");

function fail(message) {
  console.error("[verify-page-builder-toggle-profiles-consistency] FAIL");
  console.error(`- ${message}`);
  process.exit(1);
}

if (!fs.existsSync(pagesManifest)) {
  fail(`missing file: ${pagesManifest}`);
}

const manifest = fs.readFileSync(pagesManifest, "utf8");

const profileExpectations = {
  all_on: {
    "builder.enabled": "true",
    "builder.preview.enabled": "true",
    "builder.properties.enabled": "true",
    "builder.publish.enabled": "true",
    "builder.legacy_bridge_readonly": "true",
  },
  publish_off: {
    "builder.enabled": "true",
    "builder.preview.enabled": "true",
    "builder.properties.enabled": "true",
    "builder.publish.enabled": "false",
    "builder.legacy_bridge_readonly": "true",
  },
  preview_off: {
    "builder.enabled": "true",
    "builder.preview.enabled": "false",
    "builder.properties.enabled": "true",
    "builder.publish.enabled": "false",
    "builder.legacy_bridge_readonly": "true",
  },
  builder_off: {
    "builder.enabled": "false",
    "builder.preview.enabled": "false",
    "builder.properties.enabled": "false",
    "builder.publish.enabled": "false",
    "builder.legacy_bridge_readonly": "true",
  },
};

for (const [profile, flags] of Object.entries(profileExpectations)) {
  const sectionRegex = new RegExp(`${profile}\\s*=\\s*\\[(.*?)\\]`, "s");
  const sectionMatch = manifest.match(sectionRegex);
  if (!sectionMatch?.[1]) {
    fail(`missing toggle profile section: ${profile}`);
  }

  const sectionBody = sectionMatch[1];
  for (const [flag, value] of Object.entries(flags)) {
    const expected = `${flag}=${value}`;
    if (!sectionBody.includes(expected)) {
      fail(`profile '${profile}' is missing expected entry '${expected}'`);
    }
  }
}

console.log("[verify-page-builder-toggle-profiles-consistency] PASS");
