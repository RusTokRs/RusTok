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
  "let settings = RustokSettings::default();",
  "settings.runtime.is_registry_only()",
  "API compatibility export requires the full runtime host profile",
  "build_openapi_document(&settings)",
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
requireMarkers("scripts/verify/verify-api-compatibility-exceptions.mjs", [
  '"--file"',
  "policy.review_by",
  "expires_on",
  "openapi|graphql",
  "VERIFICATION_DATE",
]);
requireMarkers("scripts/verify/verify-api-compatibility-exceptions-local.mjs", [
  "verify-api-compatibility-exceptions.mjs",
  "docs/api/compatibility-exceptions.json",
  '"--file"',
]);
requireMarkers("scripts/verify/verify-api-compatibility-exception-approval.mjs", [
  "api-breaking-approved",
  "function approvalDecision",
  "--labels-json",
  "--explicitly-approved",
  "function runSelfTest",
]);
requireMarkers("scripts/verify/verify-api-compatibility-exception-approval-self-test.mjs", [
  "verify-api-compatibility-exception-approval.mjs",
  '"--self-test"',
]);

requireMarkers(".github/workflows/api-compatibility.yml", [
  "name: API Compatibility",
  "github.event.pull_request.base.repo.full_name",
  "github.event.pull_request.head.repo.full_name",
  "github.event.pull_request.base.sha",
  "github.event.pull_request.head.sha",
  "repository: ${{ env.BASE_REPOSITORY }}",
  "repository: ${{ env.HEAD_REPOSITORY }}",
  "Verify base comparator fixtures",
  'base/scripts/verify/verify-api-compatibility.mjs" --self-test',
  "Require approval for compatibility exception changes",
  "api-breaking-approved",
  "PR_LABELS_JSON",
  "EXPLICIT_APPROVAL",
  "base/scripts/verify/verify-api-compatibility-exception-approval.mjs",
  "base/docs/api/compatibility-exceptions.json",
  "head/docs/api/compatibility-exceptions.json",
  "Validate head compatibility exceptions with base policy",
  "base/scripts/verify/verify-api-compatibility-exceptions.mjs",
  "head/docs/api/compatibility-exceptions.json",
  "--locked",
  "--all-features",
  "--bin export_api_contracts",
  "Verify exported artifact set",
  "contracts/base/openapi.json",
  "contracts/base/schema.graphql",
  "contracts/head/openapi.json",
  "contracts/head/schema.graphql",
  "Compare API contracts with base policy",
  "base/scripts/verify/verify-api-compatibility.mjs",
  "--base-dir",
  "--head-dir",
  "actions/upload-artifact@v7",
]);
forbidMarkers(".github/workflows/api-compatibility.yml", [
  'head/scripts/verify/verify-api-compatibility.mjs',
  "continue-on-error: true",
  "|| true",
]);

requireMarkers(".github/workflows/hardening-gates.yml", [
  "Verify API compatibility comparator fixtures",
  "verify-api-compatibility-self-test.mjs",
  "Verify API compatibility exception approval fixtures",
  "verify-api-compatibility-exception-approval-self-test.mjs",
  "Verify API compatibility exceptions",
  "verify-api-compatibility-exceptions.mjs",
  "Verify API compatibility gate structure",
  "verify-api-compatibility-contract.mjs",
]);
requireMarkers("scripts/verify/verify-all.sh", [
  "verify-api-compatibility-self-test.mjs:API Compatibility Comparator Fixtures",
  "verify-api-compatibility-exception-approval-self-test.mjs:API Compatibility Exception Approval Fixtures",
  "verify-api-compatibility-exceptions-local.mjs:API Compatibility Exceptions",
  "verify-api-compatibility-contract.mjs:API Compatibility Gate Structure",
]);

if (failures.length > 0) {
  console.error("API compatibility contract verification failed:");
  failures.forEach((failure) => console.error(`✗ ${failure}`));
  process.exit(Math.min(failures.length, 255));
}

console.log("✔ API compatibility exporter, base-owned comparator, workflow, and exception policy are structurally bound");
