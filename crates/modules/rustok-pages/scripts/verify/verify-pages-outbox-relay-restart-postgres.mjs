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
    "crates/modules/rustok-pages/contracts/evidence/pages-outbox-relay-restart-postgres-source.json",
  ),
);
const harness = read("crates/modules/rustok-pages/tests/outbox_relay_restart_postgres.rs");
const relay = read("crates/modules/rustok-outbox/src/relay.rs");
const transactionalBus = read("crates/modules/rustok-outbox/src/transactional.rs");
const cacheOwner = read("crates/modules/rustok-pages/src/cache_invalidation.rs");
const overlay = read(
);
const failures = [];

const requireText = (content, value, label) => {
  if (!content.includes(value)) failures.push(`${label}: missing ${value}`);
};
const forbidText = (content, value, label) => {
  if (content.includes(value)) failures.push(`${label}: forbidden ${value}`);
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

if (evidence.status !== "pages_outbox_relay_restart_postgres_source_unvalidated") {
  failures.push(`evidence status mismatch: ${evidence.status}`);
}
if (!Array.isArray(evidence.execution) || evidence.execution.length !== 0) {
  failures.push("executed evidence must remain empty");
}
, marker, "relay restart continuation overlay");
}

if (failures.length > 0) {
  console.error("[verify-pages-outbox-relay-restart-postgres] FAIL");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log(
  "[verify-pages-outbox-relay-restart-postgres] PASS source_ready=true postgres_execution=pending",
);
