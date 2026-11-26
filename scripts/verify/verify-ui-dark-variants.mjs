#!/usr/bin/env node
// RusTok dark-theme pairing guardrail.
//
// The admin ships a light shell and a `data-theme="dark"` shell from the same
// markup, so a palette token (`bg-rose-50`) without a `dark:` counterpart is a
// light-theme assumption baked into a component. The first sweep over the channel
// admin missed exactly that shape: class strings *returned from functions*
// (`resolution_outcome_badge_class`) were never rewritten, because the tooling
// looked at `class="…"` attributes only, and nothing failed until a human read the
// code. This gate reads every Tailwind palette token in the source instead of every
// attribute, so the same class of miss cannot come back.
//
// Scope is an enforced-root list, not the whole repository: ~40 module crates still
// carry unpaired palette classes from before the design-system adoption. Each root
// that has been converted is listed below and must stay at zero findings; the
// remaining roots are reported as a census so the list can grow deliberately.
//
// `--json` prints the census for humans.

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

// Roots that are fully converted: any unpaired palette token here is a regression.
const ENFORCED_ROOTS = ["crates/modules/rustok-channel/admin/src"];

// Roots we still measure, so the enforced list can grow with evidence.
const CENSUS_ROOTS = ["crates/modules", "crates/ui", "apps/admin/src"];

const HUES =
  "rose|emerald|amber|sky|red|green|blue|indigo|violet|purple|pink|teal|lime|orange|yellow|cyan|slate|gray|zinc|neutral|stone";
// `bg|text|border|ring|from|to` + hue + shade, optionally behind `hover:`/`focus:` etc.
const TOKEN = new RegExp(
  String.raw`(?<![\w-])((?:[a-z-]+:)*(?:bg|text|border|ring|from|to)-(?:${HUES})-\d{2,3})(?![\w-])`,
  "g",
);

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

function isInside(relativeFile, relativeRoot) {
  return relativeFile === relativeRoot || relativeFile.startsWith(`${relativeRoot}/`);
}

// `hover:bg-rose-50` and `dark:hover:bg-rose-900/40` are the same pair: strip the
// `dark:` marker and compare the remaining variant chain plus property and hue.
function pairKey(token) {
  const naked = token.startsWith("dark:") ? token.slice("dark:".length) : token;
  const match = /^(?:(?:[a-z-]+:)*)((?:bg|text|border|ring|from|to)-(?:[a-z]+))-\d{2,3}$/.exec(
    naked,
  );
  return match ? match[1] : naked;
}

function scanFile(relativeFile) {
  const source = fs.readFileSync(path.join(repoRoot, relativeFile), "utf8");
  const findings = [];
  for (const [index, line] of source.split("\n").entries()) {
    const tokens = line.match(TOKEN);
    if (!tokens) continue;
    const darkKeys = new Set(
      tokens.filter((token) => token.startsWith("dark:")).map(pairKey),
    );
    const missing = [
      ...new Set(
        tokens
          .filter((token) => !token.startsWith("dark:"))
          .filter((token) => !darkKeys.has(pairKey(token))),
      ),
    ];
    if (missing.length > 0) {
      findings.push({ file: relativeFile, line: index + 1, missing });
    }
  }
  return findings;
}

const enforcedFindings = [];
const census = new Map();
for (const root of CENSUS_ROOTS) census.set(root, 0);

const scannedFiles = new Set();
for (const root of ENFORCED_ROOTS) {
  for (const file of collectRustFiles(root)) {
    if (scannedFiles.has(file)) continue;
    scannedFiles.add(file);
    enforcedFindings.push(...scanFile(file));
  }
}

for (const root of CENSUS_ROOTS) {
  let count = 0;
  for (const file of collectRustFiles(root)) {
    if (scannedFiles.has(file)) continue;
    scannedFiles.add(file);
    count += scanFile(file).length;
  }
  census.set(root, count);
}

for (const finding of enforcedFindings) {
  failures.push(
    `${finding.file}:${finding.line}: palette class without a dark: pair — ${finding.missing.join(", ")}`,
  );
}

const censusEntries = [...census.entries()];
const censusTotal = censusEntries.reduce((sum, [, count]) => sum + count, 0);

if (asJson) {
  console.log(
    JSON.stringify(
      {
        enforcedRoots: ENFORCED_ROOTS,
        enforcedFindings: enforcedFindings.length,
        census: Object.fromEntries(censusEntries),
        censusTotal,
      },
      null,
      2,
    ),
  );
}

if (failures.length > 0) {
  console.error("Dark-theme pairing verification failed:");
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(1);
}

console.log(
  `Dark-theme pairing passed for ${ENFORCED_ROOTS.length} enforced root(s): ` +
    `${ENFORCED_ROOTS.join(", ")} — 0 unpaired palette token(s); ` +
    `census for the rest: ${censusTotal} line(s) across ${censusEntries.length} root(s).`,
);
