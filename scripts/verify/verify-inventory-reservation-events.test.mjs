// Regression definitions only. The maintainer runs this companion; the source guard must not.
import assert from "node:assert/strict";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";
import {
  sourceFiles,
  sourcePaths,
  verifyInventoryReservationEvents,
} from "./verify-inventory-reservation-events.mjs";

const repository = path.resolve(
  path.dirname(fileURLToPath(import.meta.url)),
  "../..",
);
function fixture(run) {
  const root = fs.mkdtempSync(
    path.join(os.tmpdir(), "reservation-events-source-"),
  );
  try {
    for (const file of sourcePaths) {
      const target = path.join(root, file);
      fs.mkdirSync(path.dirname(target), { recursive: true });
      fs.copyFileSync(path.join(repository, file), target);
    }
    run(root);
  } finally {
    fs.rmSync(root, { recursive: true, force: true });
  }
}
function mutate(root, key, before, after) {
  const file = path.join(root, sourceFiles[key]),
    source = fs.readFileSync(file, "utf8");
  assert.ok(
    typeof before === "string" ? source.includes(before) : source.match(before),
    `missing mutation anchor: ${before}`,
  );
  fs.writeFileSync(file, source.replace(before, after));
}
function refused(root) {
  assert.ok(verifyInventoryReservationEvents({ root }).failures.length > 0);
}

test("current source contract does not promote digest, runtime, Index or step-8 evidence", () =>
  fixture((root) => {
    const result = verifyInventoryReservationEvents({ root });
    assert.deepEqual(result.failures, []);
    assert.equal(result.source_only, true);
    assert.equal(result.tests_executed, false);
    assert.equal(result.compiled, false);
    assert.equal(result.step8_complete, false);
  }));

test("the Inventory fact cannot become an unsealed or raw event contract", () =>
  fixture((root) => {
    mutate(
      root,
      "family",
      "impl sealed::Sealed for InventoryReservationEvent {}",
      "",
    );
    refused(root);
  }));

test("quantity calls cannot discard the current actor/context again", () =>
  fixture((root) => {
    mutate(
      root,
      "ports",
      ".reserve(&context, request.variant_id, request.quantity)",
      ".reserve(tenant_id, request.variant_id, request.quantity)",
    );
    refused(root);
  }));

test("owner commit cannot precede the required canonical reservation fact", () =>
  fixture((root) => {
    mutate(
      root,
      "service",
      "event_plan.publish(&txn).await?;",
      "txn.commit().await?; event_plan.publish(&txn).await?;",
    );
    refused(root);
  }));

test("Memory cannot regress to a root-only sealed-event adapter", () =>
  fixture((root) => {
    mutate(
      root,
      "memory",
      "self.bus.publish_contract_envelope(envelope).await",
      'Err(crate::Error::External("unsupported".to_owned()))',
    );
    refused(root);
  }));

test("local typed handler errors cannot be discarded before acknowledgement", () =>
  fixture((root) => {
    mutate(root, "core", "result?;", "drop(result);");
    refused(root);
  }));

test("Iggy local projection failure must retain the original outbox identity", () =>
  fixture((root) => {
    mutate(root, "factory", /return Err\(error\);/, "return Ok(());");
    refused(root);
  }));

test("Search refresh cannot keep a repeatable-read snapshot across the scope lock", () =>
  fixture((root) => {
    mutate(
      root,
      "projector",
      "IsolationLevel::ReadCommitted",
      "IsolationLevel::RepeatableRead",
    );
    refused(root);
  }));

test("bounded artifact subscriptions cannot be silently bypassed", () =>
  fixture((root) => {
    const file = path.join(root, sourceFiles.artifacts),
      source = fs.readFileSync(file, "utf8");
    const start = source.indexOf("pub async fn project_contract"),
      end = source.indexOf("async fn project_source", start);
    assert.ok(start >= 0 && end > start);
    fs.writeFileSync(
      file,
      source.slice(0, start) +
        source
          .slice(start, end)
          .replace("self.project_source(", "self.skip_source(") +
        source.slice(end),
    );
    refused(root);
  }));

test("source evidence cannot claim compilation or hand-authored digest admission", () =>
  fixture((root) => {
    const file = path.join(root, sourceFiles.evidence),
      evidence = JSON.parse(fs.readFileSync(file, "utf8"));
    evidence.compiled = true;
    evidence.schema_digest.regenerated = true;
    fs.writeFileSync(file, JSON.stringify(evidence));
    refused(root);
  }));
