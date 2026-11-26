#!/usr/bin/env node
/**
 * Static pins for the channel lifecycle invariants introduced by migration
 * `m20261126_000013` and the writes that rely on them.
 *
 * The module has no Rust toolchain in this sandbox, so nothing here executes the
 * migration or the service: the script is a *text* contract that fails when the
 * structural guarantee (partial unique indexes, composite FK, retired `status`
 * column) or the write ordering that keeps those indexes satisfied is edited
 * away. Runtime proof still needs `cargo test` against PostgreSQL and SQLite.
 */

import { readFileSync, readdirSync, statSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = process.env.RUSTOK_VERIFY_REPO_ROOT
  ? path.resolve(process.env.RUSTOK_VERIFY_REPO_ROOT)
  : path.resolve(scriptDir, "../..");

const MIGRATION =
  "crates/modules/rustok-channel/src/migrations/m20261126_000013_channel_lifecycle_invariants.rs";
const SERVICE = "crates/modules/rustok-channel/src/services/channel_service.rs";
const ERROR = "crates/modules/rustok-channel/src/error.rs";
const PORTS = "crates/modules/rustok-channel/src/ports.rs";
const ENTITY_CHANNEL = "crates/modules/rustok-channel/src/entities/channel.rs";
const ENTITY_TARGET = "crates/modules/rustok-channel/src/entities/channel_target.rs";
const CONTROLLER = "apps/server/src/controllers/channel.rs";
const COMMERCE_STORE = "crates/modules/rustok-commerce/src/controllers/store/mod.rs";

const problems = [];
const read = (relativePath) => {
  try {
    return readFileSync(path.join(repoRoot, relativePath), "utf8");
  } catch (error) {
    problems.push(`cannot read ${relativePath}: ${error.message}`);
    return "";
  }
};

const requireText = (source, needle, label) => {
  if (!source.includes(needle)) problems.push(`${label}: missing \`${needle}\``);
};
const requirePattern = (source, pattern, label) => {
  if (!pattern.test(source)) problems.push(`${label}: missing /${pattern.source}/`);
};
const requireOrder = (source, earlier, later, label) => {
  const earlierIndex = source.indexOf(earlier);
  const laterIndex = source.indexOf(later);
  if (earlierIndex < 0) problems.push(`${label}: missing \`${earlier}\``);
  else if (laterIndex < 0) problems.push(`${label}: missing \`${later}\``);
  else if (earlierIndex > laterIndex)
    problems.push(`${label}: \`${later}\` must follow \`${earlier}\``);
};
const functionBody = (source, signature, terminator = "\n    }") => {
  const start = source.indexOf(signature);
  if (start < 0) {
    problems.push(`missing function \`${signature}\``);
    return "";
  }
  const end = source.indexOf(terminator, start);
  return end < 0 ? source.slice(start) : source.slice(start, end);
};

// ---------------------------------------------------------------------------
// Migration: the invariants themselves, on both supported backends.
// ---------------------------------------------------------------------------
const migration = read(MIGRATION);
const sqliteArmIndex = migration.indexOf("DatabaseBackend::Sqlite =>");
const postgresUp = sqliteArmIndex < 0 ? "" : migration.slice(0, sqliteArmIndex);
const sqliteUp = sqliteArmIndex < 0 ? "" : migration.slice(sqliteArmIndex);
if (sqliteArmIndex < 0) problems.push("migration must handle SQLite explicitly");

const partialIndexes = [
  ["uq_channels_tenant_default", "WHERE is_default", "WHERE is_default = 1"],
  [
    "uq_channel_resolution_policy_sets_tenant_active",
    "WHERE is_active",
    "WHERE is_active = 1",
  ],
  [
    "uq_channel_targets_tenant_host",
    "WHERE target_type = 'web_domain'",
    "WHERE target_type = 'web_domain'",
  ],
  ["uq_channel_targets_channel_primary", "WHERE is_primary", "WHERE is_primary = 1"],
];

for (const [name, postgresPredicate, sqlitePredicate] of partialIndexes) {
  const declaration = `CREATE UNIQUE INDEX IF NOT EXISTS ${name}`;
  const statement = new RegExp(
    `CREATE UNIQUE INDEX IF NOT EXISTS ${name}\\s+ON [^;]+;`,
    "s",
  );
  for (const [half, predicate, backend] of [
    [postgresUp, postgresPredicate, "postgres"],
    [sqliteUp, sqlitePredicate, "sqlite"],
  ]) {
    const match = half.match(statement);
    if (!match) problems.push(`${backend} up() must create ${declaration}`);
    else if (!match[0].includes(predicate))
      problems.push(`${backend} up() must keep ${name} partial (${predicate})`);
  }
}

// Legacy rows are repaired before the indexes exist, otherwise creating them on
// a database written by the old code fails.
const repairs = [
  ["ranked_defaults", "uq_channels_tenant_default"],
  ["ranked_claims", "uq_channel_targets_tenant_host"],
  ["ranked_primary", "uq_channel_targets_channel_primary"],
  ["ranked_active_sets", "uq_channel_resolution_policy_sets_tenant_active"],
];
for (const [half, backend] of [
  [postgresUp, "postgres"],
  [sqliteUp, "sqlite"],
]) {
  for (const [repair, index] of repairs) {
    requireOrder(half, repair, `CREATE UNIQUE INDEX IF NOT EXISTS ${index}`, `${backend} up() repair order`);
  }
}

// Tenant-scoped target claims: denormalized tenant, NOT NULL, composite FK.
requireText(postgresUp, "ALTER TABLE channel_targets ADD COLUMN IF NOT EXISTS tenant_id UUID;", "postgres up() target tenant column");
requireText(postgresUp, "ALTER COLUMN tenant_id SET NOT NULL;", "postgres up() target tenant NOT NULL");
requireText(postgresUp, "CREATE UNIQUE INDEX IF NOT EXISTS uq_channels_tenant_id_id", "postgres up() composite FK target index");
requirePattern(
  postgresUp,
  /FOREIGN KEY \(tenant_id, channel_id\)\s+REFERENCES channels \(tenant_id, id\)\s+ON DELETE CASCADE;/,
  "postgres up() composite tenant/channel FK",
);
requireText(sqliteUp, "ALTER TABLE channel_targets ADD COLUMN tenant_id TEXT;", "sqlite up() target tenant column");
requireText(sqliteUp, "duplicate column", "sqlite up() tolerates an existing tenant_id column");
requirePattern(
  sqliteUp,
  /SET tenant_id = \(\s*SELECT channel\.tenant_id FROM channels AS channel/,
  "sqlite up() backfills target tenant",
);

// The retired column.
requireText(postgresUp, "ALTER TABLE channels DROP COLUMN IF EXISTS status;", "postgres up() retires channels.status");
requireText(sqliteUp, "ALTER TABLE channels DROP COLUMN status;", "sqlite up() retires channels.status");
requireText(sqliteUp, "no such column", "sqlite up() tolerates engines without DROP COLUMN");

const down = migration.slice(migration.indexOf("async fn down"));
if (!down) problems.push("migration must implement down()");
for (const index of [
  "uq_channels_tenant_default",
  ...partialIndexes.map(([name]) => name).filter((name) => name !== "uq_channels_tenant_default"),
  "uq_channels_tenant_id_id",
]) {
  requireText(down, `DROP INDEX IF EXISTS ${index};`, "down() drops index");
}
requireText(down, "ALTER TABLE channel_targets DROP COLUMN tenant_id;", "down() removes target tenant");
requireText(down, "no such column", "down() tolerates engines without DROP COLUMN");
requireText(down, "ColumnDef::new(Channels::Status)", "down() restores channels.status");

// ---------------------------------------------------------------------------
// Owner writes: the ordering and conflict mapping the indexes require.
// ---------------------------------------------------------------------------
const service = read(SERVICE);

requireText(service, "fn map_unique_violation(", "owner maps lost races");
requireText(service, "ChannelError::Conflict(format!(", "owner returns a typed conflict");

const promotionSites = service.split('map_unique_violation(error, "promote the tenant default channel")').length - 1;
if (promotionSites < 3) {
  problems.push(
    `default promotion must map conflicts in set_channel_active, delete_channel and replace_default_channel (found ${promotionSites})`,
  );
}
const policySites = service.split('map_unique_violation(error, "activate the tenant resolution policy set")').length - 1;
if (policySites < 1) problems.push("policy-set activation must map conflicts");

const defaultPromotion = functionBody(service, "async fn replace_default_channel(");
requireOrder(
  defaultPromotion,
  "Expr::value(false)",
  "Expr::value(true)",
  "replace_default_channel clears the previous default before setting the new one",
);

const policyPromotion = functionBody(service, "async fn replace_active_policy_set_in_tx(");
requireOrder(
  policyPromotion,
  "Expr::value(false)",
  "Expr::value(true)",
  "replace_active_policy_set_in_tx deactivates the previous set before activating the new one",
);

requireText(service, "clear_primary_target(&txn, channel_id, None, now)", "add_target demotes the previous primary");
requireText(service, "clear_primary_target(&txn, channel_id, Some(target_id), now)", "update_target demotes the previous primary");

const removeRule = functionBody(service, "pub async fn remove_resolution_rule(");
requireText(removeRule, "delete_many()", "remove_resolution_rule deletes by filter, not by model");
requireText(removeRule, "Column::PolicySetId.eq(policy_set_id)", "remove_resolution_rule is policy-set scoped");
requireText(removeRule, "rows_affected == 0", "remove_resolution_rule reports a lost race as NotFound");
if (/\.delete\(&self\.db\)/.test(service)) {
  problems.push("the owner must not use ActiveModel::delete() (it surfaces a lost race as DbErr::RecordNotDeleted -> 500)");
}

const settingsSites = service.split("normalize_channel_settings(").length - 1;
if (settingsSites < 3) {
  problems.push(
    `create_channel and update_channel must validate the settings shape through normalize_channel_settings (found ${settingsSites} references)`,
  );
}

// Target claims: the collision is classified from state, not from index names,
// so a lost race in the primary slot is a retryable conflict, not a false
// "target already exists".
const addTarget = functionBody(service, "pub async fn add_target(");
const updateTarget = functionBody(service, "pub async fn update_target(");
for (const [name, body] of [
  ["add_target", addTarget],
  ["update_target", updateTarget],
]) {
  if (!body.includes("classify_target_conflict("))
    problems.push(`${name} must classify a target unique violation`);
  if (body.includes("ChannelError::TargetAlreadyExists("))
    problems.push(`${name} must not answer every unique violation as TargetAlreadyExists`);
}
requireText(service, "async fn classify_target_conflict(", "owner classifies target conflicts");
requireText(
  service,
  "Ok(Some(_)) if target_type == ChannelTargetType::WebDomain",
  "only a web host collision may be reported as a duplicate target",
);

// Bind upserts and their read-back share one transaction, so a concurrent unbind
// cannot turn a successful bind into a `Database(RecordNotInserted)`.
for (const [name, signature, readBack] of [
  ["bind_module", "pub async fn bind_module(", "Column::ModuleSlug.eq(&module_slug))\n            .one(&txn)"],
  ["bind_oauth_app", "pub async fn bind_oauth_app(", "Column::OauthAppId.eq(input.oauth_app_id))\n            .one(&txn)"],
]) {
  const body = functionBody(service, signature);
  if (!body.includes("let txn = self.db.begin().await?;"))
    problems.push(`${name} must run the upsert inside a transaction`);
  if (!body.includes("exec_without_returning(&txn)"))
    problems.push(`${name} must upsert through the transaction`);
  if (!body.includes(readBack))
    problems.push(`${name} must read the row back from the same transaction`);
  if (!body.includes("txn.commit().await?;"))
    problems.push(`${name} must commit the transaction`);
}

// ---------------------------------------------------------------------------
// Retired column: nothing outside the migrations reads or writes it.
// ---------------------------------------------------------------------------
const entityChannel = read(ENTITY_CHANNEL);
const entityTarget = read(ENTITY_TARGET);
if (/pub\s+status\s*:/.test(entityChannel))
  problems.push("the channels entity must not declare a status column: the publication state is derived from is_active");
requireText(entityTarget, "pub tenant_id: Uuid,", "channel_targets entity exposes the denormalized tenant_id");

const skippedDirectories = new Set(["target", "node_modules", ".git", "dist", "build"]);
const scan = (root) => {
  const found = [];
  const walk = (directory) => {
    let entries;
    try {
      entries = readdirSync(directory, { withFileTypes: true });
    } catch {
      return;
    }
    for (const entry of entries) {
      const absolute = path.join(directory, entry.name);
      if (entry.isDirectory()) {
        if (!skippedDirectories.has(entry.name)) walk(absolute);
        continue;
      }
      if (!entry.name.endsWith(".rs")) continue;
      const relative = path.relative(repoRoot, absolute).split(path.sep).join("/");
      if (relative.includes("/src/migrations/") || relative.includes("/migrations/")) continue;
      found.push({ relative, source: readFileSync(absolute, "utf8") });
    }
  };
  for (const directory of ["crates", "apps"]) walk(path.join(repoRoot, directory));
  return found;
};

for (const file of scan(repoRoot)) {
  if (/\bchannels\.status\b/i.test(file.source)) {
    problems.push(`${file.relative}: reads or writes the retired channels.status column`);
  }
  if (/channel::Column::Status/.test(file.source)) {
    problems.push(`${file.relative}: references a status column on the channel entity`);
  }
}

// The commerce storefront read path must not turn the owner's typed conflict
// into a `500` (and must not claim `409` on a `GET`).
const commerceStore = read(COMMERCE_STORE);
const storefrontChannelMapper = functionBody(
  commerceStore,
  "fn map_storefront_channel_error(",
);
requireText(
  storefrontChannelMapper,
  "ChannelError::Conflict(_) =>",
  "the commerce storefront must map the typed channel conflict explicitly",
);
requireText(
  storefrontChannelMapper,
  "commerce_store_channel_conflict",
  "the commerce storefront must answer a retryable conflict code",
);
requirePattern(
  storefrontChannelMapper,
  /ChannelError::Conflict\(_\) => \(\s*StatusCode::SERVICE_UNAVAILABLE/,
  "a channel conflict on a storefront read is a retry, not a 500 or a 409",
);
if (/\|\s*ChannelError::Conflict\(_\)\s*\n\s*\|\s*ChannelError::Serialization/.test(commerceStore)) {
  problems.push("the channel conflict must not fall through to the 500 catch-all arm");
}

// ---------------------------------------------------------------------------
// Runtime proof: the SQLite suite that must actually execute these invariants.
// ---------------------------------------------------------------------------
const lifecycleTest = read(
  "crates/modules/rustok-channel/tests/lifecycle_invariants_sqlite.rs",
);
for (const caseName of [
  "only_one_default_channel_per_tenant",
  "only_one_active_policy_set_per_tenant",
  "deactivating_the_default_promotes_the_oldest_active_channel",
  "deactivating_the_last_active_channel_clears_the_default",
  "deleting_the_default_promotes_the_oldest_active_channel",
  "a_web_host_can_only_be_claimed_once_per_tenant",
  "the_same_web_host_is_allowed_in_another_tenant",
  "adding_a_second_primary_target_demotes_the_previous_one",
  "channel_settings_must_be_a_json_object",
  "removing_a_resolution_rule_twice_answers_not_found",
]) {
  requireText(lifecycleTest, `async fn ${caseName}(`, "sqlite lifecycle suite");
}

// The PostgreSQL-only half of 000013 (`SET NOT NULL`, the composite FK, the
// symmetric `down()`) cannot run on SQLite, so it has its own ignored suite.
const postgresTest = read(
  "crates/modules/rustok-channel/tests/lifecycle_invariants_postgres.rs",
);
requireText(
  postgresTest,
  '#[ignore = "requires a disposable PostgreSQL database',
  "postgres suite must be opt-in for a disposable server",
);
requireText(
  postgresTest,
  "for migration in rustok_channel::migrations::migrations()",
  "postgres suite must apply the real migrations",
);
requireText(postgresTest, "verify_up_invariants(", "postgres suite must assert the up() invariants");
requireText(
  postgresTest,
  "channel_targets.tenant_id must be NOT NULL",
  "postgres suite must prove the denormalized tenant column is NOT NULL",
);
requireText(
  postgresTest,
  '"fk_channel_targets_tenant_channel"',
  "postgres suite must prove the composite tenant/channel FK",
);
requireText(
  postgresTest,
  "verify_down_restores_the_retired_column(",
  "postgres suite must prove down() is symmetric",
);

// ---------------------------------------------------------------------------
// Transport mapping of the typed conflict.
// ---------------------------------------------------------------------------
requireText(read(ERROR), "Conflict(String)", "ChannelError carries a typed conflict");
const ports = read(PORTS);
requireText(ports, "crate::ChannelError::Conflict(message) =>", "ports map the conflict");
requireText(ports, "PortErrorKind::Conflict", "ports use PortErrorKind::Conflict");
const controller = read(CONTROLLER);
requireText(controller, "ChannelError::Conflict(_) =>", "REST maps the conflict");
requireText(controller, "StatusCode::CONFLICT", "REST answers 409");
const oauthGuard = functionBody(controller, "async fn ensure_oauth_app_belongs_to_tenant(", "\n}");
if (!oauthGuard.includes("Error::NotFound"))
  problems.push("a foreign OAuth app must answer 404, like a foreign channel or policy set");
if (/Error::BadRequest\(\s*"OAuth app/.test(controller))
  problems.push("a foreign OAuth app must not answer 400: the status code would disclose that the app exists in another tenant");

if (problems.length > 0) {
  for (const problem of problems) {
    console.error(`[verify-channel-lifecycle-invariants] ${problem}`);
  }
  process.exit(1);
}

console.log("[verify-channel-lifecycle-invariants] OK");
