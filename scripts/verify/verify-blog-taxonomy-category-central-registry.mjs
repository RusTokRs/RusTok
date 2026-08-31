#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";

const scriptPath = fileURLToPath(import.meta.url);
const repoRoot = path.resolve(path.dirname(scriptPath), "../..");

function read(relativePath) {
  return fs.readFileSync(path.join(repoRoot, relativePath), "utf8");
}

function requireMarker(source, marker, label) {
  if (!source.includes(marker)) {
    throw new Error(`${label}: missing marker ${JSON.stringify(marker)}`);
  }
}

function rejectMarker(source, marker, label) {
  if (source.includes(marker)) {
    throw new Error(`${label}: stale marker must remain absent: ${JSON.stringify(marker)}`);
  }
}

function main() {
  const registryPath = "docs/modules/registry.md";
  const surfaceRegistryPath = "docs/modules/translation-surfaces.json";
  const registry = read(registryPath);
  const surfaces = JSON.parse(read(surfaceRegistryPath));

  requireMarker(
    registry,
    "Blog Category canonical copy is Taxonomy-owned through the same-ID Blog-to-Taxonomy binding and the registered `taxonomy/term` Translation target; Blog post copy remains a separate future Blog-owned editorial surface.",
    registryPath,
  );
  requireMarker(
    registry,
    "canonical Category Translation concurrency/cursor recovery is Taxonomy-owned and tracked with the Taxonomy provider.",
    registryPath,
  );
  requireMarker(
    registry,
    "Blog domain, posts, Category bindings, tags, transport/UI; canonical Category localized copy and Translation are Taxonomy-owned through the same-ID binding, while Blog retains Blog-specific membership/settings and post/editorial ownership",
    registryPath,
  );
  requireMarker(
    registry,
    "registered `taxonomy/term` Translation target supplies exact source/target snapshots",
    registryPath,
  );

  for (const stale of [
    "Registered `blog/category` Translation target",
    "registered exact `blog/category` Translation target",
    "Blog Translation concurrency/cursor recovery execution",
  ]) {
    rejectMarker(registry, stale, registryPath);
  }

  const blogCategory = surfaces.surfaces?.find((surface) => surface.id === "blog_categories");
  if (!blogCategory || blogCategory.readiness !== "excluded" || blogCategory.provider_status !== "not_registered") {
    throw new Error(`${surfaceRegistryPath}: blog_categories must remain excluded/not_registered`);
  }
  const taxonomyTerms = surfaces.surfaces?.find((surface) => surface.id === "taxonomy_terms");
  if (!taxonomyTerms || taxonomyTerms.owner_slug !== "taxonomy" || taxonomyTerms.provider_status !== "registered") {
    throw new Error(`${surfaceRegistryPath}: taxonomy_terms must remain the registered canonical owner`);
  }

  console.log("OK  central registry uses canonical Taxonomy ownership for Blog Category copy");
}

try {
  main();
} catch (error) {
  console.error(error instanceof Error ? error.message : String(error));
  process.exit(1);
}
