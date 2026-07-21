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

function escapeRegExp(value) {
  return value.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}

function containsForbiddenMarker(source, marker) {
  if (!/^[A-Za-z_][A-Za-z0-9_]*$/.test(marker)) {
    return source.includes(marker);
  }

  const escaped = escapeRegExp(marker);
  return new RegExp(`(^|[^A-Za-z0-9_])${escaped}($|[^A-Za-z0-9_])`, "m").test(source);
}

function forbidMarkers(source, markers, label, failures) {
  for (const marker of markers ?? []) {
    if (typeof marker !== "string" || marker.trim() === "") {
      failures.push(`${label}: forbidden marker must be a non-empty string`);
    } else if (containsForbiddenMarker(source, marker)) {
      failures.push(`${label}: contains forbidden marker ${JSON.stringify(marker)}`);
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
    legacy_unknown_locale: "und_or_null_with_explicit_provenance",
    runtime_fallback_is_storage_provenance: false,
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
    [
      "DECISIONS/2026-07-21-language-agnostic-legacy-locale-provenance.md",
      [
        "store `locale = NULL` together with explicit legacy/unknown provenance",
        "store the normalized BCP47 language tag `und`",
        "must not be inserted into `tenant_locales`",
        "must never bind unknown text to `PLATFORM_FALLBACK_LOCALE`",
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
        const source = read(root, file.path);
        requireMarkers(source, file.required_markers, file.path, failures);
        forbidMarkers(source, file.forbidden_markers, file.path, failures);
      }
    }
  }

  const directGuardedFiles = [
    {
      path: "crates/rustok-search/src/migrations/m20260325_000003_create_search_query_logs.rs",
      requiredMarkers: ["ColumnDef::new(SearchQueryLogs::Locale).string_len(32)"],
    },
    {
      path: "crates/rustok-search/src/migrations/m20260721_000008_expand_search_query_locale_storage.rs",
      requiredMarkers: [
        "ALTER TABLE search_query_logs ALTER COLUMN locale TYPE VARCHAR(32)",
        "MODIFY COLUMN locale VARCHAR(32) NULL",
        "SQLite does not enforce declared VARCHAR lengths",
        "Forward-only",
      ],
    },
    {
      path: "crates/rustok-search/src/migrations/mod.rs",
      requiredMarkers: [
        "mod m20260721_000008_expand_search_query_locale_storage;",
        "Box::new(m20260721_000008_expand_search_query_locale_storage::Migration)",
      ],
    },
  ];
  for (const file of directGuardedFiles) {
    if (requireFile(root, file.path, failures)) {
      requireMarkers(read(root, file.path), file.requiredMarkers, file.path, failures);
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

    const expectedBackfillContracts = new Map([
      ["m20260721_000003_expand_pages_locale_storage_columns", { owner: "rustok-pages", mode: "none" }],
      ["m20260721_000004_expand_content_locale_storage_columns", { owner: "rustok-content", mode: "none" }],
      ["m20260721_000005_expand_blog_locale_storage_columns", { owner: "rustok-blog", mode: "none" }],
      ["m20260721_000006_expand_taxonomy_locale_storage_columns", { owner: "rustok-taxonomy", mode: "none" }],
      ["m20260721_000007_expand_comment_locale_storage_columns", { owner: "rustok-comments", mode: "none" }],
      ["m20260721_000009_expand_profile_locale_storage_columns", { owner: "rustok-profiles", mode: "none" }],
      ["m20260721_000010_move_profile_display_name_to_translations", { owner: "rustok-profiles", mode: "fixture" }],
      ["m20260721_000007_align_language_agnostic_locale_contract", { owner: "rustok-commerce", mode: "none" }],
      ["m20260721_000008_expand_search_query_locale_storage", { owner: "rustok-search", mode: "none" }],
      ["m20260721_000005_drop_seller_legacy_prose_columns", { owner: "rustok-marketplace-seller", mode: "none" }],
      ["m20260721_000009_move_oauth_app_copy_to_translations", { owner: "rustok-auth", mode: "fixture" }],
      ["m20260721_000105_expand_customer_locale_contract", { owner: "rustok-customer", mode: "none" }],
    ]);

    for (const [migration, expected] of expectedBackfillContracts) {
      const entry = register?.contracts?.find((candidate) => candidate.migration === migration);
      if (!entry) {
        failures.push(`${backfillPath}: migration ${migration} is undeclared`);
      } else if (entry.mode !== expected.mode || entry.owner !== expected.owner) {
        failures.push(
          `${backfillPath}: ${migration} must use mode ${expected.mode} and owner ${expected.owner}`,
        );
      }
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
