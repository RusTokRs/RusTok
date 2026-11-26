#!/usr/bin/env node
// Grid adapter parity: the Leptos and Dioxus adapters of `rustok-grid`.
//
// Both `CRATE_API.md` files promise "identical component APIs", and nothing checked it. The audit
// found the promise already false: the Dioxus header stuck (`sticky top-0`) while the Leptos one did
// not (fixed), the two row types apply column widths through different mechanisms, the Dioxus filter
// cell takes a `commit_mode` the Leptos one has no use for (it debounces internally), the Leptos
// header knows whether sorting is possible (`sort_enabled`, the honesty fix from this review), the
// key accessor is spelled differently in each framework's idiom, and only the Leptos grid can sort
// client-side at all.
//
// So the gate compares the *surface* — component names and prop names — and requires a reviewed
// exception, with a reason, wherever the two adapters genuinely differ. A new prop on one side that
// nobody mirrored fails the build; an exception that stopped being true fails it too, so a divergence
// cannot be parked forever by leaving a stale row behind.
//
// `--json` prints both surfaces.

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

const ADAPTERS = {
  leptos: "crates/ui/rustok-grid/leptos/src",
  dioxus: "crates/ui/rustok-grid/dioxus/src",
};

/**
 * Props that exist on one adapter only, and why.
 *
 * Keyed by `Component/prop`. The reason has to say why the other adapter does not need it, not that
 * nobody got round to it — a real gap belongs in the review, not in a smuggle list.
 */
const LEPTOS_ONLY_PROPS = new Map([
  [
    "GridHeader/sort_enabled",
    "The Leptos adapter owns client-side sorting (the core `sort_items` comparator), so its header " +
      "knows whether a click can reorder anything. The Dioxus adapter has not ported that honesty " +
      "fix and still renders sort affordances unconditionally — tracked as an open divergence in " +
      "ui-architecture-review-2026-11-26.md, not as an intended difference.",
  ],
  [
    "DataGrid/key_fn",
    "Same concept as the Dioxus `row_key`, spelled in each framework's idiom: `<For key=…>` needs an " +
      "`Fn(&T) -> K` with `K: Display` and the Leptos adapter keeps that generic, while Dioxus takes " +
      "a `Callback<T, String>`. Renaming forty-odd call sites would buy a shared identifier over a " +
      "still-different type, so the divergence stays named here.",
  ],
  [
    "DataGrid/sort_cell",
    "The Leptos grid sorts its own rows client-side through this `(row column) -> CellValue` " +
      "callback and the core comparator. The Dioxus grid has never had a client-side path: it emits " +
      "`on_sort_change` and leaves `data` untouched, so without a handler its sort buttons change " +
      "nothing — an open divergence, not an intended asymmetry.",
  ],
]);

const DIOXUS_ONLY_PROPS = new Map([
  [
    "GridFilterCell/commit_mode",
    "Dioxus has no debounce helper in its own dependency set, so it cannot publish free-text filters " +
      "on a timer: the caller picks `OnCommit` (default) or `Live`. The Leptos adapter debounces " +
      "internally (`INPUT_DEBOUNCE_MS`) and needs no such prop.",
  ],
  [
    "GridHeader/filter_commit",
    "Only meaningful alongside `commit_mode`: it is the value the header forwards to each filter " +
      "cell, so it cannot exist on an adapter that has no commit mode.",
  ],
  [
    "DataGrid/row_key",
    "The Dioxus half of the key-accessor split documented on `DataGrid/key_fn`: a " +
      "`Callback<T, String>` instead of a generic `Fn(&T) -> K`, because Dioxus has no keyed list " +
      "that would consume a borrowed key.",
  ],
  [
    "DataGrid/filter_commit",
    "Only the Dioxus grid lets the caller choose when free-text filters publish; the Leptos adapter " +
      "publishes after a fixed `INPUT_DEBOUNCE_MS` (250 ms) and exposes no such knob. Porting the " +
      "selector is an open divergence tracked in ui-architecture-review-2026-11-26.md.",
  ],
  [
    "GridRow/column_widths",
    "Widths reach the Leptos row through `<colgroup><col width=…>` under `table-fixed`, because the " +
      "host CSP (`style-src-attr 'none'`) drops per-cell `style` attributes in production; the " +
      "Dioxus row inlines the clamped width per `<td>`. Different mechanism, same rendered geometry.",
  ],
]);

function collectRustFiles(relativeRoot) {
  const absoluteRoot = path.join(repoRoot, relativeRoot);
  if (!fs.existsSync(absoluteRoot)) return [];
  const files = [];
  const stack = [absoluteRoot];
  while (stack.length > 0) {
    const current = stack.pop();
    for (const entry of fs.readdirSync(current, { withFileTypes: true })) {
      const absolute = path.join(current, entry.name);
      if (entry.isDirectory()) {
        if (entry.name === "target") continue;
        stack.push(absolute);
        continue;
      }
      if (entry.isFile() && entry.name.endsWith(".rs")) files.push(absolute);
    }
  }
  return files.sort();
}

/**
 * Text between the `(` of a signature and its matching `)`.
 *
 * Comments and string literals are dropped while counting: a doc comment that mentions
 * `DataGrid(...)` used to shift the depth and truncate the whole prop list, which made the gate
 * compare five props instead of twenty-four and pass happily.
 */
function balanced(source, openIndex) {
  let depth = 1;
  let index = openIndex + 1;
  let out = "";
  while (index < source.length) {
    const character = source[index];
    if (character === "/" && source[index + 1] === "/") {
      const newline = source.indexOf("\n", index);
      index = newline < 0 ? source.length : newline + 1;
      out += "\n";
      continue;
    }
    if (character === "/" && source[index + 1] === "*") {
      const close = source.indexOf("*/", index);
      index = close < 0 ? source.length : close + 2;
      continue;
    }
    if (character === '"') {
      let cursor = index + 1;
      while (cursor < source.length && source[cursor] !== '"') {
        if (source[cursor] === "\\") cursor += 1;
        cursor += 1;
      }
      index = cursor + 1;
      continue;
    }
    if (character === "(") {
      depth += 1;
      out += character;
    } else if (character === ")") {
      depth -= 1;
      if (depth === 0) return out;
      out += character;
    } else {
      out += character;
    }
    index += 1;
  }
  return out;
}

/** Split a parameter list on top-level commas. */
function parameters(signature) {
  const parts = [];
  let depth = 0;
  let current = "";
  let previous = "";
  for (const character of signature) {
    if ("<([{".includes(character)) depth += 1;
    // `-> ` is an arrow, not the closing half of a generic: counting it dropped the depth below
    // zero and split the list at the wrong comma, so a prop typed `Option<Box<dyn Fn() -> T>>`
    // would corrupt every parameter after it.
    else if (">)]}".includes(character) && !(character === ">" && previous === "-")) depth -= 1;
    previous = character;
    if (character === "," && depth === 0) {
      parts.push(current);
      current = "";
      continue;
    }
    current += character;
  }
  parts.push(current);
  return parts;
}

/** Remove `#[…]` attribute blocks, including nested brackets, from a parameter chunk. */
function stripAttributes(text) {
  let out = "";
  let index = 0;
  while (index < text.length) {
    if (text[index] === "#" && text[index + 1] === "[") {
      let depth = 0;
      let cursor = index + 1;
      while (cursor < text.length) {
        if (text[cursor] === "[") depth += 1;
        else if (text[cursor] === "]") {
          depth -= 1;
          if (depth === 0) break;
        }
        cursor += 1;
      }
      index = cursor + 1;
      continue;
    }
    out += text[index];
    index += 1;
  }
  return out;
}

/** Component name → prop names, lexically, from `#[component] pub fn Name(...)`. */
function componentSurfaces(relativeRoot) {
  const surface = new Map();
  for (const file of collectRustFiles(relativeRoot)) {
    const source = fs.readFileSync(file, "utf8");
    // `pub fn Name(` and `pub fn Name<T: …>(` both count: a generic component is still a component,
    // and skipping generics silently dropped `DataGrid` and `GridRow` from the comparison.
    const component = /#\[component\]\s*(?:\/\/[^\n]*\n\s*)*pub fn (\w+)\s*(?:<[^()]*?>)?\s*\(/g;
    let match;
    while ((match = component.exec(source)) !== null) {
      const name = match[1];
      const open = source.lastIndexOf("(", match.index + match[0].length);
      const signature = balanced(source, open);
      const names = [];
      for (const raw of parameters(signature)) {
        // `#[prop(optional)] is_loading: …` keeps the attribute and the parameter on one line, so
        // the attribute is stripped before looking for the name (dropping the whole line silently
        // lost every `#[prop(optional)]` prop, which is most of `DataGrid`).
        const cleaned = stripAttributes(raw);
        const prop = /(?:^|\s|mut\s+)([a-z_][a-z0-9_]*)\s*:(?!:)/.exec(cleaned);
        if (prop) names.push(prop[1]);
      }
      surface.set(name, new Set(names));
    }
  }
  return surface;
}

const leptos = componentSurfaces(ADAPTERS.leptos);
const dioxus = componentSurfaces(ADAPTERS.dioxus);

for (const [label, surface] of [
  ["leptos", leptos],
  ["dioxus", dioxus],
]) {
  if (surface.size === 0) {
    failures.push(`${ADAPTERS[label]}: no #[component] found — the extractor or the adapter layout changed`);
  }
}

for (const name of leptos.keys()) {
  if (!dioxus.has(name)) failures.push(`leptos: component \`${name}\` has no Dioxus twin`);
}
for (const name of dioxus.keys()) {
  if (!leptos.has(name)) failures.push(`dioxus: component \`${name}\` has no Leptos twin`);
}

const pairs = [];
for (const name of [...leptos.keys()].filter((component) => dioxus.has(component)).sort()) {
  const leptosProps = leptos.get(name);
  const dioxusProps = dioxus.get(name);
  pairs.push({
    component: name,
    shared: [...leptosProps].filter((prop) => dioxusProps.has(prop)).sort(),
    leptosOnly: [...leptosProps].filter((prop) => !dioxusProps.has(prop)).sort(),
    dioxusOnly: [...dioxusProps].filter((prop) => !leptosProps.has(prop)).sort(),
  });

  for (const prop of leptosProps) {
    if (dioxusProps.has(prop)) continue;
    const key = `${name}/${prop}`;
    if (LEPTOS_ONLY_PROPS.has(key)) continue;
    failures.push(
      `leptos: \`${name}\` takes \`${prop}\`, which the Dioxus twin does not — mirror it or add a reviewed LEPTOS_ONLY_PROPS row`,
    );
  }
  for (const prop of dioxusProps) {
    if (leptosProps.has(prop)) continue;
    const key = `${name}/${prop}`;
    if (DIOXUS_ONLY_PROPS.has(key)) continue;
    failures.push(
      `dioxus: \`${name}\` takes \`${prop}\`, which the Leptos twin does not — mirror it or add a reviewed DIOXUS_ONLY_PROPS row`,
    );
  }
}

// A stale exception is a finding: it must still describe a real, one-sided prop.
for (const [key, reason] of LEPTOS_ONLY_PROPS) {
  const [component, prop] = key.split("/");
  if (!leptos.get(component)?.has(prop)) {
    failures.push(`LEPTOS_ONLY_PROPS row \`${key}\` names a prop that the Leptos adapter does not take`);
  }
  if (dioxus.get(component)?.has(prop)) {
    failures.push(`LEPTOS_ONLY_PROPS row \`${key}\` is stale: the Dioxus adapter now takes \`${prop}\` too`);
  }
  if (typeof reason !== "string" || reason.trim().length < 30) {
    failures.push(`LEPTOS_ONLY_PROPS row \`${key}\` needs a reason that says why the absence is correct`);
  }
}
for (const [key, reason] of DIOXUS_ONLY_PROPS) {
  const [component, prop] = key.split("/");
  if (!dioxus.get(component)?.has(prop)) {
    failures.push(`DIOXUS_ONLY_PROPS row \`${key}\` names a prop that the Dioxus adapter does not take`);
  }
  if (leptos.get(component)?.has(prop)) {
    failures.push(`DIOXUS_ONLY_PROPS row \`${key}\` is stale: the Leptos adapter now takes \`${prop}\` too`);
  }
  if (typeof reason !== "string" || reason.trim().length < 30) {
    failures.push(`DIOXUS_ONLY_PROPS row \`${key}\` needs a reason that says why the absence is correct`);
  }
}

const sharedProps = pairs.reduce((sum, pair) => sum + pair.shared.length, 0);
const exceptions = LEPTOS_ONLY_PROPS.size + DIOXUS_ONLY_PROPS.size;

if (asJson) {
  console.log(JSON.stringify({ components: pairs, exceptions }, null, 2));
}

if (failures.length > 0) {
  console.error("Grid adapter parity failed:");
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(1);
}

// The summary goes to stderr under `--json` so stdout stays a single JSON document: a summary
// line appended to the payload broke the first consumer of this flag (and then the gate's own
// negative test, which parsed it).
const summary =
  `Grid adapter parity passed: ${pairs.length} component(s) in both adapters, ` +
  `${sharedProps} shared prop(s), ${exceptions} reviewed one-sided prop(s).`;
if (asJson) console.error(summary);
else console.log(summary);
