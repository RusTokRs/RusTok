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
 * Scopes landing CSS using modern CSS `@scope (<scope_selector>)` and normalizes
 * root-level selectors (`body`, `html`, `:root`) to `:scope`, ensuring styles from user-generated
 * or GrapesJS landings cannot leak into the host application's shell, header, or footer.
 */
export function scopeLandingCss(
  rawCss: string,
  scopeSelector: string = ".fly-page-container"
): string {
  const trimmed = rawCss.trim();
  if (!trimmed) return "";
  if (trimmed.startsWith("@scope")) return trimmed;

  let result = "";
  const blockStack: ("group" | "declaration")[] = [];
  let inQuote: string | null = null;
  let inComment = false;
  let prelude = "";

  for (let i = 0; i < trimmed.length; i++) {
    const ch = trimmed[i];

    if (inComment) {
      result += ch;
      if (ch === "*" && i + 1 < trimmed.length && trimmed[i + 1] === "/") {
        result += "/";
        i++;
        inComment = false;
      }
      continue;
    }

    if (inQuote) {
      result += ch;
      if (ch === inQuote && (i === 0 || trimmed[i - 1] !== "\\")) {
        inQuote = null;
      }
      continue;
    }

    if (ch === "/" && i + 1 < trimmed.length && trimmed[i + 1] === "*") {
      inComment = true;
      result += "/*";
      i++;
      continue;
    }

    if (ch === '"' || ch === "'") {
      inQuote = ch;
      result += ch;
      continue;
    }

    if (ch === "{") {
      const isGroup = isCssGroupRule(prelude.trim());
      if (isGroup) {
        blockStack.push("group");
      } else {
        blockStack.push("declaration");
      }
      prelude = "";
      result += ch;
      continue;
    } else if (ch === "}") {
      blockStack.pop();
      prelude = "";
      result += ch;
      continue;
    } else if (ch === ";") {
      prelude = "";
      result += ch;
      continue;
    }

    const inDeclaration = blockStack[blockStack.length - 1] === "declaration";

    if (!inDeclaration) {
      const rest = trimmed.slice(i);
      if (rest.startsWith("body") && isIdentBoundary(trimmed, i, 4)) {
        result += ":scope";
        prelude += ":scope";
        i += 3;
        continue;
      } else if (rest.startsWith("html") && isIdentBoundary(trimmed, i, 4)) {
        result += ":scope";
        prelude += ":scope";
        i += 3;
        continue;
      } else if (rest.startsWith(":root") && isRootBoundary(trimmed, i, 5)) {
        result += ":scope";
        prelude += ":scope";
        i += 4;
        continue;
      }
    }

    prelude += ch;
    result += ch;
  }

  return `@scope (${scopeSelector}) {\n  :scope {\n    isolation: isolate;\n  }\n${result}\n}`;
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

function isRootBoundary(str: string, start: number, len: number): boolean {
  if (start > 0) {
    const prev = str[start - 1];
    if (/[a-zA-Z0-9_\-:]/.test(prev)) {
      return false;
    }
  }
  const end = start + len;
  if (end < str.length) {
    const next = str[end];
    if (/[a-zA-Z0-9_\-]/.test(next)) {
      return false;
    }
  }
  return true;
}

function isIdentBoundary(str: string, start: number, len: number): boolean {
  if (start > 0) {
    const prev = str[start - 1];
    if (/[a-zA-Z0-9_\-.#:]/.test(prev)) {
      return false;
    }
  }
  const end = start + len;
  if (end < str.length) {
    const next = str[end];
    if (/[a-zA-Z0-9_\-]/.test(next)) {
      return false;
    }
  }
  return true;
}
