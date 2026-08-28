#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";

const scriptPath = fileURLToPath(import.meta.url);
const repoRoot = path.resolve(path.dirname(scriptPath), "../..");

const centralPlanPath = "docs/modules/translation-implementation-plan.md";
const moduleReadmePath = "crates/rustok-translation/docs/README.md";
const modulePlanPath = "crates/rustok-translation/docs/implementation-plan.md";
const registryPath = "docs/modules/translation-surfaces.json";

function read(relativePath) {
  return fs.readFileSync(path.join(repoRoot, relativePath), "utf8");
}

function assertIncludes(source, marker, label) {
  if (!source.includes(marker)) {
    throw new Error(`${label}: missing marker ${JSON.stringify(marker)}`);
  }
}

function assertExcludes(source, marker, label) {
  if (source.includes(marker)) {
    throw new Error(`${label}: stale marker must remain absent: ${JSON.stringify(marker)}`);
  }
}

function assertMissing(relativePath) {
  if (fs.existsSync(path.join(repoRoot, relativePath))) {
    throw new Error(`${relativePath}: retired Blog Category Translation source must remain absent`);
  }
}

function main() {
  const centralPlan = read(centralPlanPath);
  const moduleReadme = read(moduleReadmePath);
  const modulePlan = read(modulePlanPath);
  const registry = JSON.parse(read(registryPath));

  assertIncludes(centralPlan, "This is the active cross-cutting implementation plan. As of 2026-08-28:", centralPlanPath);
  assertIncludes(centralPlan, "Blog Category consumer cutover: completed through TAXONOMY-CAT-12.", centralPlanPath);
  assertIncludes(centralPlan, "[x] resolve Blog Category/Taxonomy ownership drift through TAXONOMY-CAT-1..12;", centralPlanPath);
  assertIncludes(centralPlan, "Blog Category canonical copy is not a Blog Translation target.", centralPlanPath);
  assertIncludes(moduleReadme, "Canonical Blog Category copy is\nnot a separate Translation aggregate", moduleReadmePath);
  assertIncludes(moduleReadme, "The former `blog/category` provider, Blog Category Translation\nchange journal, and Blog-local Category translation storage are retired", moduleReadmePath);
  assertIncludes(modulePlan, "last_reviewed: 2026-08-28", modulePlanPath);
  assertIncludes(modulePlan, "Blog\n  Category canonical copy is consumed through the same-ID Blog-to-Taxonomy\n  Category binding and the `taxonomy/term` provider", modulePlanPath);

  for (const [label, source] of [
    [centralPlanPath, centralPlan],
    [moduleReadmePath, moduleReadme],
    [modulePlanPath, modulePlan],
  ]) {
    assertExcludes(source, "Media, Taxonomy, Blog category, Navigation menu, and Pages metadata", label);
    assertExcludes(source, "Registered `blog/category` pilot supplies", label);
    assertExcludes(source, "Blog applies category copy through its service", label);
    assertExcludes(source, "Blog's\n  `blog/category` provider exposes", label);
  }

  const blogCategory = registry.surfaces?.find((surface) => surface.id === "blog_categories");
  if (!blogCategory) throw new Error(`${registryPath}: blog_categories classification is missing`);
  if (blogCategory.owner_slug !== "blog" || blogCategory.resource_kind !== "category") {
    throw new Error(`${registryPath}: blog_categories identity drifted`);
  }
  if (blogCategory.readiness !== "excluded") {
    throw new Error(`${registryPath}: blog_categories must be explicitly excluded`);
  }
  if (blogCategory.provider_status !== "not_registered") {
    throw new Error(`${registryPath}: blog_categories must stay not_registered`);
  }
  if (blogCategory.ai_export !== "forbidden") {
    throw new Error(`${registryPath}: excluded Blog Category duplicate target must forbid direct AI export`);
  }
  if (!blogCategory.exclusion_reason?.includes("Taxonomy-owned")) {
    throw new Error(`${registryPath}: blog_categories exclusion must name Taxonomy ownership`);
  }
  if (blogCategory.evidence_paths?.some((entry) => entry.includes("rustok-blog/src/translation_target"))) {
    throw new Error(`${registryPath}: blog_categories must not cite retired Blog provider source`);
  }

  const duplicateRegistered = registry.surfaces?.find(
    (surface) =>
      surface.owner_slug === "blog" &&
      surface.resource_kind === "category" &&
      surface.provider_status === "registered",
  );
  if (duplicateRegistered) {
    throw new Error(`${registryPath}: duplicate Blog Category Translation provider is registered`);
  }

  const taxonomyTerms = registry.surfaces?.find((surface) => surface.id === "taxonomy_terms");
  if (!taxonomyTerms || taxonomyTerms.owner_slug !== "taxonomy" || taxonomyTerms.provider_status !== "registered") {
    throw new Error(`${registryPath}: canonical taxonomy_terms provider must remain registered`);
  }

  for (const retiredPath of [
    "crates/rustok-blog/src/translation_target.rs",
    "crates/rustok-blog/src/translation_target_tests.rs",
    "crates/rustok-blog/src/translation_evidence.rs",
    "crates/rustok-blog/tests/category_translation_target_postgres_test.rs",
    "crates/rustok-blog/contracts/evidence/blog-category-translation-postgres-source.json",
    "scripts/verify/verify-blog-category-translation-postgres-source.mjs",
  ]) {
    assertMissing(retiredPath);
  }

  if (!fs.existsSync(path.join(repoRoot, "crates/rustok-taxonomy/src/translation_target.rs"))) {
    throw new Error("crates/rustok-taxonomy/src/translation_target.rs: canonical Translation owner source is missing");
  }

  console.log("OK  Blog Category Translation docs/registry use canonical Taxonomy ownership");
}

try {
  main();
} catch (error) {
  console.error(error instanceof Error ? error.message : String(error));
  process.exit(1);
}
