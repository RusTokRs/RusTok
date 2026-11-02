#!/usr/bin/env node
// Definitions only. Running this file executes tests; the authoring agent does not run it.
import { test } from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import os from "node:os";
import {
  files,
  verifyInventorySourceDelivery,
} from "./verify-inventory-source-delivery.mjs";

function fixture(run) {
  const root = fs.mkdtempSync(
    path.join(os.tmpdir(), "inventory-source-delivery-"),
  );
  try {
    const packet = JSON.parse(fs.readFileSync(files.packet, "utf8"));
    for (const name of new Set([
      ...Object.values(files),
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
  const file = path.join(root, name),
    text = fs.readFileSync(file, "utf8");
  assert.ok(text.includes(before), `mutation marker exists: ${before}`);
  fs.writeFileSync(file, text.replace(before, after));
}
const refused = (root) =>
  assert.ok(verifyInventorySourceDelivery({ root }).failures.length > 0);

test("source delivery discovers real owner/relay definitions without claiming runtime", () =>
  fixture((root) => {
    const result = verifyInventorySourceDelivery({ root });
    assert.deepEqual(result.failures, []);
    assert.equal(result.rust_definitions, 26);
    assert.equal(result.testsExecuted, false);
    assert.equal(result.runtimeVerified, false);
  }));

test("source delivery refuses unregistered schema or substituted write-once identity", () => {
  for (const [name, marker] of [
    [files.eventRegistry, ".chain(INVENTORY_SOURCE_EVENT_SCHEMAS.iter())"],
    [
      files.eventRegistry,
      ".or_else(|| inventory_source_event_schema(event_type))",
    ],
    [files.publisher, "publish_contract_once_direct_in_tx_with_envelope_id"],
  ])
    fixture((root) => {
      change(root, name, marker, "unverified_source_route");
      refused(root);
    });
});

test("source delivery refuses missing current actor proof or creator-derived activation authority", () => {
  for (const [name, marker] of [
    [files.publisher, "actor.is_some_and(|id| id.is_nil())"],
    [files.baseline, "crate::stock_events::event_actor_id(context)"],
    [files.bootstrap, "Some(actor_id)"],
    [files.deletion, "Some(actor_id)"],
  ])
    fixture((root) => {
      change(root, name, marker, "unverified_actor");
      refused(root);
    });
});

test("source delivery refuses replaced Inventory version or unbound original deletion transaction", () => {
  for (const marker of [
    "next_version(saved.source_version)",
    "record::Column::SourceVersion.eq(saved.source_version)",
    "update(self.conn, saved, 0, 0, 0, true, self.actor)",
  ])
    fixture((root) => {
      change(root, files.writer, marker, "unverified_revision");
      refused(root);
    });
});

test("source delivery refuses disconnected final activation and publication hidden in comments", () => {
  fixture((root) => {
    change(
      root,
      files.baseline,
      "super::publication::activated",
      "unverified_activation",
    );
    refused(root);
  });
  fixture((root) => {
    change(root, files.publisher, "saved != envelope_id", "false");
    fs.appendFileSync(
      path.join(root, files.publisher),
      "\n// saved != envelope_id\n",
    );
    refused(root);
  });
});

test("source delivery refuses unmatched-handler acknowledgement before broker send", () =>
  fixture((root) => {
    change(
      root,
      files.fanout,
      "if payload.requires_module_delivery() && !deliver_locally",
      "if false && !deliver_locally",
    );
    refused(root);
  }));

test("source delivery cannot capture remote-only Product routes in Inventory module policy", () =>
  fixture((root) => {
    change(
      root,
      files.wrapper,
      "Self::InventorySource(_)",
      "Self::ProductIndexRefresh(_)",
    );
    refused(root);
  }));

test("source delivery refuses observational journal publishers and adoption-side versions", () => {
  for (const [name, injected] of [
    [
      files.reader,
      "fn observed_state_is_not_a_publisher() { publication::changed(); }",
    ],
    [
      files.adoption,
      "fn metadata_is_not_stock_movement() { source_history::record_changed(); }",
    ],
  ])
    fixture((root) => {
      fs.appendFileSync(path.join(root, name), `\n${injected}\n`);
      refused(root);
    });
});

test("source delivery preserves potentially applied I027 and the unregenerated release artifact", () => {
  const packet = JSON.parse(fs.readFileSync(files.packet, "utf8"));
  for (const name of Object.keys(packet.preserved_hashes).filter(
    (name) =>
      name.includes("000027") || name.endsWith("event-contract-digests.json"),
  ))
    fixture((root) => {
      fs.appendFileSync(path.join(root, name), "\n ");
      refused(root);
    });
});

test("source delivery never promotes compilation, selected context, replay or Index readiness", () => {
  for (const name of [
    "compiled",
    "tests_executed",
    "source_context_authority_bound",
    "initial_replay_durable",
    "index_ready",
    "step8_complete",
  ])
    fixture((root) => {
      const target = path.join(root, files.packet),
        packet = JSON.parse(fs.readFileSync(target, "utf8"));
      packet[name] = true;
      fs.writeFileSync(target, JSON.stringify(packet));
      refused(root);
    });
});
