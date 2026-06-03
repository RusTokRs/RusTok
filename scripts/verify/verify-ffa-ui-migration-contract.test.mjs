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
    pipeline: "npm run verify:ffa:ui:migration:contract && npm run verify:ffa:ui:migration:docs",
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


test("passes when pipeline uses extra whitespace", () => {
  const fixture = withFixture({
    pipeline: "npm   run verify:ffa:ui:migration:contract   &&   npm run verify:ffa:ui:migration:docs",
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
    pipeline: "npm run verify:ffa:ui:migration:contract && npm run verify:ffa:ui:migration:docs",
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
    pipeline: "npm run verify:ffa:ui:migration:contract && npm run verify:ffa:ui:migration:docs",
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
    pipeline: "npm run verify:ffa:ui:migration:contract && npm run verify:ffa:ui:migration:docs",
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

test("passes when docs script uses sh variant", () => {
  const fixture = withFixture({
    pipeline: "npm run verify:ffa:ui:migration:contract && npm run verify:ffa:ui:migration:docs",
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
    pipeline: "npm run verify:ffa:ui:migration:contract && npm run verify:ffa:ui:migration:docs",
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
    pipeline: "npm run verify:ffa:ui:migration:contract && npm run verify:ffa:ui:migration:docs",
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
