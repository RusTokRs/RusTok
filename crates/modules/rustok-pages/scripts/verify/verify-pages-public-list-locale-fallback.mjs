#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..", "..", "..", "..");
const read = (relativePath) => fs.readFileSync(path.join(root, relativePath), "utf8");
const failures = [];

const evidence = JSON.parse(
  read("crates/modules/rustok-pages/contracts/evidence/pages-public-list-locale-fallback-source.json"),
);
const owner = read("crates/modules/rustok-pages/src/services/page/read.rs");
const nativeAdapter = read(
  "crates/modules/rustok-pages/storefront/src/transport/native_server_adapter.rs",
);
const graphql = read("crates/modules/rustok-pages/src/graphql/query.rs");
const regression = read("crates/modules/rustok-pages/tests/page_locale_fallback.rs");
const cacheGuard = read(
  "crates/modules/rustok-pages/scripts/verify/verify-pages-native-storefront-cache.mjs",
);
const serverFnGuard = read(
  "crates/modules/rustok-pages/scripts/verify/verify-pages-native-storefront-server-fn.mjs",
);
const channelGuard = read(
  "crates/modules/rustok-pages/scripts/verify/verify-pages-native-storefront-channel-admission.mjs",
);
const plan = read("docs/modules/pages-page-builder-parity-continuation-plan.md");
const localPlan = read("crates/modules/rustok-pages/docs/implementation-plan.md");
const packet = read(
);

const need = (text, marker, label) => {
  if (!text.includes(marker)) failures.push(`${label}: missing ${marker}`);
};
const forbid = (text, marker, label) => {
  if (text.includes(marker)) failures.push(`${label}: forbidden ${marker}`);
};
const ordered = (text, markers, label) => {
  let previous = -1;
  for (const marker of markers) {
    const index = text.indexOf(marker, previous + 1);
    if (index < 0) {
      failures.push(`${label}: missing or out of order ${marker}`);
      return;
    }
    previous = index;
  }
};
const between = (text, start, end, label) => {
  const from = text.indexOf(start);
  if (from < 0) {
    failures.push(`${label}: missing start ${start}`);
    return "";
  }
  const to = text.indexOf(end, from + start.length);
  if (to < 0) {
    failures.push(`${label}: missing end ${end}`);
    return "";
  }
  return text.slice(from, to);
};

if (evidence.format !== "pages_public_list_locale_fallback_source_v1") {
  failures.push(`evidence format mismatch: ${evidence.format}`);
}
if (evidence.status !== "pages_public_list_locale_fallback_source_unvalidated") {
  failures.push(`evidence status mismatch: ${evidence.status}`);
}
if (!Array.isArray(evidence.execution) || evidence.execution.length !== 0) {
  failures.push("source evidence execution must remain empty");
}
for (const [key, value] of Object.entries(evidence.validation ?? {})) {
  if (value !== false) failures.push(`validation.${key} must remain false`);
}
, marker, "locale fallback packet");
}

forbid(owner, "Iggy", "Pages owner read path");
forbid(nativeAdapter, "Iggy", "native Pages storefront");
forbid(graphqlPublicList, "Iggy", "GraphQL public list");

if (failures.length > 0) {
  console.error("[verify-pages-public-list-locale-fallback] FAIL");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}
console.log(
  "[verify-pages-public-list-locale-fallback] PASS source_ready=true execution=pending requested=fr tenant_fallback=ru",
);
