#!/usr/bin/env node
// RusTok design-token parity guardrail.
//
// The audit found three radius scales in one design system: the tokens said 6/10/14,
// the storefront hard-coded `--radius: 0.875rem`, the admin hard-coded
// `--radius: 0.5rem` (4/6/8 after the Tailwind offsets), and `UI/tokens/base.css`
// — exported to Flutter and TypeScript — was imported but consumed by nothing.
// Two sources of truth is a design defect that no compiler catches, so this gate
// pins the chain instead of the individual files:
//
//   tokens.json  →  UI/tokens/base.css  →  Tailwind `borderRadius` in both hosts
//                →  generated Rust / Dart artifacts
//
// A host may not invent a `--radius` of its own, both hosts must import the token
// layer (otherwise `var(--iu-radius-*)` resolves to nothing and every corner
// silently becomes square), and the generated artifacts must still agree with the
// JSON they claim to be generated from.
//
// `--json` prints the census.

import fs from "node:fs";
import path from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = process.env.RUSTOK_VERIFY_REPO_ROOT
  ? path.resolve(process.env.RUSTOK_VERIFY_REPO_ROOT)
  : path.resolve(scriptDir, "../..");
const asJson = process.argv.includes("--json");
const failures = [];

const read = (relativePath) => fs.readFileSync(path.join(repoRoot, relativePath), "utf8");
const exists = (relativePath) => fs.existsSync(path.join(repoRoot, relativePath));

const HOSTS = [
  {
    name: "admin",
    input: "apps/admin/input.css",
    config: "apps/admin/tailwind.config.js",
  },
  {
    name: "storefront",
    input: "apps/storefront/assets/input.css",
    config: "apps/storefront/tailwind.config.cjs",
  },
];

for (const required of [
  "UI/tokens/tokens.json",
  "UI/tokens/base.css",
  "UI/tokens/rustok_tokens.g.dart",
  "crates/ui/rustok-ui/src/tokens_generated.rs",
  ...HOSTS.flatMap((host) => [host.input, host.config]),
]) {
  if (!exists(required)) failures.push(`${required}: missing`);
}
if (failures.length > 0) {
  console.error("Design-token parity failed:");
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(1);
}

const tokens = JSON.parse(read("UI/tokens/tokens.json"));
const baseCss = read("UI/tokens/base.css");

/** Read a `--iu-*` declaration as its literal value. */
function cssVar(name) {
  const match = new RegExp(`--${name}\\s*:\\s*([^;]+);`).exec(baseCss);
  return match ? match[1].trim() : null;
}

const checked = [];

// 1. base.css must carry exactly the values tokens.json declares.
const radius = tokens.radius ?? {};
for (const step of ["sm", "md", "lg"]) {
  if (radius[step] === undefined) {
    failures.push(`UI/tokens/tokens.json: radius.${step} is missing`);
    continue;
  }
  const declared = cssVar(`iu-radius-${step}`);
  const expected = `${radius[step]}px`;
  if (declared !== expected) {
    failures.push(
      `UI/tokens/base.css: --iu-radius-${step} is ${declared ?? "missing"}, tokens.json says ${expected}`,
    );
    continue;
  }
  checked.push(`radius-${step}=${expected}`);
}

const spacing = tokens.spacing ?? {};
for (const [step, px] of Object.entries(spacing)) {
  const declared = cssVar(`iu-space-${step}`);
  const expected = `${px}px`;
  if (declared !== expected) {
    failures.push(
      `UI/tokens/base.css: --iu-space-${step} is ${declared ?? "missing"}, tokens.json says ${expected}`,
    );
    continue;
  }
  checked.push(`space-${step}=${expected}`);
}

for (const [step, px] of Object.entries(tokens.typography?.fontSize ?? {})) {
  const declared = cssVar(`iu-text-${step}`);
  const expected = `${px}px`;
  if (declared !== expected) {
    failures.push(
      `UI/tokens/base.css: --iu-text-${step} is ${declared ?? "missing"}, tokens.json says ${expected}`,
    );
    continue;
  }
  checked.push(`text-${step}=${expected}`);
}

for (const [step, shadow] of Object.entries(tokens.shadows ?? {})) {
  const declared = cssVar(`iu-shadow-${step}`);
  if (declared !== shadow) {
    failures.push(
      `UI/tokens/base.css: --iu-shadow-${step} is ${declared ?? "missing"}, tokens.json says "${shadow}"`,
    );
    continue;
  }
  checked.push(`shadow-${step}`);
}

// 2. The dark block must match the class the web sets (`dark`), not only the attribute.
const darkSelector = /\[data-theme="dark"\][^{]*\{/.exec(baseCss)?.[0] ?? "";
const selectorLine = baseCss
  .split("\n")
  .slice(0, baseCss.split("\n").indexOf(darkSelector.trim().split("\n")[0] || ""))
  .slice(-3)
  .join("\n");
if (!/\.dark\b/.test(darkSelector + selectorLine)) {
  failures.push(
    "UI/tokens/base.css: the dark block does not target `.dark`, which is what apps/admin/index.html sets",
  );
}

// 3. Every host must import the token layer; otherwise the variables are undefined.
for (const host of HOSTS) {
  const input = read(host.input);
  if (!input.includes("UI/tokens/base.css")) {
    failures.push(
      `${host.input}: does not import UI/tokens/base.css — var(--iu-radius-*) would resolve to nothing`,
    );
  }
  if (/--radius\s*:/.test(input)) {
    failures.push(
      `${host.input}: declares a private --radius scale; the radius scale belongs to UI/tokens/base.css`,
    );
  }

  // 4. The Tailwind radius scale must read the tokens, not a local literal.
  const config = read(host.config);
  for (const step of ["sm", "md", "lg"]) {
    if (!config.includes(`var(--iu-radius-${step})`)) {
      failures.push(
        `${host.config}: borderRadius.${step} does not read var(--iu-radius-${step}) — the token layer would be decorative`,
      );
    }
  }
  if (/borderRadius:[\s\S]{0,240}var\(--radius\)/.test(config)) {
    failures.push(`${host.config}: still derives borderRadius from a local --radius`);
  }
}

// 5. Generated artifacts must agree with the JSON they are generated from.
const rust = read("crates/ui/rustok-ui/src/tokens_generated.rs");
const rustRadius = /pub mod radius_px \{([\s\S]*?)\}/.exec(rust)?.[1] ?? "";
for (const [constant, step] of [
  ["SM", "sm"],
  ["MD", "md"],
  ["LG", "lg"],
]) {
  const value = new RegExp(`const ${constant}: f64 = ([0-9.]+);`).exec(rustRadius)?.[1];
  const expected = Number(radius[step]).toFixed(1);
  if (value !== expected) {
    failures.push(
      `crates/ui/rustok-ui/src/tokens_generated.rs: radius_px::${constant} is ${value ?? "missing"}, tokens.json says ${expected}`,
    );
  }
}

const dart = read("UI/tokens/rustok_tokens.g.dart");
const dartRadius = /class RusTokRadius \{([\s\S]*?)\n\}/.exec(dart)?.[1] ?? "";
for (const step of ["sm", "md", "lg"]) {
  const value = new RegExp(`static const double ${step} = ([0-9.]+);`).exec(dartRadius)?.[1];
  const expected = Number(radius[step]).toFixed(1);
  if (value !== expected) {
    failures.push(
      `UI/tokens/rustok_tokens.g.dart: RusTokRadius.${step} is ${value ?? "missing"}, tokens.json says ${expected}`,
    );
  }
}

const mobileDartPath = "rustok_mobile/packages/app_ui_kit/lib/tokens/rustok_tokens.g.dart";
if (exists(mobileDartPath) && read(mobileDartPath) !== dart) {
  failures.push(`${mobileDartPath}: differs from UI/tokens/rustok_tokens.g.dart — regenerate the tokens`);
}

if (asJson) {
  console.log(
    JSON.stringify(
      { hosts: HOSTS.map((host) => host.name), checked: checked.length, radius, spacing },
      null,
      2,
    ),
  );
}

if (failures.length > 0) {
  console.error("Design-token parity failed:");
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(1);
}

console.log(
  `Design-token parity passed: ${HOSTS.length} host(s) (${HOSTS.map((host) => host.name).join(", ")}), ` +
    `${checked.length} token value(s) in sync between tokens.json, base.css and the generated artifacts; ` +
    `radius scale ${["sm", "md", "lg"].map((step) => `${step}=${radius[step]}px`).join(" ")}.`,
);
