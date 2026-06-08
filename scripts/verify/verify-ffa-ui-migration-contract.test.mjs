#!/usr/bin/env node

import { test } from "node:test";
import assert from "node:assert/strict";
import {
  mkdtempSync,
  writeFileSync,
  mkdirSync,
  rmSync,
} from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { spawnSync } from "node:child_process";

const scriptPath = path.resolve("scripts/verify/verify-ffa-ui-migration-contract.mjs");

function withFixture({
  pipeline,
  contractCommand,
  docsCommand,
  registryShape = "core_transport_ui",
  localShape = "core_transport_ui",
}) {
  const root = mkdtempSync(path.join(tmpdir(), "rustok-ffa-verify-"));
  mkdirSync(path.join(root, "docs", "research"), { recursive: true });
  mkdirSync(path.join(root, "docs", "verification"), { recursive: true });
  mkdirSync(path.join(root, "docs", "modules"), { recursive: true });
  mkdirSync(path.join(root, "crates", "rustok-cart", "docs"), { recursive: true });
  mkdirSync(path.join(root, "crates", "rustok-cart", "storefront", "src", "core"), { recursive: true });
  mkdirSync(path.join(root, "crates", "rustok-cart", "storefront", "src", "transport"), { recursive: true });
  mkdirSync(path.join(root, "crates", "rustok-cart", "storefront", "src", "ui"), { recursive: true });
  mkdirSync(path.join(root, "crates", "rustok-region", "storefront", "src", "ui"), { recursive: true });
  mkdirSync(path.join(root, "crates", "rustok-region", "storefront", "locales"), { recursive: true });
  mkdirSync(path.join(root, "crates", "rustok-pages", "storefront", "src", "ui"), { recursive: true });
  mkdirSync(path.join(root, "crates", "rustok-product", "storefront", "src", "ui"), { recursive: true });
  mkdirSync(path.join(root, "crates", "rustok-product", "storefront", "src", "transport"), { recursive: true });
  mkdirSync(path.join(root, "crates", "rustok-product", "admin", "src", "ui"), { recursive: true });
  mkdirSync(path.join(root, "crates", "rustok-customer", "admin", "src", "transport"), { recursive: true });
  mkdirSync(path.join(root, "crates", "rustok-customer", "admin", "src", "ui"), { recursive: true });

  writeFileSync(
    path.join(root, "docs", "research", "dioxus-ffa-ui-migration-plan.md"),
    [
      "## Фазы реализации",
      "## Принцип исполнения backlog (одна задача за итерацию)",
      "## Политика актуализации verification scripts",
      "## Phase-gate критерии (обязательные переходы между фазами)",
      "## KPI parity (измеримые пороги)",
      "Функциональный parity",
      "Error parity",
      "Performance guard",
      "Contract guard",
      "Docs guard",
      "## RACI (кто принимает phase-gates)",
    ].join("\n"),
  );

  writeFileSync(
    path.join(root, "docs", "research", "dioxus-ffa-pilot-connectivity-map.md"),
    "rustok-pages\nrustok-search\n",
  );

  writeFileSync(
    path.join(root, "docs", "verification", "ffa-ui-parity-checklist.md"),
    [
      "- [ ] Native path (Leptos SSR/hydrate) работает для целевого сценария.",
      "- [ ] GraphQL fallback работает для того же сценария.",
      "- [ ] Headless host path (Next/mobile/external) не сломан.",
      "- [ ] GraphQL/REST surface не удалён и не ослаблен.",
      "- [ ] UI слой не владеет transport/business логикой.",
      "- [ ] UI adapter обращается к transport только через module-owned facade; request/command/state construction и business/policy остаются в core ports/helpers.",
      "- [ ] Core слой не зависит от `leptos*`.",
      "- [ ] Transport adapters разделены по ролям: native и GraphQL fallback либо явно зафиксирован temporary single-adapter state с next-step parity plan.",
      "- [ ] Host-visible UI status/error contracts имеют stable machine-readable codes и documented locale keys.",
      "- [ ] Выполнен `npm run verify:ffa:ui:migration`.",
      "- [ ] Для изменённых error/status контрактов приложен список stable codes и locale keys.",
    ].join("\n"),
  );

  writeFileSync(
    path.join(root, "docs", "modules", "registry.md"),
    [
      "Structural shape фиксирует глубину code-level FFA split.",
      "- `none`",
      "- `docs_boundary`",
      "- `core_only`",
      "- `core_transport`",
      "- `core_transport_ui`",
      "- `no_ui_boundary`",
      "| Module slug | UI surfaces | FFA status | FBA status | Structural shape | Source plan |",
      "|---|---|---|---|---|---|",
      "| `cart` | storefront | `in_progress` | `in_progress` | `" + registryShape + "` | `crates/rustok-cart/docs/implementation-plan.md` fixture |",
    ].join("\n"),
  );

  writeFileSync(
    path.join(root, "crates", "rustok-cart", "docs", "implementation-plan.md"),
    [
      "## FFA/FBA status",
      "",
      "- FFA status: `in_progress`",
      "- FBA status: `in_progress`",
      "- Structural shape: `" + localShape + "`",
    ].join("\n"),
  );

  writeFileSync(
    path.join(root, "docs", "index.md"),
    [
      "[plan](./research/dioxus-ffa-ui-migration-plan.md)",
      "[map](./research/dioxus-ffa-pilot-connectivity-map.md)",
      "[check](./verification/ffa-ui-parity-checklist.md)",
    ].join("\n"),
  );


  writeFileSync(
    path.join(root, "crates", "rustok-pages", "storefront", "src", "lib.rs"),
    [
      "mod core;",
      "mod transport;",
      "mod ui;",
      "pub use ui::leptos::PagesView;",
    ].join("\n"),
  );

  writeFileSync(
    path.join(root, "crates", "rustok-pages", "storefront", "src", "ui", "leptos.rs"),
    "#[component] fn PagesView() { Resource::new_blocking(); transport::fetch_pages(); }",
  );

  writeFileSync(
    path.join(root, "crates", "rustok-pages", "storefront", "README.md"),
    "src/ui/leptos.rs core.rs transport.rs",
  );


  writeFileSync(
    path.join(root, "crates", "rustok-product", "storefront", "src", "core.rs"),
    [
      "pub struct ProductTransportErrorDomEvidence;",
      "pub fn build_product_transport_error_dom_evidence() {}",
    ].join("\n"),
  );

  writeFileSync(
    path.join(root, "crates", "rustok-product", "storefront", "src", "transport", "mod.rs"),
    "pub struct ProductTransportError; ProductTransportPath NativeServer Graphql native_server graphql fallback_attempted native_error graphql_error",
  );

  writeFileSync(
    path.join(root, "crates", "rustok-product", "storefront", "src", "ui", "leptos.rs"),
    [
      "build_product_transport_error_dom_evidence",
      "data-product-transport-failed-path",
      "data-product-transport-fallback-attempted",
      "data-product-transport-native-error",
      "data-product-transport-graphql-error",
    ].join(" "),
  );

  writeFileSync(
    path.join(root, "crates", "rustok-product", "storefront", "README.md"),
    "ProductTransportError ProductTransportErrorDomEvidence data-product-transport-*",
  );

  writeFileSync(
    path.join(root, "crates", "rustok-product", "admin", "src", "core.rs"),
    [
      "ProductAdminShellViewModel",
      "build_product_admin_shell_view_model",
      "ProductAdminProfilePanelViewModel",
      "build_product_admin_profile_panel_loading_view_model",
      "build_product_admin_profile_panel_error_view_model",
      "build_product_admin_profile_panel_ready_view_model",
    ].join("\n"),
  );

  writeFileSync(
    path.join(root, "crates", "rustok-product", "admin", "src", "ui", "leptos.rs"),
    [
      "build_product_admin_shell_view_model",
      "build_product_admin_profile_panel_loading_view_model",
      "build_product_admin_profile_panel_error_view_model",
      "build_product_admin_profile_panel_ready_view_model",
    ].join("\n"),
  );

  writeFileSync(
    path.join(root, "crates", "rustok-product", "admin", "README.md"),
    "admin shell copy profile-panel state",
  );

  writeFileSync(
    path.join(root, "crates", "rustok-customer", "admin", "src", "core.rs"),
    [
      "CustomerAdminDraftInput",
      "CustomerAdminSubmitCommand",
      "CustomerAdminSubmitCommandError",
      "build_customer_admin_submit_command",
      "EmailRequired",
      "LocaleUnavailable",
    ].join("\n"),
  );

  writeFileSync(
    path.join(root, "crates", "rustok-customer", "admin", "src", "lib.rs"),
    [
      "mod core;",
      "mod i18n;",
      "mod model;",
      "mod transport;",
      "mod ui;",
      "pub use ui::CustomerAdmin;",
    ].join("\n"),
  );

  writeFileSync(
    path.join(root, "crates", "rustok-customer", "admin", "src", "transport", "mod.rs"),
    [
      "mod native_server_adapter;",
      "pub use native_server_adapter::ApiError;",
      "use native_server_adapter as native;",
      "native::fetch_bootstrap().await",
      "native::fetch_customers(search, page, per_page).await",
      "native::fetch_customer_detail(customer_id).await",
      "native::create_customer(payload).await",
      "native::update_customer(customer_id, payload).await",
    ].join("\n"),
  );

  writeFileSync(
    path.join(root, "crates", "rustok-customer", "admin", "src", "transport", "native_server_adapter.rs"),
    [
      "#[server(prefix = \"/api/fn\", endpoint = \"customer/bootstrap\")]",
      "#[server(prefix = \"/api/fn\", endpoint = \"customer/list\")]",
      "#[server(prefix = \"/api/fn\", endpoint = \"customer/detail\")]",
      "#[server(prefix = \"/api/fn\", endpoint = \"customer/create\")]",
      "#[server(prefix = \"/api/fn\", endpoint = \"customer/update\")]",
    ].join("\n"),
  );

  writeFileSync(
    path.join(root, "crates", "rustok-customer", "admin", "src", "ui", "leptos.rs"),
    "transport::fetch_bootstrap transport::fetch_customers build_customer_admin_submit_command CustomerAdminDraftInput CustomerAdminSubmitCommandError::EmailRequired CustomerAdminSubmitCommandError::LocaleUnavailable",
  );

  writeFileSync(
    path.join(root, "crates", "rustok-customer", "admin", "README.md"),
    "admin/src/core.rs submit-command admin/src/transport/mod.rs admin/src/transport/native_server_adapter.rs admin/src/ui/leptos.rs",
  );


  writeFileSync(
    path.join(root, "crates", "rustok-region", "storefront", "src", "core.rs"),
    [
      "pub enum RegionErrorStatusCode { NativeUnavailable, FallbackUnavailable }",
      "pub struct RegionErrorStatusDescriptor { pub stable_code: &'static str, pub locale_key: &'static str }",
      "const REGION_ERROR_STATUS_DESCRIPTORS: [RegionErrorStatusDescriptor; 2] = [",
      "  RegionErrorStatusDescriptor { stable_code: \"native_unavailable\", locale_key: \"region.error.status.nativeUnavailable\" },",
      "  RegionErrorStatusDescriptor { stable_code: \"fallback_unavailable\", locale_key: \"region.error.status.fallbackUnavailable\" },",
      "];",
      "pub const SELECTED_REGION_QUERY_KEY: &str = \"region\";",
      "pub struct RegionRouteState;",
      "pub struct RegionRouteSelectionUpdate;",
      "fn _uses_variants() { let _ = RegionErrorStatusCode::NativeUnavailable; let _ = RegionErrorStatusCode::FallbackUnavailable; }",
    ].join("\n"),
  );

  writeFileSync(
    path.join(root, "crates", "rustok-region", "storefront", "src", "ui", "leptos.rs"),
    "data-region-error-status data-region-error-locale-key data-region-route-query-key data-region-route-query-value",
  );

  writeFileSync(
    path.join(root, "crates", "rustok-region", "storefront", "README.md"),
    [
      "native_unavailable region.error.status.nativeUnavailable",
      "fallback_unavailable region.error.status.fallbackUnavailable",
      "data-region-error-status data-region-error-locale-key data-region-route-query-key data-region-route-query-value",
      "RegionRouteState RegionRouteSelectionUpdate SELECTED_REGION_QUERY_KEY",
    ].join("\n"),
  );

  ["en", "ru"].forEach((locale) => {
    writeFileSync(
      path.join(root, "crates", "rustok-region", "storefront", "locales", `${locale}.json`),
      JSON.stringify({
        "region.error.status.nativeUnavailable": "Native unavailable",
        "region.error.status.fallbackUnavailable": "Fallback unavailable",
      }),
    );
  });

  writeFileSync(
    path.join(root, "package.json"),
    JSON.stringify(
      {
        scripts: {
          "verify:ffa:ui:migration": pipeline,
          "verify:ffa:ui:migration:contract":
            contractCommand ?? "node scripts/verify/verify-ffa-ui-migration-contract.mjs",
          "verify:ffa:ui:migration:docs":
            docsCommand ?? "bash scripts/verify/verify-ffa-ui-doc-patterns.sh",
          "verify:channel:admin-boundary":
            "node scripts/verify/verify-channel-admin-boundary.mjs",
        },
      },
      null,
      2,
    ),
  );

  return {
    root,
    cleanup: () => rmSync(root, { recursive: true, force: true }),
  };
}

function runVerifier(root, options = {}) {
  const args = [scriptPath];
  if (options.rootArgMode === "equals") {
    args.push(`--root=${root}`);
  }
  if (options.rootArgMode === "separate") {
    args.push("--root", root);
  }

  return spawnSync(process.execPath, args, {
    env: options.rootArgMode ? process.env : { ...process.env, RUSTOK_VERIFY_ROOT: root },
    encoding: "utf8",
  });
}

test("passes when migration pipeline includes contract and docs commands", () => {
  const fixture = withFixture({
    pipeline: "npm run verify:ffa:ui:migration:contract && npm run verify:ffa:ui:migration:docs && npm run verify:channel:admin-boundary",
  });

  try {
    const result = runVerifier(fixture.root);
    assert.equal(result.status, 0, `Expected PASS fixture to succeed:\n${result.stdout}\n${result.stderr}`);
  } finally {
    fixture.cleanup();
  }
});

test("fails when migration pipeline misses docs command", () => {
  const fixture = withFixture({ pipeline: "npm run verify:ffa:ui:migration:contract" });

  try {
    const result = runVerifier(fixture.root);
    assert.notEqual(result.status, 0, "Expected FAIL fixture to fail");
    assert.match(result.stderr, /verify:ffa:ui:migration:docs/);
  } finally {
    fixture.cleanup();
  }
});

test("fails when migration pipeline misses channel boundary command", () => {
  const fixture = withFixture({
    pipeline: "npm run verify:ffa:ui:migration:contract && npm run verify:ffa:ui:migration:docs",
  });

  try {
    const result = runVerifier(fixture.root);
    assert.notEqual(result.status, 0, "Expected FAIL fixture to fail");
    assert.match(result.stderr, /verify:channel:admin-boundary/);
  } finally {
    fixture.cleanup();
  }
});


test("passes when pipeline uses extra whitespace", () => {
  const fixture = withFixture({
    pipeline: "npm   run verify:ffa:ui:migration:contract   &&   npm run verify:ffa:ui:migration:docs   &&   npm   run verify:channel:admin-boundary",
  });

  try {
    const result = runVerifier(fixture.root);
    assert.equal(result.status, 0, `Expected whitespace-tolerant fixture to succeed:
${result.stdout}
${result.stderr}`);
  } finally {
    fixture.cleanup();
  }
});

test("fails when contract script command is drifted", () => {
  const fixture = withFixture({
    pipeline: "npm run verify:ffa:ui:migration:contract && npm run verify:ffa:ui:migration:docs && npm run verify:channel:admin-boundary",
    contractCommand: "node scripts/verify/some-other-command.mjs",
  });

  try {
    const result = runVerifier(fixture.root);
    assert.notEqual(result.status, 0, "Expected drifted contract command fixture to fail");
    assert.match(result.stderr, /должен быть одним из/);
  } finally {
    fixture.cleanup();
  }
});


test("fails when registry structural shape drifts from local module plan", () => {
  const fixture = withFixture({
    pipeline: "npm run verify:ffa:ui:migration:contract && npm run verify:ffa:ui:migration:docs && npm run verify:channel:admin-boundary",
    registryShape: "core_transport_ui",
    localShape: "core_only",
  });

  try {
    const result = runVerifier(fixture.root);
    assert.notEqual(result.status, 0, "Expected local structural shape drift fixture to fail");
    assert.match(result.stderr, /Structural shape: `core_transport_ui`/);
  } finally {
    fixture.cleanup();
  }
});

test("fails when structural shape has no matching code layout", () => {
  const fixture = withFixture({
    pipeline: "npm run verify:ffa:ui:migration:contract && npm run verify:ffa:ui:migration:docs && npm run verify:channel:admin-boundary",
    registryShape: "core_transport_ui",
    localShape: "core_transport_ui",
  });

  try {
    rmSync(path.join(fixture.root, "crates", "rustok-cart", "storefront", "src", "ui"), { recursive: true, force: true });
    const result = runVerifier(fixture.root);
    assert.notEqual(result.status, 0, "Expected missing ui adapter fixture to fail");
    assert.match(result.stderr, /требует ui\/leptos\.rs или ui\/leptos\//);
  } finally {
    fixture.cleanup();
  }
});

test("passes when a temporary single-adapter native transport is documented as native.rs", () => {
  const fixture = withFixture({
    pipeline: "npm run verify:ffa:ui:migration:contract && npm run verify:ffa:ui:migration:docs && npm run verify:channel:admin-boundary",
    registryShape: "core_transport_ui",
    localShape: "core_transport_ui",
  });

  try {
    rmSync(path.join(fixture.root, "crates", "rustok-cart", "storefront", "src", "transport"), {
      recursive: true,
      force: true,
    });
    writeFileSync(
      path.join(fixture.root, "crates", "rustok-cart", "storefront", "src", "native.rs"),
      "// single-adapter native transport fixture\n",
    );
    const result = runVerifier(fixture.root);
    assert.equal(result.status, 0, `Expected native.rs transport fixture to succeed:
${result.stdout}
${result.stderr}`);
  } finally {
    fixture.cleanup();
  }
});

test("passes when docs script uses sh variant", () => {
  const fixture = withFixture({
    pipeline: "npm run verify:ffa:ui:migration:contract && npm run verify:ffa:ui:migration:docs && npm run verify:channel:admin-boundary",
    docsCommand: "sh scripts/verify/verify-ffa-ui-doc-patterns.sh",
  });

  try {
    const result = runVerifier(fixture.root);
    assert.equal(result.status, 0, `Expected sh docs command fixture to succeed:
${result.stdout}
${result.stderr}`);
  } finally {
    fixture.cleanup();
  }
});


test("passes when root is provided via --root argument", () => {
  const fixture = withFixture({
    pipeline: "npm run verify:ffa:ui:migration:contract && npm run verify:ffa:ui:migration:docs && npm run verify:channel:admin-boundary",
  });

  try {
    const result = runVerifier(fixture.root, { rootArgMode: "equals" });
    assert.equal(result.status, 0, `Expected --root fixture to succeed:
${result.stdout}
${result.stderr}`);
  } finally {
    fixture.cleanup();
  }
});


test("passes when root is provided via --root <path> arguments", () => {
  const fixture = withFixture({
    pipeline: "npm run verify:ffa:ui:migration:contract && npm run verify:ffa:ui:migration:docs && npm run verify:channel:admin-boundary",
  });

  try {
    const result = runVerifier(fixture.root, { rootArgMode: "separate" });
    assert.equal(result.status, 0, `Expected --root <path> fixture to succeed:
${result.stdout}
${result.stderr}`);
  } finally {
    fixture.cleanup();
  }
});


test("prints usage for --help", () => {
  const result = spawnSync(process.execPath, [scriptPath, "--help"], {
    encoding: "utf8",
  });

  assert.equal(result.status, 0);
  assert.match(result.stdout, /Usage: node scripts\/verify\/verify-ffa-ui-migration-contract\.mjs/);
});

test("fails on unknown cli arguments", () => {
  const result = spawnSync(process.execPath, [scriptPath, "--unknown-arg"], {
    encoding: "utf8",
  });

  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /Неизвестные аргументы/);
});
