#!/usr/bin/env node
// RusTok design-system adoption ratchet.
//
// The audit found 1 259 raw `<button|input|select|textarea>` controls against 33
// design-system import sites across the 46 module admin crates, and no gate at
// all: a new module could ship another hundred hand-rolled controls and nothing
// would notice. A "you must not use raw controls" gate cannot be introduced in one
// step (the backlog is measured in thousands of lines), so the rule is a ratchet:
//
//   * `raw` may only go down — a crate that grows its raw controls fails;
//   * `svg` may only go down — the design system owns the icon set (`Icon`), so a new hand-drawn
//     glyph in a host or a module (either `<svg>` in a Leptos view or `svg { … }` in RSX) is the
//     same duplication as a new hand-rolled `<button>`;
//   * `ds` may only go up — a crate that loses its design-system imports fails.
//
// Both halves matter: migrating controls without importing the DS is the same
// duplication under a new name, and deleting an import while keeping the raw
// markup is a silent regression.
//
// Baseline lives in `scripts/verify/ui-ds-adoption-baseline.json` and is only ever
// legitimate to lower; `--update-baseline` rewrites it, and the gate prints the
// direction so a careless update is visible in review.
//
// `--json` prints the census. `--update-baseline` regenerates the baseline file.

import fs from "node:fs";
import path from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = process.env.RUSTOK_VERIFY_REPO_ROOT
  ? path.resolve(process.env.RUSTOK_VERIFY_REPO_ROOT)
  : path.resolve(scriptDir, "../..");
const asJson = process.argv.includes("--json");
const updateBaseline = process.argv.includes("--update-baseline");
const baselinePath = path.join(repoRoot, "scripts/verify/ui-ds-adoption-baseline.json");
const failures = [];

// Raw interactive controls. `DataGrid` itself is allowed to keep its native
// `<button>` (it *is* the table primitive), which is why only module admin crates
// and the host are measured.
const RAW_CONTROL = /<(button|input|select|textarea)\b/g;
// Any use of the design-system Leptos adapter, by import.
const DS_IMPORT = /rustok_ui_leptos::/g;
// Hand-drawn glyphs, which `rustok_ui_leptos::Icon` replaces. Both markup forms count: Leptos
// writes `<svg>` inside `view!`, Dioxus writes `svg { … }` inside `rsx!`, and a metric that read
// only the first form silently reported zero for every Dioxus glyph (the page-builder canvas is
// the one inside a measured root today).
const RAW_SVG = /<svg\b|(?<![\w])svg\s*\{/g;

function collectRustFiles(relativeRoot) {
  const absoluteRoot = path.join(repoRoot, relativeRoot);
  if (!fs.existsSync(absoluteRoot)) return [];
  const files = [];
  const stack = [absoluteRoot];
  while (stack.length > 0) {
    const current = stack.pop();
    for (const entry of fs.readdirSync(current, { withFileTypes: true })) {
      if (entry.name === "target" || entry.name === "node_modules") continue;
      const absolute = path.join(current, entry.name);
      if (entry.isDirectory()) {
        stack.push(absolute);
        continue;
      }
      if (entry.isFile() && entry.name.endsWith(".rs")) {
        files.push(path.relative(repoRoot, absolute));
      }
    }
  }
  return files.sort();
}

function countIn(relativeRoot, pattern) {
  let total = 0;
  for (const file of collectRustFiles(relativeRoot)) {
    const source = fs.readFileSync(path.join(repoRoot, file), "utf8");
    total += (source.match(pattern) ?? []).length;
  }
  return total;
}

function moduleCrates() {
  const modulesRoot = path.join(repoRoot, "crates/modules");
  if (!fs.existsSync(modulesRoot)) return [];
  return fs
    .readdirSync(modulesRoot, { withFileTypes: true })
    .filter((entry) => entry.isDirectory())
    .map((entry) => `crates/modules/${entry.name}/admin`)
    .filter((relativeRoot) => fs.existsSync(path.join(repoRoot, relativeRoot, "src")))
    .sort();
}

const measured = new Map();
for (const crate of moduleCrates()) {
  measured.set(crate, {
    raw: countIn(`${crate}/src`, RAW_CONTROL),
    svg: countIn(`${crate}/src`, RAW_SVG),
    ds: countIn(`${crate}/src`, DS_IMPORT),
  });
}
measured.set("apps/admin", {
  raw: countIn("apps/admin/src", RAW_CONTROL),
  svg: countIn("apps/admin/src", RAW_SVG),
  ds: countIn("apps/admin/src", DS_IMPORT),
});

const census = Object.fromEntries(
  [...measured.entries()].sort(([a], [b]) => a.localeCompare(b)),
);
const totals = [...measured.values()].reduce(
  (sum, entry) => ({
    raw: sum.raw + entry.raw,
    svg: sum.svg + entry.svg,
    ds: sum.ds + entry.ds,
  }),
  { raw: 0, svg: 0, ds: 0 },
);

if (updateBaseline) {
  fs.writeFileSync(
    baselinePath,
    `${JSON.stringify(
      {
        note: "Ratchet baseline: `raw` and `svg` may only go down, `ds` may only go up. Regenerate with --update-baseline after a migration.",
        totals,
        crates: census,
      },
      null,
      2,
    )}\n`,
  );
  console.log(
    `Wrote ${path.relative(repoRoot, baselinePath)}: ${Object.keys(census).length} crate(s), ` +
      `raw ${totals.raw}, svg ${totals.svg}, ds ${totals.ds}.`,
  );
  process.exit(0);
}

if (!fs.existsSync(baselinePath)) {
  console.error("DS adoption ratchet failed: baseline file is missing.");
  console.error(`✗ ${path.relative(repoRoot, baselinePath)} does not exist`);
  process.exit(1);
}

const baseline = JSON.parse(fs.readFileSync(baselinePath, "utf8"));
const baselineCrates = baseline.crates ?? {};

for (const [crate, entry] of Object.entries(census)) {
  const base = baselineCrates[crate];
  if (!base) {
    failures.push(`${crate}: not in the baseline — run --update-baseline after measuring`);
    continue;
  }
  if (entry.raw > base.raw) {
    failures.push(
      `${crate}: raw controls ${base.raw} → ${entry.raw} (+${entry.raw - base.raw}); ` +
        "migrate to the design system instead of adding markup",
    );
  }
  if (entry.svg > base.svg) {
    failures.push(
      `${crate}: hand-drawn <svg> ${base.svg} → ${entry.svg} (+${entry.svg - base.svg}); ` +
        "use rustok_ui_leptos::Icon instead of new geometry",
    );
  }
  if (entry.ds < base.ds) {
    failures.push(
      `${crate}: design-system imports ${base.ds} → ${entry.ds} (${entry.ds - base.ds}); ` +
        "adoption may only grow",
    );
  }
}

for (const crate of Object.keys(baselineCrates)) {
  if (!census[crate]) {
    failures.push(`${crate}: baseline entry has no sources — run --update-baseline`);
  }
}

const improved = Object.entries(census)
  .filter(([crate, entry]) => {
    const base = baselineCrates[crate];
    return base && (entry.raw < base.raw || entry.svg < base.svg || entry.ds > base.ds);
  })
  .map(([crate, entry]) => {
    const base = baselineCrates[crate];
    return `${crate} (raw ${base.raw}→${entry.raw}, svg ${base.svg}→${entry.svg}, ds ${base.ds}→${entry.ds})`;
  });

if (asJson) {
  console.log(
    JSON.stringify({ totals, baselineTotals: baseline.totals ?? null, crates: census, improved }, null, 2),
  );
}

if (failures.length > 0) {
  console.error("DS adoption ratchet failed:");
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(1);
}

const remaining = totals.raw;
console.log(
  `DS adoption ratchet passed: ${Object.keys(census).length} crate(s), ` +
    `${totals.raw} raw control(s) (baseline ${baseline.totals?.raw ?? "?"}), ` +
    `${totals.svg} hand-drawn <svg> (baseline ${baseline.totals?.svg ?? "?"}), ` +
    `${totals.ds} design-system import site(s) (baseline ${baseline.totals?.ds ?? "?"}).` +
    (improved.length > 0
      ? ` Improved since baseline: ${improved.join(", ")} — lower the baseline with --update-baseline.`
      : ` ${remaining} raw control(s) still to migrate.`),
);
