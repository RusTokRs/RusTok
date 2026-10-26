// Shared source-reading helpers for the Forum verification guards.
//
// Every function here exists because a guard's *pin* went stale while the rule it encodes did not:
// `rustok-forum` composes owner modules from several `include!` files, `rustok-events` keeps its
// event types in a module directory, and rustfmt re-wraps `impl Trait for Type`, long call chains and
// struct-field accesses at the column limit. A guard that reads one file or one spelling reports those
// layout facts as missing behaviour, which is how a green tree ends up with dozens of red gates and a
// team that stops reading them. Matching is therefore layout-tolerant, while absence of a whole file
// or module stays a finding.
//
// Deliberate limits: nothing here relaxes *what* a guard requires (that stays in the guard), and no
// helper invents a file. A missing source is reported, never substituted.

import { existsSync, readFileSync, readdirSync, statSync } from "node:fs";
import path from "node:path";

const MAX_SOURCE_BYTES = 4 * 1024 * 1024;

// Layout-tolerant marker matching lives in `source-text.mjs` so guards outside the Forum corpus can
// use the same rules without importing a Forum-specific module. Re-exported here to keep the Forum
// guards' existing import path.
import { containsMarker } from "./source-text.mjs";

export { containsMarker, markeredIndexes, spaced } from "./source-text.mjs";

/**
 * Read a source file, or - when the path is a Rust module directory - the concatenation of that module
 * and its submodules. Guards that named `crates/libs/rustok-events/src/types.rs` failed on a module that
 * lives in `src/types/` with `mod.rs` plus siblings, which is a layout fact, not a missing contract.
 *
 * Returns `{ text, files, missing }`; `missing` lists paths that do not exist so the caller can report
 * them with its own message idiom instead of an `ENOENT` thrown before any rule runs.
 */
export function readSource(repoRoot, relativePath) {
  const files = [];
  const missing = [];
  if (!relativePath) {
    return { text: "", files, missing: ["<no path>"] };
  }
  const absolute = path.join(repoRoot, relativePath);
  if (existsSync(absolute) && isReadableFile(absolute)) {
    files.push(relativePath);
    return { text: readFileSync(absolute, "utf8"), files, missing };
  }
  if (absolute.endsWith(".rs")) {
    const moduleDir = absolute.slice(0, -".rs".length);
    if (isDirectory(moduleDir)) {
      for (const name of sortedRustFiles(moduleDir)) {
        files.push(path.relative(repoRoot, path.join(moduleDir, name)).split(path.sep).join("/"));
      }
      if (files.length > 0) {
        return { text: files.map((file) => readFileSync(path.join(repoRoot, file), "utf8")).join("\n"), files, missing };
      }
    }
  }
  return { text: "", files, missing: [relativePath] };
}

/**
 * Read every path given, reporting the ones that are absent rather than throwing.
 */
export function readSources(repoRoot, relativePaths) {
  const parts = [];
  const files = [];
  const missing = [];
  for (const relativePath of relativePaths) {
    const source = readSource(repoRoot, relativePath);
    parts.push(source.text);
    files.push(...source.files);
    missing.push(...source.missing);
  }
  return { text: parts.filter((part) => part.length > 0).join("\n"), files, missing };
}

/**
 * An owner service is what its module composes. `crates/modules/rustok-forum/src/services/mod.rs`
 * declares `mod reply_owner` and stitches the two sibling files `reply_owner.rs` and
 * `reply_owner_inline.rs` into its body,
 * and the moderation decisions live in the siblings, so a rule about "the reply owner" has to read the
 * composition. Falls back to `fallbackRelativePath` when the module is a plain file or a `#[path]`
 * declaration, and reports a finding only when the module is not declared at all.
 *
 * Returns `{ text, files, finding }`.
 */
export function moduleComposition(repoRoot, modRelativePath, moduleName, fallbackRelativePath) {
  const modSource = readSource(repoRoot, modRelativePath);
  if (modSource.missing.length > 0) {
    return {
      text: "",
      files: [],
      finding: `${modRelativePath}: required module index is missing`,
    };
  }
  const declaration = new RegExp(
    `(?:^|\\n)\\s*(?:pub(?:\\([^)]*\\))?\\s+)?mod\\s+${escapeRegExp(moduleName)}\\s*(\\{|;)`,
  );
  const match = modSource.text.match(declaration);
  if (!match) {
    return {
      text: "",
      files: [],
      finding: `${modRelativePath}: module ${moduleName} is not declared`,
    };
  }
  if (match[1] === ";") {
    // A file module or a `#[path]` alias: the fallback path *is* the whole module.
    const fallback = readSource(repoRoot, fallbackRelativePath);
    return { text: fallback.text, files: fallback.files, missing: fallback.missing, finding: undefined };
  }
  const open = modSource.text.indexOf("{", match.index);
  const end = closingBrace(modSource.text, open);
  if (end < 0) {
    return {
      text: "",
      files: [],
      finding: `${modRelativePath}: module ${moduleName} has no balanced body`,
    };
  }
  const directory = path.posix.dirname(modRelativePath);
  const includes = [...modSource.text.slice(open, end).matchAll(/include!\(\s*"([^"]+)"\s*\)/g)].map(
    (entry) => entry[1],
  );
  if (includes.length === 0) {
    const fallback = readSource(repoRoot, fallbackRelativePath);
    return { text: fallback.text, files: fallback.files, missing: fallback.missing, finding: undefined };
  }
  const files = includes.map((included) => path.posix.join(directory, included));
  const owned = fallbackRelativePath && files.includes(fallbackRelativePath) ? [] : [fallbackRelativePath];
  const source = readSources(repoRoot, [...owned, ...files]);
  return { text: source.text, files: source.files, missing: source.missing, finding: undefined };
}

/**
 * Production region of a file: everything before its `#[cfg(test)]` module. Guards that ban a pattern
 * "in production code" must not fire on the test module that exists in the same file, and must not
 * pretend a pattern is absent just because only tests use it - hence a dedicated helper instead of a
 * per-guard slice.
 */
export function productionRegion(text) {
  const tests = text.indexOf("#[cfg(test)]");
  return tests < 0 ? text : text.slice(0, tests);
}

function isReadableFile(absolute) {
  try {
    const stats = statSync(absolute);
    return stats.isFile() && stats.size <= MAX_SOURCE_BYTES;
  } catch {
    return false;
  }
}

function isDirectory(absolute) {
  try {
    return statSync(absolute).isDirectory();
  } catch {
    return false;
  }
}

function sortedRustFiles(directory) {
  return readdirSync(directory)
    .filter((name) => name.endsWith(".rs"))
    .sort((left, right) => {
      // `mod.rs` first so a module's own declarations precede its submodules.
      if (left === "mod.rs") return -1;
      if (right === "mod.rs") return 1;
      return left < right ? -1 : 1;
    });
}

function closingBrace(text, open) {
  let depth = 0;
  for (let index = open; index >= 0 && index < text.length; index += 1) {
    if (text[index] === "{") depth += 1;
    else if (text[index] === "}") {
      depth -= 1;
      if (depth === 0) return index;
    }
  }
  return -1;
}

function escapeRegExp(value) {
  return value.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}

// --- Canonical ledger ---------------------------------------------------------------------------
//
// The Forum plan's delivery ledger is append-only: each slice appears as `### Delivered in
// `FORUM-20AV`` and consecutive slices are collapsed into `### Delivered in `FORUM-20H` through
// `FORUM-20Q``. Guards used to assert the *current* summary row instead (`FORUM-20A-G provide`), so
// every delivered slice turned twenty unrelated guards red at once and each guard pinned a different
// historical frontier. Nineteen of them could never be green together.
//
// These helpers read the append-only headings instead: a range heading covers every slice inside it,
// and the mutable summary row is never matched. A slice counts as delivered exactly when a heading
// names it, which is the property the ledger actually guarantees.

const LETTERS = "ABCDEFGHIJKLMNOPQRSTUVWXYZ";

/** `A` = 1, `Z` = 26, `AA` = 27, `AF` = 32. */
export function taskSuffixNumber(suffix) {
  let value = 0;
  for (const character of suffix) {
    value = value * 26 + (LETTERS.indexOf(character) + 1);
  }
  return value;
}

/** Inverse of `taskSuffixNumber`, used to expand a range's upper bound. */
export function taskSuffixLetters(value) {
  let remaining = value;
  let suffix = "";
  while (remaining > 0) {
    const digit = (remaining - 1) % 26;
    suffix = LETTERS[digit] + suffix;
    remaining = Math.floor((remaining - 1) / 26);
  }
  return suffix;
}

/**
 * Every task id a ledger records as delivered.
 *
 * Two house styles exist and both are append-only: the Forum plan writes
 * `### Delivered in `FORUM-20H` through `FORUM-20Q``, the Notifications plan writes
 * `### `FORUM-20AI / FORUM-20AJ``. Both name the slices they cover, so a single parser can read either
 * ledger and no guard has to know which spelling its owner chose.
 */
export function deliveredLedgerSlices(ledger) {
  const delivered = new Set();
  for (const line of ledger.split("\n")) {
    if (!line.startsWith("### ")) continue;
    const ids = [...line.matchAll(/FORUM-([0-9]+)([A-Z]+)/g)];
    if (ids.length === 0) continue;
    const module = ids[0][1];
    if (ids.length === 2 && /\bthrough\b/.test(line)) {
      const first = taskSuffixNumber(ids[0][2]);
      const last = taskSuffixNumber(ids[1][2]);
      if (ids[1][1] !== module || last < first) {
        delivered.add(`FORUM-${module}${ids[0][2]}`);
        continue;
      }
      for (let value = first; value <= last; value += 1) {
        delivered.add(`FORUM-${module}${taskSuffixLetters(value)}`);
      }
      continue;
    }
    for (const [, idModule, suffix] of ids) delivered.add(`FORUM-${idModule}${suffix}`);
  }
  return delivered;
}

/**
 * Assert that the canonical ledger records every slice the contract requires, and that the contract
 * itself has moved off the historical `pending` state once the ledger advanced past it.
 */
export function assertCanonicalLedger({
  failures,
  plan,
  slices = [],
  markers = [],
  planSync,
  label,
}) {
  if (slices.length > 0 && !plan.includes("| `FORUM-20` |")) {
    failures.push(`${label}: the canonical plan no longer carries the FORUM-20 program-ledger row`);
  }
  const delivered = deliveredLedgerSlices(plan);
  for (const slice of slices) {
    if (!delivered.has(slice)) {
      failures.push(`${label}: the canonical plan does not record ${slice} as delivered`);
    }
  }
  // Content markers are matched layout-tolerantly: a delivered slice's sentence may be re-wrapped, and
  // that is not a missing fact. They must come from an append-only delivered section, never from the
  // mutable summary row - pinning the row is what made nineteen guards mutually unsatisfiable.
  for (const marker of markers) {
    if (!containsMarker(plan, marker)) {
      failures.push(`${label}: the canonical plan does not record ${marker}`);
    }
  }
  if (planSync && planSync.status !== "synchronized") {
    failures.push(
      `${label}: the canonical ledger records these slices as delivered; canonical_plan_sync.status must be synchronized`,
    );
  }
}

/**
 * The owner notes end with a validation disclosure, and every guard used to pin one exact spelling of
 * it (`not run by the implementation agent`, `were not run by`, ...). The sentence differs only in
 * where the note wraps, so the guard checks that the note discloses *that nothing was executed*
 * instead of matching one line break.
 */
export function requireValidationDisclosure(failures, note, label) {
  const flattened = note.replace(/\s+/g, " ");
  // Disclosures are written in several equivalent forms across the owner notes; match the claim,
  // not one sentence layout.
  const disclosed =
    /were not run by the implementation|not run by the implementation|No (tests|compile or runtime status)|None were run by the implementation/.test(
      flattened,
    );
  if (!disclosed) {
    failures.push(`${label}: the note does not disclose that no tests, Cargo, formatting, or CI were run`);
  }
}

/**
 * Prose pins for owner documentation.
 *
 * A guard that pins one exact sentence of a README breaks on a re-wrap, and a guard that pins the
 * *old* sentence breaks when the owner rewrites a paragraph to say the same thing better. Both are
 * layout and revision facts, not contract drift. `requireProse` compares on a whitespace-flattened copy,
 * so only a change of meaning can fail it.
 *
 * The rules for the marker still live in the guard: pin the numbers, the API names and the sentence the
 * owner actually writes today, never sentence scaffolding such as the article that opens it.
 */
export function requireProse(failures, source, marker, label) {
  const haystack = (source ?? "").replace(/\s+/g, " ");
  const needle = marker.replace(/\s+/g, " ");
  if (!haystack.includes(needle)) failures.push(`${label}: missing ${marker}`);
}
