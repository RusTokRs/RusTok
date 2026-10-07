import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import process from "node:process";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";

// Negative and positive fixtures for verify-rust-constructor-arity.mjs: a
// multi-argument `Box::new` must be reported, and every single-argument shape the
// scanner has to tolerate (closures, turbofish, matches, struct literals,
// trailing commas, comparisons, bitwise-or arguments) must stay silent.

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..");
const verifier = path.join(repoRoot, "scripts/verify/verify-rust-constructor-arity.mjs");

const BAD = `use sea_orm_migration::MigrationTrait;

pub fn migrations() -> Vec<Box<dyn MigrationTrait>> {
    let mut migrations: Vec<Box<dyn MigrationTrait>> = vec![];
    migrations.push(Box::new(
        m20261007_000012_add_checkout_operation_admission::Migration,
        m20261007_000013_drop_provider_execution_checkout_guard::Migration,
    ));
    migrations
}
`;

const GOOD = `use std::collections::HashMap;
use std::sync::Arc;

pub fn ok() {
    let _ = Arc::new(|_, _| {});
    let _ = Arc::new(move |entries, observations| entries + observations);
    let _ = Arc::new(move || 1);
    let _ = Box::new(HashMap::<String, u32>::new());
    let _ = Box::new(
        factory(),
    );
    let _ = Box::new(
        a | b,
    );
    let _ = Box::new(if a < b { 1 } else { 2 });
    let _ = Box::new(match kind { Kind::A => 1, Kind::B => 2 });
    let _ = Box::new(Thing { first: 1, second: 2 });
    let _ = Box::new((1, 2));
    let _ = Arc::new(Outer::new(Box::new(Inner::new())));
    let _ = Arc::new(
        move |first: Type, second: Other| {
            first.merge(second, third)
        },
    );
    let _ = Box::new(A::<B<C, D>, E>::new());
}
`;

const failures = [];
const fixtures = [];

function makeFixture(source) {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "rustok-arity-"));
  const directory = path.join(root, "crates/fixture/src");
  fs.mkdirSync(directory, { recursive: true });
  fs.writeFileSync(path.join(directory, "lib.rs"), source);
  fixtures.push(root);
  return root;
}

function run(fixtureRoot) {
  return spawnSync(process.execPath, [verifier], {
    encoding: "utf8",
    env: { ...process.env, RUSTOK_VERIFY_REPO_ROOT: fixtureRoot },
  });
}

try {
  const bad = run(makeFixture(BAD));
  if (bad.status === 0) {
    failures.push("a multi-argument Box::new must fail the verifier");
  } else if (!bad.stderr.includes("crates/fixture/src/lib.rs:5")) {
    failures.push(`the failure must name the file and line, got: ${bad.stderr.trim()}`);
  }

  const good = run(makeFixture(GOOD));
  if (good.status !== 0) {
    failures.push(`single-argument shapes must pass, got: ${good.stderr.trim()}`);
  }

  const full = spawnSync(process.execPath, [verifier], { encoding: "utf8", cwd: repoRoot });
  if (full.status !== 0) {
    failures.push(`the repository itself must pass, got: ${full.stderr.trim()}`);
  }
} finally {
  for (const root of fixtures) fs.rmSync(root, { recursive: true, force: true });
}

if (failures.length > 0) {
  console.error("rust constructor arity self-test failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}
console.log("rust constructor arity self-test passed");
