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

function withFixture({ pipeline, contractCommand, docsCommand }) {
  const root = mkdtempSync(path.join(tmpdir(), "rustok-ffa-verify-"));
  mkdirSync(path.join(root, "docs", "research"), { recursive: true });
  mkdirSync(path.join(root, "docs", "verification"), { recursive: true });

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
      "- [ ] Доступ к transport идёт через core ports.",
      "- [ ] Core слой не зависит от `leptos*`.",
      "- [ ] Выполнен `npm run verify:ffa:ui:migration`.",
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

function runVerifier(root) {
  return spawnSync(process.execPath, [scriptPath], {
    env: { ...process.env, RUSTOK_VERIFY_ROOT: root },
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
