import test from "node:test";
import assert from "node:assert/strict";
import { mkdtempSync, mkdirSync, writeFileSync, rmSync, symlinkSync } from "node:fs";
import { join } from "node:path";
import { tmpdir } from "node:os";
import { lintNativeModule, resolveNativeModule } from "./native-module-layout.mjs";

function fixture(run) {
  const root = mkdtempSync(join(tmpdir(), "rustok-native-layout-"));
  const modulePath = "crates/modules/rustok-sample";
  const writeRoot = (path, value) => {
    const full = join(root, path);
    mkdirSync(join(full, ".."), { recursive: true });
    writeFileSync(full, value);
  };
  const write = (path, value) => writeRoot(path, value);
  const path = (name) => join(modulePath, name);
  write("modules.toml",
    '[modules]\nsample = { crate = "rustok-sample", source = "path", path = "' + modulePath + '" }\n');
  write(path("Cargo.toml"), '[package]\nname = "rustok-sample"\n');
  write(path("rustok-module.toml"),
    '[module]\nslug = "sample"\nui_classification = "no_ui"\n[crate]\nentry_type = "SampleModule"\n');
  write(path("src/lib.rs"),
    'mod module;\npub use module::SampleModule;\n#[cfg(test)]\nmod tests;\n');
  write(path("src/module.rs"),
    'pub struct SampleModule;\nimpl RusToKModule for SampleModule {}\n');
  write(path("src/tests/mod.rs"), "mod contract;\n");
  write(path("src/tests/contract.rs"), "#[test]\nfn contract() {}\n");
  try {
    return run({
      root,
      write: (name, value) => write(path(name), value),
      writeRoot,
      path: (name) => path(name),
    });
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
    ctx.writeRoot("modules.toml",
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
    ctx.write("src/lib.rs",
      'mod module;\nmod domain;\nmod services;\npub use module::SampleModule;\n#[cfg(test)]\nmod tests;\n');
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
      '[module]\nslug = "sample"\nui_classification = "admin_only"\n[crate]\nentry_type = "SampleModule"\n' +
      '[provides.admin_ui]\nleptos_crate = "rustok-sample-admin"\n');
    const failures = lint(ctx).join("\n");
    assert.match(failures, /admin: declared UI package directory is missing/);
    assert.doesNotMatch(failures, /Next.js package metadata/);
  });
});

test("Leptos FFA without any Next.js metadata passes native architecture lint", () => {
  fixture((ctx) => {
    ctx.write("rustok-module.toml",
      '[module]\nslug = "sample"\nui_classification = "admin_only"\n[crate]\nentry_type = "SampleModule"\n' +
      '[provides.admin_ui]\nleptos_crate = "rustok-sample-admin"\n');
    ctx.write("admin/Cargo.toml", '[package]\nname = "rustok-sample-admin"\n');
    ctx.write("admin/src/lib.rs",
      'mod core;\nmod model;\nmod i18n;\nmod transport;\nmod ui;\n#[cfg(test)]\nmod tests;\n');
    ctx.write("admin/src/core/mod.rs", "pub enum State { Pending }\n");
    ctx.write("admin/src/model.rs", "pub struct OwnerModel;\n");
    ctx.write("admin/src/i18n.rs", "pub fn t() {}\n");
    ctx.write("admin/src/transport/mod.rs",
      'mod graphql_adapter;\nmod native_server_adapter;\n' +
      'pub async fn fetch_owner_capability() -> Result<(), ()> { Err(()) }\n');
    ctx.write("admin/src/transport/graphql_adapter.rs",
      "pub async fn fetch_owner_capability() -> Result<(), ()> { Err(()) }\n");
    ctx.write("admin/src/transport/native_server_adapter.rs",
      "pub async fn fetch_owner_capability() -> Result<(), ()> { Err(()) }\n");
    ctx.write("admin/src/ui/mod.rs", "mod leptos;\n");
    ctx.write("admin/src/ui/leptos.rs", "#[component]\npub fn Admin() {}\n");
    ctx.write("admin/src/tests/mod.rs", "mod contract;\n");
    ctx.write("admin/src/tests/contract.rs", "#[test]\nfn contract() {}\n");
    ctx.write("admin/locales/en.ftl", "sample-state = Pending\n");
    assert.deepEqual(lint(ctx), []);

    // A scaffolded but unmounted FFA development package is still linted.
    ctx.write("rustok-module.toml",
      '[module]\nslug = "sample"\nui_classification = "future_ui"\n' +
      '[crate]\nentry_type = "SampleModule"\n');
    assert.deepEqual(lint(ctx), [], "future_ui accepts real draft package files");

    // A no_ui module may not silently hide a physical UI package.
    ctx.write("rustok-module.toml",
      '[module]\nslug = "sample"\nui_classification = "no_ui"\n' +
      '[crate]\nentry_type = "SampleModule"\n');
    assert.match(lint(ctx).join("\n"), /undeclared UI package requires explicit/);

    // Draft is not a free pass for a wrong Cargo package name.
    ctx.write("rustok-module.toml",
      '[module]\nslug = "sample"\nui_classification = "future_ui"\n' +
      '[crate]\nentry_type = "SampleModule"\n');
    ctx.write("admin/Cargo.toml", '[package]\nname = "wrong-admin"\n');
    assert.match(lint(ctx).join("\n"), /Cargo package name differs/);
  });
});

test("fake UI actions and Leptos in pure core fail FFA lint", () => {
  fixture((ctx) => {
    ctx.write("rustok-module.toml",
      '[module]\nslug = "sample"\nui_classification = "admin_only"\n[crate]\nentry_type = "SampleModule"\n' +
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

test("live UI classification requires both its package and provides metadata", () => {
  fixture((ctx) => {
    ctx.write("rustok-module.toml",
      '[module]\nslug = "sample"\nui_classification = "admin_only"\n' +
      '[crate]\nentry_type = "SampleModule"\n');
    const failures = lint(ctx).join("\n");
    assert.match(failures, /declared admin UI package is missing/);
    assert.match(failures, /live admin UI requires \[provides\.admin_ui\]/);
  });
});

test("future_ui cannot claim live provides metadata", () => {
  fixture((ctx) => {
    ctx.write("rustok-module.toml",
      '[module]\nslug = "sample"\nui_classification = "future_ui"\n' +
      '[crate]\nentry_type = "SampleModule"\n' +
      '[provides.admin_ui]\nleptos_crate = "rustok-sample-admin"\n');
    assert.match(lint(ctx).join("\n"), /\[provides\.admin_ui\] contradicts future_ui/);
  });
});

test("no_ui rejects both unadvertised packages and invented live provides metadata", () => {
  fixture((ctx) => {
    ctx.write("admin/Cargo.toml", '[package]\nname = "rustok-sample-admin"\n');
    ctx.write("rustok-module.toml",
      '[module]\nslug = "sample"\nui_classification = "no_ui"\n' +
      '[crate]\nentry_type = "SampleModule"\n' +
      '[provides.admin_ui]\nleptos_crate = "rustok-sample-admin"\n');
    const failures = lint(ctx).join("\n");
    assert.match(failures, /\[provides\.admin_ui\] contradicts no_ui/);
    assert.match(failures, /no_ui forbids unadvertised admin UI package/);
  });
});

test("native Cargo identity matches registered snake_case slug", () => {
  fixture((ctx) => {
    ctx.write("Cargo.toml", '[package]\nname = "rustok-other"\n');
    assert.match(lint(ctx).join("\n"), /Cargo package.name must match module slug/);
  });
});


test("capability_only is a valid headless native module classification", () => {
  fixture((ctx) => {
    ctx.write("rustok-module.toml",
      '[module]\nslug = "sample"\nui_classification = "capability_only"\n' +
      '[crate]\nentry_type = "SampleModule"\n');
    assert.deepEqual(lint(ctx), []);
  });
});

test("declared support sub-crates are not mistaken for live FFA surfaces", () => {
  fixture((ctx) => {
    ctx.write("rustok-module.toml",
      '[module]\nslug = "sample"\nui_classification = "future_ui"\n' +
      '[crate]\nentry_type = "SampleModule"\n' +
      '[provides.storefront_support]\nleptos_crate = "rustok-sample-storefront-support"\n');
    ctx.write("storefront/Cargo.toml",
      '[package]\nname = "rustok-sample-storefront-support"\n');
    ctx.write("storefront/src/lib.rs", "pub struct ReactionBar;\n");
    ctx.write("storefront/src/contract_tests.rs", "#[test] fn support_contract() {}\n");
    assert.deepEqual(lint(ctx), []);
  });
});

test("the opposite-side support crate is valid beside a single live surface", () => {
  fixture((ctx) => {
    ctx.write("rustok-module.toml",
      '[module]\nslug = "sample"\nui_classification = "admin_only"\n' +
      '[crate]\nentry_type = "SampleModule"\n' +
      '[provides.admin_ui]\nleptos_crate = "rustok-sample-admin"\n' +
      '[provides.storefront_support]\nleptos_crate = "rustok-sample-storefront-support"\n');
    ctx.write("admin/Cargo.toml", '[package]\nname = "rustok-sample-admin"\n');
    ctx.write("admin/src/lib.rs",
      'mod model;\nmod core;\nmod transport;\nmod ui;\nmod i18n;\n#[cfg(test)]\nmod tests;\n');
    ctx.write("admin/src/model.rs", "pub struct Model;\n");
    ctx.write("admin/src/core/mod.rs", "pub struct Core;\n");
    ctx.write("admin/src/transport/mod.rs", "mod native_server_adapter;\n");
    ctx.write("admin/src/transport/native_server_adapter.rs", "pub struct Adapter;\n");
    ctx.write("admin/src/ui/mod.rs", "mod leptos;\n");
    ctx.write("admin/src/ui/leptos.rs", "#[component]\npub fn Admin() {}\n");
    ctx.write("admin/src/i18n.rs", "pub fn t() {}\n");
    ctx.write("admin/src/tests/contract.rs", "#[test]\nfn contract() {}\n");
    ctx.write("admin/locales/en.ftl", "sample-title = Sample\n");
    ctx.write("storefront/Cargo.toml",
      '[package]\nname = "rustok-sample-storefront-support"\n');
    ctx.write("storefront/src/lib.rs", "pub struct Support;\n");
    assert.deepEqual(lint(ctx), []);
  });
});

test("support crates cannot impersonate a no_ui or capability_only module surface", () => {
  fixture((ctx) => {
    ctx.write("rustok-module.toml",
      '[module]\nslug = "sample"\nui_classification = "no_ui"\n' +
      '[crate]\nentry_type = "SampleModule"\n' +
      '[provides.storefront_support]\nleptos_crate = "rustok-sample-storefront-support"\n');
    ctx.write("storefront/Cargo.toml",
      '[package]\nname = "rustok-sample-storefront-support"\n');
    ctx.write("storefront/src/lib.rs", "pub struct Support;\n");
    assert.match(lint(ctx).join("\n"), /storefront_support is incompatible/);
  });
});

test("a sub-crate cannot be declared as both a UI surface and a support package", () => {
  fixture((ctx) => {
    ctx.write("rustok-module.toml",
      '[module]\nslug = "sample"\nui_classification = "admin_only"\n' +
      '[crate]\nentry_type = "SampleModule"\n' +
      '[provides.admin_ui]\nleptos_crate = "rustok-sample-admin"\n' +
      '[provides.admin_support]\nleptos_crate = "rustok-sample-admin"\n');
    ctx.write("admin/Cargo.toml", '[package]\nname = "rustok-sample-admin"\n');
    ctx.write("admin/src/lib.rs", "pub struct Surface;\n");
    assert.match(lint(ctx).join("\n"), /cannot be both provides\.admin_ui and provides\.admin_support/);
  });
});

test("optional runtime.rs is allowed only when wired by the crate facade", () => {
  fixture((ctx) => {
    ctx.write("src/runtime.rs", "pub struct Runtime;\n");
    assert.match(lint(ctx).join("\n"), /runtime\.rs must be declared/);
    ctx.write("src/lib.rs",
      'mod module;\nmod runtime;\npub use module::SampleModule;\n#[cfg(test)]\nmod tests;\n');
    assert.deepEqual(lint(ctx), []);
  });
});

test("unknown responsibility directories and empty optional slots are rejected", () => {
  fixture((ctx) => {
    ctx.write("src/helpers/mod.rs", "pub fn helper() {}\n");
    ctx.write("src/integrations/mod.rs", "//! Placeholder only.\n");
    ctx.write("src/lib.rs",
      'mod module;\nmod integrations;\npub use module::SampleModule;\n#[cfg(test)]\nmod tests;\n');
    const failures = lint(ctx).join("\n");
    assert.match(failures, /unrecognized native responsibility directory/);
    assert.match(failures, /empty responsibility slot is forbidden/);
  });
});

test("optional responsibility directories require their canonical mod.rs entrypoint", () => {
  fixture((ctx) => {
    ctx.write("src/domain/model.rs", "pub struct Model;\n");
    assert.match(lint(ctx).join("\n"), /responsibility directory requires mod\.rs/);
  });
});

test("Rust comments and string literals cannot fake dependencies or module implementations", () => {
  fixture((ctx) => {
    ctx.write("src/module.rs",
      '// impl RusToKModule for SampleModule {}\n' +
      'pub struct SampleModule;\n');
    ctx.write("src/domain/mod.rs",
      '/* outer comment /* nested use sea_orm::EntityTrait; */ still a comment */\n' +
      'const DOC: &str = "use sea_orm::EntityTrait;";\n' +
      'pub struct Pure;\n');
    const failures = lint(ctx).join("\n");
    assert.match(failures, /missing native RusToKModule implementation/);
    assert.doesNotMatch(failures, /forbidden SeaORM/);
  });
});

test("symlinks are rejected instead of followed through native source trees", () => {
  fixture((ctx) => {
    ctx.write("src/domain/mod.rs", "pub struct Domain;\n");
    const target = join(ctx.root, "outside.rs");
    ctx.writeRoot("outside.rs", "pub struct Outside;\n");
    symlinkSync(target, join(ctx.root, ctx.path("src/domain/outside.rs")));
    assert.match(lint(ctx).join("\n"), /symlinks are forbidden/);
  });
});

test("Leptos UI code cannot bypass the transport facade through raw adapters", () => {
  fixture((ctx) => {
    ctx.write("rustok-module.toml",
      '[module]\nslug = "sample"\nui_classification = "admin_only"\n' +
      '[crate]\nentry_type = "SampleModule"\n' +
      '[provides.admin_ui]\nleptos_crate = "rustok-sample-admin"\n');
    ctx.write("admin/Cargo.toml", '[package]\nname = "rustok-sample-admin"\n');
    ctx.write("admin/src/lib.rs",
      'mod model;\nmod core;\nmod transport;\nmod ui;\nmod i18n;\n#[cfg(test)]\nmod tests;\n');
    ctx.write("admin/src/model.rs", "pub struct Model;\n");
    ctx.write("admin/src/core/mod.rs", "pub struct Core;\n");
    ctx.write("admin/src/transport/mod.rs", "mod native_server_adapter;\n");
    ctx.write("admin/src/transport/native_server_adapter.rs", "pub struct Adapter;\n");
    ctx.write("admin/src/ui/mod.rs", "mod leptos;\n");
    ctx.write("admin/src/ui/leptos.rs",
      "use crate::transport::native_server_adapter::request;\n");
    ctx.write("admin/src/i18n.rs", "pub fn t() {}\n");
    ctx.write("admin/src/tests/contract.rs", "#[test]\nfn contract() {}\n");
    ctx.write("admin/locales/en.ftl", "sample-title = Sample\n");
    assert.match(lint(ctx).join("\n"), /UI must use the module transport facade/);
  });
});


test("optional error.rs is a canonical root responsibility when wired", () => {
  fixture((ctx) => {
    ctx.write("src/error.rs", "pub struct DomainError;\n");
    assert.match(lint(ctx).join("\n"), /error\.rs must be declared/);
    ctx.write("src/lib.rs",
      'mod module;\nmod error;\npub use module::SampleModule;\n#[cfg(test)]\nmod tests;\n');
    assert.deepEqual(lint(ctx), []);
  });
});

test("native lib.rs rejects business definitions instead of acting as a dumping ground", () => {
  fixture((ctx) => {
    ctx.write("src/lib.rs",
      'mod module;\npub use module::SampleModule;\npub fn create() {}\n#[cfg(test)]\nmod tests;\n');
    assert.match(lint(ctx).join("\n"), /lib\.rs must remain a thin module\/re-export facade/);
  });
});

test("an optional responsibility directory must be wired through lib.rs", () => {
  fixture((ctx) => {
    ctx.write("src/domain/mod.rs", "pub struct Domain;\n");
    assert.match(lint(ctx).join("\n"), /responsibility directory is not declared by lib\.rs/);
  });
});

test("FFA model remains framework-independent", () => {
  fixture((ctx) => {
    ctx.write("rustok-module.toml",
      '[module]\nslug = "sample"\nui_classification = "admin_only"\n' +
      '[crate]\nentry_type = "SampleModule"\n' +
      '[provides.admin_ui]\nleptos_crate = "rustok-sample-admin"\n');
    ctx.write("admin/Cargo.toml", '[package]\nname = "rustok-sample-admin"\n');
    ctx.write("admin/src/lib.rs",
      'mod model;\nmod core;\nmod transport;\nmod ui;\nmod i18n;\n#[cfg(test)]\nmod tests;\n');
    ctx.write("admin/src/model.rs", "use leptos::prelude::*;\n");
    ctx.write("admin/src/core/mod.rs", "pub struct Core;\n");
    ctx.write("admin/src/transport/mod.rs", "mod native_server_adapter;\n");
    ctx.write("admin/src/transport/native_server_adapter.rs", "pub struct Adapter;\n");
    ctx.write("admin/src/ui/mod.rs", "mod leptos;\n");
    ctx.write("admin/src/ui/leptos.rs", "#[component]\npub fn Admin() {}\n");
    ctx.write("admin/src/i18n.rs", "pub fn t() {}\n");
    ctx.write("admin/src/tests/contract.rs", "#[test]\nfn contract() {}\n");
    ctx.write("admin/locales/en.ftl", "sample-title = Sample\n");
    assert.match(lint(ctx).join("\n"), /forbidden Leptos dependency in FFA model/);
  });
});
