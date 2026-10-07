#!/usr/bin/env node

// Rust constructors that take exactly one argument must be given exactly one.
//
// `Box::new`, `Rc::new` and `Arc::new` are single-argument constructors, so a
// second top-level argument is a compile error that no source-shape gate in this
// repository catches: the mistake reached a pushed branch once already
// (`rustok-commerce` migration registration, where two `::Migration` values were
// passed to one `Box::new`). The compiler catches it in CI, but a gate that names
// the file and the line would have caught it in the pull request.
//
// The scan is deliberately conservative and never guesses twice in the same
// direction: comments and string bodies are blanked first, closure parameter
// lists (`|a, b|`) and generic argument lists (`Foo::<A, B>`) are treated as
// single units, and a trailing comma after the only argument — rustfmt's
// multiline shape — does not count. A `|` only opens a parameter list when it
// closes on the same construct, and a `<` only opens a generic list when it is
// attached to the identifier or path before it and a matching `>` follows, so
// comparisons and bitwise-or arguments keep their commas counted.

import fs from "node:fs";
import path from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const defaultRoot = path.resolve(scriptDir, "../..");
const configuredRoot = process.env.RUSTOK_VERIFY_REPO_ROOT?.trim();
const repoRoot = configuredRoot ? path.resolve(configuredRoot) : defaultRoot;

const CONSTRUCTORS = ["Box::new", "Rc::new", "Arc::new"];
const SCAN_DIRECTORIES = ["crates", "apps", "xtask", "examples", "tests"];
const failures = [];

function collectRustSources(directory, collected) {
  let entries;
  try {
    entries = fs.readdirSync(directory, { withFileTypes: true });
  } catch {
    return collected;
  }
  for (const entry of entries) {
    if (entry.name === "target" || entry.name === "node_modules" || entry.name === ".git") {
      continue;
    }
    const absolute = path.join(directory, entry.name);
    if (entry.isDirectory()) {
      collectRustSources(absolute, collected);
      continue;
    }
    if (entry.isFile() && entry.name.endsWith(".rs")) collected.push(absolute);
  }
  return collected;
}

// Replaces comments and string/char bodies with spaces so that offsets stay
// stable while structural characters inside them stop being parsed.
function blankNonCode(source) {
  const out = source.split("");
  const blank = (from, to) => {
    for (let index = from; index < to && index < out.length; index += 1) {
      if (out[index] !== "\n") out[index] = " ";
    }
  };
  let index = 0;
  while (index < source.length) {
    const char = source[index];
    const next = source[index + 1];
    if (char === "/" && next === "/") {
      let end = source.indexOf("\n", index);
      if (end < 0) end = source.length;
      blank(index, end);
      index = end;
      continue;
    }
    if (char === "/" && next === "*") {
      const start = index;
      let cursor = index + 2;
      let nesting = 1;
      while (cursor < source.length && nesting > 0) {
        if (source[cursor] === "/" && source[cursor + 1] === "*") {
          nesting += 1;
          cursor += 2;
          continue;
        }
        if (source[cursor] === "*" && source[cursor + 1] === "/") {
          nesting -= 1;
          cursor += 2;
          continue;
        }
        cursor += 1;
      }
      blank(start, cursor);
      index = cursor;
      continue;
    }
    if (char === "r" && (next === '"' || next === "#")) {
      let cursor = index + 1;
      let hashes = 0;
      while (source[cursor] === "#") {
        hashes += 1;
        cursor += 1;
      }
      if (source[cursor] === '"') {
        cursor += 1;
        const terminator = `"${"#".repeat(hashes)}`;
        let end = source.indexOf(terminator, cursor);
        if (end < 0) end = source.length;
        else end += terminator.length;
        blank(index, end);
        index = end;
        continue;
      }
    }
    if (char === '"') {
      let cursor = index + 1;
      while (cursor < source.length) {
        if (source[cursor] === "\\") {
          cursor += 2;
          continue;
        }
        if (source[cursor] === '"') {
          cursor += 1;
          break;
        }
        cursor += 1;
      }
      blank(index, cursor);
      index = cursor;
      continue;
    }
    if (char === "'") {
      const closing = source.indexOf("'", index + 1);
      const candidate = closing > 0 ? source.slice(index + 1, closing) : "";
      const isCharLiteral =
        closing > 0 && closing - index <= 4 && !/^[A-Za-z_][A-Za-z0-9_]*$/.test(candidate);
      if (isCharLiteral) {
        blank(index, closing + 1);
        index = closing + 1;
        continue;
      }
    }
    index += 1;
  }
  return out.join("");
}

function previousMeaningful(code, index) {
  let cursor = index - 1;
  while (cursor >= 0 && /\s/.test(code[cursor])) cursor -= 1;
  return cursor;
}

function nextMeaningful(code, index) {
  let cursor = index + 1;
  while (cursor < code.length && /\s/.test(code[cursor])) cursor += 1;
  return cursor;
}

// Index of the `)` that closes the argument list opening at `openParenIndex`,
// counting only brackets. `-1` when the call is unterminated.
function matchingCloseParen(code, openParenIndex) {
  const stack = ["("];
  let cursor = openParenIndex + 1;
  while (cursor < code.length && stack.length > 0) {
    const char = code[cursor];
    if (char === "(" || char === "[" || char === "{") stack.push(char);
    else if (char === ")" || char === "]" || char === "}") stack.pop();
    cursor += 1;
  }
  return stack.length === 0 ? cursor - 1 : -1;
}

// `|a, b|` / `move |a, b|` parameter list opening at `pipeIndex`.
function closureParametersEnd(code, pipeIndex, limit) {
  let cursor = pipeIndex + 1;
  while (cursor < limit) {
    const char = code[cursor];
    if (char === "|") return cursor;
    if (char === "{" || char === "}" || char === "(" || char === ")") return -1;
    cursor += 1;
  }
  return -1;
}

function opensClosureParameters(code, pipeIndex) {
  const before = previousMeaningful(code, pipeIndex);
  if (before < 0) return false;
  const char = code[before];
  if (char === "(" || char === "," || char === ":") return true;
  let wordStart = before;
  while (wordStart >= 0 && /[A-Za-z0-9_]/.test(code[wordStart])) wordStart -= 1;
  const word = code.slice(wordStart + 1, before + 1);
  return word === "move" || word === "async" || word === "return";
}

// `Foo::<A, B>` / `Vec<T>` — a `<` attached to the identifier or path before it
// whose `>` follows before the argument list ends and before any statement
// separator or brace. Comparison operators (`a < b`) and bitwise-or arguments
// therefore stay outside this region and keep their commas counted.
function genericRegionEnd(code, openAngle, limit) {
  const before = openAngle - 1;
  if (before < 0 || /\s/.test(code[before])) return -1;
  if (!/[A-Za-z0-9_:>]/.test(code[before])) return -1;
  const after = nextMeaningful(code, openAngle);
  if (after >= limit || !/[A-Za-z0-9_&'([!*]/.test(code[after])) return -1;
  let depth = 1;
  let cursor = openAngle + 1;
  while (cursor < limit) {
    const char = code[cursor];
    if (char === "<") {
      depth += 1;
      cursor += 1;
      continue;
    }
    if (char === ">") {
      depth -= 1;
      if (depth === 0) return cursor;
      cursor += 1;
      continue;
    }
    if (char === "{" || char === "}" || char === ";" || char === '"') return -1;
    if (char === "(" || char === "[") return -1;
    cursor += 1;
  }
  return -1;
}

// Top-level commas of the argument list, with closure parameter lists and
// generic argument lists collapsed into single units and the rustfmt trailing
// comma ignored.
function topLevelCommas(code, openParenIndex) {
  const closeParen = matchingCloseParen(code, openParenIndex);
  if (closeParen < 0) return [];

  const regions = [];
  for (let cursor = openParenIndex + 1; cursor < closeParen; cursor += 1) {
    const char = code[cursor];
    if (char === "|" && opensClosureParameters(code, cursor)) {
      const end = closureParametersEnd(code, cursor, closeParen);
      if (end > 0) {
        regions.push([cursor, end]);
        cursor = end;
        continue;
      }
    }
    if (char === "<") {
      const end = genericRegionEnd(code, cursor, closeParen);
      if (end > 0) {
        regions.push([cursor, end]);
        cursor = end;
        continue;
      }
    }
  }
  regions.sort((left, right) => left[0] - right[0]);

  const inRegion = (index) => {
    for (const [start, end] of regions) {
      if (index >= start && index <= end) return true;
      if (start > index) break;
    }
    return false;
  };

  const commas = [];
  const stack = [];
  let cursor = openParenIndex + 1;
  while (cursor < closeParen) {
    const char = code[cursor];
    if (inRegion(cursor)) {
      cursor += 1;
      continue;
    }
    if (char === "(" || char === "[" || char === "{") {
      stack.push(char);
      cursor += 1;
      continue;
    }
    if (char === ")" || char === "]" || char === "}") {
      stack.pop();
      cursor += 1;
      continue;
    }
    if (char === "," && stack.length === 0) commas.push(cursor);
    cursor += 1;
  }

  const last = commas.length > 0 ? commas[commas.length - 1] : -1;
  if (last >= 0 && code.slice(last + 1, closeParen).trim() === "") commas.pop();
  return commas;
}

function lineOf(source, offset) {
  let line = 1;
  for (let index = 0; index < offset && index < source.length; index += 1) {
    if (source[index] === "\n") line += 1;
  }
  return line;
}

const sources = [];
for (const directory of SCAN_DIRECTORIES) {
  collectRustSources(path.join(repoRoot, directory), sources);
}
sources.sort();

if (sources.length === 0) {
  console.error(`rust constructor arity verification failed: no Rust sources under ${repoRoot}`);
  process.exit(1);
}

for (const absolute of sources) {
  const relative = path.relative(repoRoot, absolute).split(path.sep).join("/");
  const raw = fs.readFileSync(absolute, "utf8");
  const code = blankNonCode(raw);
  for (const constructor of CONSTRUCTORS) {
    const escaped = constructor.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
    for (const match of code.matchAll(new RegExp(`${escaped}\\s*\\(`, "g"))) {
      const openParen = match.index + match[0].length - 1;
      const commas = topLevelCommas(code, openParen);
      if (commas.length === 0) continue;
      failures.push(
        `${relative}:${lineOf(raw, match.index)}: \`${constructor}\` takes exactly one argument, found ${commas.length + 1}`,
      );
    }
  }
}

if (failures.length > 0) {
  console.error("rust constructor arity verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}
console.log(`rust constructor arity verification passed (${sources.length} files)`);
