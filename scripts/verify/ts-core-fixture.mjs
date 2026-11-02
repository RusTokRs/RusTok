/*
 * Copyright (c) 2026 RusTokRs.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 */
import { readFileSync } from "node:fs";
import { stripTypeScriptTypes } from "node:module";
import { fileURLToPath } from "node:url";
import vm from "node:vm";

/** Evaluate actual framework-free source without loading a Next/UI barrel. Callers bind the
 * actual imported owner helpers explicitly; this does not mock a resolver or an IO transport.
 * Node 22.13+ is required, as for the repository's existing TypeScript core fixtures.
 */
export function loadTypeScriptCore(file, names, bindings = {}) {
  const source = stripTypeScriptTypes(readFileSync(file, "utf8"), {
    mode: "transform",
  })
    .replace(/^\s*import[\s\S]*?;\s*/gm, "")
    .replace(/^export /gm, "");
  const factory = vm.runInThisContext(
    `(function (${Object.keys(bindings).join(",")}) {\n${source}\nreturn { ${names.join(",")} };\n})`,
    { filename: fileURLToPath(file) },
  );
  return factory(...Object.values(bindings));
}
