#!/usr/bin/env node

import { readFile } from "node:fs/promises";
import process from "node:process";

const files = {
  test: "crates/rustok-index/tests/drift_finding_writer_postgres_test.rs",
  doc: "crates/rustok-index/docs/m6-drift-finding-postgres-harness.md",
  recheck: "crates/rustok-index/docs/implementation-recheck-2026-08-03.md",
  plan: "crates/rustok-index/docs/implementation-plan.md",
};

const [test, doc, recheck, plan] = await Promise.all(
  Object.values(files).map((path) => readFile(path, "utf8")),
);

const requiredTestMarkers = [
  'const DATABASE_ENV: &str = "RUSTOK_INDEX_TEST_DATABASE_URL";',
  "for migration in IndexModule.migrations()",
  "tokio::join!(",
  "IndexDriftFindingWriteOutcome::Created",
  "IndexDriftFindingWriteOutcome::Refreshed",
  "IndexDriftFindingWriteOutcome::Reopened",
  "IndexDriftFindingWriteOutcome::Suppressed",
  "PostgresIndexDriftFindingWriter::new",
  "count_all_findings",
  'DROP SCHEMA IF EXISTS',
];

const requiredDocMarkers = [
  "advisory-lock serialization",
  "retained execution evidence until the repository owner runs",
  "--test drift_finding_writer_postgres_test",
];

const requiredPlanMarkers = [
  "M6 bounded replay interruption, timeout, and dry-run",
  "M6 reconciliation retry, dead-letter, and host scheduling",
  "M6 bounded drift-finding inspection and persistence",
  "Add bounded drift-finding inspection and persistence for already-computed digest mismatches.",
  "Add authoritative digest production, orphan diagnosis, finding lifecycle commands, targeted repair, and admitted repair evidence.",
  "drift_finding_writer_postgres_test",
];

function requireMarkers(label, content, markers) {
  for (const marker of markers) {
    if (!content.includes(marker)) {
      throw new Error(`${label} is missing required marker: ${marker}`);
    }
  }
}

requireMarkers(files.test, test, requiredTestMarkers);
requireMarkers(files.doc, doc, requiredDocMarkers);
requireMarkers(files.plan, plan, requiredPlanMarkers);
requireMarkers(files.recheck, recheck, [
  "Audited commit: `e66540bceffe0ae23ee2d04e0f39a1a6ab08aaeb`",
  "The harness is source-ready only.",
  "Define the bounded producer contract",
]);

const forbiddenClaims = [
  "tests passed",
  "PostgreSQL execution passed",
  "retained evidence admitted",
  "repair is complete",
];
for (const claim of forbiddenClaims) {
  if (doc.toLowerCase().includes(claim.toLowerCase()) || recheck.toLowerCase().includes(claim.toLowerCase())) {
    throw new Error(`documentation makes forbidden execution claim: ${claim}`);
  }
}

if (!plan.includes("- [ ] Add authoritative digest production")) {
  throw new Error("canonical plan must keep complete drift diagnosis and repair open");
}

console.log("Index drift-finding PostgreSQL harness contract verified");
