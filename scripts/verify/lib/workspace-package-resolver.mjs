/**
 * Node resolver hook for verifier harnesses that execute a workspace package.
 *
 * A Next app consumes `@rustok/*` packages through `file:` dependencies, so their sources import
 * each other by package name and without file extensions — Node cannot resolve either on its own.
 * Installing anything to test that code would defeat the point, so this helper writes a tiny
 * resolver hook that maps the package name (and its subpaths) to the workspace sources and appends
 * `.ts` / `/index.ts` to extensionless relative imports inside the package.
 *
 * Usage:
 *   const register = writeWorkspacePackageResolver(tmpDir, {
 *     packageName: "@rustok/ui-grid",
 *     packageRoot: path.join(repoRoot, "packages/rustok-ui-grid"),
 *   });
 *   spawnSync(process.execPath, ["--experimental-strip-types", "--import", register, harness]);
 */

import { writeFileSync } from "node:fs";
import path from "node:path";

const RESOLVER_SOURCE = `import { existsSync } from "node:fs";
import { fileURLToPath, pathToFileURL } from "node:url";

const packageName = {packageName};
const packageRoot = {packageRoot};
const entry = {entry};
const base = pathToFileURL(packageRoot + "/");

function firstExisting(urls) {
  for (const url of urls) {
    if (existsSync(fileURLToPath(url))) return url;
  }
  return urls[0];
}

export async function resolve(specifier, context, next) {
  if (specifier === packageName) {
    return { url: entry, shortCircuit: true };
  }
  if (specifier.startsWith(packageName + "/")) {
    const name = specifier.slice(packageName.length + 1);
    return {
      url: firstExisting([
        new URL("src/" + name + ".ts", base).href,
        new URL("src/" + name + "/index.ts", base).href,
      ]),
      shortCircuit: true,
    };
  }
  if (
    (specifier.startsWith("./") || specifier.startsWith("../")) &&
    context.parentURL &&
    context.parentURL.startsWith(base.href)
  ) {
    const parent = new URL(context.parentURL);
    const target = new URL(specifier, parent);
    return {
      url: firstExisting([
        target.href,
        new URL(specifier + ".ts", parent).href,
        new URL(specifier + "/index.ts", parent).href,
      ]),
      shortCircuit: true,
    };
  }
  return next(specifier, context);
}
`;

/**
 * Writes the resolver and its registration file into `directory` and returns the path to pass to
 * `node --import`.
 */
export function writeWorkspacePackageResolver(
  directory,
  { packageName, packageRoot, entry = "src/index.ts", id = "workspace-package" },
) {
  const entryUrl = `new URL(${JSON.stringify(entry)}, pathToFileURL(packageRoot + "/")).href`;
  const resolverPath = path.join(directory, `${id}-resolver.mjs`);
  writeFileSync(
    resolverPath,
    RESOLVER_SOURCE.replace("{packageName}", JSON.stringify(packageName))
      .replace("{packageRoot}", JSON.stringify(packageRoot))
      .replace("{entry}", entryUrl),
  );

  const registerPath = path.join(directory, `${id}-register.mjs`);
  writeFileSync(
    registerPath,
    [
      'import { register } from "node:module";',
      `register(${JSON.stringify(`./${id}-resolver.mjs`)}, import.meta.url);`,
      "",
    ].join("\n"),
  );

  return registerPath;
}
