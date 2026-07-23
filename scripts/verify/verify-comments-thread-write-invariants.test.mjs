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
  missingCounterActivationGuard = false,
  missingExactCount = false,
  nonUniqueIndex = false,
  missingPostgresHarness = false,
  missingIdentityRowLock = false,
  missingFirstThreadHarness = false,
} = {}) {
  const root = mkdtempSync(path.join(tmpdir(), "rustok-comments-thread-invariants-"));
  const commentPath = "crates/rustok-comments/src/entities/comment.rs";
  const threadPath = "crates/rustok-comments/src/entities/comment_thread.rs";
  const identityEntityPath =
    "crates/rustok-comments/src/entities/comment_thread_identity_lock.rs";
  const servicesPath = "crates/rustok-comments/src/services.rs";
  const counterMigrationPath =
    "crates/rustok-comments/src/migrations/m20260723_000008_repair_comment_thread_counters.rs";
  const identityMigrationPath =
    "crates/rustok-comments/src/migrations/m20260723_000009_add_comment_thread_identity_locks.rs";
  const migrationRegistryPath = "crates/rustok-comments/src/migrations/mod.rs";
  const writeTestPath = "crates/rustok-comments/tests/thread_write_invariants.rs";
  const firstThreadTestPath =
    "crates/rustok-comments/tests/thread_creation_concurrency.rs";
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
          serialize_thread_identity(db, &self).await?;
          ${
            missingCounterActivationGuard
              ? ""
              : "matches!(&self.comment_count, ActiveValue::Set(_));"
          }
          comment thread {thread_id} is missing while refreshing counters
          update_many();
          Column::TenantId.eq(tenant_id);
          DeletedAt.is_null();
          ${missingExactCount ? "" : ".count(db); self.comment_count = Set(count);"}
        }
      }
      OnConflict::columns;
      ${missingIdentityRowLock ? "" : "identity_lock::Entity::update_many();"}
      comment thread identity {target_type}:{target_id} already belongs to
    `,
  );
  write(
    root,
    identityEntityPath,
    `
      #[sea_orm(table_name = "comment_thread_identity_locks")]
      pub tenant_id: Uuid
      pub target_type: String
      pub target_id: Uuid
      impl ActiveModelBehavior for ActiveModel
    `,
  );
  write(
    root,
    servicesPath,
    `
      find_or_create_thread_in_tx
      match thread.insert(txn).await
      Err(_) => comment_thread::Entity::find()
      async fn update_thread_counters_in_tx() {
        active.update(txn).await?;
      }
      fn next_item() {}
    `,
  );
  write(
    root,
    counterMigrationPath,
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
    identityMigrationPath,
    `
      CommentThreadIdentityLocks::Table
      CommentThreadIdentityLocks::TenantId
      CommentThreadIdentityLocks::TargetType
      CommentThreadIdentityLocks::TargetId
      name("idx_comment_thread_identity_locks_identity")
      .unique()
    `,
  );
  write(
    root,
    migrationRegistryPath,
    `
      mod m20260723_000008_repair_comment_thread_counters;
      Box::new(m20260723_000008_repair_comment_thread_counters::Migration)
      mod m20260723_000009_add_comment_thread_identity_locks;
      Box::new(m20260723_000009_add_comment_thread_identity_locks::Migration)
    `,
  );
  write(
    root,
    writeTestPath,
    `
      active_model_hooks_override_stale_positions_and_counts
      status_only_thread_update_preserves_comment_count
      unique_position_index_rejects_active_model_bypass
      ${
        missingPostgresHarness
          ? ""
          : `
            postgres_concurrent_creates_and_delete_preserve_thread_invariants
            RUSTOK_COMMENTS_TEST_DATABASE_URL
            tokio::join!
            max_connections(1)
            SET search_path TO "{schema_name}", public
            assert_eq!(positions, vec![1, 2, 3])
            assert_eq!(thread.comment_count, active_count as i32)
          `
      }
    `,
  );
  write(
    root,
    firstThreadTestPath,
    missingFirstThreadHarness
      ? ""
      : `
        postgres_concurrent_first_comments_share_one_thread
        CommentsService::new(test_db.db_a.clone())
        CommentsService::new(test_db.db_b.clone())
        tokio::join!
        assert_eq!(first.thread_id, second.thread_id)
        assert_eq!(positions, HashSet::from([1, 2]))
        assert_eq!(threads.len(), 1)
        assert_eq!(threads[0].comment_count, 2)
        RUSTOK_COMMENTS_TEST_DATABASE_URL
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
        counter_and_identity_owner: threadPath,
        identity_lock_entity: identityEntityPath,
        counter_repair_migration: counterMigrationPath,
        identity_lock_migration: identityMigrationPath,
        migration_registry: migrationRegistryPath,
        write_invariant_test: writeTestPath,
        first_thread_test: firstThreadTestPath,
        postgres_environment: "RUSTOK_COMMENTS_TEST_DATABASE_URL",
      },
      cases: [
        { name: "serialized_position_allocation" },
        { name: "exact_active_comment_count" },
        { name: "status_only_update_preserves_count" },
        { name: "historical_counter_repair" },
        { name: "historical_position_repair" },
        { name: "bulk_bypass_rejection" },
        { name: "postgres_concurrent_create_delete" },
        { name: "postgres_concurrent_first_thread_creation" },
      ],
    }),
  );
  write(
    root,
    "crates/rustok-comments/docs/implementation-plan.md",
    "comments-thread-write-invariants.json thread_write_invariants ActiveModelBehavior UNIQUE(thread_id, position) RUSTOK_COMMENTS_TEST_DATABASE_URL concurrent PostgreSQL identity-lock thread_creation_concurrency",
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
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("rejects position allocation without tenant lock", () => {
  const root = fixture({ missingPositionTenantLock: true });
  try {
    const result = run(root);
    assert.notEqual(result.status, 0);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("rejects counter writes without activation guard", () => {
  const root = fixture({ missingCounterActivationGuard: true });
  try {
    const result = run(root);
    assert.notEqual(result.status, 0);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("rejects a counter owner without exact active count", () => {
  const root = fixture({ missingExactCount: true });
  try {
    const result = run(root);
    assert.notEqual(result.status, 0);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("rejects a non-unique position index", () => {
  const root = fixture({ nonUniqueIndex: true });
  try {
    const result = run(root);
    assert.notEqual(result.status, 0);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("rejects missing PostgreSQL write concurrency harness", () => {
  const root = fixture({ missingPostgresHarness: true });
  try {
    const result = run(root);
    assert.notEqual(result.status, 0);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("rejects missing identity row lock", () => {
  const root = fixture({ missingIdentityRowLock: true });
  try {
    const result = run(root);
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /missing identity_lock::Entity::update_many/);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("rejects missing first-thread concurrency harness", () => {
  const root = fixture({ missingFirstThreadHarness: true });
  try {
    const result = run(root);
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /missing postgres_concurrent_first_comments_share_one_thread/);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});
