import fs from "node:fs";

const failures = [];

function requirePattern(source, pattern, message) {
  if (!pattern.test(source)) {
    failures.push(message);
  }
}

// ---------------------------------------------------------------------------
// CC-08.01 — Atomic Multi-Table Mutations & Outbox Dual-Write Hazards
// ---------------------------------------------------------------------------

// 1. rustok-blog: updating post, translations, channel visibility, and outbox event in single transaction
const blogCommands = fs.readFileSync(
  "crates/modules/rustok-blog/src/services/post/commands.rs",
  "utf8",
);
requirePattern(
  blogCommands,
  /let txn = self\.db\.begin\(\)\.await/,
  "CC-08.01: rustok-blog commands must begin a database transaction",
);
requirePattern(
  blogCommands,
  /self\.event_bus\s*\.publish_in_tx\(&txn,\s*tenant_id,\s*security\.user_id,\s*event\)/,
  "CC-08.01: rustok-blog must publish outbox events inside transaction (publish_in_tx)",
);
requirePattern(
  blogCommands,
  /txn\.commit\(\)\.await/,
  "CC-08.01: rustok-blog commands must commit transaction",
);

// 2. rustok-comments: comments, thread counter adjustments, and outbox events in single transaction
const commentsServices = fs.readFileSync(
  "crates/modules/rustok-comments/src/services.rs",
  "utf8",
);
requirePattern(
  commentsServices,
  /let txn = self\.db\.begin\(\)\.await\?;[\s\S]*?publish_comment_created_in_tx[\s\S]*?txn\.commit\(\)\.await\?/,
  "CC-08.01: rustok-comments must couple comment creation, counter updates, and outbox event in single transaction",
);
requirePattern(
  commentsServices,
  /event_bus\s*\.publish_in_tx\(\s*txn,\s*event\.tenant_id/,
  "CC-08.01: rustok-comments event bus must publish inside transaction",
);

// 3. rustok-tenant: tenant insert, default locale insert, and outbox event in single transaction
const tenantServices = fs.readFileSync(
  "crates/modules/rustok-tenant/src/services/tenant_service.rs",
  "utf8",
);
requirePattern(
  tenantServices,
  /let txn = self\.db\.begin\(\)\.await\?;[\s\S]*?TenantActiveModel[\s\S]*?TenantLocaleActiveModel[\s\S]*?self\.publish_event_in_tx\(&txn,[\s\S]*?txn\.commit\(\)\.await\?/,
  "CC-08.01: rustok-tenant create_tenant must atomically persist tenant, locale, and outbox event",
);

// 4. rustok-forum: topic merge multi-entity reconciliation in single transaction
const topicMerge = fs.readFileSync(
  "crates/modules/rustok-forum/src/services/topic_merge.rs",
  "utf8",
);
requirePattern(
  topicMerge,
  /let txn = self\.db\.begin\(\)\.await\?;[\s\S]*?reconcile_topic_merge[\s\S]*?txn\.commit\(\)\.await\?/,
  "CC-08.01: rustok-forum topic merge must atomically reconcile merge state within single transaction",
);

// ---------------------------------------------------------------------------
// CC-08.02 — TOCTOU Race Condition Guards & Concurrency Fencing
// ---------------------------------------------------------------------------

// 1. rustok-blog: optimistic locking / CAS version checking
requirePattern(
  blogCommands,
  /\.filter\(blog_post::Column::Version\.eq\(version\)\)/,
  "CC-08.02: rustok-blog update_post must guard concurrency with optimistic version CAS check",
);
requirePattern(
  blogCommands,
  /if result\.rows_affected != 1\s*\{\s*return Err\(BlogError::conflict\(/,
  "CC-08.02: rustok-blog must detect CAS version collision on zero affected rows",
);

// 2. rustok-comments: exclusive row locking on comment and thread mutations
requirePattern(
  commentsServices,
  /comment::Entity::find_by_id\(comment_id\)[\s\S]*?\.filter\(comment::Column::DeletedAt\.is_null\(\)\)[\s\S]*?\.lock_exclusive\(\)/,
  "CC-08.02: rustok-comments must acquire exclusive lock (FOR UPDATE) when updating comment status",
);
requirePattern(
  commentsServices,
  /comment_thread::Entity::find_by_id\(existing\.thread_id\)[\s\S]*?\.lock_exclusive\(\)/,
  "CC-08.02: rustok-comments must acquire exclusive lock on thread when updating comment status or deleting comment",
);

// 3. rustok-outbox: Postgres candidate polling must use FOR UPDATE SKIP LOCKED
const outboxRelay = fs.readFileSync(
  "crates/modules/rustok-outbox/src/relay.rs",
  "utf8",
);
requirePattern(
  outboxRelay,
  /\.lock_with_behavior\(LockType::Update,\s*LockBehavior::SkipLocked\)/,
  "CC-08.02: rustok-outbox relay must use LockBehavior::SkipLocked on PostgreSQL",
);

// 4. flex: durable generation row locking fence
const flexGuard = fs.readFileSync(
  "crates/modules/flex/src/attached_translation_guard.rs",
  "utf8",
);
requirePattern(
  flexGuard,
  /lock_attached_translation_schema_in_tx[\s\S]*?FOR UPDATE/,
  "CC-08.02: flex attached translation guard must acquire FOR UPDATE lock on cache generation row",
);

// 5. rustok-forum: reply exclusive locking
const forumReply = fs.readFileSync(
  "crates/modules/rustok-forum/src/services/reply.rs",
  "utf8",
);
requirePattern(
  forumReply,
  /query\.lock_exclusive\(\)\.one\(txn\)\.await/,
  "CC-08.02: rustok-forum reply service must use lock_exclusive on PostgreSQL",
);

// ---------------------------------------------------------------------------
// CC-08.03 — Soft Delete & Archive Discipline
// ---------------------------------------------------------------------------

// 1. rustok-comments: deleted_at filter on active queries
requirePattern(
  commentsServices,
  /comment::Column::DeletedAt\.is_null\(\)/,
  "CC-08.03: rustok-comments must filter out soft-deleted comments (DeletedAt.is_null()) on active queries",
);
// 2. rustok-comments: atomic delete comment and counter decrement in tx
requirePattern(
  commentsServices,
  /active\.deleted_at\s*=\s*Set\(Some\(Utc::now\(\)\.into\(\)\)\);[\s\S]*?self\.update_thread_counters_in_tx\(txn,\s*&thread,\s*-1,\s*None\)/,
  "CC-08.03: rustok-comments soft delete must atomically mark deleted_at and decrement thread counter in transaction",
);

// ---------------------------------------------------------------------------
// CC-08.04 — Non-Blocking Transaction Scope
// ---------------------------------------------------------------------------

// 1. rustok-auth bootstrap: hash_password must precede db execution
const authBootstrap = fs.readFileSync(
  "crates/modules/rustok-auth/src/bootstrap.rs",
  "utf8",
);
requirePattern(
  authBootstrap,
  /let password_hash = hash_password\(&request\.password\);?[\s\S]*?let result = db\s*\.execute_raw/,
  "CC-08.04: rustok-auth bootstrap must hash password before executing database insertion",
);

// 2. rustok-auth cli: hash_password must precede db.begin()
const authCli = fs.readFileSync(
  "crates/modules/rustok-auth/cli/src/lib.rs",
  "utf8",
);
requirePattern(
  authCli,
  /let client_secret_hash = hash_password\(&client_secret\)[\s\S]*?let transaction = db\.begin\(\)\.await/,
  "CC-08.04: rustok-auth cli must hash client secret before beginning database transaction",
);

// 3. rustok-blog: schedule validation and clock sampling must precede db.begin()
requirePattern(
  blogCommands,
  /let now = chrono::Utc::now\(\);[\s\S]*?validate_schedule_for_update[\s\S]*?let txn = self\.db\.begin\(\)\.await/,
  "CC-08.04: rustok-blog update_post must compute schedule validation and clocks before opening database transaction",
);

// ---------------------------------------------------------------------------
// Verification outcome
// ---------------------------------------------------------------------------
if (failures.length > 0) {
  console.error("Database transactions and concurrency guards verification failed:");
  for (const failure of failures) {
    console.error(`- ${failure}`);
  }
  process.exit(1);
}

console.log("Database transactions, concurrency guards, and data integrity verification passed successfully!");
