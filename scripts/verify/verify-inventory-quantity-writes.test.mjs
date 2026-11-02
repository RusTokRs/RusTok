#!/usr/bin/env node
// Definitions only in the agent handoff; executing this file runs Node tests.
import { test } from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import {
  files,
  adapters,
  verifyInventoryQuantityWrites,
} from "./verify-inventory-quantity-writes.mjs";

function fixture(run) {
  const root = fs.mkdtempSync(
    path.join(os.tmpdir(), "inventory-quantity-source-"),
  );
  try {
    const packet = JSON.parse(fs.readFileSync(files.packet, "utf8"));
    for (const name of new Set([
      ...Object.values(files),
      ...adapters,
      ...Object.keys(packet.preserved_hashes),
    ])) {
      const target = path.join(root, name);
      fs.mkdirSync(path.dirname(target), { recursive: true });
      fs.copyFileSync(name, target);
    }
    run(root);
  } finally {
    fs.rmSync(root, { recursive: true, force: true });
  }
}

function change(root, name, before, after) {
  const file = path.join(root, name);
  const source = fs.readFileSync(file, "utf8");
  assert.ok(source.includes(before), `mutation marker exists: ${before}`);
  fs.writeFileSync(file, source.replace(before, after));
}

function refused(root) {
  assert.ok(verifyInventoryQuantityWrites({ root }).failures.length > 0);
}

test("canonical quantity source is discoverable without runtime promotion", () =>
  fixture((root) => {
    const result = verifyInventoryQuantityWrites({ root });
    assert.deepEqual(result.failures, []);
    assert.equal(result.compiled, false);
    assert.equal(result.tests_executed, false);
    assert.equal(result.rust_definitions, 33);
    assert.equal(result.node_definitions, 10);
  }));

test("quantity source rejects missing tenant and positive item proof", () => {
  for (const marker of [
    "tenant_id.is_nil()",
    "product::Column::TenantId.eq(tenant_id)",
    "items[0].id.is_nil()",
    "items.len() != 1",
  ])
    fixture((root) => {
      change(root, files.plan, marker, "false");
      refused(root);
    });
});

test("quantity source rejects omitted complete bound and original current binding proof", () => {
  for (const marker of [
    "MAX_QUANTITY_LEVELS + 1",
    "Level::binding",
    "query.lock_exclusive()",
    "performance_schema.events_transactions_current AS trx",
    "thread.PROCESSLIST_ID = CONNECTION_ID()",
    "trx.STATE = 'ACTIVE'",
    "trx.END_EVENT_ID IS NULL",
    "trx.AUTOCOMMIT = 'NO'",
    "current_location.deleted_at.is_some()",
  ])
    fixture((root) => {
      change(root, files.plan, marker, "removed_admission");
      refused(root);
    });
});

test("quantity source rejects saturating arithmetic and missing counter validation", () => {
  for (const [before, after] of [
    ["checked_add(adjustment)", "saturating_add(adjustment)"],
    ["level.incoming_quantity < 0", "false"],
    ["i32::try_from(target)", "target as i32"],
  ])
    fixture((root) => {
      change(root, files.plan, before, after);
      refused(root);
    });
});

test("quantity source rejects a lost noop gate or independent helper commit", () => {
  fixture((root) => {
    change(root, files.plan, "before == after", "false");
    refused(root);
  });
  fixture((root) => {
    fs.appendFileSync(
      path.join(root, files.plan),
      "\n// forbidden txn.commit()\n",
    );
    refused(root);
  });
});

test("quantity source rejects weakened exact effects or a raw publication path", () => {
  fixture((root) => {
    change(root, files.plan, "updated.rows_affected != 1", "false");
    refused(root);
  });
  fixture((root) => {
    change(root, files.plan, "publish_root_in_tx", "publish_in_tx");
    refused(root);
  });
});

test("quantity source rejects disconnected native and Product owner hooks", () => {
  fixture((root) => {
    change(
      root,
      files.projection,
      "BootstrapService::load_available_quantities(conn, &variant_ids)",
      "BootstrapService::load_available_quantities(&self.db, &variant_ids)",
    );
    refused(root);
  });
  fixture((root) => {
    change(
      root,
      files.native,
      "super::quantity::write_quantity",
      "unproved_write",
    );
    refused(root);
  });
  fixture((root) => {
    change(
      root,
      files.commands,
      "BootstrapService::update_initial_quantity_in_tx",
      "BootstrapService::unproved_quantity",
    );
    refused(root);
  });
});

test("quantity source rejects missing finite policy and a saturating visible projection", () => {
  fixture((root) => {
    change(
      root,
      files.commandPort,
      "inventory_quantity_blocker(error)",
      "unknown_provider_kind(error)",
    );
    refused(root);
  });
  fixture((root) => {
    change(
      root,
      adapters[1],
      "CommerceError::InventoryQuantityBlocked",
      "CommerceError::UnknownDependency",
    );
    refused(root);
  });
  fixture((root) => {
    fs.appendFileSync(
      path.join(root, files.visible),
      "\n// forbidden saturating_add\n",
    );
    refused(root);
  });
});

test("quantity source rejects historical/event digest rewriting", () =>
  fixture((root) => {
    const packet = JSON.parse(
      fs.readFileSync(path.join(root, files.packet), "utf8"),
    );
    const digest = Object.keys(packet.preserved_hashes).find((name) =>
      name.includes("event-contract-digests"),
    );
    fs.appendFileSync(path.join(root, digest), " ");
    refused(root);
  }));

test("quantity packet cannot claim compiled executed baselined versioned or Index-ready evidence", () => {
  for (const key of [
    "compiled",
    "tests_executed",
    "sql_executed",
    "runtime_verified",
    "baseline_initialized",
    "inventory_revisions_added",
    "generic_index_complete",
    "step8_complete",
    "status",
    "rust_count",
    "node_count",
    "nested_executed",
  ])
    fixture((root) => {
      const file = path.join(root, files.packet);
      const packet = JSON.parse(fs.readFileSync(file, "utf8"));
      if (key === "status") packet.status = "runtime_verified";
      else if (key === "rust_count") packet.regressions.rust_definitions = 0;
      else if (key === "node_count") packet.regressions.node_definitions = 0;
      else if (key === "nested_executed") packet.regressions.executed = true;
      else packet[key] = true;
      fs.writeFileSync(file, JSON.stringify(packet));
      refused(root);
    });
});
