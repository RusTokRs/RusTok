#!/usr/bin/env node

import { test } from "node:test";
import assert from "node:assert/strict";
import { mkdtempSync, mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";

const scriptPath = path.resolve(
  path.dirname(fileURLToPath(import.meta.url)),
  "verify-channel-lifecycle-invariants.mjs",
);
const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../..");

const FIXTURE_FILES = [
  "crates/modules/rustok-channel/src/migrations/m20261126_000013_channel_lifecycle_invariants.rs",
  "crates/modules/rustok-channel/src/services/channel_service.rs",
  "crates/modules/rustok-channel/src/error.rs",
  "crates/modules/rustok-channel/src/ports.rs",
  "crates/modules/rustok-channel/src/entities/channel.rs",
  "crates/modules/rustok-channel/src/entities/channel_target.rs",
  "apps/server/src/controllers/channel.rs",
  "crates/modules/rustok-channel/tests/lifecycle_invariants_sqlite.rs",
  "crates/modules/rustok-channel/tests/lifecycle_invariants_postgres.rs",
  "crates/modules/rustok-commerce/src/controllers/store/mod.rs",
];

function buildFixture(mutate = () => {}) {
  const root = mkdtempSync(path.join(tmpdir(), "channel-lifecycle-invariants-"));
  const originals = new Map();
  for (const relativePath of FIXTURE_FILES) {
    const source = readFileSync(path.join(repoRoot, relativePath), "utf8");
    originals.set(relativePath, source);
    const target = path.join(root, relativePath);
    mkdirSync(path.dirname(target), { recursive: true });
    writeFileSync(target, source);
  }
  mutate({
    root,
    write(relativePath, content) {
      const target = path.join(root, relativePath);
      mkdirSync(path.dirname(target), { recursive: true });
      writeFileSync(target, content);
    },
    original(relativePath) {
      return originals.get(relativePath);
    },
  });
  return root;
}

function run(root) {
  return spawnSync(process.execPath, [scriptPath], {
    encoding: "utf8",
    env: { ...process.env, RUSTOK_VERIFY_REPO_ROOT: root },
  });
}

function expectFailure(mutate, expectedFragment) {
  const root = buildFixture(mutate);
  try {
    const result = run(root);
    assert.equal(result.status, 1, `expected failure, got ${result.status}`);
    assert.match(result.stderr, new RegExp(expectedFragment.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")));
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
}

const MIGRATION = FIXTURE_FILES[0];
const SERVICE = FIXTURE_FILES[1];
const ENTITY_CHANNEL = FIXTURE_FILES[4];
const CONTROLLER = FIXTURE_FILES[6];

test("passes on the current repository state", () => {
  const root = buildFixture();
  try {
    const result = run(root);
    assert.equal(result.status, 0, result.stderr);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("fails when a partial unique index loses its predicate", () => {
  expectFailure(({ write, original }) => {
    write(
      MIGRATION,
      original(MIGRATION)
        .replace("    WHERE is_primary;", "    ;")
        .replace("    WHERE is_primary = 1;", "    ;"),
    );
  }, "uq_channel_targets_channel_primary partial");
});

test("fails when the composite tenant/channel foreign key is dropped", () => {
  expectFailure(({ write, original }) => {
    write(
      MIGRATION,
      original(MIGRATION).replace(
        /FOREIGN KEY \(tenant_id, channel_id\)[\s\S]*?ON DELETE CASCADE;/,
        "FOREIGN KEY (channel_id) REFERENCES channels (id) ON DELETE CASCADE;",
      ),
    );
  }, "composite tenant/channel FK");
});

test("fails when the legacy repair runs after the index is created", () => {
  expectFailure(({ write, original }) => {
    const source = original(MIGRATION);
    const repair = source.indexOf("WITH ranked_primary AS (");
    const index = source.indexOf("CREATE UNIQUE INDEX IF NOT EXISTS uq_channel_targets_channel_primary");
    const reordered =
      source.slice(0, repair) +
      source.slice(index) +
      source.slice(repair, index) +
      source.slice(index);
    write(MIGRATION, reordered);
  }, "repair order");
});

test("fails when channels.status is not dropped", () => {
  expectFailure(({ write, original }) => {
    write(
      MIGRATION,
      original(MIGRATION).replace("ALTER TABLE channels DROP COLUMN IF EXISTS status;", ""),
    );
  }, "postgres up() retires channels.status");
});

test("fails when down() stops restoring the retired column", () => {
  expectFailure(({ write, original }) => {
    write(
      MIGRATION,
      original(MIGRATION).replace("ColumnDef::new(Channels::Status)", "ColumnDef::new(Channels::Name)"),
    );
  }, "down() restores channels.status");
});

test("fails when default promotion stops mapping the unique violation", () => {
  expectFailure(({ write, original }) => {
    write(
      SERVICE,
      original(SERVICE).replace(
        'map_unique_violation(error, "promote the tenant default channel")',
        "ChannelError::Database(error)",
      ),
    );
  }, "default promotion must map conflicts");
});

test("fails when the default pointer is set before it is cleared", () => {
  expectFailure(({ write, original }) => {
    const source = original(SERVICE);
    const body = source.slice(source.indexOf("async fn replace_default_channel("));
    const rewritten = body
      .replace("Expr::value(false)", "__FIRST__")
      .replace("Expr::value(true)", "Expr::value(false)")
      .replace("__FIRST__", "Expr::value(true)");
    write(SERVICE, source.slice(0, source.indexOf("async fn replace_default_channel(")) + rewritten);
  }, "replace_default_channel clears the previous default");
});

test("fails when remove_resolution_rule goes back to ActiveModel::delete", () => {
  expectFailure(({ write, original }) => {
    write(
      SERVICE,
      original(SERVICE).replace(
        /let deleted = channel_resolution_policy_rule::Entity::delete_many\(\)[\s\S]*?await\?;/,
        "channel_resolution_policy_rule::ActiveModel::from(rule).delete(&self.db).await?;",
      ),
    );
  }, "ActiveModel::delete()");
});

test("fails when update_channel stops validating the settings shape", () => {
  expectFailure(({ write, original }) => {
    write(
      SERVICE,
      original(SERVICE).replace(
        "active.settings = Set(normalize_channel_settings(settings)?);",
        "active.settings = Set(settings);",
      ),
    );
  }, "validate the settings shape");
});

test("fails when the channels entity reintroduces a status column", () => {
  expectFailure(({ write, original }) => {
    write(
      ENTITY_CHANNEL,
      original(ENTITY_CHANNEL).replace("pub is_active: bool,", "pub is_active: bool,\n    pub status: String,"),
    );
  }, "must not declare a status column");
});

test("fails when any non-migration file reads channels.status", () => {
  expectFailure(({ write }) => {
    write(
      "crates/modules/rustok-channel/src/services/legacy_reader.rs",
      "pub fn legacy() -> &'static str { \"SELECT channels.status FROM channels\" }\n",
    );
  }, "retired channels.status column");
});

test("fails when a target write goes back to a blanket TargetAlreadyExists", () => {
  expectFailure(({ write, original }) => {
    write(
      SERVICE,
      original(SERVICE).replace(
        /return Err\(\s*classify_target_conflict\([\s\S]*?\.await,?\s*\);/,
        "return Err(ChannelError::TargetAlreadyExists(target_type.as_str().to_string(), target_value.clone()));",
      ),
    );
  }, "must not answer every unique violation as TargetAlreadyExists");
});

test("fails when any target type collision is reported as a duplicate", () => {
  expectFailure(({ write, original }) => {
    write(
      SERVICE,
      original(SERVICE).replace(
        "Ok(Some(_)) if target_type == ChannelTargetType::WebDomain => {",
        "Ok(Some(_)) => {",
      ),
    );
  }, "only a web host collision may be reported as a duplicate target");
});

test("fails when a bind upsert reads the row back outside its transaction", () => {
  expectFailure(({ write, original }) => {
    write(
      SERVICE,
      original(SERVICE).replace(
        "            .filter(channel_module_binding::Column::ModuleSlug.eq(&module_slug))\n            .one(&txn)",
        "            .filter(channel_module_binding::Column::ModuleSlug.eq(&module_slug))\n            .one(&self.db)",
      ),
    );
  }, "must read the row back from the same transaction");
});

test("fails when the SQLite lifecycle suite loses a case", () => {
  const LIFECYCLE_TEST = FIXTURE_FILES[7];
  expectFailure(({ write, original }) => {
    write(
      LIFECYCLE_TEST,
      original(LIFECYCLE_TEST).replace(
        "async fn removing_a_resolution_rule_twice_answers_not_found()",
        "async fn renamed_case()",
      ),
    );
  }, "removing_a_resolution_rule_twice_answers_not_found");
});

test("fails when a foreign OAuth app answers 400 instead of 404", () => {
  expectFailure(({ write, original }) => {
    write(
      CONTROLLER,
      original(CONTROLLER)
        .replace("    if !oauth_apps.iter().any(|app| app.id == oauth_app_id) {\n        return Err(Error::NotFound);", '    if !oauth_apps.iter().any(|app| app.id == oauth_app_id) {\n        return Err(Error::BadRequest("OAuth app does not belong to the current tenant".to_string()));'),
    );
  }, "must not answer 400");
});

test("fails when the PostgreSQL suite stops proving the composite FK", () => {
  const POSTGRES_TEST = FIXTURE_FILES[8];
  expectFailure(({ write, original }) => {
    write(
      POSTGRES_TEST,
      original(POSTGRES_TEST).replace('"fk_channel_targets_tenant_channel"', '"some_other_constraint"'),
    );
  }, "must prove the composite tenant/channel FK");
});

test("fails when the storefront read turns a channel conflict into a 500", () => {
  const COMMERCE_STORE = FIXTURE_FILES[9];
  expectFailure(({ write, original }) => {
    write(
      COMMERCE_STORE,
      original(COMMERCE_STORE)
        .replace(
          /        ChannelError::Conflict\(_\) => \(\s*StatusCode::SERVICE_UNAVAILABLE,\s*"commerce_store_channel_conflict",\s*"Store channel is temporarily unavailable",\s*"conflict",\s*\),/,
          "",
        )
        .replace(
          "        | ChannelError::InvalidPolicyOperation(_)\n        | ChannelError::Serialization(_) => (",
          "        | ChannelError::InvalidPolicyOperation(_)\n        | ChannelError::Conflict(_)\n        | ChannelError::Serialization(_) => (",
        ),
    );
  }, "must map the typed channel conflict explicitly");
});

test("fails when REST stops answering 409 for the typed conflict", () => {
  expectFailure(({ write, original }) => {
    write(
      CONTROLLER,
      original(CONTROLLER).replace("ChannelError::Conflict(_) =>", "ChannelError::Validation(_) =>"),
    );
  }, "REST maps the conflict");
});
