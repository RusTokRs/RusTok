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
 * Confines landing CSS to its container with CSS `@scope (<scopeSelector>)` and maps the
 * document-level selectors (`body`, `html`, `:root`) to `:scope`, so styles of a published
 * landing apply to the landing and not to the host shell, header or footer.
 *
 * `@scope` needs Chrome/Edge 118+, Safari 17.4+ or Firefox 128+. A browser without it drops the
 * whole block: the landing renders unstyled there, and still cannot restyle the host.
 *
 * The publish policy on the Rust side already limits what the stylesheet may contain. This
 * function does not rely on that for containment: it tokenizes the stylesheet by the rules of
 * the CSS Syntax specification (comments, strings, escapes, names, numbers, `url()` tokens and
 * `{}`/`()`/`[]` blocks), drops a `}` that would close the wrapper early, and closes whatever
 * the stylesheet left open, so the returned text is always one complete `@scope` block. The
 * result is also safe as the text of a `<style>` element: it never contains `</style`.
 *
 * Not covered: at-rules that define document-wide names (`@keyframes`, `@font-face`,
 * `@property`, …). Where a browser accepts them inside `@scope`, the names stay global.
 *
 * The Leptos storefront has the same function in Rust (`scope_landing_css` in
 * `crates/modules/rustok-page-builder/storefront/src/css_scope.rs`). Both must return the same
 * text for the same input: change them together, and add a case to the shared vectors in
 * `crates/modules/rustok-page-builder/contracts/landing-css-scope-parity.json`.
 */
export function scopeLandingCss(
  rawCss: string,
  scopeSelector: string = ".fly-page-container"
): string {
  const css = trimCssWhitespace(rawCss);
  if (!css) return "";
  // Only this function's own output passes through. A stylesheet that merely starts with
  // `@scope` may scope to something else, or be followed by rules outside the block.
  if (isSingleScopeBlock(css, scopeSelector)) return keepInsideStyleElement(css);

  const { text, open } = scanBlocks(css, true);
  const closers = open
    .map((block) => block.closer)
    .reverse()
    .join("");
  // The newline before the closers matters: a stylesheet may end with a backslash, and
  // backslash-newline is not an escape, so the closers stay structure.
  const completed = closers ? `${text}\n${closers}` : text;
  return keepInsideStyleElement(
    `@scope (${scopeSelector}) {\n  :scope {\n    isolation: isolate;\n  }\n${completed}\n}`
  );
}

/**
 * The stylesheet is emitted as the text of a `<style>` element, which the HTML parser ends at
 * the first `</style`. Escaping the slash keeps that sequence out of the markup: inside a CSS
 * string or `url()` token `\/` is still `/`, and anywhere else `</style` meant nothing to CSS.
 * Only `</style` is touched, so the inserted backslash can never land in front of a `*` and
 * dissolve a comment delimiter.
 */
function keepInsideStyleElement(css: string): string {
  return css.replace(/<\/(?=style)/gi, "<\\/");
}

type BlockKind = "group" | "declaration" | "paren" | "bracket";

interface OpenBlock {
  kind: BlockKind;
  closer: "}" | ")" | "]";
}

interface BlockScan {
  /** The stylesheet with root selectors remapped and stray `}` removed. */
  text: string;
  /** Blocks still open at the end of input, outermost first. */
  open: OpenBlock[];
  /** Index just past the `}` that closed the first top-level `{}` block, or -1. */
  firstBlockEnd: number;
  /** Whether a `}` was found outside every block. */
  strayClose: boolean;
}

/**
 * Walks `css` token by token and tracks blocks the way the CSS syntax specification consumes
 * simple blocks. With `remap`, root selectors in selector position become `:scope`.
 *
 * Tokens are recognised exactly where the tokenizer starts them. That is what decides whether
 * `url(` opens a `url()` token, whose content up to `)` is opaque, or a function, whose
 * content is ordinary structure: `#url(`, `@url(`, `myurl(` and `3url(` are the latter, and
 * `u\72l(` is the former. Guessing it from the surrounding text lets a stylesheet show this
 * scanner one block structure and the browser another.
 */
function scanBlocks(css: string, remap: boolean): BlockScan {
  const open: OpenBlock[] = [];
  let text = "";
  let prelude = "";
  let firstBlockEnd = -1;
  let strayClose = false;

  for (let i = 0; i < css.length; i++) {
    const ch = css[i];

    // Tokens whose content is never structure: copied verbatim, and terminated if the
    // stylesheet ends inside one so that it cannot swallow what is appended after it.
    const opaque = commentOrStringToken(css, i);
    if (opaque) {
      text += css.slice(i, opaque.end) + opaque.terminator;
      i = opaque.end - 1;
      continue;
    }

    // Hash, at-keyword and numeric tokens: a name that is part of one never starts `url(`.
    const plainEnd = hashAtKeywordOrNumberEnd(css, i);
    if (plainEnd > i) {
      const raw = css.slice(i, plainEnd);
      text += raw;
      prelude += raw;
      i = plainEnd - 1;
      continue;
    }

    if (startsIdent(css, i)) {
      const name = consumeName(css, i);
      if (css[name.end] === "(" && name.value.toLowerCase() === "url") {
        const url = unquotedUrlToken(css, name.end + 1);
        if (url) {
          text += css.slice(i, url.end) + url.terminator;
          i = url.end - 1;
          continue;
        }
      }
      const raw = css.slice(i, name.end);
      const isRootElement =
        (raw === "body" || raw === "html") &&
        css[name.end] !== "(" &&
        css[i - 1] !== "." &&
        css[i - 1] !== ":";
      const out = remap && isRootElement && isSelectorPosition(open, prelude) ? ":scope" : raw;
      text += out;
      prelude += out;
      i = name.end - 1;
      continue;
    }

    if (remap && ch === ":" && css[i - 1] !== ":" && isSelectorPosition(open, prelude)) {
      const name = consumeName(css, i + 1);
      if (css.slice(i + 1, name.end) === "root" && css[name.end] !== "(") {
        text += ":scope";
        prelude += ":scope";
        i = name.end - 1;
        continue;
      }
    }

    if (ch === "{" || ch === "(" || ch === "[") {
      if (ch === "{") {
        open.push({
          kind: isCssGroupRule(trimCssWhitespace(prelude)) ? "group" : "declaration",
          closer: "}",
        });
        prelude = "";
      } else {
        open.push(
          ch === "(" ? { kind: "paren", closer: ")" } : { kind: "bracket", closer: "]" }
        );
        prelude += ch;
      }
      text += ch;
      continue;
    }

    if (ch === "}" || ch === ")" || ch === "]") {
      const top = open[open.length - 1];
      if (top && top.closer === ch) {
        open.pop();
        if (ch === "}") {
          prelude = "";
          if (open.length === 0 && firstBlockEnd < 0) firstBlockEnd = i + 1;
        } else {
          prelude += ch;
        }
        text += ch;
      } else if (ch === "}" && !top) {
        // Outside every block a `}` would end the `@scope` wrapper and release the rest of the
        // stylesheet onto the host page.
        strayClose = true;
      } else {
        // A closer that does not match the innermost block is an ordinary token to the parser.
        text += ch;
        if (ch !== "}") prelude += ch;
      }
      continue;
    }

    if (ch === ";" && !isInsideParensOrBrackets(open)) {
      prelude = "";
      text += ch;
      continue;
    }

    prelude += ch;
    text += ch;
  }

  return { text, open, firstBlockEnd, strayClose };
}

interface OpaqueToken {
  /** Index just past the token. */
  end: number;
  /** What closes the token when the input ends inside it, otherwise empty. */
  terminator: string;
}

/** The comment or string token starting at `start`, if any. */
function commentOrStringToken(css: string, start: number): OpaqueToken | null {
  const ch = css[start];

  if (ch === "/" && css[start + 1] === "*") {
    const end = css.indexOf("*/", start + 2);
    return end < 0
      ? { end: css.length, terminator: "*/" }
      : { end: end + 2, terminator: "" };
  }

  if (ch === '"' || ch === "'") {
    for (let i = start + 1; i < css.length; i++) {
      const c = css[i];
      if (c === "\\") {
        // The input ends on a backslash: it would escape a single closing quote.
        if (i + 1 >= css.length) return { end: css.length, terminator: ch + ch };
        // An escaped newline continues the string, and CR LF is one newline.
        i += css[i + 1] === "\r" && css[i + 2] === "\n" ? 2 : 1;
      } else if (c === ch) {
        return { end: i + 1, terminator: "" };
      } else if (isNewline(c)) {
        // An unescaped newline ends the (bad) string; the newline is not part of it.
        return { end: i, terminator: "" };
      }
    }
    return { end: css.length, terminator: ch };
  }

  return null;
}

/**
 * The rest of a `url()` token whose `(` ends just before `start`, or null when a quoted string
 * follows: then `url(` is a function token and its argument an ordinary string token.
 */
function unquotedUrlToken(css: string, start: number): OpaqueToken | null {
  let i = start;
  while (isCssWhitespace(css[i])) i++;
  if (css[i] === '"' || css[i] === "'") return null;

  for (i = start; i < css.length; i++) {
    if (css[i] === "\\") {
      // The input ends on a backslash: it would escape a single closing parenthesis.
      if (i + 1 >= css.length) return { end: css.length, terminator: "))" };
      i++;
    } else if (css[i] === ")") {
      return { end: i + 1, terminator: "" };
    }
  }
  return { end: css.length, terminator: ")" };
}

/**
 * End of the hash (`#name`), at-keyword (`@name`) or numeric token (number, percentage or
 * dimension) starting at `start`; `start` itself when none starts there. An exponent is left
 * to be read as part of the unit, which never changes where a later `url(` token can start.
 */
function hashAtKeywordOrNumberEnd(css: string, start: number): number {
  const ch = css[start];
  if (ch === "#") {
    return isNameChar(css[start + 1]) || isValidEscape(css, start + 1)
      ? consumeName(css, start + 1).end
      : start;
  }
  if (ch === "@") {
    return startsIdent(css, start + 1) ? consumeName(css, start + 1).end : start;
  }
  if (!startsNumber(css, start)) return start;

  let i = start;
  if (css[i] === "+" || css[i] === "-") i++;
  while (isDigit(css[i])) i++;
  if (css[i] === "." && isDigit(css[i + 1])) {
    i += 2;
    while (isDigit(css[i])) i++;
  }
  return startsIdent(css, i) ? consumeName(css, i).end : i;
}

function startsNumber(css: string, start: number): boolean {
  const ch = css[start];
  if (isDigit(ch)) return true;
  if (ch === ".") return isDigit(css[start + 1]);
  if (ch === "+" || ch === "-") {
    return isDigit(css[start + 1]) || (css[start + 1] === "." && isDigit(css[start + 2]));
  }
  return false;
}

/** Whether an ident sequence (a name, possibly written with escapes) starts at `start`. */
function startsIdent(css: string, start: number): boolean {
  const ch = css[start];
  if (ch === "-") {
    const next = css[start + 1];
    return isNameStart(next) || next === "-" || isValidEscape(css, start + 1);
  }
  return isNameStart(ch) || isValidEscape(css, start);
}

interface ConsumedName {
  /** Index just past the name. */
  end: number;
  /** The name with its escapes decoded, as the tokenizer compares it. */
  value: string;
}

function consumeName(css: string, start: number): ConsumedName {
  let i = start;
  let value = "";
  while (i < css.length) {
    if (isNameChar(css[i])) {
      value += css[i];
      i++;
    } else if (isValidEscape(css, i)) {
      const escape = consumeEscape(css, i);
      value += escape.value;
      i = escape.end;
    } else {
      break;
    }
  }
  return { end: i, value };
}

/** Consumes the escape whose backslash is at `start`. */
function consumeEscape(css: string, start: number): ConsumedName {
  let i = start + 1;
  if (i >= css.length) return { end: css.length, value: "\ufffd" };
  if (!isHexDigit(css[i])) return { end: i + 1, value: css.charAt(i) };

  let hex = "";
  while (hex.length < 6 && isHexDigit(css[i])) {
    hex += css[i];
    i++;
  }
  // One whitespace after the digits belongs to the escape; CR LF is one whitespace.
  if (css[i] === "\r" && css[i + 1] === "\n") i += 2;
  else if (isCssWhitespace(css[i])) i++;

  const code = parseInt(hex, 16);
  const isScalar = code > 0 && code <= 0x10ffff && !(code >= 0xd800 && code <= 0xdfff);
  return { end: i, value: isScalar ? String.fromCodePoint(code) : "\ufffd" };
}

/** A backslash starts an escape unless a newline follows it (at the end of input it does). */
function isValidEscape(css: string, start: number): boolean {
  return css[start] === "\\" && !isNewline(css[start + 1]);
}

function isNewline(ch: string | undefined): boolean {
  return ch === "\n" || ch === "\r" || ch === "\f";
}

/** CSS whitespace only: `\s` also matches code points that CSS reads as part of a name. */
function isCssWhitespace(ch: string | undefined): boolean {
  return ch === " " || ch === "\t" || isNewline(ch);
}

/** `String.prototype.trim` removes more: a no-break space, for one, which CSS reads as a name. */
function trimCssWhitespace(value: string): string {
  return value.replace(/^[ \t\n\r\f]+|[ \t\n\r\f]+$/g, "");
}

function isDigit(ch: string | undefined): boolean {
  return ch !== undefined && ch >= "0" && ch <= "9";
}

function isHexDigit(ch: string | undefined): boolean {
  return ch !== undefined && /^[0-9a-fA-F]$/.test(ch);
}

/** Letters, `_`, every non-ASCII code point, and NUL (which CSS replaces with U+FFFD). */
function isNameStart(ch: string | undefined): boolean {
  return ch !== undefined && /^[a-zA-Z_\u0000\u0080-\uffff]$/.test(ch);
}

function isNameChar(ch: string | undefined): boolean {
  return isNameStart(ch) || isDigit(ch) || ch === "-";
}

/**
 * Whether the scan is inside a selector: not in a declaration block, not in an attribute
 * selector, and not in the prelude of an at-rule (`@keyframes body`, `@media …`).
 */
function isSelectorPosition(open: OpenBlock[], prelude: string): boolean {
  return (
    open.every((block) => block.kind === "group" || block.kind === "paren") &&
    !prelude.replace(/^[ \t\n\r\f]+/, "").startsWith("@")
  );
}

function isInsideParensOrBrackets(open: OpenBlock[]): boolean {
  const top = open[open.length - 1];
  return top !== undefined && top.kind !== "group" && top.kind !== "declaration";
}

/** Whether `css` is exactly one complete `@scope (<scopeSelector>) { … }` block. */
function isSingleScopeBlock(css: string, scopeSelector: string): boolean {
  const braceIndex = css.indexOf("{");
  if (braceIndex < 0) return false;
  const collapse = (value: string) => trimCssWhitespace(value.replace(/[ \t\n\r\f]+/g, " "));
  if (collapse(css.slice(0, braceIndex)) !== `@scope (${collapse(scopeSelector)})`) return false;
  const scan = scanBlocks(css, false);
  return scan.open.length === 0 && !scan.strayClose && scan.firstBlockEnd === css.length;
}

function isCssGroupRule(prelude: string): boolean {
  return (
    prelude.startsWith("@media") ||
    prelude.startsWith("@supports") ||
    prelude.startsWith("@container") ||
    prelude.startsWith("@document") ||
    (prelude.startsWith("@layer") && !prelude.includes(";"))
  );
}
