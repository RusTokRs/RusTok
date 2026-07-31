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
  missingIdentityClassifier = false,
  missingClassifierUuidValidation = false,
  missingClassifierUnitHarness = false,
  missingClassifierTestRegistration = false,
  broadInsertFallback = false,
  missingStoragePropagation = false,
} = {}) {
  const root = mkdtempSync(path.join(tmpdir(), "rustok-comments-thread-invariants-"));
  const commentPath = "crates/rustok-comments/src/entities/comment.rs";
  const threadPath = "crates/rustok-comments/src/entities/comment_thread.rs";
  const entitiesModulePath = "crates/rustok-comments/src/entities/mod.rs";
  const classifierTestPath =
    "crates/rustok-comments/src/entities/thread_insert_error_tests.rs";
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

  const classifier = missingIdentityClassifier
    ? ""
    : missingClassifierUuidValidation
      ? `
          pub(crate) const THREAD_IDENTITY_CONFLICT_MARKER: &str = "comment_thread_identity_conflict";
          pub(crate) fn is_thread_identity_conflict(error: &DbErr) {
            matches!(error, DbErr::Custom(message) if message.starts_with(&expected_prefix));
          }
        `
      : `
          pub(crate) const THREAD_IDENTITY_CONFLICT_MARKER: &str = "comment_thread_identity_conflict";
          pub(crate) fn is_thread_identity_conflict(error: &DbErr) {
            let DbErr::Custom(message) = error else { return false; };
            let Some(existing_thread_id) = message.strip_prefix(&expected_prefix) else { return false; };
            Uuid::parse_str(existing_thread_id).is_ok();
          }
        `;
  write(
    root,
    threadPath,
    `
      ${classifier}
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
      {THREAD_IDENTITY_CONFLICT_MARKER}:{tenant_id}:{target_type}:{target_id}:{}
    `,
  );

  write(
    root,
    entitiesModulePath,
    missingClassifierTestRegistration
      ? "pub mod comment_thread;"
      : `
          pub mod comment_thread;
          #[cfg(test)]
          mod thread_insert_error_tests;
        `,
  );

  write(
    root,
    classifierTestPath,
    missingClassifierUnitHarness
      ? ""
      : `
          thread_identity_conflict_classifier_accepts_exact_scope_and_owner_uuid
          thread_identity_conflict_classifier_rejects_malformed_owner_uuid
          thread_identity_conflict_classifier_rejects_wrong_scope
          unrelated_custom_error_remains_a_database_error
          THREAD_IDENTITY_CONFLICT_MARKER
          is_thread_identity_conflict(
          CommentsError::Database(DbErr::Custom(message))
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
      async fn find_or_create_thread_in_tx() {
        match thread.insert(txn).await {
          Ok(thread) => Ok(thread),
          ${
            broadInsertFallback
              ? "Err(_) => comment_thread::Entity::find()"
              : `
                Err(error)
                  if comment_thread::is_thread_identity_conflict(
                    &error,
                    tenant_id,
                    target_type,
                    target_id,
                  ) => comment_thread::Entity::find(),
                ${missingStoragePropagation ? "" : "Err(error) => Err(error.into()),"}
              `
          }
        }
      }
      async fn next_position_in_tx() {}
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
      schema_version: 3,
      module: "comments",
      surface: "thread_write_invariants",
      status: "executable_no_run",
      compile_policy: "not_run_by_request",
      owner: "rustok-comments",
      production_contract: {
        position_owner: commentPath,
        counter_and_identity_owner: threadPath,
        thread_service: servicesPath,
        entities_module: entitiesModulePath,
        classifier_unit_test: classifierTestPath,
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
        { name: "identity_conflict_only_fallback" },
        { name: "identity_conflict_marker_structure" },
        { name: "postgres_concurrent_create_delete" },
        { name: "postgres_concurrent_first_thread_creation" },
      ],
    }),
  );

  write(
    root,
    "crates/rustok-comments/docs/implementation-plan.md",
    "comments-thread-write-invariants.json thread_write_invariants ActiveModelBehavior UNIQUE(thread_id, position) RUSTOK_COMMENTS_TEST_DATABASE_URL concurrent PostgreSQL identity-lock thread_creation_concurrency thread_insert_error_tests identity-conflict-only fallback valid canonical thread UUID unrelated storage errors propagate",
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

function expectFailure(options, pattern) {
  const root = fixture(options);
  try {
    const result = run(root);
    assert.notEqual(result.status, 0);
    if (pattern) assert.match(result.stderr, pattern);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
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
  expectFailure({ missingPositionTenantLock: true });
});

test("rejects counter writes without activation guard", () => {
  expectFailure({ missingCounterActivationGuard: true });
});

test("rejects a counter owner without exact active count", () => {
  expectFailure({ missingExactCount: true });
});

test("rejects a non-unique position index", () => {
  expectFailure({ nonUniqueIndex: true });
});

test("rejects missing PostgreSQL write concurrency harness", () => {
  expectFailure({ missingPostgresHarness: true });
});

test("rejects missing identity row lock", () => {
  expectFailure({ missingIdentityRowLock: true }, /missing identity_lock::Entity::update_many/);
});

test("rejects missing first-thread concurrency harness", () => {
  expectFailure(
    { missingFirstThreadHarness: true },
    /missing postgres_concurrent_first_comments_share_one_thread/,
  );
});

test("rejects a broad insert fallback", () => {
  expectFailure({ broadInsertFallback: true }, /forbidden Err\(_\) =>/);
});

test("rejects a missing identity-conflict classifier", () => {
  expectFailure(
    { missingIdentityClassifier: true },
    /missing pub\(crate\) const THREAD_IDENTITY_CONFLICT_MARKER/,
  );
});

test("rejects a prefix-only identity-conflict classifier", () => {
  expectFailure(
    { missingClassifierUuidValidation: true },
    /missing let DbErr::Custom\(message\) = error else|forbidden message\.starts_with/,
  );
});

test("rejects a missing identity classifier unit harness", () => {
  expectFailure(
    { missingClassifierUnitHarness: true },
    /missing thread_identity_conflict_classifier_accepts_exact_scope_and_owner_uuid/,
  );
});

test("rejects an unregistered identity classifier unit harness", () => {
  expectFailure(
    { missingClassifierTestRegistration: true },
    /missing #\[cfg\(test\)\]|missing mod thread_insert_error_tests;/,
  );
});

test("rejects missing unrelated storage error propagation", () => {
  expectFailure(
    { missingStoragePropagation: true },
    /missing Err\(error\) => Err\(error\.into\(\)\)/,
  );
});
