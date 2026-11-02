#!/usr/bin/env node
// Definitions only in the agent handoff. Running this file executes Node tests.
import { test } from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import os from "node:os";
import {
  files,
  verifyInventorySourceHistory,
} from "./verify-inventory-source-history.mjs";

function fixture(run) {
  const root = fs.mkdtempSync(
    path.join(os.tmpdir(), "inventory-source-history-"),
  );
  try {
    const packet = JSON.parse(fs.readFileSync(files.packet, "utf8"));
    for (const file of new Set([
      ...Object.values(files),
      ...Object.keys(packet.preserved_hashes),
    ])) {
      const target = path.join(root, file);
      fs.mkdirSync(path.dirname(target), { recursive: true });
      fs.copyFileSync(file, target);
    }
    run(root);
  } finally {
    fs.rmSync(root, { recursive: true, force: true });
  }
}
function change(root, name, before, after) {
  const file = path.join(root, name);
  const text = fs.readFileSync(file, "utf8");
  assert.ok(text.includes(before), `mutation marker exists: ${before}`);
  fs.writeFileSync(file, text.replace(before, after));
}
const refused = (root) =>
  assert.ok(verifyInventorySourceHistory({ root }).failures.length > 0);

test("source history discovers real owner definitions without runtime claims", () =>
  fixture((root) => {
    const result = verifyInventorySourceHistory({ root });
    assert.deepEqual(result.failures, []);
    assert.equal(result.testsExecuted, false);
    assert.equal(result.rust_definitions, 18);
  }));

test("source history refuses omitted current tenant and nonblocking writer baseline gates", () => {
  for (const marker of [
    "tenant.is_nil()",
    "actor.is_nil()",
    "pg_try_advisory_xact_lock_shared(",
    "if !saved.active",
  ])
    fixture((root) => {
      change(root, files.api, marker, "unverified_gate");
      refused(root);
    });
});

test("source history refuses mutable page identity or weakened atomic checkpoint", () => {
  for (const marker of [
    "receipt.after_variant_id != request.after_variant_id",
    "saved.last_variant_id != request.after_variant_id",
    "update.exec(txn).await?.rows_affected != 1",
    "page::Entity::insert",
  ])
    fixture((root) => {
      change(root, files.baseline, marker, "unverified_checkpoint");
      refused(root);
    });
});

test("source history refuses incomplete original snapshot and negative counter admission", () => {
  for (const marker of [
    "product::Column::TenantId.eq(tenant)",
    "MAX_SOURCE_LEVELS + 1",
    "level.incoming_quantity < 0",
    "sha256_digest",
  ])
    fixture((root) => {
      change(root, files.snapshot, marker, "unverified_snapshot");
      refused(root);
    });
});

test("source history refuses guessed version and missing retained delete provenance", () => {
  for (const marker of [
    "next_version(saved.source_version)",
    "saved.binding_digest != snapshot.binding_digest",
    "change::Entity::find_by_id(saved.last_change_id)",
    "record::Column::SourceVersion.eq(saved.source_version)",
  ])
    fixture((root) => {
      change(root, files.writer, marker, "unverified_version");
      refused(root);
    });
});

test("source history refuses disconnected actual writers or revision on metadata adoption", () => {
  fixture((root) => {
    change(root, files.reservation, "record_changed", "untracked_change");
    refused(root);
  });
  fixture((root) => {
    change(root, files.deletion, "record_deleted", "untracked_delete");
    refused(root);
  });
  fixture((root) => {
    fs.appendFileSync(
      path.join(root, files.adoption),
      "\n// source_history::record_changed\n",
    );
    refused(root);
  });
});

test("source history refuses changed immutable historical payloads and event digests", () =>
  fixture((root) => {
    const packet = JSON.parse(
      fs.readFileSync(path.join(root, files.packet), "utf8"),
    );
    const file = Object.keys(packet.preserved_hashes).find((name) =>
      name.includes("event-contract-digests"),
    );
    fs.appendFileSync(path.join(root, file), " ");
    refused(root);
  }));

test("source history packet cannot claim baseline execution publication selection or Index readiness", () => {
  for (const key of [
    "compiled",
    "tests_executed",
    "migrations_executed",
    "installation_baseline_initialized",
    "canonical_versioned_publication_wired",
    "distribution_selected",
    "index_ready",
    "step8_complete",
  ])
    fixture((root) => {
      const file = path.join(root, files.packet);
      const packet = JSON.parse(fs.readFileSync(file, "utf8"));
      packet[key] = true;
      fs.writeFileSync(file, JSON.stringify(packet));
      refused(root);
    });
});
