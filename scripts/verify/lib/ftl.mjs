/**
 * Minimal but structurally faithful Project Fluent reader for repository gates.
 *
 * The previous i18n verifiers matched `^id\s*=` line by line, which cannot see
 * attributes (`.aria-label`), terms (`-brand`), multiline patterns, or select
 * expressions. Every gate that compares catalogs now shares this reader so the
 * JS side observes the same structure that `rustok-ui-i18n` validates in Rust.
 *
 * This is deliberately not a full FTL parser: it does not build an AST for
 * placeable expressions. It extracts the entry/attribute skeleton plus the raw
 * pattern text, which is what parity and reference checks need.
 */

const ENTRY_RE = /^(-?)([a-zA-Z][a-zA-Z0-9_-]*)[ \t]*=(.*)$/;
const ATTRIBUTE_RE = /^[ \t]+\.([a-zA-Z][a-zA-Z0-9_-]*)[ \t]*=(.*)$/;
const COMMENT_RE = /^#{1,3}(?:[ \t].*)?$/;

/**
 * @typedef {{ kind: 'message'|'term', id: string, line: number,
 *             value: string|null, attributes: Map<string, string>,
 *             comment: string }} FtlEntry
 */

/**
 * Parses FTL source into entries.
 *
 * @param {string} source
 * @returns {{ entries: FtlEntry[], junkLines: number[] }}
 */
export function parseFtl(source) {
  const lines = source.split(/\r?\n/);
  /** @type {FtlEntry[]} */
  const entries = [];
  const junkLines = [];

  /** @type {FtlEntry|null} */
  let current = null;
  /** @type {string|null} */
  let currentAttribute = null;
  /** Comment lines seen since the last entry, attached to the next one. */
  let pendingComment = [];

  const commit = () => {
    if (!current) return;
    if (current.value !== null && current.value.trim() === "") {
      current.value = null;
    }
    entries.push(current);
    current = null;
    currentAttribute = null;
  };

  for (let index = 0; index < lines.length; index += 1) {
    const line = lines[index];
    const lineNumber = index + 1;

    if (line.trim() === "") {
      commit();
      pendingComment = [];
      continue;
    }

    if (COMMENT_RE.test(line)) {
      commit();
      pendingComment.push(line.replace(/^#{1,3}[ \t]?/, ""));
      continue;
    }

    const attributeMatch = ATTRIBUTE_RE.exec(line);
    if (attributeMatch && current) {
      currentAttribute = attributeMatch[1];
      const previous = current.attributes.get(currentAttribute);
      current.attributes.set(
        currentAttribute,
        previous === undefined
          ? attributeMatch[2]
          : `${previous}\n${attributeMatch[2]}`,
      );
      if (previous !== undefined) {
        current.duplicateAttributes ??= new Set();
        current.duplicateAttributes.add(currentAttribute);
      }
      continue;
    }

    const entryMatch = ENTRY_RE.exec(line);
    if (entryMatch) {
      commit();
      current = {
        kind: entryMatch[1] === "-" ? "term" : "message",
        id: entryMatch[2],
        line: lineNumber,
        value: entryMatch[3],
        attributes: new Map(),
        comment: pendingComment.join("\n"),
      };
      pendingComment = [];
      continue;
    }

    if (current && /^[ \t]+\S/.test(line)) {
      if (currentAttribute) {
        current.attributes.set(
          currentAttribute,
          `${current.attributes.get(currentAttribute)}\n${line}`,
        );
      } else {
        current.value = `${current.value ?? ""}\n${line}`;
      }
      continue;
    }

    commit();
    junkLines.push(lineNumber);
  }

  commit();
  return { entries, junkLines };
}

/** Removes `{"literal"}` placeables so their contents are not scanned. */
function stripStringLiterals(pattern) {
  return pattern.replace(/\{\s*"(?:[^"\\]|\\.)*"\s*\}/g, " ");
}

/**
 * Returns the external variables (`$name`) referenced by a raw pattern.
 *
 * @param {string|null|undefined} pattern
 * @returns {Set<string>}
 */
export function patternVariables(pattern) {
  const variables = new Set();
  if (!pattern) return variables;
  for (const match of stripStringLiterals(pattern).matchAll(
    /\$([a-zA-Z][a-zA-Z0-9_-]*)/g,
  )) {
    variables.add(match[1]);
  }
  return variables;
}

/** Fluent builtins that are function calls rather than message references. */
const FLUENT_BUILTINS = new Set(["NUMBER", "DATETIME"]);

/**
 * Returns message and term references made by a raw pattern.
 *
 * @param {string|null|undefined} pattern
 * @returns {{ messages: Set<string>, terms: Set<string> }}
 */
export function patternReferences(pattern) {
  const messages = new Set();
  const terms = new Set();
  if (!pattern) return { messages, terms };

  const cleaned = stripStringLiterals(pattern);
  for (const match of cleaned.matchAll(/\{\s*-([a-zA-Z][a-zA-Z0-9_-]*)/g)) {
    terms.add(match[1]);
  }
  for (const match of cleaned.matchAll(
    /\{\s*([a-zA-Z][a-zA-Z0-9_-]*)(?:\.[a-zA-Z][a-zA-Z0-9_-]*)?\s*[}\s]/g,
  )) {
    if (FLUENT_BUILTINS.has(match[1])) continue;
    messages.add(match[1]);
  }
  return { messages, terms };
}

/**
 * Returns the selector variables of every select expression in a pattern.
 *
 * `{ $count -> ... }` yields `count`. Selectors that are not plain variable
 * references (for example `{ NUMBER($n) -> ... }`) yield their inner variable
 * so plural parity can still be compared across locales.
 *
 * @param {string|null|undefined} pattern
 * @returns {Set<string>}
 */
export function selectorVariables(pattern) {
  const selectors = new Set();
  if (!pattern) return selectors;
  for (const match of stripStringLiterals(pattern).matchAll(
    /\{\s*([^{}]*?)\s*->/g,
  )) {
    const variable = /\$([a-zA-Z][a-zA-Z0-9_-]*)/.exec(match[1]);
    if (variable) selectors.add(variable[1]);
  }
  return selectors;
}

/**
 * Returns every CLDR plural/ordinal category named by a select expression's
 * variants, keyed by selector variable.
 *
 * @param {string|null|undefined} pattern
 * @returns {Set<string>}
 */
export function variantKeys(pattern) {
  const keys = new Set();
  if (!pattern) return keys;
  for (const match of stripStringLiterals(pattern).matchAll(
    /^\s*\*?\[\s*([a-zA-Z0-9_-]+)\s*\]/gm,
  )) {
    keys.add(match[1]);
  }
  return keys;
}

/** Convenience: every pattern of an entry, value first then attributes. */
export function entryPatterns(entry) {
  const patterns = [];
  if (entry.value !== null) patterns.push({ part: "value", pattern: entry.value });
  for (const [name, pattern] of entry.attributes) {
    patterns.push({ part: `.${name}`, pattern });
  }
  return patterns;
}
