import test from "node:test";
import assert from "node:assert/strict";
import { mkdtempSync, mkdirSync, writeFileSync, rmSync } from "node:fs";
import { join } from "node:path";
import { tmpdir } from "node:os";
import { lintNativeModule, resolveNativeModule } from "./native-module-layout.mjs";

function fixture(run) {
  const root = mkdtempSync(join(tmpdir(), "rustok-native-layout-"));
  const modulePath = "crates/modules/rustok-sample";
  const write = (path, value) => {
    const full = join(root, path);
    mkdirSync(join(full, ".."), { recursive: true });
    writeFileSync(full, value);
  };
  const path = (name) => join(modulePath, name);
  write("modules.toml",
    '[modules]\nsample = { crate = "rustok-sample", source = "path", path = "' + modulePath + '" }\n');
  write(path("Cargo.toml"), '[package]\nname = "rustok-sample"\n');
  write(path("rustok-module.toml"),
    '[module]\nslug = "sample"\n[crate]\nentry_type = "SampleModule"\n');
  write(path("src/lib.rs"),
    'mod module;\npub use module::SampleModule;\n#[cfg(test)]\nmod tests;\n');
  write(path("src/module.rs"),
    'pub struct SampleModule;\nimpl RusToKModule for SampleModule {}\n');
  write(path("src/tests/mod.rs"), "mod contract;\n");
  write(path("src/tests/contract.rs"), "#[test]\nfn contract() {}\n");
  try {
    return run({ root, write: (name, value) => write(path(name), value), path: (name) => path(name) });
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
}

function lint(ctx) {
  return lintNativeModule(ctx.root, ctx.path("."), "sample");
}

test("a minimal native module omits unused responsibility directories", () => {
  fixture((ctx) => {
    assert.equal(resolveNativeModule(ctx.root, "sample"), ctx.path(""));
    assert.deepEqual(lint(ctx), []);
  });
});

test("unknown and non-path module registry entries are rejected", () => {
  fixture((ctx) => {
    assert.throws(() => resolveNativeModule(ctx.root, "missing"), /Unknown native/);
    ctx.write("../../../modules.toml",
      '[modules]\nsample = { crate = "rustok-sample", source = "registry" }\n');
    assert.throws(() => resolveNativeModule(ctx.root, "sample"), /not a native local path/);
  });
});

test("private persistence and canonical root slots are enforced", () => {
  fixture((ctx) => {
    ctx.write("src/lib.rs",
      'mod module;\npub use module::SampleModule;\npub mod entities;\n#[cfg(test)]\nmod tests;\n');
    ctx.write("src/contract_tests.rs", "pub fn wrong() {}\n");
    const failures = lint(ctx).join("\n");
    assert.match(failures, /entities must remain private/);
    assert.match(failures, /noncanonical src\/ root Rust file/);
  });
});

test("domain and application boundaries reject transport dependencies", () => {
  fixture((ctx) => {
    ctx.write("src/domain/mod.rs", "use sea_orm::EntityTrait;\n");
    ctx.write("src/services/mod.rs", "use axum::Router;\n");
    const failures = lint(ctx).join("\n");
    assert.match(failures, /forbidden SeaORM/);
    assert.match(failures, /forbidden Axum/);
  });
});

test("source file size and module registration are checked", () => {
  fixture((ctx) => {
    ctx.write("src/module.rs", " ".repeat(32769));
    ctx.write("src/tests/contract.rs", "");
    const failures = lint(ctx).join("\n");
    assert.match(failures, /exceeds canonical source limit/);
    assert.match(failures, /missing native RusToKModule implementation/);
    assert.match(failures, /contract test file has no tests/);
  });
});

test("declared UI requires actual FFA structure independent of Next.js", () => {
  fixture((ctx) => {
    ctx.write("rustok-module.toml",
      '[module]\nslug = "sample"\n[crate]\nentry_type = "SampleModule"\n' +
      '[provides.admin_ui]\nleptos_crate = "rustok-sample-admin"\n');
    const failures = lint(ctx).join("\n");
    assert.match(failures, /admin\/Cargo.toml.*required file missing/);
    assert.match(failures, /model.rs.*required file missing/);
    assert.doesNotMatch(failures, /Next.js package metadata/);
  });
});

test("fake UI actions and Leptos in pure core fail FFA lint", () => {
  fixture((ctx) => {
    ctx.write("rustok-module.toml",
      '[module]\nslug = "sample"\n[crate]\nentry_type = "SampleModule"\n' +
      '[provides.admin_ui]\nleptos_crate = "rustok-sample-admin"\n');
    ctx.write("admin/Cargo.toml", '[package]\nname = "rustok-sample-admin"\n');
    ctx.write("admin/src/lib.rs",
      'mod model;\nmod core;\nmod transport;\nmod ui;\n#[cfg(test)]\nmod tests;\n');
    ctx.write("admin/src/model.rs", "pub struct Model;\n");
    ctx.write("admin/src/i18n.rs", "pub fn t() {}\n");
    ctx.write("admin/src/core.rs", "use leptos::prelude::*;\n");
    ctx.write("admin/src/transport/mod.rs", "pub fn fetch() {}\n");
    ctx.write("admin/src/ui/mod.rs", "let sample_rows = vec![];\n");
    ctx.write("admin/src/ui/leptos.rs", "pub fn render() {}\n");
    ctx.write("admin/src/tests/contract.rs", "#[test]\nfn contract() {}\n");
    ctx.write("admin/locales/en.ftl", "sample-title = Sample\n");
    const failures = lint(ctx).join("\n");
    assert.match(failures, /forbidden Leptos dependency in FFA core/);
    assert.match(failures, /fake CRUD\/sample content/);
  });
});

test("path traversal is never accepted as a module root", () => {
  fixture((ctx) => {
    assert.match(lintNativeModule(ctx.root, "../outside", "sample").join("\n"), /escapes/);
  });
});
