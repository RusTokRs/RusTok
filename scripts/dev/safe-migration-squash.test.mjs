#!/usr/bin/env node

import test from 'node:test';
import assert from 'node:assert/strict';
import { cpSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { spawnSync } from 'node:child_process';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const REPO_ROOT = path.resolve(__dirname, '..', '..');
const SCRIPT_PATH = path.join(REPO_ROOT, 'scripts', 'dev', 'safe-migration-squash.mjs');

function run(args = []) {
  return spawnSync(process.execPath, [SCRIPT_PATH, ...args], {
    cwd: REPO_ROOT,
    encoding: 'utf8',
  });
}

test('safe-migration-squash succeeds on canonical rustok-blog consolidated baseline', () => {
  const result = run(['--module', 'blog']);
  assert.equal(result.status, 0, `Expected status 0, got ${result.status}\nstdout: ${result.stdout}\nstderr: ${result.stderr}`);
  assert.match(result.stdout, /PARITY PERFECT/);
  assert.match(result.stdout, /All 9 tables and columns strictly verified/);
  assert.match(result.stdout, /Zero obsolete migration references detected/);
});

test('safe-migration-squash succeeds on multi-migration rustok-channel module', () => {
  const result = run(['--module', 'channel']);
  assert.equal(result.status, 0, `Expected status 0, got ${result.status}\nstdout: ${result.stdout}\nstderr: ${result.stderr}`);
  assert.match(result.stdout, /PARITY PERFECT/);
  assert.match(result.stdout, /All 6 tables and columns strictly verified/);
});

test('safe-migration-squash succeeds on multi-migration rustok-taxonomy module with alter_table and custom Iden', () => {
  const result = run(['--module', 'taxonomy']);
  assert.equal(result.status, 0, `Expected status 0, got ${result.status}\nstdout: ${result.stdout}\nstderr: ${result.stderr}`);
  assert.match(result.stdout, /PARITY PERFECT/);
  assert.match(result.stdout, /All 7 tables and columns strictly verified/);
});

test('adversarial test: detects missing table in migration', () => {
  const tempDir = mkdtempSync(path.join(tmpdir(), 'squash-test-missing-table-'));
  try {
    const fakeModule = path.join(tempDir, 'crates', 'modules', 'fake-module');
    mkdirSync(path.join(fakeModule, 'src', 'entities'), { recursive: true });
    mkdirSync(path.join(fakeModule, 'src', 'migrations'), { recursive: true });

    // Entity with table "fake_items"
    writeFileSync(
      path.join(fakeModule, 'src', 'entities', 'item.rs'),
      `#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "fake_items")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: Uuid,
    pub name: String,
}
`,
    );

    // Migration that creates unrelated table "other_table"
    writeFileSync(
      path.join(fakeModule, 'src', 'migrations', 'm20261029_000001_init.rs'),
      `use sea_orm_migration::prelude::*;
pub struct Migration;
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager.create_table(Table::create().table(OtherTable::Table).col(ColumnDef::new(OtherTable::Id).uuid().primary_key()).to_owned()).await
    }
}
#[derive(Iden)]
enum OtherTable { Table, Id }
`,
    );

    writeFileSync(
      path.join(fakeModule, 'src', 'migrations', 'mod.rs'),
      `pub mod m20261029_000001_init;
pub fn migrations() -> Vec<Box<dyn MigrationTrait>> {
    vec![Box::new(m20261029_000001_init::Migration)]
}
`,
    );

    const result = spawnSync(process.execPath, [SCRIPT_PATH, '--module', fakeModule], {
      cwd: tempDir,
      encoding: 'utf8',
    });

    assert.notEqual(result.status, 0, 'Must fail when table is missing');
    assert.match(result.stderr, /Table 'fake_items' .* NOT FOUND in any migration definition/);
  } finally {
    rmSync(tempDir, { recursive: true, force: true });
  }
});

test('adversarial test: detects missing column in migration', () => {
  const tempDir = mkdtempSync(path.join(tmpdir(), 'squash-test-missing-col-'));
  try {
    const fakeModule = path.join(tempDir, 'crates', 'modules', 'fake-module');
    mkdirSync(path.join(fakeModule, 'src', 'entities'), { recursive: true });
    mkdirSync(path.join(fakeModule, 'src', 'migrations'), { recursive: true });

    // Entity has id, name, AND secret_code
    writeFileSync(
      path.join(fakeModule, 'src', 'entities', 'item.rs'),
      `#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "fake_items")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: Uuid,
    pub name: String,
    pub secret_code: String,
}
`,
    );

    // Migration only creates id and name (missed secret_code)
    writeFileSync(
      path.join(fakeModule, 'src', 'migrations', 'm20261029_000001_init.rs'),
      `use sea_orm_migration::prelude::*;
pub struct Migration;
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager.create_table(
            Table::create()
                .table(FakeItems::Table)
                .col(ColumnDef::new(FakeItems::Id).uuid().primary_key())
                .col(ColumnDef::new(FakeItems::Name).string().not_null())
                .to_owned()
        ).await
    }
}
#[derive(Iden)]
enum FakeItems { Table, Id, Name }
`,
    );

    writeFileSync(
      path.join(fakeModule, 'src', 'migrations', 'mod.rs'),
      `pub mod m20261029_000001_init;
pub fn migrations() -> Vec<Box<dyn MigrationTrait>> {
    vec![Box::new(m20261029_000001_init::Migration)]
}
`,
    );

    const result = spawnSync(process.execPath, [SCRIPT_PATH, '--module', fakeModule], {
      cwd: tempDir,
      encoding: 'utf8',
    });

    assert.notEqual(result.status, 0, 'Must fail when column is missing');
    assert.match(result.stderr, /Column 'secret_code' .* missing in migration definition/);
  } finally {
    rmSync(tempDir, { recursive: true, force: true });
  }
});

test('adversarial test: detects nullability mismatch (Option<T> in entity but not_null in migration)', () => {
  const tempDir = mkdtempSync(path.join(tmpdir(), 'squash-test-nullability-'));
  try {
    const fakeModule = path.join(tempDir, 'crates', 'modules', 'fake-module');
    mkdirSync(path.join(fakeModule, 'src', 'entities'), { recursive: true });
    mkdirSync(path.join(fakeModule, 'src', 'migrations'), { recursive: true });

    // Entity has nickname: Option<String>
    writeFileSync(
      path.join(fakeModule, 'src', 'entities', 'item.rs'),
      `#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "fake_items")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: Uuid,
    pub nickname: Option<String>,
}
`,
    );

    // Migration mistakenly declared Nickname as .not_null()!
    writeFileSync(
      path.join(fakeModule, 'src', 'migrations', 'm20261029_000001_init.rs'),
      `use sea_orm_migration::prelude::*;
pub struct Migration;
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager.create_table(
            Table::create()
                .table(FakeItems::Table)
                .col(ColumnDef::new(FakeItems::Id).uuid().primary_key())
                .col(ColumnDef::new(FakeItems::Nickname).string().not_null())
                .to_owned()
        ).await
    }
}
#[derive(Iden)]
enum FakeItems { Table, Id, Nickname }
`,
    );

    writeFileSync(
      path.join(fakeModule, 'src', 'migrations', 'mod.rs'),
      `pub mod m20261029_000001_init;
pub fn migrations() -> Vec<Box<dyn MigrationTrait>> {
    vec![Box::new(m20261029_000001_init::Migration)]
}
`,
    );

    const result = spawnSync(process.execPath, [SCRIPT_PATH, '--module', fakeModule], {
      cwd: tempDir,
      encoding: 'utf8',
    });

    assert.notEqual(result.status, 0, 'Must fail when nullable entity field is NOT NULL in migration');
    assert.match(result.stderr, /Column 'nickname' is Option<T> in Entity but NOT NULL in migration/);
  } finally {
    rmSync(tempDir, { recursive: true, force: true });
  }
});

