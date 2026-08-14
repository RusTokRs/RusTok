#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";

const root = path.resolve(
  process.env.RUSTOK_TAXONOMY_CONTRACT_ROOT || process.cwd(),
);
const failures = [];

function absolute(relative) {
  return path.join(root, relative);
}

function exists(relative) {
  return fs.existsSync(absolute(relative));
}

function read(relative) {
  return fs.readFileSync(absolute(relative), "utf8");
}

function fail(message) {
  failures.push(message);
}

function requireFile(relative) {
  if (!exists(relative)) {
    fail(`missing Taxonomy contract artifact: ${relative}`);
    return null;
  }
  return read(relative);
}

function requireMarkers(relative, markers) {
  const source = requireFile(relative);
  if (source === null) return;
  for (const marker of markers) {
    if (!source.includes(marker)) {
      fail(`${relative}: missing contract marker: ${marker}`);
    }
  }
}

function dependencyBlock(source) {
  const header = /^\[dependencies\]\s*$/m.exec(source);
  if (!header) return null;
  const rest = source.slice((header.index ?? 0) + header[0].length);
  const nextHeader = rest.search(/^\[[^\]]+\]\s*$/m);
  return nextHeader >= 0 ? rest.slice(0, nextHeader) : rest;
}

function requireTaxonomyDependency(consumer) {
  const source = requireFile(consumer.manifest);
  if (source === null) return;

  const moduleSlugPattern = new RegExp(
    `^slug\\s*=\\s*"${consumer.slug}"\\s*$`,
    "m",
  );
  if (!moduleSlugPattern.test(source)) {
    fail(`${consumer.manifest}: missing module slug ${consumer.slug}`);
  }

  const dependencies = dependencyBlock(source);
  if (dependencies === null) {
    fail(`${consumer.manifest}: missing [dependencies] block`);
    return;
  }

  if (!/^taxonomy\s*=\s*\{[^\n}]*version_req\s*=\s*">=0\.1\.0"[^\n}]*\}\s*$/m.test(dependencies)) {
    fail(
      `${consumer.manifest}: ${consumer.name} must declare taxonomy >=0.1.0 in [dependencies]`,
    );
  }
}

const consumers = [
  {
    name: "Blog",
    slug: "blog",
    manifest: "crates/rustok-blog/rustok-module.toml",
    contract: "crates/rustok-blog/CRATE_API.md",
    contractMarkers: [
      "keeps `blog_post_tags` as the module-owned relation table",
      "Canonical tag identity now lives in shared `rustok-taxonomy`",
    ],
  },
  {
    name: "Forum",
    slug: "forum",
    manifest: "crates/rustok-forum/rustok-module.toml",
    contract: "crates/rustok-forum/docs/README.md",
    contractMarkers: [
      "tag attachments via `forum_topic_tags` with shared vocabulary in `rustok-taxonomy`",
      "uses `rustok-taxonomy` as a shared dictionary for tag identity",
    ],
  },
  {
    name: "Product",
    slug: "product",
    manifest: "crates/rustok-product/rustok-module.toml",
    contract: "crates/rustok-product/README.md",
    contractMarkers: [
      "Product-owned relation storage for taxonomy-backed tags (`product_tags`).",
      "Depends on `rustok-taxonomy` for shared scope-aware tag dictionary",
    ],
  },
  {
    name: "Profiles",
    slug: "profiles",
    manifest: "crates/rustok-profiles/rustok-module.toml",
    contract: "crates/rustok-profiles/README.md",
    contractMarkers: [
      "Own profile-to-taxonomy relation storage via `profile_tags`.",
      "Depends on `rustok-taxonomy` for shared scope-aware tags while keeping `profile_tags` module-owned.",
    ],
  },
];

for (const consumer of consumers) {
  requireTaxonomyDependency(consumer);
  requireMarkers(consumer.contract, consumer.contractMarkers);
}

const dtoPath = "crates/rustok-taxonomy/src/dto.rs";
const dto = requireFile(dtoPath);
if (dto !== null) {
  const kindEnum = dto.match(/pub enum TaxonomyTermKind\s*\{([\s\S]*?)\n\}/);
  if (!kindEnum) {
    fail(`${dtoPath}: TaxonomyTermKind enum is missing`);
  } else {
    const variants = [...kindEnum[1].matchAll(/^\s*([A-Z][A-Za-z0-9_]*)\s*,\s*$/gm)].map(
      (match) => match[1],
    );
    if (variants.length !== 1 || variants[0] !== "Tag") {
      fail(
        `${dtoPath}: demonstrated kind baseline must remain exactly Tag until a new kind has an explicit ownership/lookup contract; found ${variants.join(", ") || "none"}`,
      );
    }
  }
}

requireMarkers("crates/rustok-taxonomy/tests/localized_route_lookup.rs", [
  "public_route_lookup_uses_registry_authority_over_unregistered_legacy_alias",
  "owner_batch_collapses_equivalent_labels_and_normalizes_scope_and_locale",
  "owner_batch_prefers_module_term_before_global_across_locale_fallback",
  "owner_batch_reuses_global_term_when_module_term_is_absent",
  "owner_batch_prefers_module_canonical_key_before_global_route",
  "owner_batch_reuses_global_canonical_key_without_shadow_module_term",
  "owner_batch_canonical_key_lookup_is_tenant_isolated",
]);

requireMarkers("crates/rustok-taxonomy/tests/route_key_registry.rs", [
  "hard_delete_removes_lookup_and_allows_route_identity_reuse",
  "database_primary_key_rejects_second_route_owner",
]);

const lookupWorkflow = ".github/workflows/taxonomy-lookup-contract.yml";
requireMarkers(lookupWorkflow, [
  '"crates/rustok-taxonomy/src/dto.rs"',
  "--test localized_route_lookup",
  "--test route_key_registry",
]);

if (failures.length > 0) {
  console.error("Taxonomy contract matrix verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log(
  `Taxonomy contract matrix checks passed: ${consumers.length} consumer manifests/public relation contracts are synchronized; demonstrated kinds=Tag; focused lookup coverage remains wired.`,
);
