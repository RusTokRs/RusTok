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

/**
 * Tailwind source-coverage gate.
 *
 * Tailwind only emits a utility when it can *see* a class name while building a
 * host stylesheet, and both hosts configure those scan roots **twice**:
 *
 *   * `@source` in the stylesheet — resolved relative to the stylesheet;
 *   * `content` in the `@config` JavaScript file — resolved relative to the config.
 *
 * A crate that renders markup but is missing from both lists compiles fine,
 * passes every Rust gate, looks right under `trunk serve` (no CSS build runs) and
 * then loses its styling in the built CSS. That is not hypothetical: in this
 * repository `crates/ui/rustok-grid/leptos` was missing from both admin lists, so
 * 14 utilities — `border-collapse`, `align-middle`, `max-h-56` and the grid's
 * focus rings among them — were never generated, for every table in the admin.
 *
 * The gate therefore checks three things:
 *
 *   1. every declared root is covered by the stylesheet's `@source` list;
 *   2. every declared root is covered by the config's `content` list — the two
 *      lists drift silently otherwise, and only one of them may be authoritative
 *      for a given Tailwind version;
 *   3. every class-emitting root that exists is declared by some host, so a new UI
 *      crate cannot be left out of both stylesheets.
 *
 * A root is the directory that owns the rendering sources, not a single file:
 * both mechanisms take globs, and coverage is a property of the directory.
 */

import { existsSync, readdirSync, readFileSync } from "node:fs";
import path from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));

/**
 * Host stylesheets, their `@config` files, and the roots each host renders.
 *
 * The lists are explicit rather than derived from Cargo dependencies on purpose:
 * which crate renders inside which host is a reviewed statement, and a diff to
 * this table is the point of the gate.
 */
const HOSTS = [
  {
    name: "admin",
    css: "apps/admin/input.css",
    config: "apps/admin/tailwind.config.js",
    roots: [
      "apps/admin/src",
      "crates/ui/fly/leptos/src",
      "crates/ui/rustok-grid/leptos/src",
      "crates/ui/rustok-ui/leptos/src",
      "crates/ui/rustok-ui/src",
      "crates/ui/rustok-ui-forms/leptos/src",
    ],
    moduleRoots: ["crates/modules/*/admin/src"],
    // `index.html` is a file, not a root, and both lists carry it.
    extraContent: ["apps/admin/index.html"],
  },
  {
    name: "storefront",
    css: "apps/storefront/assets/input.css",
    config: "apps/storefront/tailwind.config.cjs",
    roots: [
      "apps/storefront/src",
      "crates/ui/fly/leptos/src",
      "crates/ui/rustok-ui/leptos/src",
      "crates/ui/rustok-ui/src",
      "crates/ui/rustok-ui-forms/leptos/src",
    ],
    moduleRoots: ["crates/modules/*/storefront/src"],
    extraContent: ["apps/storefront/assets/input.css"],
  },
];

/** Directories walked when discovering class-emitting roots. */
const DISCOVERY_ROOTS = ["apps/admin/src", "apps/storefront/src", "crates/ui", "crates/modules"];

/**
 * Markup or class-resolver markers.
 *
 * `class=` without surrounding spaces is markup (`class="…"`, `class=full_class`);
 * `let class = …` in plain code is not. Resolver crates (`rustok-ui`) carry no
 * attributes at all — their whole output is a class string — so they are found
 * through the merge helper they are built on.
 */
const CLASS_MARKERS = [/(^|[^A-Za-z0-9_])class=/, /\bmerge_classes\(/];

function parseCliArgs(argv) {
  let cliRoot;
  let showHelp = false;
  const unknownArgs = [];

  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index];
    if (arg === "--help" || arg === "-h") {
      showHelp = true;
      continue;
    }
    if (arg.startsWith("--root=")) {
      cliRoot = arg.slice("--root=".length);
      continue;
    }
    if (arg === "--root") {
      cliRoot = argv[index + 1];
      index += 1;
      continue;
    }
    unknownArgs.push(arg);
  }

  return { cliRoot, showHelp, unknownArgs };
}

function printUsage() {
  console.log(
    "Usage: node scripts/verify/verify-ui-tailwind-sources.mjs [--root <path>|--root=<path>] [-h|--help]",
  );
}

function resolveRepoRoot(cliRoot, env) {
  if (typeof cliRoot === "string" && cliRoot.trim().length > 0) {
    return path.resolve(cliRoot);
  }
  if (env.RUSTOK_VERIFY_ROOT) {
    return path.resolve(env.RUSTOK_VERIFY_ROOT);
  }
  if (env.RUSTOK_VERIFY_REPO_ROOT) {
    return path.resolve(env.RUSTOK_VERIFY_REPO_ROOT);
  }
  return path.resolve(scriptDir, "../..");
}

const cli = parseCliArgs(process.argv.slice(2));
if (cli.showHelp) {
  printUsage();
  process.exit(0);
}
if (cli.unknownArgs.length > 0) {
  console.error("Tailwind source-coverage verification failed:");
  console.error(`✗ Unknown arguments: ${cli.unknownArgs.join(" ")}`);
  printUsage();
  process.exit(1);
}

const repoRoot = resolveRepoRoot(cli.cliRoot, process.env);
const failures = [];

/** Translate one Tailwind content glob into a regular expression. */
function globToRegExp(glob) {
  const parts = glob.split("/");
  let source = "^";

  parts.forEach((part, index) => {
    const last = index === parts.length - 1;
    if (part === "**") {
      // `a/**/b` also matches `a/b`, so a zero-segment match is allowed.
      source += last ? ".*" : "(?:[^/]+/)*";
      return;
    }
    let segment = "";
    for (const char of part) {
      if (char === "*") {
        segment += "[^/]*";
      } else if ("\\^$+.()|{}[]".includes(char)) {
        segment += `\\${char}`;
      } else {
        segment += char;
      }
    }
    source += segment + (last ? "" : "/");
  });

  return new RegExp(`${source}$`);
}

/** Glob patterns from a stylesheet's `@source` list, resolved to repo paths. */
function readSourceGlobs(cssPath, relativeCssPath) {
  const css = readFileSync(cssPath, "utf8");
  const baseDir = path.posix.dirname(relativeCssPath);
  const globs = [];

  for (const match of css.matchAll(/^\s*@source\s+"([^"]+)"\s*;/gm)) {
    const raw = match[1].trim();
    if (raw.length === 0) continue;
    globs.push(path.posix.normalize(path.posix.join(baseDir, raw)));
  }

  return globs;
}

/** Glob patterns from a `@config` JavaScript file's `content` array. */
function readConfigGlobs(configPath, relativeConfigPath) {
  const config = readFileSync(configPath, "utf8");
  const baseDir = path.posix.dirname(relativeConfigPath);
  const arrayMatch = config.match(/content\s*:\s*\[([\s\S]*?)\]/);
  if (!arrayMatch) return [];

  const globs = [];
  for (const match of arrayMatch[1].matchAll(/["'`]([^"'`]+)["'`]/g)) {
    const raw = match[1].trim();
    if (raw.length === 0) continue;
    globs.push(path.posix.normalize(path.posix.join(baseDir, raw)));
  }

  return globs;
}

function listRustFiles(directory) {
  const files = [];
  if (!existsSync(directory)) return files;

  for (const entry of readdirSync(directory, { withFileTypes: true })) {
    const absolute = path.join(directory, entry.name);
    if (entry.isDirectory()) {
      files.push(...listRustFiles(absolute));
    } else if (entry.isFile() && entry.name.endsWith(".rs")) {
      files.push(absolute);
    }
  }

  return files;
}

function emitsClasses(relativeDirectory) {
  const absolute = path.join(repoRoot, relativeDirectory);
  return listRustFiles(absolute).some((file) => {
    const source = readFileSync(file, "utf8");
    return CLASS_MARKERS.some((marker) => marker.test(source));
  });
}

/** Expand a root glob that carries one wildcard segment (one root per module). */
function expandRootGlob(pattern) {
  const segments = pattern.split("/");
  const wildcardIndex = segments.findIndex((segment) => segment.includes("*"));

  if (wildcardIndex === -1) {
    return existsSync(path.join(repoRoot, pattern)) ? [pattern] : [];
  }

  const base = segments.slice(0, wildcardIndex).join("/");
  const tail = segments.slice(wildcardIndex + 1);
  const absoluteBase = path.join(repoRoot, base);
  if (!existsSync(absoluteBase)) return [];

  return readdirSync(absoluteBase, { withFileTypes: true })
    .filter((entry) => entry.isDirectory())
    .map((entry) => [base, entry.name, ...tail].filter(Boolean).join("/"))
    .filter((candidate) => existsSync(path.join(repoRoot, candidate)));
}

/** Every class-emitting root that exists in this checkout. */
function discoverRoots() {
  const candidates = new Set();

  for (const universe of DISCOVERY_ROOTS) {
    if (!existsSync(path.join(repoRoot, universe))) continue;

    if (universe === "crates/ui" || universe === "crates/modules") {
      const absolute = path.join(repoRoot, universe);
      // Module `src/` is domain/server code: it may carry rich-text HTML with
      // BEM class names, which no Tailwind host renders. Only the UI surfaces and
      // the `crates/ui` crates are candidates.
      const surfaces = universe === "crates/ui" ? ["src", "leptos/src"] : ["admin/src", "storefront/src"];
      for (const crate of readdirSync(absolute, { withFileTypes: true })) {
        if (!crate.isDirectory()) continue;
        for (const surface of surfaces) {
          const candidate = `${universe}/${crate.name}/${surface}`;
          if (existsSync(path.join(repoRoot, candidate))) candidates.add(candidate);
        }
      }
    } else {
      candidates.add(universe);
    }
  }

  // `dioxus` renders a desktop/mobile shell: no Tailwind host scans it.
  return [...candidates].filter((root) => !/\/dioxus(\/|$)/.test(root) && emitsClasses(root));
}

const discoveredRoots = discoverRoots();
const discovered = new Set(discoveredRoots);
const declaredEverywhere = new Set();

for (const host of HOSTS) {
  const cssPath = path.join(repoRoot, host.css);
  const configPath = path.join(repoRoot, host.config);

  if (!existsSync(cssPath)) {
    failures.push(`${host.css}: host stylesheet is missing`);
  }
  if (!existsSync(configPath)) {
    failures.push(`${host.config}: host Tailwind config is missing`);
  }
  if (!existsSync(cssPath) || !existsSync(configPath)) continue;

  const sourceGlobs = readSourceGlobs(cssPath, host.css);
  if (sourceGlobs.length === 0) failures.push(`${host.css}: no @source entries found`);
  const sourceMatchers = sourceGlobs.map(globToRegExp);

  const contentGlobs = readConfigGlobs(configPath, host.config);
  if (contentGlobs.length === 0) failures.push(`${host.config}: no content globs found`);
  const contentMatchers = contentGlobs.map(globToRegExp);

  const declaredRoots = [];
  for (const root of host.roots) {
    if (!existsSync(path.join(repoRoot, root))) {
      failures.push(
        `${host.name}: declared source root is missing from the checkout: ${root} ` +
          "(remove it from HOSTS in this gate)",
      );
      continue;
    }
    declaredRoots.push(root);
  }

  for (const modulePattern of host.moduleRoots ?? []) {
    const expanded = expandRootGlob(modulePattern);
    if (expanded.length === 0) {
      failures.push(`${host.name}: module root glob matches no directory: ${modulePattern}`);
    }
    declaredRoots.push(...expanded);
  }

  for (const root of declaredRoots) {
    declaredEverywhere.add(root);
    const probe = `${root}/__tailwind_probe__.rs`;

    if (!sourceMatchers.some((matcher) => matcher.test(probe))) {
      const relative = path.posix.relative(path.posix.dirname(host.css), `${root}/**/*.rs`);
      failures.push(
        `${host.css}: ${root} renders Tailwind classes but @source does not cover it — ` +
          `add @source "${relative}";`,
      );
    }

    if (!contentMatchers.some((matcher) => matcher.test(probe))) {
      const relative = path.posix.relative(path.posix.dirname(host.config), `${root}/**/*.rs`);
      failures.push(
        `${host.config}: ${root} renders Tailwind classes but content does not cover it — ` +
          `add "${relative}" to the content array;`,
      );
    }
  }

  for (const entry of host.extraContent ?? []) {
    if (!existsSync(path.join(repoRoot, entry))) {
      failures.push(`${host.name}: declared extra content entry is missing: ${entry}`);
      continue;
    }
    if (!contentMatchers.some((matcher) => matcher.test(entry))) {
      const relative = path.posix.relative(path.posix.dirname(host.config), entry);
      failures.push(
        `${host.config}: content does not cover ${entry} — add "${relative}";`,
      );
    }
  }
}

// A new class-emitting crate must be declared by some host, otherwise it ships
// unstyled and only a production screenshot would notice.
for (const root of discoveredRoots) {
  if (!declaredEverywhere.has(root)) {
    failures.push(
      `class-emitting root is not declared by any host stylesheet: ${root} ` +
        "(add it to HOSTS in this gate, then to that host's @source and content lists)",
    );
  }
}

if (failures.length > 0) {
  console.error("Tailwind source-coverage verification failed:");
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(1);
}

console.log(
  `Tailwind source coverage is complete: ${HOSTS.length} hosts, ` +
    `${discoveredRoots.length} class-emitting roots, both @source and content in sync.`,
);
