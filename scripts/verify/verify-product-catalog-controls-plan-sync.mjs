#!/usr/bin/env node

import { existsSync, readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = process.env.RUSTOK_VERIFY_REPO_ROOT
  ? path.resolve(process.env.RUSTOK_VERIFY_REPO_ROOT)
  : path.resolve(scriptDir, "../..");
const failures = [];

function read(relativePath) {
  const absolutePath = path.join(repoRoot, relativePath);
  if (!existsSync(absolutePath)) {
    failures.push(`${relativePath}: required file is missing`);
    return "";
  }
  return readFileSync(absolutePath, "utf8");
}

function requireText(source, marker, message) {
  if (!source.includes(marker)) failures.push(message);
}

function forbidText(source, marker, message) {
  if (source.includes(marker)) failures.push(message);
}

const planPath = "crates/rustok-product/docs/implementation-plan.md";
const registryPath = "docs/modules/implementation-plans-registry.md";
const sources = [
  {
    path: "crates/rustok-product/storefront/src/core.rs",
    markers: [
      "pub search: Option<String>",
      "pub category_id: Option<String>",
      "pub sort_by: Option<String>",
      "pub sort_direction: Option<String>",
      "pub attribute_filters:",
    ],
  },
  {
    path: "crates/rustok-product/storefront/src/ui/leptos.rs",
    markers: ["category_id", "sort_by", "sort_direction", "attribute_filters"],
  },
  {
    path: "crates/rustok-product/storefront/src/transport/native_server_adapter.rs",
    markers: ["category_id", "sort_by", "sort_direction", "attribute_filters"],
  },
  {
    path: "crates/rustok-product/storefront/src/transport/graphql_adapter.rs",
    markers: ["category_id", "sort_by", "sort_direction", "attribute_filters"],
  },
  {
    path: "crates/rustok-product/admin/src/transport.rs",
    markers: ["category_id", "sort_by", "sort_direction", "attribute_filters"],
  },
  {
    path: "crates/rustok-product/admin/src/ui/leptos.rs",
    markers: ["category_id", "sort_by", "sort_direction", "attribute_filters"],
  },
  {
    path: "crates/rustok-product/admin/src/transport/graphql_adapter.rs",
    markers: ["category_id", "sort_by", "sort_direction", "attribute_filters"],
  },
  {
    path: "crates/rustok-commerce/src/graphql/types.rs",
    markers: ["category_id", "sort_by", "sort_direction", "attribute_filters"],
  },
];

const plan = read(planPath);
const registry = read(registryPath);
const sourceChecks = sources.map(({ path: sourcePath, markers }) => {
  const source = read(sourcePath);
  return {
    sourcePath,
    complete: markers.every((marker) => source.includes(marker)),
  };
});
const implementationComplete = sourceChecks.every(({ complete }) => complete);

const pendingMarker =
  "- [ ] Connect storefront/admin UI controls to optional catalog filters/sorts.";
const completeMarker =
  "- [x] Connect storefront/admin UI controls to optional catalog filters/sorts.";
const pending = plan.includes(pendingMarker);
const complete = plan.includes(completeMarker);

if (pending === complete) {
  failures.push(
    `${planPath}: catalog controls task must have exactly one pending/completed checkbox`,
  );
} else if (implementationComplete && pending) {
  failures.push(
    `${planPath}: catalog controls are source-complete but the task is still pending`,
  );
} else if (!implementationComplete && complete) {
  const missingSources = sourceChecks
    .filter(({ complete: sourceComplete }) => !sourceComplete)
    .map(({ sourcePath }) => sourcePath)
    .join(", ");
  failures.push(
    `${planPath}: catalog controls must remain pending until all source layers are complete; incomplete: ${missingSources}`,
  );
}

forbidText(
  plan,
  "optional catalog filters/sorts, detached-value marker contract",
  `${planPath}: catalog controls must not be described as source-locked before end-to-end execution exists`,
);
requireText(
  plan,
  "Recheck on 2026-07-28",
  `${planPath}: plan must retain the source recheck provenance`,
);
requireText(
  plan,
  "node scripts/verify/verify-product-catalog-controls-plan-sync.mjs",
  `${planPath}: plan must list the catalog controls plan-sync verifier`,
);
requireText(
  plan,
  "node scripts/verify/verify-product-catalog-controls-plan-sync.test.mjs",
  `${planPath}: plan must list the catalog controls plan-sync mutation tests`,
);

const productRegistryRow = registry
  .split("\n")
  .find((line) => line.startsWith("| `product` |"));
if (!productRegistryRow) {
  failures.push(`${registryPath}: product live-plan row is missing`);
} else if (!/catalog filter|filters\/sorts/i.test(productRegistryRow)) {
  failures.push(
    `${registryPath}: product nearest priority must retain the open catalog filter/sort slice`,
  );
}

if (failures.length > 0) {
  console.error("product catalog controls plan synchronization verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("product catalog controls plan synchronization verification passed");
