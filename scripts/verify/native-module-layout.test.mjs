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

function writeFfaSurface(ctx, side) {
  const crateName = "rustok-sample-" + side;
  ctx.write(side + "/Cargo.toml", '[package]\nname = "' + crateName + '"\n');
  ctx.write(side + "/src/lib.rs",
    'mod core;\nmod model;\nmod i18n;\nmod transport;\nmod ui;\n#[cfg(test)]\nmod tests;\n');
  ctx.write(side + "/src/core/mod.rs", "pub enum State { Pending }\n");
  ctx.write(side + "/src/model.rs", "pub struct OwnerModel;\n");
  ctx.write(side + "/src/i18n.rs", "pub fn t() {}\n");
  ctx.write(side + "/src/transport/mod.rs",
    'mod graphql_adapter;\nmod native_server_adapter;\n' +
    'pub async fn fetch_owner_capability() -> Result<(), ()> { graphql_adapter::fetch_owner_capability().await }\n');
  ctx.write(side + "/src/transport/graphql_adapter.rs",
    "pub async fn fetch_owner_capability() -> Result<(), ()> { Err(()) }\n");
  ctx.write(side + "/src/transport/native_server_adapter.rs",
    "pub async fn fetch_owner_capability() -> Result<(), ()> { Err(()) }\n");
  ctx.write(side + "/src/ui/mod.rs", "mod leptos;\n");
  ctx.write(side + "/src/ui/leptos.rs", "#[component]\npub fn Surface() {}\n");
  ctx.write(side + "/src/tests/mod.rs", "mod contract;\n");
  ctx.write(side + "/src/tests/contract.rs", "#[test]\nfn contract() {}\n");
  ctx.write(side + "/locales/en.ftl", "sample-state = Pending\n");
}

function writeNextSurface(ctx, side, packageName, routeSegment) {
  const host = side === "admin" ? "next-admin" : "next-frontend";
  const packageDir = side === "admin" ? "sample-admin" : "rustok-sample";
  const main = side === "admin" ? "./src/index.ts" : "./src/index.tsx";
  const packageRoot = "apps/" + host + "/packages/" + packageDir;
  ctx.writeRoot(packageRoot + "/package.json", JSON.stringify({ name: packageName, main }));
  ctx.writeRoot(packageRoot + "/" + main.replace(/^\.\//, ""), "export const Page = () => null;\n");
  const hostIndex = side === "admin"
    ? "apps/next-admin/src/modules/index.ts"
    : "apps/next-frontend/src/modules/index.ts";
  const hostImport = side === "admin"
    ? 'import "' + packageName + '";\n'
    : 'import "../../packages/' + packageDir + '/src";\n';
  ctx.writeRoot(hostIndex, hostImport);
  const route = side === "admin"
    ? "apps/next-admin/src/app/dashboard/" + routeSegment + "/page.tsx"
    : "apps/next-frontend/src/app/[locale]/" + routeSegment + "/page.tsx";
  ctx.writeRoot(route, 'import { Page } from "' + packageName + '";\nexport default Page;\n');
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

test("an unmounted Leptos FFA draft does not claim Next.js readiness", () => {
  fixture((ctx) => {
    ctx.write("rustok-module.toml",
      '[module]\nslug = "sample"\nui_classification = "future_ui"\n' +
      '[crate]\nentry_type = "SampleModule"\n');
    writeFfaSurface(ctx, "admin");
    assert.deepEqual(lint(ctx), []);
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

test("native Cargo identity must match the registered crate name", () => {
  fixture((ctx) => {
    ctx.write("Cargo.toml", '[package]\nname = "rustok-other"\n');
    assert.match(lint(ctx).join("\n"), /Cargo package.name must match modules.toml crate identity/);
  });
});

test("native module package names may differ from slugs when modules.toml declares them", () => {
  fixture((ctx) => {
    ctx.writeRoot("modules.toml",
      '[modules]\nsample = { crate = "sample-runtime", source = "path", path = "' +
      ctx.path(".") + '" }\n');
    ctx.write("Cargo.toml", '[package]\nname = "sample-runtime"\n');
    assert.deepEqual(lint(ctx), []);
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
      'next_package = "@rustok/sample-admin"\nroute_segment = "sample"\nnav_label = "Sample"\n' +
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
    writeNextSurface(ctx, "admin", "@rustok/sample-admin", "sample");
    assert.deepEqual(lint(ctx), []);
  });
});

test("support crates cannot impersonate no_ui or capability_only modules", () => {
  for (const classification of ["no_ui", "capability_only"]) {
    fixture((ctx) => {
      ctx.write("rustok-module.toml",
        '[module]\nslug = "sample"\nui_classification = "' + classification + '"\n' +
        '[crate]\nentry_type = "SampleModule"\n' +
        '[provides.storefront_support]\nleptos_crate = "rustok-sample-storefront-support"\n');
      ctx.write("storefront/Cargo.toml",
        '[package]\nname = "rustok-sample-storefront-support"\n');
      ctx.write("storefront/src/lib.rs", "pub struct Support;\n");
      assert.match(lint(ctx).join("\n"), /storefront_support is incompatible/);
    });
  }
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
    ctx.write("admin/src/contract_tests.rs", "#[test]\nfn legacy_contract() {}\n");
    ctx.write("admin/locales/en.ftl", "sample-title = Sample\n");
    const failures = lint(ctx).join("\n");
    assert.match(failures, /UI must use the module transport facade/);
    assert.doesNotMatch(failures, /contract_tests\.rs.*belong in src\/tests\/contract\.rs/);
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
    ctx.write("admin/src/model.rs",
      "use leptos_ui_routing::use_route_query_value;\nuse sea_orm::EntityTrait;\n");
    ctx.write("admin/src/core/mod.rs", "pub struct Core;\n");
    ctx.write("admin/src/transport/mod.rs", "mod native_server_adapter;\n");
    ctx.write("admin/src/transport/native_server_adapter.rs", "pub struct Adapter;\n");
    ctx.write("admin/src/ui/mod.rs", "mod leptos;\n");
    ctx.write("admin/src/ui/leptos.rs", "#[component]\npub fn Admin() {}\n");
    ctx.write("admin/src/i18n.rs", "pub fn t() {}\n");
    ctx.write("admin/src/tests/contract.rs", "#[test]\nfn contract() {}\n");
    ctx.write("admin/locales/en.ftl", "sample-title = Sample\n");
    assert.match(lint(ctx).join("\n"), /forbidden Leptos dependency in FFA model/);
    assert.match(lint(ctx).join("\n"), /forbidden SeaORM dependency in FFA model/);
  });
});


test("root error.rs and error/ cannot both claim the same responsibility", () => {
  fixture((ctx) => {
    ctx.write("src/error.rs", "pub struct Error;\n");
    ctx.write("src/error/mod.rs", "pub struct ErrorModule;\n");
    ctx.write("src/lib.rs",
      'mod module;\nmod error;\npub use module::SampleModule;\n#[cfg(test)]\nmod tests;\n');
    assert.match(lint(ctx).join("\n"), /use either error\.rs or error\//);
  });
});


test("DTO and port contracts cannot import persistence or transport frameworks", () => {
  fixture((ctx) => {
    ctx.write("src/dto/mod.rs", "use async_graphql::InputObject;\n");
    ctx.write("src/ports/mod.rs", "use sea_orm::DatabaseConnection;\n");
    ctx.write("src/lib.rs",
      'mod module;\nmod dto;\nmod ports;\npub use module::SampleModule;\n#[cfg(test)]\nmod tests;\n');
    const failures = lint(ctx).join("\n");
    assert.match(failures, /forbidden GraphQL dependency/);
    assert.match(failures, /forbidden SeaORM dependency/);
  });
});

test("persistence entities may use SeaORM but cannot depend on transport frameworks", () => {
  fixture((ctx) => {
    ctx.write("src/entities/mod.rs", "use sea_orm::EntityTrait;\nuse axum::Router;\n");
    ctx.write("src/lib.rs",
      'mod module;\nmod entities;\npub use module::SampleModule;\n#[cfg(test)]\nmod tests;\n');
    const failures = lint(ctx).join("\n");
    assert.match(failures, /forbidden Axum dependency/);
    assert.doesNotMatch(failures, /forbidden SeaORM dependency/);
  });
});

test("service code cannot depend on migration tooling", () => {
  fixture((ctx) => {
    ctx.write("src/services/mod.rs", "use sea_orm_migration::MigrationTrait;\n");
    ctx.write("src/lib.rs",
      'mod module;\nmod services;\npub use module::SampleModule;\n#[cfg(test)]\nmod tests;\n');
    assert.match(lint(ctx).join("\n"), /forbidden SeaORM migrations dependency/);
  });
});


test("native module paths cannot traverse a symlinked ancestor directory", () => {
  fixture((ctx) => {
    const modules = join(ctx.root, "crates/modules");
    rmSync(modules, { recursive: true, force: true });
    symlinkSync(join(ctx.root, "outside-modules"), modules);
    assert.match(lint(ctx).join("\n"), /native module path cannot traverse symlinks/);
  });
});


test("service dependency direction ignores test-only migration fixtures", () => {
  fixture((ctx) => {
    ctx.write("src/services/mod.rs",
      "pub mod post;\n#[cfg(test)]\nmod tag_tenant_integrity_tests;\n");
    ctx.write("src/services/post/mod.rs",
      "pub struct PostService;\n#[cfg(test)]\nmod tests;\n");
    ctx.write("src/services/post/tests.rs", "use sea_orm_migration::MigrationTrait;\n");
    ctx.write("src/services/tag_tenant_integrity_tests.rs",
      "use sea_orm_migration::SchemaManager;\n");
    ctx.write("src/lib.rs",
      'mod module;\nmod services;\npub use module::SampleModule;\n#[cfg(test)]\nmod tests;\n');
    assert.deepEqual(lint(ctx), []);
  });
});


test("test-named files without cfg(test) wiring remain production dependencies", () => {
  fixture((ctx) => {
    ctx.write("src/services/mod.rs", "mod tag_tenant_integrity_tests;\n");
    ctx.write("src/services/tag_tenant_integrity_tests.rs",
      "use sea_orm_migration::SchemaManager;\n");
    ctx.write("src/lib.rs",
      'mod module;\nmod services;\npub use module::SampleModule;\n#[cfg(test)]\nmod tests;\n');
    assert.match(lint(ctx).join("\n"), /forbidden SeaORM migrations dependency/);
  });
});


test("a tests directory without cfg(test) module wiring remains production source", () => {
  fixture((ctx) => {
    ctx.write("src/services/mod.rs", "mod tests;\n");
    ctx.write("src/services/tests/production.rs",
      "use sea_orm_migration::SchemaManager;\n");
    ctx.write("src/lib.rs",
      'mod module;\nmod services;\npub use module::SampleModule;\n#[cfg(test)]\nmod tests;\n');
    assert.match(lint(ctx).join("\n"), /forbidden SeaORM migrations dependency/);
  });
});

test("live admin UI requires a Next.js package and host route", () => {
  fixture((ctx) => {
    ctx.write("rustok-module.toml",
      '[module]\nslug = "sample"\nui_classification = "admin_only"\n' +
      '[crate]\nentry_type = "SampleModule"\n' +
      '[provides.admin_ui]\nleptos_crate = "rustok-sample-admin"\n' +
      'next_package = "@rustok/sample-admin"\nroute_segment = "sample"\nnav_label = "Sample"\n');
    writeFfaSurface(ctx, "admin");
    const failures = lint(ctx).join("\n");
    assert.match(failures, /Next.js package '@rustok\/sample-admin' is missing/);
  });
});

test("live admin UI passes only with matching Next.js package, host registry and route", () => {
  fixture((ctx) => {
    ctx.write("rustok-module.toml",
      '[module]\nslug = "sample"\nui_classification = "admin_only"\n' +
      '[crate]\nentry_type = "SampleModule"\n' +
      '[provides.admin_ui]\nleptos_crate = "rustok-sample-admin"\n' +
      'next_package = "@rustok/sample-admin"\nroute_segment = "sample"\nnav_label = "Sample"\n');
    writeFfaSurface(ctx, "admin");
    writeNextSurface(ctx, "admin", "@rustok/sample-admin", "sample");
    assert.deepEqual(lint(ctx), []);
  });
});

test("live storefront UI requires its Next.js package, host registry and localized route", () => {
  fixture((ctx) => {
    ctx.write("rustok-module.toml",
      '[module]\nslug = "sample"\nui_classification = "storefront_only"\n' +
      '[crate]\nentry_type = "SampleModule"\n' +
      '[provides.storefront_ui]\nleptos_crate = "rustok-sample-storefront"\n' +
      'next_package = "@rustok/sample-frontend"\nroute_segment = "sample"\nslot = "home_after_catalog"\npage_title = "Sample"\n');
    writeFfaSurface(ctx, "storefront");
    writeNextSurface(ctx, "storefront", "@rustok/sample-frontend", "sample");
    assert.deepEqual(lint(ctx), []);
  });
});

test("live Next.js route and host registry must import the declared package", () => {
  fixture((ctx) => {
    ctx.write("rustok-module.toml",
      '[module]\nslug = "sample"\nui_classification = "admin_only"\n' +
      '[crate]\nentry_type = "SampleModule"\n' +
      '[provides.admin_ui]\nleptos_crate = "rustok-sample-admin"\n' +
      'next_package = "@rustok/sample-admin"\nroute_segment = "sample"\nnav_label = "Sample"\n');
    writeFfaSurface(ctx, "admin");
    const packageRoot = "apps/next-admin/packages/sample-admin";
    ctx.writeRoot(packageRoot + "/package.json",
      JSON.stringify({ name: "@rustok/sample-admin", main: "./src/index.ts" }));
    ctx.writeRoot(packageRoot + "/src/index.ts", "export const Page = () => null;\n");
    ctx.writeRoot("apps/next-admin/src/modules/index.ts", "export {};\n");
    ctx.writeRoot("apps/next-admin/src/app/dashboard/sample/page.tsx", "export default function Page() {}\n");
    const failures = lint(ctx).join("\n");
    assert.match(failures, /Next.js host module registry does not import declared package/);
    assert.match(failures, /Next.js route must import declared package/);
  });
});
