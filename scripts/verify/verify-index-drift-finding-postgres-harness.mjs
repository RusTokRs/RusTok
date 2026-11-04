#!/usr/bin/env node

import { readFile } from "node:fs/promises";

const files = {
  test: "crates/modules/rustok-index/tests/drift_finding_writer_postgres_test.rs",
  doc: "crates/modules/rustok-index/docs/m6-drift-finding-postgres-harness.md",
  plan: "crates/modules/rustok-index/docs/implementation-plan.md",
};

const [test, doc, plan] = await Promise.all(
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
  "M6 source-version-fenced PostgreSQL drift snapshot reader",
  "Add bounded drift-finding inspection and persistence for already-computed digest mismatches.",
  "drift_finding_writer_postgres_test",
  "drift_snapshot_reader_postgres_test",
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

const forbiddenClaims = [
  "tests passed",
  "PostgreSQL execution passed",
  "retained evidence admitted",
  "repair is complete",
];
for (const claim of forbiddenClaims) {
  if (
    doc.toLowerCase().includes(claim.toLowerCase())
  ) {
    throw new Error(`documentation makes forbidden execution claim: ${claim}`);
  }
}

if (!plan.includes("- [ ] Run and admit `drift_snapshot_reader_postgres_test` evidence.")) {
  throw new Error("current plan overlay must keep snapshot-reader execution evidence open");
}
if (!plan.includes("- [ ] Add targeted repair with before/after admitted evidence.")) {
  throw new Error("current plan overlay must keep targeted repair and evidence open");
}

console.log("Index drift-finding PostgreSQL harness contract verified");
