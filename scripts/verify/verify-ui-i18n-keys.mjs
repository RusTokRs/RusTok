#!/usr/bin/env node
/**
 * UI i18n key-inventory gate.
 *
 * Verifies that every statically spelled message key used by Rust UI code
 * exists in the owning package's `en` catalog, and that every statically
 * spelled Fluent attribute exists on that message.
 *
 * Differences from the original implementation, each of which was a real gap:
 *   - packages are discovered by the presence of a catalog, not by assuming a
 *     `crates/modules/<m>/{admin,storefront}` layout, so `apps/*`,
 *     `crates/ui/*` and flat module crates are covered too;
 *   - `#[cfg(test)]` modules are removed by brace matching instead of
 *     truncating the file at the first occurrence, which used to hide every
 *     production call site placed after an inline test module;
 *   - `format(...)`, `format_attribute(...)`, `try_format*(...)`,
 *     `t_for_locale(...)` and `module_t!(...)` are recognised, not only
 *     `t(...)` / `t!(...)`;
 *   - Fluent attributes are validated against the catalog.
 */

import fs from "node:fs";
import path from "node:path";

import { entryPatterns, parseFtl, patternVariables } from "./lib/ftl.mjs";


const workspaceRoot = process.cwd();
const isStrict = process.argv.includes("--strict");
// `--report-missing` emits `package\tkey\tfallback` for every uncataloged
// reference so a catalog can be reconstructed from real call sites.
const reportMissing = process.argv.includes("--report-missing");
const scanRoots = ["apps", "crates", "packages"];
const skippedDirNames = new Set(["node_modules", "target", "dist", "out"]);
const catalogDirNames = new Set(["locales", "messages"]);

/**
 * Ratchet for call sites that still resolve a variable-bearing message through
 * a module-local helper which takes a pre-resolved template string. Each entry
 * is a known legacy site: the gate fails on anything NOT listed, and also fails
 * when a listed entry disappears, so the baseline can only shrink.
 */
const ARGUMENT_BASELINE_PATH = path.join(
  workspaceRoot,
  "scripts",
  "verify",
  "ui-i18n-argument-baseline.json",
);

/** Call shapes and the zero-based argument index that carries the key. */
const FUNCTION_CALLS = new Map([
  ["t", { key: 1, fallback: 2 }],
  ["t_for_locale", { key: 1, fallback: 2 }],
  ["format", { key: 1, fallback: 3, acceptsArguments: true }],
  ["try_format", { key: 1, acceptsArguments: true }],
  ["try_format_with_locale", { key: 1, acceptsArguments: true }],
  ["format_attribute", { key: 1, attribute: 2, fallback: 4, acceptsArguments: true }],
  ["try_format_attribute", { key: 1, attribute: 2, acceptsArguments: true }],
]);

const MACRO_CALLS = new Map([
  ["t", { key: 2, fallback: 3, argumentsFrom: 4 }],
  ["module_t", { key: 1, fallback: 2, argumentsFrom: 3 }],
]);

function walk(dir, onFile) {
  if (!fs.existsSync(dir)) return;
  const stack = [dir];
  while (stack.length > 0) {
    const current = stack.pop();
    for (const entry of fs.readdirSync(current, { withFileTypes: true })) {
      const full = path.join(current, entry.name);
      if (entry.isDirectory()) {
        if (skippedDirNames.has(entry.name) || entry.name.startsWith(".")) continue;
        stack.push(full);
        continue;
      }
      onFile(full, entry.name);
    }
  }
}

/**
 * Removes `#[cfg(test)]`-gated modules by matching braces, keeping the rest of
 * the file intact. Offsets of surviving code are preserved by replacing the
 * removed span with spaces so reported line numbers stay accurate.
 */
function stripTestModules(source) {
  let result = source;
  for (;;) {
    const attributeIndex = result.indexOf("#[cfg(test)]");
    if (attributeIndex < 0) break;

    const braceIndex = result.indexOf("{", attributeIndex);
    if (braceIndex < 0) {
      result = blankSpan(result, attributeIndex, result.length);
      break;
    }

    let depth = 0;
    let end = result.length;
    for (let index = braceIndex; index < result.length; index += 1) {
      const character = result[index];
      if (character === "{") depth += 1;
      else if (character === "}") {
        depth -= 1;
        if (depth === 0) {
          end = index + 1;
          break;
        }
      }
    }
    result = blankSpan(result, attributeIndex, end);
  }
  return result;
}

function blankSpan(source, start, end) {
  const removed = source.slice(start, end).replace(/[^\n]/g, " ");
  return source.slice(0, start) + removed + source.slice(end);
}

/** Splits a call's argument list on top-level commas. */
function splitArguments(source, openParenIndex) {
  const args = [];
  let depth = 0;
  let current = "";
  let index = openParenIndex;
  let inString = false;
  let escaped = false;

  for (; index < source.length; index += 1) {
    const character = source[index];

    if (inString) {
      current += character;
      if (escaped) escaped = false;
      else if (character === "\\") escaped = true;
      else if (character === '"') inString = false;
      continue;
    }

    if (character === '"') {
      inString = true;
      current += character;
      continue;
    }
    if (character === "(" || character === "[" || character === "{") {
      depth += 1;
      if (depth === 1 && character === "(") continue;
      current += character;
      continue;
    }
    if (character === ")" || character === "]" || character === "}") {
      depth -= 1;
      if (depth === 0) {
        args.push(current);
        return { args, endIndex: index };
      }
      current += character;
      continue;
    }
    if (character === "," && depth === 1) {
      args.push(current);
      current = "";
      continue;
    }
    current += character;
  }

  return null;
}

const STRING_LITERAL_RE = /^\s*"((?:[^"\\]|\\.)*)"\s*$/;

function literalOf(argument) {
  if (argument === undefined) return null;
  const match = STRING_LITERAL_RE.exec(argument);
  return match ? match[1] : null;
}

const CALL_RE = /\b([a-zA-Z_][a-zA-Z0-9_]*)\s*(!?)\s*\(/g;

function extractOccurrences(source) {
  const code = stripTestModules(source);
  const occurrences = [];

  CALL_RE.lastIndex = 0;
  let match;
  while ((match = CALL_RE.exec(code)) !== null) {
    const [, name, bang] = match;
    const spec = bang ? MACRO_CALLS.get(name) : FUNCTION_CALLS.get(name);
    if (!spec) continue;

    const openParenIndex = code.indexOf("(", match.index + name.length);
    const parsed = splitArguments(code, openParenIndex);
    if (!parsed) continue;

    const key = literalOf(parsed.args[spec.key]);
    if (key === null) continue;
    if (!key.includes(".") && !key.includes("-")) continue;
    if (!/^[a-zA-Z][a-zA-Z0-9_.-]*$/.test(key)) continue;

    occurrences.push({
      line: code.slice(0, match.index).split("\n").length,
      call: bang ? `${name}!` : name,
      key,
      kebabKey: key.replaceAll(".", "-"),
      attribute:
        spec.attribute === undefined ? null : literalOf(parsed.args[spec.attribute]),
      fallback:
        spec.fallback === undefined ? null : literalOf(parsed.args[spec.fallback]),
      // `t`/`t_for_locale`/`module_t!` have no argument slot at all; `t!` only
      // carries arguments when extra pairs follow the fallback.
      suppliesArguments: spec.argumentsFrom !== undefined
        ? parsed.args.length > spec.argumentsFrom
        : spec.acceptsArguments === true,
    });
  }

  return occurrences;
}

function readCatalog(filePath) {
  const { entries } = parseFtl(fs.readFileSync(filePath, "utf8"));
  const messages = new Map();
  for (const entry of entries) {
    if (entry.kind !== "message") continue;
    const variables = new Set();
    for (const { pattern } of entryPatterns(entry)) {
      for (const variable of patternVariables(pattern)) variables.add(variable);
    }
    messages.set(entry.id, {
      attributes: new Set(entry.attributes.keys()),
      variables,
    });
  }
  return messages;
}

/**
 * A package is any directory that owns a catalog directory containing
 * `en.ftl`. Its Rust sources are the `src` tree next to that catalog.
 */
function discoverPackages() {
  const packages = new Map();

  for (const relativeRoot of scanRoots) {
    walk(path.join(workspaceRoot, relativeRoot), (filePath, fileName) => {
      if (fileName !== "en.ftl") return;
      const catalogDir = path.dirname(filePath);
      if (!catalogDirNames.has(path.basename(catalogDir))) return;

      const packageDir = path.dirname(catalogDir);
      const sourceDir = path.join(packageDir, "src");
      if (!fs.existsSync(sourceDir)) return;

      packages.set(packageDir, {
        name: path.relative(workspaceRoot, packageDir).split(path.sep).slice(-2).join("/"),
        sourceDir,
        catalog: filePath,
        isRustPackage: fs.existsSync(path.join(packageDir, "Cargo.toml")),
      });
    });
  }

  return [...packages.values()].sort((left, right) => left.name.localeCompare(right.name));
}

const argumentBaseline = new Set(
  fs.existsSync(ARGUMENT_BASELINE_PATH)
    ? JSON.parse(fs.readFileSync(ARGUMENT_BASELINE_PATH, "utf8")).sites
    : [],
);
const observedBaseline = new Set();

/**
 * A package that ships a catalog but never loads it is shipping dead weight
 * while its UI hardcodes copy. Both signals are cheap and precise:
 *   - no `declare_module_i18n!` / `UiMessages::new` anywhere under `src`;
 *   - a `let russian = ..` two-language branch in a catalog-owning package.
 */
const CATALOG_LOADERS =
  /declare_module_i18n!|UiMessages::new|LazyUiMessages::new/;
const HARDCODED_LOCALE_BRANCH = /\blet\s+russian\s*=/;

function inspectCatalogWiring(pkg) {
  let loadsCatalog = false;
  let hardcodesLocale = false;
  walk(pkg.sourceDir, (filePath, fileName) => {
    if (!fileName.endsWith(".rs")) return;
    const source = fs.readFileSync(filePath, "utf8");
    if (CATALOG_LOADERS.test(source)) loadsCatalog = true;
    if (HARDCODED_LOCALE_BRANCH.test(source)) hardcodesLocale = true;
  });
  return { loadsCatalog, hardcodesLocale };
}

const packages = discoverPackages();
if (packages.length === 0) {
  console.error("No UI i18n packages discovered.");
  process.exit(1);
}

let hasError = false;
let totalChecked = 0;
let totalMissing = 0;

const wiringBaseline = new Set(
  fs.existsSync(ARGUMENT_BASELINE_PATH)
    ? JSON.parse(fs.readFileSync(ARGUMENT_BASELINE_PATH, "utf8")).unwiredCatalogs ?? []
    : [],
);
const observedWiring = new Set();

for (const pkg of packages) {
  const catalog = readCatalog(pkg.catalog);
  const missing = [];

  // Only Rust packages load catalogs through this crate; the Next apps use
  // @rustok/next-fluent and legitimately have no Rust loader.
  const wiring = pkg.isRustPackage
    ? inspectCatalogWiring(pkg)
    : { loadsCatalog: true, hardcodesLocale: false };
  if (!wiring.loadsCatalog || wiring.hardcodesLocale) {
    const reasons = [];
    if (!wiring.loadsCatalog) {
      reasons.push(
        `ships ${catalog.size} catalog key(s) but never loads them ` +
          `(no declare_module_i18n! / UiMessages::new under src)`,
      );
    }
    if (wiring.hardcodesLocale) {
      reasons.push("hardcodes a two-language `let russian = ..` branch");
    }
    if (wiringBaseline.has(pkg.name)) {
      observedWiring.add(pkg.name);
    } else {
      hasError = true;
      console.error(`FAIL ${pkg.name}: ${reasons.join("; ")}`);
    }
  }

  walk(pkg.sourceDir, (filePath, fileName) => {
    if (!fileName.endsWith(".rs")) return;
    if (fileName === "i18n.rs") return;

    for (const occurrence of extractOccurrences(fs.readFileSync(filePath, "utf8"))) {
      totalChecked += 1;

      const message =
        catalog.get(occurrence.kebabKey) ?? catalog.get(occurrence.key) ?? null;

      if (message === null) {
        missing.push({
          file: path.relative(workspaceRoot, filePath),
          line: occurrence.line,
          detail: `key "${occurrence.key}" (lookup: "${occurrence.kebabKey}")`,
          kebabKey: occurrence.kebabKey,
          fallback: occurrence.fallback,
        });
        continue;
      }

      if (occurrence.attribute !== null && !message.attributes.has(occurrence.attribute)) {
        missing.push({
          file: path.relative(workspaceRoot, filePath),
          line: occurrence.line,
          detail: `attribute "${occurrence.key}.${occurrence.attribute}"`,
        });
        continue;
      }

      // A catalog entry that declares Fluent variables can only be rendered by
      // an argument-bearing API. Calling it through the no-argument `t` family
      // makes every locale fail formatting and silently fall back to the
      // English literal, which is how `String::replace("{slug}", ..)` call
      // sites used to "work".
      if (message.variables.size > 0 && !occurrence.suppliesArguments) {
        const baselineId = `${pkg.name} ${occurrence.key}`;
        if (argumentBaseline.has(baselineId)) {
          observedBaseline.add(baselineId);
          continue;
        }
        missing.push({
          file: path.relative(workspaceRoot, filePath),
          line: occurrence.line,
          detail:
            `"${occurrence.key}" declares Fluent variables ` +
            `[${[...message.variables].sort().join(", ")}] but is resolved through ` +
            `\`${occurrence.call}\` without arguments`,
        });
      }
    }
  });

  if (missing.length === 0) {
    console.log(`OK   ${pkg.name} (${catalog.size} catalog keys)`);
    continue;
  }

  hasError = true;
  totalMissing += missing.length;
  if (reportMissing) {
    const seen = new Set();
    for (const item of missing) {
      if (!item.kebabKey || seen.has(item.kebabKey)) continue;
      seen.add(item.kebabKey);
      process.stdout.write(
        `${pkg.name}\t${item.kebabKey}\t${item.fallback ?? ""}\n`,
      );
    }
    continue;
  }
  console.warn(
    `${isStrict ? "FAIL" : "WARN"} ${pkg.name}: ${missing.length} uncataloged reference(s)`,
  );
  for (const item of missing.slice(0, 10)) {
    console.warn(`  ${item.file}:${item.line} - ${item.detail}`);
  }
  if (missing.length > 10) {
    console.warn(`  ... and ${missing.length - 10} more`);
  }
}

const staleWiring = [...wiringBaseline]
  .filter((entry) => !observedWiring.has(entry))
  .sort();
if (staleWiring.length > 0) {
  hasError = true;
  console.error(
    `\nFAIL stale unwiredCatalogs entries in ${path.relative(workspaceRoot, ARGUMENT_BASELINE_PATH)}:`,
  );
  for (const entry of staleWiring) {
    console.error(`  ${entry} - now wired; delete it from the baseline`);
  }
}

const staleBaseline = [...argumentBaseline]
  .filter((entry) => !observedBaseline.has(entry))
  .sort();

if (staleBaseline.length > 0) {
  hasError = true;
  console.error(
    `\nFAIL stale entries in ${path.relative(workspaceRoot, ARGUMENT_BASELINE_PATH)}:`,
  );
  for (const entry of staleBaseline) {
    console.error(`  ${entry} - fixed or removed; delete it from the baseline`);
  }
}

console.log(
  `\nValidated ${totalChecked} UI key occurrences across ${packages.length} packages. ` +
    `(${totalMissing} uncataloged references, ${observedBaseline.size}/${argumentBaseline.size} ` +
    `baselined legacy interpolation sites, ${observedWiring.size}/${wiringBaseline.size} baselined unwired catalogs)`,
);

if (isStrict && hasError) {
  process.exit(1);
}
