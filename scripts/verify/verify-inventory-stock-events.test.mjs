// Maintainer-run guard mutations. Definitions only; not executed by the authoring agent.
import test from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";
import {
  sourceFiles,
  sourcePaths,
  verifyInventoryStockEvents,
} from "./verify-inventory-stock-events.mjs";

const repository = path.resolve(
  path.dirname(fileURLToPath(import.meta.url)),
  "../..",
);
function fixture(run) {
  const root = fs.mkdtempSync(
    path.join(os.tmpdir(), "inventory-stock-outbox-"),
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
function replace(root, key, from, to) {
  const file = path.join(root, sourceFiles[key]);
  const source = fs.readFileSync(file, "utf8");
  const changed = source.replace(from, to);
  assert.notEqual(changed, source, `mutation did not change ${key}`);
  fs.writeFileSync(file, changed);
}
function rejected(root, label) {
  const result = verifyInventoryStockEvents({ root });
  assert.equal(result.status, "failed");
  assert.ok(
    result.failures.some((failure) => failure.includes(label)),
    result.failures.join("\n"),
  );
}

test("native source contract is guarded without claiming runtime readiness", () => {
  fixture((root) =>
    assert.deepEqual(verifyInventoryStockEvents({ root }).failures, []),
  );
});
test("canonical root transactional publisher cannot be replaced by another pipeline", () => {
  fixture((root) => {
    replace(
      root,
      "events",
      "TransactionalEventBus::publish_root_in_tx(",
      "AnotherPipeline::publish_later(",
    );
    rejected(root, "canonical same-transaction physical invalidation");
  });
});
test("equal available counters cannot suppress a real physical shipment invalidation", () => {
  fixture((root) => {
    replace(
      root,
      "events",
      /change\.old_stocked == change\.new_stocked\s*&&\s*change\.old_reserved == change\.new_reserved/,
      "change.available_delta() == 0",
    );
    rejected(root, "equal availability must still invalidate");
  });
});
test("the event plan cannot be postponed until after the preparation-only return", () => {
  fixture((root) => {
    replace(
      root,
      "shipment",
      /let\s+stock_events\s*=\s*StockEventPlan::prepare\(\s*txn,\s*context,\s*OPERATION,\s*changes,?\s*\)\s*\.await\s*\?;/,
      "let stock_events = unchecked_stock_event_plan(changes);",
    );
    rejected(root, "preflight event representability before carrier");
  });
});
test("global snapshots cannot ignore a foreign or missing stock location", () => {
  fixture((root) => {
    replace(root, "events", "foreign_locations != 0", "false");
    rejected(root, "bounded tenant-proved global quantity snapshots");
  });
});
test("return stock cannot commit before its required canonical invalidation", () => {
  fixture((root) => {
    replace(
      root,
      "returns",
      /stock_events\s*\.publish\(\s*&txn,\s*&context,\s*RESTOCK_OPERATION,?\s*\)\s*\.await\s*\?;/,
      "defer_stock_event_after_commit(stock_events);",
    );
    rejected(root, "return outbox commits with receipt and stock");
  });
});
test("a zero-row unconditional outbox insert cannot become publication success", () => {
  fixture((root) => {
    replace(root, "outbox", "if affected != 1 {", "if affected > 1 {");
    rejected(root, "zero-row insert cannot ACK");
  });
});
test("service and system actors cannot be fabricated into human UUIDs", () => {
  fixture((root) => {
    replace(
      root,
      "events",
      "PortActorKind::Service | PortActorKind::System => Ok(None)",
      "PortActorKind::Service | PortActorKind::System => Ok(Some(Uuid::nil()))",
    );
    rejected(root, "no fabricated service human");
  });
});
test("outbox IO cannot bypass the final native shipment budget gate", () => {
  fixture((root) => {
    replace(
      root,
      "shipment",
      /require_remaining_budget\(context, started\)\s*\}\s*\n\s*fn require_remaining_budget/,
      "Ok(())\n}\n\nfn require_remaining_budget",
    );
    rejected(root, "final native budget gate includes outbox IO");
  });
});
test("physical regressions must stay in the registered parent target", () => {
  fixture((root) => {
    replace(
      root,
      "parent",
      "mod stock_events;",
      "// detached stock-events definitions",
    );
    rejected(root, "physical regression module is registered");
  });
});
