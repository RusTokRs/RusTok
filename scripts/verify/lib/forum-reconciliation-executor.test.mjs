#!/usr/bin/env node

// The reconciliation guards delegate their fence, metric and page-bound rules to
// `forum-reconciliation-executor.mjs`. These cases mutate the live sources one rule at a time, so a
// rule that stops matching anything shows up here as a mutation that is no longer caught.

import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";

import {
  RECONCILIATION_EXECUTOR_PATH,
  assertReconciliationReport,
  reconciliationExecutorFindings,
  reconciliationReportFindings,
} from "./forum-reconciliation-executor.mjs";

const SERVICES = "crates/modules/rustok-forum/src/services";
const REPORTS = {
  counter: `${SERVICES}/counter_reconciliation.rs`,
  solution: `${SERVICES}/solution_reconciliation.rs`,
  mention: `${SERVICES}/mention_reconciliation.rs`,
  subscription: `${SERVICES}/subscription/reconciliation.rs`,
};

const executor = readFileSync(RECONCILIATION_EXECUTOR_PATH, "utf8");
const report = (name) => readFileSync(REPORTS[name], "utf8");

function mutate(source, from, to) {
  assert.ok(source.includes(from), `fixture drift: the live source no longer contains ${from}`);
  return source.replace(from, to);
}

test("the live executor and every live report satisfy the contract", () => {
  assert.deepEqual(reconciliationExecutorFindings(executor), []);
  for (const name of Object.keys(REPORTS)) {
    assert.deepEqual(reconciliationReportFindings(report(name), name), [], name);
    assertReconciliationReport({ executorSource: executor, serviceSource: report(name), name });
  }
});

test("an executor that reuses the span operation as a counter label is rejected", () => {
  const entrypoint = mutate(executor, "self.entrypoint,", "self.operation,");
  assert.match(reconciliationExecutorFindings(entrypoint).join("\n"), /self\.entrypoint/);

  const errorType = mutate(executor, "self.error_type, \"error\"", "self.operation, \"error\"");
  assert.match(reconciliationExecutorFindings(errorType).join("\n"), /self\.error_type/);
});

test("an executor without one half of the operator fence is rejected", () => {
  const topicsOnly = mutate(
    executor,
    "enforce_scope(security, Resource::ForumCategories, Action::Manage)?;",
    "",
  );
  assert.match(reconciliationExecutorFindings(topicsOnly).join("\n"), /ForumCategories/);
});

test("a report with a renamed metric label is rejected", () => {
  const renamed = mutate(
    report("counter"),
    'entrypoint: "counter_reconciliation_report",',
    'entrypoint: "forum.counter_reconciliation_report",',
  );
  assert.match(reconciliationReportFindings(renamed, "counter").join("\n"), /entrypoint/);
});

test("a report that bypasses the executor is rejected", () => {
  const bypass = mutate(report("mention"), "EXECUTOR\n            .execute(", "self.unfenced(");
  assert.match(reconciliationReportFindings(bypass, "mention").join("\n"), /EXECUTOR\.execute/);
  assert.throws(() =>
    assertReconciliationReport({ executorSource: executor, serviceSource: bypass, name: "mention" }),
  );
});

test("a report that keeps its own page clamp or its own fence is rejected", () => {
  const ownClamp = mutate(
    report("subscription"),
    "EXECUTOR.effective_limit(requested_limit)",
    "requested_limit.unwrap_or(100).clamp(1, 500)",
  );
  const clampFindings = reconciliationReportFindings(ownClamp, "subscription").join("\n");
  assert.match(clampFindings, /effective_limit/);
  assert.match(clampFindings, /must leave \.clamp\(/);

  const ownFence = `fn enforce_operations_scope(security: &SecurityContext) {}\n${report("solution")}`;
  assert.match(
    reconciliationReportFindings(ownFence, "solution").join("\n"),
    /must leave fn enforce_operations_scope/,
  );
});

test("a rule used only by a report's test module does not count as a second copy", () => {
  const withTestOnlyClamp = `${report("counter")}\n#[cfg(test)]\nmod extra { fn c() { 1u64.clamp(1, 2); } }\n`;
  assert.deepEqual(reconciliationReportFindings(withTestOnlyClamp, "counter"), []);
});
