#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(scriptDir, "../..");
const failures = [];

function read(relativePath) {
  return fs.readFileSync(path.join(repoRoot, relativePath), "utf8");
}

function requireFile(relativePath) {
  if (!fs.existsSync(path.join(repoRoot, relativePath))) {
    failures.push(`${relativePath}: required file is missing`);
    return false;
  }
  return true;
}

function requireMarkers(relativePath, markers) {
  if (!requireFile(relativePath)) return;
  const source = read(relativePath);
  for (const marker of markers) {
    if (!source.includes(marker)) failures.push(`${relativePath}: missing marker ${marker}`);
  }
}

function forbidMarkers(relativePath, markers) {
  if (!requireFile(relativePath)) return;
  const source = read(relativePath);
  for (const marker of markers) {
    if (source.includes(marker)) failures.push(`${relativePath}: forbidden marker ${marker}`);
  }
}

function parseDate(value, label) {
  const timestamp = Date.parse(`${value}T23:59:59Z`);
  if (!Number.isFinite(timestamp)) failures.push(`${label}: expected ISO date`);
  return timestamp;
}

const registerPath = "docs/api/compatibility-exceptions.json";
if (requireFile(registerPath)) {
  let register;
  try {
    register = JSON.parse(read(registerPath));
  } catch (error) {
    failures.push(`${registerPath}: invalid JSON: ${error.message}`);
  }

  if (register) {
    if (register.schema_version !== 1) failures.push(`${registerPath}: schema_version must be 1`);
    for (const field of ["owner", "review_by", "exit_criteria"]) {
      if (typeof register.policy?.[field] !== "string" || register.policy[field].trim() === "") {
        failures.push(`${registerPath}: policy.${field} must be non-empty`);
      }
    }

    const reviewBy = parseDate(register.policy?.review_by, `${registerPath}: policy.review_by`);
    const verificationDate = process.env.VERIFICATION_DATE
      ? Date.parse(`${process.env.VERIFICATION_DATE}T00:00:00Z`)
      : Date.now();
    if (!Number.isFinite(verificationDate)) {
      failures.push("VERIFICATION_DATE must be an ISO date when provided");
    } else if (Number.isFinite(reviewBy) && verificationDate > reviewBy) {
      failures.push(`${registerPath}: policy review expired on ${register.policy.review_by}`);
    }

    if (!Array.isArray(register.exceptions)) {
      failures.push(`${registerPath}: exceptions must be an array`);
    } else {
      const ids = new Set();
      for (const [index, entry] of register.exceptions.entries()) {
        const label = `${registerPath}: exceptions[${index}]`;
        for (const field of ["id", "owner", "reason", "expires_on"]) {
          if (typeof entry[field] !== "string" || entry[field].trim() === "") {
            failures.push(`${label}.${field} must be non-empty`);
          }
        }
        if (typeof entry.id === "string" && !/^(?:openapi|graphql):/.test(entry.id)) {
          failures.push(`${label}.id must use an openapi: or graphql: change id`);
        }
        if (ids.has(entry.id)) failures.push(`${label}.id duplicates ${entry.id}`);
        ids.add(entry.id);

        const expiresOn = parseDate(entry.expires_on, `${label}.expires_on`);
        if (Number.isFinite(expiresOn) && Number.isFinite(reviewBy) && expiresOn > reviewBy) {
          failures.push(`${label}.expires_on must not exceed policy.review_by`);
        }
      }
    }
  }
}

requireMarkers("apps/server/src/controllers/swagger.rs", [
  "pub fn build_openapi_document(settings: &RustokSettings) -> OpenApiDoc",
  "openapi.merge(rustok_blog::openapi::openapi_document())",
  "openapi.merge(rustok_forum::openapi::openapi_document())",
  "openapi.merge(rustok_pages::openapi::openapi_document())",
  "openapi.merge(rustok_commerce::openapi::openapi_document())",
]);
requireMarkers("apps/server/src/bin/export_api_contracts.rs", [
  "build_openapi_document",
  "Schema::build(",
  "Query::default()",
  "Mutation::default()",
  "Subscription::default()",
  "openapi.json",
  "schema.graphql",
]);
forbidMarkers("apps/server/src/bin/export_api_contracts.rs", [
  "rustok_blog::openapi",
  "rustok_forum::openapi",
  "rustok_pages::openapi",
  "rustok_commerce::openapi",
]);

requireMarkers("scripts/verify/verify-api-compatibility.mjs", [
  "function compareOpenApi",
  "function compareGraphql",
  "openapi:path-removed:",
  "openapi:required-parameter-added:",
  "graphql:field-removed:",
  "graphql:required-argument-added:",
  "graphql:required-input-field-added:",
  "function runSelfTest",
  "stale API compatibility exception",
]);
requireMarkers("scripts/verify/verify-api-compatibility-self-test.mjs", [
  "verify-api-compatibility.mjs",
  '"--self-test"',
]);

requireMarkers(".github/workflows/api-compatibility.yml", [
  "name: API Compatibility",
  "github.event.pull_request.base.sha",
  "github.event.pull_request.head.sha",
  "--locked",
  "--all-features",
  "--bin export_api_contracts",
  "Verify exported artifact set",
  "contracts/base/openapi.json",
  "contracts/base/schema.graphql",
  "contracts/head/openapi.json",
  "contracts/head/schema.graphql",
  "--base-dir",
  "--head-dir",
  "docs/api/compatibility-exceptions.json",
  "actions/upload-artifact@v7",
]);
forbidMarkers(".github/workflows/api-compatibility.yml", [
  "continue-on-error: true",
  "|| true",
]);

requireMarkers(".github/workflows/hardening-gates.yml", [
  "Verify API compatibility comparator fixtures",
  "verify-api-compatibility-self-test.mjs",
  "Verify API compatibility gate structure",
  "verify-api-compatibility-contract.mjs",
]);

if (failures.length > 0) {
  console.error("API compatibility contract verification failed:");
  failures.forEach((failure) => console.error(`✗ ${failure}`));
  process.exit(Math.min(failures.length, 255));
}

console.log("✔ API compatibility exporter, comparator, workflow, and exception policy are structurally bound");
