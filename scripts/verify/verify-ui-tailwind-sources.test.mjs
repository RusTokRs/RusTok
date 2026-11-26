#!/usr/bin/env node
/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

import { test } from "node:test";
import assert from "node:assert/strict";
import { mkdtempSync, mkdirSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { spawnSync } from "node:child_process";

const scriptPath = path.resolve("scripts/verify/verify-ui-tailwind-sources.mjs");

/**
 * Mirror of the gate's HOSTS table.
 *
 * It is duplicated on purpose: a root added to the gate without a fixture here
 * fails this test with "declared source root is missing", which is the reminder
 * to extend the fixture — the alternative (importing the gate) would execute it.
 */
const ADMIN_ROOTS = [
  "apps/admin/src",
  "crates/ui/fly/leptos/src",
  "crates/ui/rustok-grid/leptos/src",
  "crates/ui/rustok-ui/leptos/src",
  "crates/ui/rustok-ui/src",
  "crates/ui/rustok-ui-forms/leptos/src",
];
const STOREFRONT_ROOTS = [
  "apps/storefront/src",
  "crates/ui/fly/leptos/src",
  "crates/ui/rustok-ui/leptos/src",
  "crates/ui/rustok-ui/src",
  "crates/ui/rustok-ui-forms/leptos/src",
];

function write(root, relativePath, content) {
  const filePath = path.join(root, relativePath);
  mkdirSync(path.dirname(filePath), { recursive: true });
  writeFileSync(filePath, content);
}

function classesEntry(root) {
  return `${root}/component.rs`;
}

/**
 * Build a fixture tree: one rendering file per declared root, one stylesheet and
 * one config per host, both listing the same globs — the state the real repo is
 * required to be in.
 */
function fixture({
  adminSources = ADMIN_ROOTS,
  adminContent = ADMIN_ROOTS,
  storefrontSources = STOREFRONT_ROOTS,
  storefrontContent = STOREFRONT_ROOTS,
  extraAdminRenderedRoot = null,
} = {}) {
  const root = mkdtempSync(path.join(tmpdir(), "rustok-tailwind-sources-"));

  for (const surface of new Set([...ADMIN_ROOTS, ...STOREFRONT_ROOTS])) {
    write(root, classesEntry(surface), "pub fn view() -> &'static str { \"class=\\\"flex\\\"\" }\n");
  }
  write(root, "apps/admin/index.html", "<html class=\"dark\"></html>\n");
  write(root, "crates/modules/demo/admin/src/view.rs", "pub fn view() -> &'static str { \"class=\\\"flex\\\"\" }\n");
  write(root, "crates/modules/demo/storefront/src/view.rs", "pub fn view() -> &'static str { \"class=\\\"flex\\\"\" }\n");

  if (extraAdminRenderedRoot) {
    write(root, classesEntry(extraAdminRenderedRoot), "pub fn view() -> &'static str { \"class=\\\"grid\\\"\" }\n");
  }

  // `from` is the file the glob is resolved against: stylesheet or config.
  const toGlob = (from, target) =>
    path.posix.relative(path.posix.dirname(from), `${target}/**/*.rs`) || ".";

  const adminSourceLines = [
    '@source "./index.html";',
    '@source "../../crates/**/admin/src/**/*.rs";',
    ...adminSources.map((entry) => `@source "${toGlob("apps/admin/input.css", entry)}";`),
  ];
  write(root, "apps/admin/input.css", ['@import "tailwindcss";', ...adminSourceLines, ""].join("\n"));

  const quoteRelative = (relative) => (relative.startsWith(".") ? relative : `./${relative}`);
  const contentLines = (from, entries) =>
    entries.map((entry) => `    "${quoteRelative(toGlob(from, entry))}",`);

  write(
    root,
    "apps/admin/tailwind.config.js",
    [
      "export default {",
      "  content: [",
      '    "./src/**/*.rs",',
      '    "./index.html",',
      '    "../../crates/**/admin/src/**/*.rs",',
      ...contentLines("apps/admin/tailwind.config.js", adminContent),
      "  ],",
      "};",
      "",
    ].join("\n"),
  );

  const storefrontSourceLines = [
    '@source "./input.css";',
    '@source "../../../crates/modules/*/storefront/src/**/*.rs";',
    ...storefrontSources.map((entry) =>
      `@source "${toGlob("apps/storefront/assets/input.css", entry)}";`,
    ),
  ];
  write(
    root,
    "apps/storefront/assets/input.css",
    ['@import "tailwindcss";', ...storefrontSourceLines, ""].join("\n"),
  );

  write(
    root,
    "apps/storefront/tailwind.config.cjs",
    [
      "module.exports = {",
      "  content: [",
      '    "./src/**/*.rs",',
      '    "./assets/**/*.css",',
      '    "../../crates/modules/*/storefront/src/**/*.rs",',
      ...contentLines("apps/storefront/tailwind.config.cjs", storefrontContent),
      "  ],",
      "};",
      "",
    ].join("\n"),
  );

  return root;
}

function runGate(root) {
  return spawnSync("node", [scriptPath, "--root", root], {
    cwd: path.resolve("."),
    encoding: "utf8",
  });
}

test("passes when both lists cover every rendered surface", () => {
  const root = fixture();
  try {
    const result = runGate(root);
    assert.equal(result.status, 0, `${result.stdout}\n${result.stderr}`);
    assert.match(result.stdout, /Tailwind source coverage is complete/);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("fails when @source misses a root the config content already covers", () => {
  const root = fixture({
    adminSources: ADMIN_ROOTS.filter((entry) => !entry.includes("rustok-grid")),
  });
  try {
    const result = runGate(root);
    assert.equal(result.status, 1);
    assert.match(result.stderr, /apps\/admin\/input\.css: crates\/ui\/rustok-grid\/leptos\/src/);
    assert.match(result.stderr, /add @source "\.\.\/\.\.\/crates\/ui\/rustok-grid\/leptos\/src\/\*\*\/\*\.rs";/);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("fails when content misses a root @source already covers", () => {
  const root = fixture({
    adminContent: ADMIN_ROOTS.filter((entry) => !entry.includes("rustok-grid")),
  });
  try {
    const result = runGate(root);
    assert.equal(result.status, 1);
    assert.match(result.stderr, /apps\/admin\/tailwind\.config\.js: crates\/ui\/rustok-grid\/leptos\/src/);
    assert.match(result.stderr, /add "\.\.\/\.\.\/crates\/ui\/rustok-grid\/leptos\/src\/\*\*\/\*\.rs" to the content array;/);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("fails when a class-emitting root is declared by no host", () => {
  const root = fixture({ extraAdminRenderedRoot: "crates/ui/demo/leptos/src" });
  try {
    const result = runGate(root);
    assert.equal(result.status, 1);
    assert.match(
      result.stderr,
      /class-emitting root is not declared by any host stylesheet: crates\/ui\/demo\/leptos\/src/,
    );
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("reports a stale declaration instead of silently accepting it", () => {
  const root = fixture();
  try {
    rmSync(path.join(root, "crates/ui/fly/leptos"), { recursive: true, force: true });
    const result = runGate(root);
    assert.equal(result.status, 1);
    assert.match(
      result.stderr,
      /declared source root is missing from the checkout: crates\/ui\/fly\/leptos\/src/,
    );
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});
