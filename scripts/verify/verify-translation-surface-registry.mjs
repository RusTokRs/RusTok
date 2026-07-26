#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";

const scriptPath = fileURLToPath(import.meta.url);
const repoRoot = path.resolve(path.dirname(scriptPath), "../..");
const registryPath = "docs/modules/translation-surfaces.json";
const providerPath = "crates/rustok-translation-targets/src/lib.rs";

function fail(message, failures) {
  failures.push(message);
}

function nonEmptyString(value) {
  return typeof value === "string" && value.trim() !== "";
}

function moduleSlugs() {
  const source = fs.readFileSync(path.join(repoRoot, "modules.toml"), "utf8");
  const section = source.match(/\[modules\]([\s\S]*?)\n\[settings\]/)?.[1] ?? "";
  return new Set(
    [...section.matchAll(/^([a-z][a-z0-9_]*)\s*=\s*\{/gm)].map((match) => match[1]),
  );
}

function main() {
  const failures = [];
  const absoluteRegistry = path.join(repoRoot, registryPath);
  let registry;
  try {
    registry = JSON.parse(fs.readFileSync(absoluteRegistry, "utf8"));
  } catch (error) {
    console.error(`${registryPath}: invalid or missing JSON: ${error.message}`);
    process.exit(1);
  }

  if (registry.schema_version !== 1) fail(`${registryPath}: schema_version must be 1`, failures);
  if (registry.provider_contract !== "rustok-translation-targets/v1") {
    fail(`${registryPath}: provider_contract must be rustok-translation-targets/v1`, failures);
  }

  const states = new Set(registry.readiness_states ?? []);
  const profiles = new Set(registry.field_profiles ?? []);
  const owners = moduleSlugs();
  for (const owner of registry.non_module_owners ?? []) owners.add(owner);
  const ids = new Set();
  const seenStates = new Set();
  const requiredIds = new Set([
    "taxonomy_terms",
    "media_copy",
    "blog_categories",
    "content_nodes",
    "pages_visual_documents",
    "navigation_menus",
    "forum_ugc",
    "product_catalog",
    "commerce_collection_copy",
    "flex_attached_values",
    "module_settings_copy",
    "oauth_app_copy",
    "registry_artifact_copy",
    "order_transaction_snapshots",
    "search_linguistic_dictionaries",
    "static_ui_catalogs",
  ]);

  if (!Array.isArray(registry.surfaces) || registry.surfaces.length === 0) {
    fail(`${registryPath}: surfaces must be a non-empty array`, failures);
  } else {
    for (const [index, surface] of registry.surfaces.entries()) {
      const label = `${registryPath}: surfaces[${index}]`;
      for (const field of ["id", "owner_slug", "resource_kind", "readiness", "provider_status"]) {
        if (!nonEmptyString(surface[field])) fail(`${label}.${field} must be non-empty`, failures);
      }
      if (ids.has(surface.id)) fail(`${label}.id duplicates ${surface.id}`, failures);
      ids.add(surface.id);
      seenStates.add(surface.readiness);
      if (!owners.has(surface.owner_slug)) {
        fail(`${label}.owner_slug ${surface.owner_slug} is not declared in modules.toml`, failures);
      }
      if (!states.has(surface.readiness)) {
        fail(`${label}.readiness ${surface.readiness} is not declared`, failures);
      }
      if (surface.provider_status !== "not_registered") {
        fail(`${label}.provider_status must remain not_registered until an owner provider exists`, failures);
      }
      if (!Array.isArray(surface.field_profiles) || surface.field_profiles.length === 0) {
        fail(`${label}.field_profiles must be non-empty`, failures);
      } else {
        for (const profile of surface.field_profiles) {
          if (!profiles.has(profile)) fail(`${label}: unknown field profile ${profile}`, failures);
        }
      }
      if (!["allowed", "restricted", "forbidden"].includes(surface.ai_export)) {
        fail(`${label}.ai_export is invalid`, failures);
      }
      if (!Array.isArray(surface.evidence_paths) || surface.evidence_paths.length === 0) {
        fail(`${label}.evidence_paths must be non-empty`, failures);
      } else {
        for (const evidencePath of surface.evidence_paths) {
          if (!fs.existsSync(path.join(repoRoot, evidencePath))) {
            fail(`${label}: evidence path is missing: ${evidencePath}`, failures);
          }
        }
      }
      if (["pilot_candidate", "blocked"].includes(surface.readiness)) {
        if (!Array.isArray(surface.blockers) || surface.blockers.length === 0) {
          fail(`${label}.blockers must explain the remaining owner work`, failures);
        }
      }
      if (["excluded", "separate_track"].includes(surface.readiness)) {
        if (!nonEmptyString(surface.exclusion_reason)) {
          fail(`${label}.exclusion_reason is required`, failures);
        }
      }
    }
  }

  for (const id of requiredIds) {
    if (!ids.has(id)) fail(`${registryPath}: required surface ${id} is missing`, failures);
  }
  for (const state of states) {
    if (!seenStates.has(state)) fail(`${registryPath}: no surface uses readiness ${state}`, failures);
  }

  const storageContractPath = "docs/architecture/database-multilingual-contract.json";
  const storageContract = JSON.parse(
    fs.readFileSync(path.join(repoRoot, storageContractPath), "utf8"),
  );
  const mappedStorageIds = new Set(Object.keys(registry.storage_contract_mapping ?? {}));
  const nonTargetStorageIds = new Set(
    Object.keys(registry.non_target_storage_contracts ?? {}),
  );
  const storageIds = [
    ...(storageContract.guarded_surfaces ?? []).map((surface) => surface.id),
    ...(storageContract.known_gaps ?? []).map((surface) => surface.id),
  ];
  for (const storageId of storageIds) {
    if (!mappedStorageIds.has(storageId) && !nonTargetStorageIds.has(storageId)) {
      fail(`${registryPath}: DB multilingual surface ${storageId} is unclassified`, failures);
    }
  }
  for (const [storageId, surfaceIds] of Object.entries(
    registry.storage_contract_mapping ?? {},
  )) {
    if (!storageIds.includes(storageId)) {
      fail(`${registryPath}: unknown DB multilingual mapping ${storageId}`, failures);
    }
    if (!Array.isArray(surfaceIds) || surfaceIds.length === 0) {
      fail(`${registryPath}: ${storageId} must map to at least one translation surface`, failures);
    } else {
      for (const surfaceId of surfaceIds) {
        if (!ids.has(surfaceId)) {
          fail(`${registryPath}: ${storageId} maps to missing surface ${surfaceId}`, failures);
        }
      }
    }
  }
  for (const storageId of nonTargetStorageIds) {
    if (!storageIds.includes(storageId)) {
      fail(`${registryPath}: unknown non-target DB contract ${storageId}`, failures);
    }
  }

  const providerSource = fs.readFileSync(path.join(repoRoot, providerPath), "utf8");
  for (const marker of [
    "pub trait TranslationTargetProvider",
    "pub struct TranslationTargetRegistry",
    "pub struct TranslationResourceSnapshot",
    "pub struct TranslationPatchRequest",
    "expected_source_revision",
    "expected_target_revision",
    "register_translation_target_provider",
  ]) {
    if (!providerSource.includes(marker)) fail(`${providerPath}: missing marker ${marker}`, failures);
  }

  if (failures.length > 0) {
    console.error("Translation surface registry drift detected:");
    failures.forEach((failure) => console.error(`- ${failure}`));
    process.exit(Math.min(failures.length, 255));
  }
  console.log(`OK  Translation surface registry (${registry.surfaces.length} surfaces)`);
}

main();
