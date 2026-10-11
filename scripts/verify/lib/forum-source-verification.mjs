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

/**
 * Remove formatting only: a method chain continued on the next line is folded into one line and runs
 * of whitespace collapse. Tokens are never touched, so an absent call stays absent.
 */
export function spaced(text) {
  return text
    .replace(/\r?\n\s*\.\s*/g, ".")
    .replace(/\s+/g, " ")
    .trim();
}

/**
 * A marker is a substring or a regular expression, compared on a layout-normalized copy of the source
 * (and of the marker, so a multi-line marker string in a guard still works).
 */
export function containsMarker(source, marker) {
  if (source === undefined || source === null) return false;
  const haystack = spaced(source);
  return marker instanceof RegExp ? marker.test(haystack) : haystack.includes(spaced(marker));
}

/**
 * Every occurrence index of a marker, so "exactly one" and ordering rules do not have to spell out one
 * formatting of the expression they count.
 */
export function markeredIndexes(source, marker) {
  const haystack = spaced(source);
  if (marker instanceof RegExp) {
    const flags = marker.flags.includes("g") ? marker : new RegExp(marker.source, `${marker.flags}g`);
    const indexes = [];
    for (let match = flags.exec(haystack); match !== null; match = flags.exec(haystack)) {
      indexes.push(match.index);
      if (match.index === flags.lastIndex) flags.lastIndex += 1;
    }
    return indexes;
  }
  const needle = spaced(marker);
  const indexes = [];
  for (let index = haystack.indexOf(needle); index >= 0; index = haystack.indexOf(needle, index + 1)) {
    indexes.push(index);
  }
  return indexes;
}

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
