#!/usr/bin/env node

import test from "node:test";
import assert from "node:assert/strict";
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { spawnSync } from "node:child_process";

const verifier = path.resolve(
  "scripts/verify/verify-comments-thread-write-invariants.mjs",
);

function write(root, relativePath, content) {
  const target = path.join(root, relativePath);
  mkdirSync(path.dirname(target), { recursive: true });
  writeFileSync(target, content);
}

function fixture({
  missingPositionTenantLock = false,
  missingExactCount = false,
  nonUniqueIndex = false,
} = {}) {
  const root = mkdtempSync(path.join(tmpdir(), "rustok-comments-thread-invariants-"));
  const commentPath = "crates/rustok-comments/src/entities/comment.rs";
  const threadPath = "crates/rustok-comments/src/entities/comment_thread.rs";
  const servicesPath = "crates/rustok-comments/src/services.rs";
  const migrationPath =
    "crates/rustok-comments/src/migrations/m20260723_000008_repair_comment_thread_counters.rs";
  const migrationRegistryPath = "crates/rustok-comments/src/migrations/mod.rs";
  const testPath = "crates/rustok-comments/tests/thread_write_invariants.rs";
  const evidencePath =
    "crates/rustok-comments/contracts/evidence/comments-thread-write-invariants.json";

  write(
    root,
    commentPath,
    `
      impl ActiveModelBehavior for ActiveModel {
        async fn before_save() {
          if !insert { return Ok(self); }
          comment thread {thread_id} is missing while allocating a position
          update_many();
          Column::Id.eq(thread_id);
          ${missingPositionTenantLock ? "" : "Column::TenantId.eq(tenant_id);"}
          order_by_desc(Column::Position);
          checked_add(1);
          self.position = Set(next_position);
        }
      }
    `,
  );
  write(
    root,
    threadPath,
    `
      impl ActiveModelBehavior for ActiveModel {
        async fn before_save() {
          if insert { return Ok(self); }
          comment thread {thread_id} is missing while refreshing counters
          update_many();
          Column::Id.eq(thread_id);
          Column::TenantId.eq(tenant_id);
          DeletedAt.is_null();
          ${missingExactCount ? "" : ".count(db); self.comment_count = Set(count);"}
        }
      }
    `,
  );
  write(
    root,
    servicesPath,
    `
      async fn update_thread_counters_in_tx() {
        active.update(txn).await?;
      }
      fn next_item() {}
    `,
  );
  write(
    root,
    migrationPath,
    `
      DatabaseBackend::Postgres;
      DatabaseBackend::Sqlite;
      UPDATE comment_threads;
      COUNT(comment_row.id)::INTEGER;
      ROW_NUMBER() OVER;
      PARTITION BY thread_id;
      ORDER BY position ASC, created_at ASC, id ASC;
      name("idx_comments_thread_position");
      ${nonUniqueIndex ? "" : ".unique();"}
    `,
  );
  write(
    root,
    migrationRegistryPath,
    `
      mod m20260723_000008_repair_comment_thread_counters;
      Box::new(m20260723_000008_repair_comment_thread_counters::Migration)
    `,
  );
  write(
    root,
    testPath,
    `
      active_model_hooks_override_stale_positions_and_counts
      unique_position_index_rejects_active_model_bypass
      stale_thread.comment_count = Set(999)
      assert_eq!(first.position, 1)
      assert_eq!(second.position, 2)
      assert_eq!(repaired.comment_count, 1)
      comment::Entity::insert
    `,
  );
  write(
    root,
    evidencePath,
    JSON.stringify({
      schema_version: 1,
      module: "comments",
      surface: "thread_write_invariants",
      status: "executable_no_run",
      compile_policy: "not_run_by_request",
      owner: "rustok-comments",
      production_contract: {
        position_owner: commentPath,
        counter_owner: threadPath,
        repair_migration: migrationPath,
        migration_registry: migrationRegistryPath,
        executable_test: testPath,
      },
      cases: [
        { name: "serialized_position_allocation" },
        { name: "exact_active_comment_count" },
        { name: "historical_counter_repair" },
        { name: "historical_position_repair" },
        { name: "bulk_bypass_rejection" },
      ],
    }),
  );
  write(
    root,
    "crates/rustok-comments/docs/implementation-plan.md",
    "comments-thread-write-invariants.json thread_write_invariants ActiveModelBehavior UNIQUE(thread_id, position)",
  );

  return root;
}

function run(root) {
  return spawnSync(process.execPath, [verifier], {
    cwd: path.resolve("."),
    env: { ...process.env, RUSTOK_VERIFY_REPO_ROOT: root },
    encoding: "utf8",
  });
}

test("thread write verifier accepts the owner invariant contract", () => {
  const root = fixture();
  try {
    const result = run(root);
    assert.equal(result.status, 0, result.stderr || result.stdout);
    assert.match(result.stdout, /verification passed/);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("thread write verifier rejects position allocation without tenant lock", () => {
  const root = fixture({ missingPositionTenantLock: true });
  try {
    const result = run(root);
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /comment\.rs: missing Column::TenantId\.eq\(tenant_id\)/);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("thread write verifier rejects a counter owner without exact active count", () => {
  const root = fixture({ missingExactCount: true });
  try {
    const result = run(root);
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /missing \.count\(db\)|missing self\.comment_count/);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("thread write verifier rejects a non-unique position index", () => {
  const root = fixture({ nonUniqueIndex: true });
  try {
    const result = run(root);
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /missing \.unique\(\)/);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});
