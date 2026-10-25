// Layout-tolerant matching for *code* sources.
//
// A guard pins an invariant, and an invariant is a fact about the code, not about where rustfmt put
// the newline. Exact substring matching over Rust sources couples every gate to layout: a method chain
// continued on the next line, a struct literal that grew a field, or a `cargo fmt` run turns a correct
// implementation red. When that happens often enough the red stops being read, which is the failure
// mode this corpus is already in.
//
// The rules here remove formatting only. Tokens are never touched, so an absent call stays absent and
// no helper invents a file. Deliberate limits:
//   * they do not relax *what* a guard requires — the marker stays in the guard;
//   * prose sources use `verify-text.mjs` (`flattenWhitespace`), because a wrapped sentence in a plan
//     document is a different problem from a wrapped call chain;
//   * a missing file is still reported as missing by the caller, never substituted.
//
// `forum-source-verification.mjs` re-exports these so the Forum corpus keeps one import path.

/**
 * Remove formatting only: a method chain continued on the next line is folded into one line and runs
 * of whitespace collapse.
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
