#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __filename = fileURLToPath(import.meta.url);
const repoRoot = path.resolve(path.dirname(__filename), "..", "..", "..", "..", "..");
const read = (relativePath) =>
  fs.readFileSync(path.join(repoRoot, relativePath), "utf8");

const evidence = JSON.parse(
  read(
    "crates/modules/rustok-pages/contracts/evidence/pages-native-storefront-cache-source.json",
  ),
);
const harness = read(
  "crates/modules/rustok-pages/tests/native_storefront_cache_contract.rs",
);
const nativeAdapter = read(
  "crates/modules/rustok-pages/storefront/src/transport/native_server_adapter.rs",
);
const cacheContract = read("crates/modules/rustok-pages/src/cache_invalidation.rs");
const parityPlan = read(
  "docs/modules/pages-page-builder-parity-continuation-plan.md",
);
const packet = read(
);
const actualization = read(
);
const failures = [];

const requireText = (content, value, label) => {
  if (!content.includes(value)) failures.push(`${label}: missing ${value}`);
};
const forbidText = (content, value, label) => {
  if (content.includes(value)) failures.push(`${label}: forbidden ${value}`);
};
const requireOrder = (content, values, label) => {
  let previous = -1;
  for (const value of values) {
    const index = content.indexOf(value, previous + 1);
    if (index < 0) {
      failures.push(`${label}: missing or out of order ${value}`);
      return;
    }
    previous = index;
  }
};
const sliceBetween = (content, start, end, label) => {
  const startIndex = content.indexOf(start);
  if (startIndex < 0) {
    failures.push(`${label}: missing ${start}`);
    return "";
  }
  const endIndex = content.indexOf(end, startIndex + start.length);
  if (endIndex < 0) {
    failures.push(`${label}: missing ${end}`);
    return "";
  }
  return content.slice(startIndex, endIndex);
};

if (evidence.status !== "pages_native_storefront_cache_source_unvalidated") {
  failures.push(`evidence status mismatch: ${evidence.status}`);
}
if (!Array.isArray(evidence.execution) || evidence.execution.length !== 0) {
  failures.push("execution evidence must remain empty");
}
, marker, "native storefront cache packet");
}
, marker, "Page Builder parity actualization");
}

for (const forbidden of [
  "redis::",
  'cmd("SCAN")',
  'cmd("KEYS")',
  "PageService::new",
  "PageBuilderArtifactService::new",
  "HostRuntimeContext",
]) {
  forbidText(harness, forbidden, "contract harness ownership boundary");
}

if (failures.length > 0) {
  console.error("[verify-pages-native-storefront-cache] FAIL");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log(
  "[verify-pages-native-storefront-cache] PASS source_ready=true execution=pending",
);
