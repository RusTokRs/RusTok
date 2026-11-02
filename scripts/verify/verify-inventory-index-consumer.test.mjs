// Focused verifier regression definitions only. Maintainer-run; never imported by the verifier.
import test from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";
import {
  files,
  verifyInventoryIndexConsumer,
} from "./verify-inventory-index-consumer.mjs";

const repository = path.resolve(
  path.dirname(fileURLToPath(import.meta.url)),
  "../..",
);
function fixture(t) {
  const root = fs.mkdtempSync(
    path.join(os.tmpdir(), "inventory-index-source-"),
  );
  t.after(() => fs.rmSync(root, { recursive: true, force: true }));
  const packet = JSON.parse(
    fs.readFileSync(path.join(repository, files.packet), "utf8"),
  );
  for (const name of new Set([
    ...Object.values(files),
    ...Object.keys(packet.preserved_hashes),
  ])) {
    fs.mkdirSync(path.dirname(path.join(root, name)), { recursive: true });
    fs.copyFileSync(path.join(repository, name), path.join(root, name));
  }
  return root;
}
function mutate(root, file, from, to) {
  const target = path.join(root, file);
  const old = fs.readFileSync(target, "utf8");
  const changed = old.replace(from, to);
  assert.notEqual(changed, old, `mutation fixture must locate ${from}`);
  fs.writeFileSync(target, changed);
}
function refuses(root, label) {
  assert.ok(
    verifyInventoryIndexConsumer({ root }).some((failure) =>
      failure.includes(label),
    ),
    label,
  );
}

test("actual selected source obligations are present without promoting runtime readiness", (t) => {
  assert.deepEqual(verifyInventoryIndexConsumer({ root: fixture(t) }), []);
});

test("selection cannot be inferred from a feature or guessed module instead of the real marker", (t) => {
  const root = fixture(t);
  mutate(
    root,
    files.selection,
    "extensions.contains::<rustok_inventory::InventoryRuntimeSelected>()",
    "extensions.contains::<GuessedInventoryPresence>()",
  );
  refuses(root, "actual selected module gate");
});

test("the host must reuse canonical current access-token authentication", (t) => {
  const root = fixture(t);
  mutate(
    root,
    files.provider,
    /resolve_current_user_from_access_token\s*\(\s*&self\.auth/,
    "reconstruct_saved_actor_context(&self.auth",
  );
  refuses(root, "canonical current service authentication");
});

test("a source cannot use an invented system actor as Inventory read authority", (t) => {
  const root = fixture(t);
  const target = path.join(root, files.source);
  fs.writeFileSync(
    target,
    fs.readFileSync(target, "utf8") +
      '\nfn invented_actor() { PortActor::system("index"); }\n',
  );
  refuses(root, "no fabricated authority/source SQL");
});

test("changed cannot return success before durable generic persistence", (t) => {
  const root = fixture(t);
  mutate(
    root,
    files.consumer,
    "persist_index_source_refresh(",
    "return Ok(()); persist_index_source_refresh(",
  );
  refuses(root, "changed persistence before handler ACK");
});

test("activation cannot enqueue a replacement for the original baseline job UUID", (t) => {
  const root = fixture(t);
  mutate(
    root,
    files.consumer,
    /baseline_id,\s+schema\.reference/,
    "uuid::Uuid::new_v4(), schema.reference",
  );
  refuses(root, "original initial replay job identity");
});

test("owner tombstone and upsert mutation identities cannot be freshly generated", (t) => {
  const root = fixture(t);
  mutate(
    root,
    files.source,
    "event_id: row.change_id",
    "event_id: Uuid::new_v4()",
  );
  refuses(root, "stable source mutation UUID");
});

test("background execution must retain the exact original job fence", (t) => {
  const root = fixture(t);
  mutate(
    root,
    files.scheduler,
    ".for_job(item.id)",
    ".for_job(Uuid::new_v4())",
  );
  refuses(root, "worker exact original job");
});

test("discovery cannot let an unselected source starve all selected sources", (t) => {
  const root = fixture(t);
  mutate(
    root,
    files.scheduler,
    "self.schemas.registry().iter()",
    "unselected_schemas.iter()",
  );
  refuses(root, "selected-only discovery");
});

test("source-only metadata cannot turn unrun compilation into admitted evidence", (t) => {
  const root = fixture(t);
  const target = path.join(root, files.packet);
  const packet = JSON.parse(fs.readFileSync(target, "utf8"));
  packet.compiled = true;
  fs.writeFileSync(target, JSON.stringify(packet));
  refuses(root, "source-only evidence cannot promote compiled");
});

test("worker actor mapping must agree with the canonical service principal contract", (t) => {
  const root = fixture(t);
  mutate(
    root,
    files.provider,
    "auth.port_actor()",
    "PortActor::service(current.user.id.to_string())",
  );
  refuses(root, "worker actor follows canonical service principal mapping");
});

test("credential resolution cannot regress to the shared cached path", (t) => {
  const root = fixture(t);
  mutate(
    root,
    files.provider,
    "resolve_fresh_for_tenant(request.tenant_id(), reference)",
    "resolve_for_tenant(request.tenant_id(), reference)",
  );
  refuses(root, "canonical current service authentication");
});

test("an orphan completed checkpoint must not certify a newly enqueued replay job", (t) => {
  const root = fixture(t);
  mutate(
    root,
    files.enqueue,
    "return Err(IndexReplayJobError::CheckpointWithoutJob);",
    "record_inconsistent_history_without_refusing();",
  );
  refuses(root, "orphan checkpoint refuses a fresh initial replay job");
});

test("regression SQL cannot silently target a nonexistent derived table", (t) => {
  const root = fixture(t);
  mutate(root, files.regression, "FROM index_entities", "FROM index_records");
  refuses(root, "regression SQL must use the canonical derived table");
});

test("inbox duplicate schema decoding cannot rely on PostgreSQL INT4 to i64 widening", (t) => {
  const root = fixture(t);
  mutate(
    root,
    files.mutationStore,
    "CAST(schema_version AS BIGINT) AS schema_version_value",
    "schema_version",
  );
  refuses(root, "portable stored schema integer decode");
});
