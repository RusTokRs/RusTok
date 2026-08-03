#!/usr/bin/env node

import { readFile } from "node:fs/promises";

const files = {
  test: "crates/rustok-index/tests/drift_finding_writer_postgres_test.rs",
  doc: "crates/rustok-index/docs/m6-drift-finding-postgres-harness.md",
  recheck: "crates/rustok-index/docs/implementation-recheck-2026-08-03.md",
  plan: "crates/rustok-index/docs/implementation-plan-current-2026-08-03.md",
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
  "M6 bounded replay interruption, source timeout, and no-write dry-run",
  "M6 reconciliation retry, dead-letter, recovery, and generic host scheduling",
  "M6 bounded drift-finding inspection and persistence",
  "M6 snapshot-pair digest producer and mismatch-only recorder delegation",
  "M6 locale-optional persisted entity finding scope",
  "Add bounded drift-finding inspection and persistence for already-computed digest mismatches.",
  "drift_finding_writer_postgres_test",
  "Add one production snapshot reader",
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
  if (
    doc.toLowerCase().includes(claim.toLowerCase()) ||
    recheck.toLowerCase().includes(claim.toLowerCase())
  ) {
    throw new Error(`documentation makes forbidden execution claim: ${claim}`);
  }
}

if (!plan.includes("- [ ] Add one production snapshot reader")) {
  throw new Error("current plan overlay must keep authoritative snapshot capture open");
}
if (!plan.includes("- [ ] Add targeted repair with before/after admitted evidence.")) {
  throw new Error("current plan overlay must keep targeted repair and evidence open");
}

console.log("Index drift-finding PostgreSQL harness contract verified");
