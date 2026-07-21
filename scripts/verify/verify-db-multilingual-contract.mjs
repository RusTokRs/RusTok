#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";

const scriptPath = fileURLToPath(import.meta.url);
const repoRoot = path.resolve(path.dirname(scriptPath), "../..");
const contractPath = "docs/architecture/database-multilingual-contract.json";
const auditPath = "docs/architecture/database-multilingual-audit.md";

function read(root, relativePath) {
  return fs.readFileSync(path.join(root, relativePath), "utf8");
}

function requireFile(root, relativePath, failures) {
  const absolutePath = path.join(root, relativePath);
  if (!fs.existsSync(absolutePath)) {
    failures.push(`${relativePath}: required file is missing`);
    return false;
  }
  if (!fs.statSync(absolutePath).isFile()) {
    failures.push(`${relativePath}: expected a regular file`);
    return false;
  }
  return true;
}

function requireMarkers(source, markers, label, failures) {
  for (const marker of markers ?? []) {
    if (typeof marker !== "string" || marker.trim() === "") {
      failures.push(`${label}: required marker must be a non-empty string`);
    } else if (!source.includes(marker)) {
      failures.push(`${label}: missing marker ${JSON.stringify(marker)}`);
    }
  }
}

function requireNonEmptyString(value, label, failures) {
  if (typeof value !== "string" || value.trim() === "") {
    failures.push(`${label} must be a non-empty string`);
  }
}

export function collectDbMultilingualContractFailures(root = repoRoot) {
  const failures = [];
  if (!requireFile(root, contractPath, failures)) return failures;
  if (!requireFile(root, auditPath, failures)) return failures;

  let contract;
  try {
    contract = JSON.parse(read(root, contractPath));
  } catch (error) {
    failures.push(`${contractPath}: invalid JSON: ${error.message}`);
    return failures;
  }

  if (contract.schema_version !== 1) {
    failures.push(`${contractPath}: schema_version must be 1`);
  }
  if (contract.contract_id !== "rustok-db-multilingual-storage") {
    failures.push(`${contractPath}: unexpected contract_id`);
  }

  const rules = contract.rules ?? {};
  const expectedRules = {
    base_rows: "language_agnostic",
    localized_short_text: "parallel_translation_rows",
    localized_heavy_content: "parallel_body_rows",
    tenant_locale_policy_owns_fields: false,
    normalized_locale_min_varchar_width: 32,
    locale_width_rollback: "never_narrow",
  };
  for (const [key, expected] of Object.entries(expectedRules)) {
    if (rules[key] !== expected) {
      failures.push(
        `${contractPath}: rules.${key} must equal ${JSON.stringify(expected)}`,
      );
    }
  }

  const authorityMarkers = new Map([
    [
      "docs/architecture/database.md",
      [
        "base business tables store language-agnostic state",
        "localized short texts live in `*_translations`",
        "safe width of\n  `VARCHAR(32)`",
        "rollback must not narrow such columns back",
      ],
    ],
    [
      "docs/architecture/i18n.md",
      [
        "language-agnostic state lives in base tables",
        "localized short fields live in `*_translations`",
        "safe width of `VARCHAR(32)`",
      ],
    ],
    [
      "DECISIONS/2026-04-05-multilingual-db-storage-parallel-localized-records.md",
      [
        "base business rows store only language-agnostic state",
        "short localized text lives in parallel `*_translations` records",
        "platform-safe column width of `VARCHAR(32)`",
      ],
    ],
  ]);

  if (!Array.isArray(contract.authority)) {
    failures.push(`${contractPath}: authority must be an array`);
  } else {
    for (const [authorityPath, markers] of authorityMarkers) {
      if (!contract.authority.includes(authorityPath)) {
        failures.push(`${contractPath}: authority is missing ${authorityPath}`);
        continue;
      }
      if (requireFile(root, authorityPath, failures)) {
        requireMarkers(read(root, authorityPath), markers, authorityPath, failures);
      }
    }
  }

  const guardedIds = new Set();
  if (!Array.isArray(contract.guarded_surfaces) || contract.guarded_surfaces.length === 0) {
    failures.push(`${contractPath}: guarded_surfaces must be a non-empty array`);
  } else {
    for (const [surfaceIndex, surface] of contract.guarded_surfaces.entries()) {
      const label = `${contractPath}: guarded_surfaces[${surfaceIndex}]`;
      requireNonEmptyString(surface.id, `${label}.id`, failures);
      if (guardedIds.has(surface.id)) failures.push(`${label}.id duplicates ${surface.id}`);
      guardedIds.add(surface.id);
      if (!["enforced", "delegated_guard"].includes(surface.status)) {
        failures.push(`${label}.status must be enforced or delegated_guard`);
      }
      if (!Array.isArray(surface.files) || surface.files.length === 0) {
        failures.push(`${label}.files must be a non-empty array`);
        continue;
      }
      for (const [fileIndex, file] of surface.files.entries()) {
        const fileLabel = `${label}.files[${fileIndex}]`;
        requireNonEmptyString(file.path, `${fileLabel}.path`, failures);
        if (typeof file.path !== "string" || !requireFile(root, file.path, failures)) continue;
        requireMarkers(read(root, file.path), file.required_markers, file.path, failures);
      }
    }
  }

  const audit = read(root, auditPath);
  const gapIds = new Set();
  if (!Array.isArray(contract.known_gaps)) {
    failures.push(`${contractPath}: known_gaps must be an array`);
  } else {
    for (const [gapIndex, gap] of contract.known_gaps.entries()) {
      const label = `${contractPath}: known_gaps[${gapIndex}]`;
      for (const field of ["id", "owner", "kind", "status", "path", "next_action"]) {
        requireNonEmptyString(gap[field], `${label}.${field}`, failures);
      }
      if (gapIds.has(gap.id)) failures.push(`${label}.id duplicates ${gap.id}`);
      gapIds.add(gap.id);
      if (guardedIds.has(gap.id)) {
        failures.push(`${label}.id cannot also be a guarded surface`);
      }
      if (typeof gap.id === "string" && !audit.includes(`\`${gap.id}\``)) {
        failures.push(`${auditPath}: missing known gap ${gap.id}`);
      }
      if (typeof gap.path === "string" && requireFile(root, gap.path, failures)) {
        requireMarkers(read(root, gap.path), gap.evidence_markers, gap.path, failures);
      }
    }
  }

  const backfillPath = "docs/migrations/backfill-contracts.json";
  if (requireFile(root, backfillPath, failures)) {
    let register;
    try {
      register = JSON.parse(read(root, backfillPath));
    } catch (error) {
      failures.push(`${backfillPath}: invalid JSON: ${error.message}`);
      register = null;
    }
    const pagesContract = register?.contracts?.find(
      (entry) => entry.migration === "m20260721_000003_expand_pages_locale_storage_columns",
    );
    if (!pagesContract) {
      failures.push(`${backfillPath}: Pages locale widening migration is undeclared`);
    } else if (pagesContract.mode !== "none" || pagesContract.owner !== "rustok-pages") {
      failures.push(`${backfillPath}: Pages locale widening must be DDL-only and owned by rustok-pages`);
    }
  }

  return failures;
}

function main() {
  const failures = collectDbMultilingualContractFailures();
  if (failures.length > 0) {
    console.error("DB multilingual contract drift detected:");
    failures.forEach((failure) => console.error(`- ${failure}`));
    process.exit(Math.min(failures.length, 255));
  }
  console.log("OK  DB multilingual storage contract");
}

if (process.argv[1] && path.resolve(process.argv[1]) === scriptPath) {
  main();
}
