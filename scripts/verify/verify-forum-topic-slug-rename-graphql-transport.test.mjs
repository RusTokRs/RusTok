import assert from "node:assert/strict";
import { copyFileSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import { test } from "node:test";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../..");
const moduleRoot = "crates/modules/rustok-forum";
const graphql = `${moduleRoot}/src/graphql/topic_slug_rename_mutation.rs`;
const api = `${moduleRoot}/CRATE_API.md`;
const files = [
  graphql, api, `${moduleRoot}/README.md`, `${moduleRoot}/docs/README.md`,
  `${moduleRoot}/contracts/forum-topic-slug-rename-owner.json`,
  `${moduleRoot}/contracts/forum-topic-slug-rename-graphql-transport.json`,
  `${moduleRoot}/src/services/topic_route.rs`, `${moduleRoot}/src/services/topic_owner.rs`,
  `${moduleRoot}/src/services/topic_facade.rs`, `${moduleRoot}/src/graphql/mod.rs`,
  `${moduleRoot}/tests/topic_slug_rename_sqlite.rs`,
  `${moduleRoot}/tests/topic_slug_rename_graphql_contract.rs`,
];

function withFixture(change, assertion) {
  const root = mkdtempSync(path.join(tmpdir(), "forum-slug-transport-"));
  try {
    for (const file of files) {
      const target = path.join(root, file);
      mkdirSync(path.dirname(target), { recursive: true });
      copyFileSync(path.join(repoRoot, file), target);
    }
    const replace = (file, before, after) => {
      const target = path.join(root, file);
      const source = readFileSync(target, "utf8");
      assert.ok(source.includes(before), `fixture mutation must apply: ${before}`);
      writeFileSync(target, source.replace(before, after));
    };
    change(replace);
    const run = (name) => spawnSync(process.execPath, [path.join(repoRoot, `scripts/verify/${name}.mjs`)], {
      cwd: root, encoding: "utf8",
    });
    assertion(run);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
}

const transportGuard = "verify-forum-topic-slug-rename-graphql-transport";

test("slug guards use live contracts without removed delivery headings or handoff files", () => {
  withFixture(() => {}, (run) => {
    for (const guard of [transportGuard, "verify-forum-topic-slug-rename-owner"]) {
      const result = run(guard);
      assert.equal(result.status, 0, result.stderr || result.stdout);
    }
  });
});

test("slug transport guard rejects bypassing the configured runtime owner", () => {
  withFixture((replace) => replace(graphql,
    "super::forum_graphql_runtime(ctx).topic_service(db.clone(), event_bus.clone())",
    "TopicService::new(db.clone(), event_bus.clone())"), (run) => {
    const result = run(transportGuard);
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /forum_graphql_runtime/);
  });
});

test("slug transport guard still rejects loss of routed tenant admission", () => {
  withFixture((replace) => replace(graphql,
    "super::resolve_tenant_scope(tenant, tenant_id)?", "tenant.id"), (run) => {
    const result = run(transportGuard);
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /resolve_tenant_scope/);
  });
});

test("slug transport guard requires the current replay contract", () => {
  withFixture((replace) => replace(api, "changed = false", "unchanged result"), (run) => {
    const result = run(transportGuard);
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /changed = false/);
  });
});
