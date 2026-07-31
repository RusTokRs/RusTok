import fs from "node:fs";
import path from "node:path";
import process from "node:process";

const root = process.cwd();
const read = (file) => fs.readFileSync(path.join(root, file), "utf8");
const fail = (message) => {
  console.error(`index reconciliation process restart guard failed: ${message}`);
  process.exit(1);
};
const requireText = (source, marker, label) => {
  if (!source.includes(marker)) fail(`${label} must retain ${marker}`);
};

const targetPath =
  "crates/rustok-index/tests/source_reconciliation_process_restart_postgres_test.rs";
const directory = "crates/rustok-index/tests/reconciliation_process_restart";
const docsPath =
  "crates/rustok-index/docs/m6-reconciliation-process-restart-postgres-harness.md";

for (const file of [
  targetPath,
  `${directory}/connection.rs`,
  `${directory}/database.rs`,
  `${directory}/parent.rs`,
  `${directory}/process.rs`,
  `${directory}/query.rs`,
  `${directory}/runner.rs`,
  `${directory}/schema.rs`,
  `${directory}/source.rs`,
  `${directory}/worker.rs`,
  docsPath,
]) {
  if (!fs.existsSync(path.join(root, file))) fail(`missing ${file}`);
}

const target = read(targetPath);
const parent = read(`${directory}/parent.rs`);
const processFixture = read(`${directory}/process.rs`);
const worker = read(`${directory}/worker.rs`);
const query = read(`${directory}/query.rs`);
const docs = read(docsPath);

requireText(
  target,
  "reconciliation_yield_resumes_across_two_test_processes",
  "integration target",
);
requireText(
  target,
  "process_restart_worker_resumes_reconciliation_from_env",
  "private process worker",
);
requireText(processFixture, "env::current_exe()", "OS process boundary");
requireText(processFixture, '.arg("--exact")', "filtered child invocation");
requireText(processFixture, "PROCESS_WORKER_ENV", "private worker marker");
requireText(parent, "spawn_worker(&fixture, YIELD_PHASE)", "first child");
requireText(parent, "spawn_worker(&fixture, COMPLETE_PHASE)", "second child");
requireText(parent, "completed.job_id, yielded.job_id", "durable job identity");
requireText(parent, 'assert_eq!(completed.state, "succeeded")', "terminal state");
requireText(parent, "assert_eq!(completed.attempt_count, 2)", "attempt progression");
requireText(parent, "assert_eq!(completed.pages_processed, 2)", "cursor progression");
requireText(worker, "IndexReconciliationRunStatus::Yielded", "yield worker assertion");
requireText(worker, "IndexReconciliationRunStatus::Complete", "complete worker assertion");
requireText(query, "cursor->>'completed_passes'", "completed-pass evidence");
requireText(query, "cursor->>'pages_processed'", "processed-page evidence");
requireText(docs, "executable, not run", "validation disclosure");
requireText(docs, "does not prove a full server", "restart non-claim");

for (const forbidden of [
  "runtime_status: passed",
  "PostgreSQL execution passed",
  "full host restart proved",
]) {
  if (docs.includes(forbidden)) fail(`documentation must not claim ${forbidden}`);
}

console.log("index reconciliation process restart harness guard passed");
