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
 * Public-surface extraction shared by the grid parity gates.
 *
 * Deliberately lexical rather than a Rust AST: the audit sandbox has no toolchain, and the surface
 * of these two libraries is written in a narrow, documented style (one `pub` item per line, methods
 * inside inherent `impl` blocks). A parse that fails to see an item would also fail the *count*
 * check in `verify-ui-grid-parity.mjs`, which is what stops this extractor from going quietly stale.
 */

/** Strip comments' influence on structure by dropping the test module: tests are not surface. */
function productionSource(source) {
  const index = source.indexOf("#[cfg(test)]");
  return index < 0 ? source : source.slice(0, index);
}

/**
 * Public items of one Rust module.
 *
 * Free items are reported by name; the public methods and associated functions of an inherent
 * `impl` block are reported as `Type::method`, so an adapter implementing a trait method of the same
 * name cannot be mistaken for the core's own API. Trait implementations (`impl X for Y`) are skipped
 * for that reason, and so is anything below `#[cfg(test)]`.
 */
export function rustPublicSymbols(source) {
  const symbols = [];
  let owner = null;
  let ownerDepth = null;
  let skipDepth = null;
  let depth = 0;
  for (const raw of productionSource(source).split("\n")) {
    const line = raw.trim();
    depth += (line.match(/\{/g) || []).length - (line.match(/\}/g) || []).length;

    if (skipDepth !== null) {
      if (depth < skipDepth) skipDepth = null;
      else continue;
    }

    const impl = /^impl\b(.*)$/.exec(line);
    if (impl) {
      if (/\bfor\b/.test(impl[1])) {
        // A trait implementation: its items belong to the trait, not to the grid's own surface.
        skipDepth = depth;
        owner = null;
        ownerDepth = null;
      } else {
        const name = /([A-Za-z0-9_]+)\s*(?:\{)?$/.exec(impl[1].replace(/^<[^>]*>/, "").trim());
        owner = name ? name[1] : null;
        ownerDepth = depth;
      }
    }
    if (ownerDepth !== null && depth < ownerDepth) {
      owner = null;
      ownerDepth = null;
    }

    if (line.startsWith("pub use ") || line.startsWith("pub(crate)") || line.startsWith("impl")) continue;

    const item = /^pub (fn|const|struct|enum|type|trait|static) ([A-Za-z0-9_]+)/.exec(line);
    if (item) {
      const isMethod = owner !== null && depth >= ownerDepth;
      if (item[1] === "fn" || item[1] === "const") {
        symbols.push({ name: isMethod ? `${owner}::${item[2]}` : item[2], kind: "member" });
      } else {
        symbols.push({ name: item[2], kind: "type" });
      }
      continue;
    }
    const member = /^pub (fn|const) ([A-Za-z0-9_]+)/.exec(line);
    if (member && owner !== null && depth >= ownerDepth) {
      symbols.push({ name: `${owner}::${member[2]}`, kind: "member" });
    }
  }
  return symbols;
}

/** Public exports of one TypeScript module (functions, constants and types; index re-exports aside). */
export function tsPublicSymbols(source) {
  const symbols = [];
  for (const raw of source.split("\n")) {
    const line = raw.trim();
    const item =
      /^export (?:declare )?(?:function|const|let) ([A-Za-z0-9_]+)/.exec(line) ||
      /^export (?:type|interface|enum) ([A-Za-z0-9_]+)/.exec(line);
    if (item) symbols.push({ name: item[1], kind: "member" });
  }
  return symbols;
}

export function camelCase(name) {
  return name.replace(/_([a-z0-9])/g, (_, char) => char.toUpperCase());
}

/** `parseSelection` ↔ `parse_selection`: the inverse of `camelCase`, for the other direction. */
export function snakeCase(name) {
  return name.replace(/([a-z0-9])([A-Z])/g, "$1_$2").toLowerCase();
}

/**
 * Names a crate root re-exports from one module (`pub use <module>::…`).
 *
 * The FBA owner crates moved from `pub use ports::*;` to explicit lists: a wildcard re-export makes
 * every future item part of the public API by accident, and several guards in this corpus forbid it
 * (`verify-pricing-read-local-context`, `verify-tax-calculation-local-context`,
 * `verify-inventory-reservation-owner-context`). A gate that keeps *requiring* the wildcard therefore
 * contradicts the policy it is meant to protect, and reports a correct crate as red.
 *
 * `glob` is still reported, because this helper describes what a source does rather than what a style
 * guide prefers; each guard decides which shape it accepts.
 */
export function reExportedNames(source, moduleName) {
  const names = new Set();
  let glob = false;
  const body = source.replace(/\/\/[^\n]*/g, "");
  const pattern = new RegExp(
    `pub use ${moduleName}::(\\*|\\{[^}]*\\}|[A-Za-z0-9_]+)`,
    "g",
  );
  for (const match of body.matchAll(pattern)) {
    const target = match[1];
    if (target === "*") {
      glob = true;
      continue;
    }
    if (!target.startsWith("{")) {
      names.add(target);
      continue;
    }
    for (const raw of target.slice(1, -1).split(",")) {
      const entry = raw.trim();
      if (!entry || entry.startsWith("#")) continue;
      const name = entry.split(/\s+as\s+/).pop().trim();
      if (/^[A-Za-z0-9_]+$/.test(name)) names.add(name);
    }
  }
  return { glob, names: [...names].sort() };
}

/**
 * Check that a crate root publishes the owner ports its registry declares.
 *
 * The invariant is "every port in the registry is reachable from the crate root", not "the crate root
 * contains one particular spelling of the re-export". Returns `{ ok, message }` so a caller can report
 * it with its own failure idiom.
 */
export function requirePortSurface(library, registry, { moduleName = "ports", label = "lib.rs" } = {}) {
  if (!new RegExp(`pub mod ${moduleName}\\s*;`).test(library)) {
    return { ok: false, message: `${label} must declare \`pub mod ${moduleName};\`` };
  }
  const { glob, names } = reExportedNames(library, moduleName);
  const required = (registry?.ports ?? [])
    .map((port) => port?.name)
    .filter((name) => typeof name === "string" && name.length > 0);
  if (required.length === 0 && !glob && names.length === 0) {
    return { ok: false, message: `${label} must re-export the owner ports from \`${moduleName}\`` };
  }
  const exported = new Set(names);
  const missing = glob ? [] : required.filter((name) => !exported.has(name));
  if (missing.length > 0) {
    return {
      ok: false,
      message: `${label} must re-export every registry port: missing ${missing.join(", ")}`,
    };
  }
  return { ok: true };
}
