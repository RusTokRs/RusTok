#!/usr/bin/env node
// Repository-wide FFA UI boundary sweep.
//
// This is a broad guardrail for every module declared as core_transport_ui in
// docs/modules/registry.md. Module-specific scripts still own deeper checks.

import { existsSync, readdirSync, readFileSync, statSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const failures = [];

function parseCliArgs(argv) {
  let cliRoot;
  let showHelp = false;
  const unknownArgs = [];

  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index];
    if (arg === "--help" || arg === "-h") {
      showHelp = true;
      continue;
    }
    if (arg.startsWith("--root=")) {
      cliRoot = arg.slice("--root=".length);
      continue;
    }
    if (arg === "--root") {
      cliRoot = argv[index + 1];
      index += 1;
      continue;
    }
    unknownArgs.push(arg);
  }

  return { cliRoot, showHelp, unknownArgs };
}

function printUsage() {
  console.log("Usage: node scripts/verify/verify-ffa-ui-boundary-sweep.mjs [--root <path>|--root=<path>] [-h|--help]");
}

function resolveRepoRoot(cliRoot, env) {
  if (typeof cliRoot === "string" && cliRoot.trim().length > 0) {
    return path.resolve(cliRoot);
  }
  if (env.RUSTOK_VERIFY_ROOT) {
    return path.resolve(env.RUSTOK_VERIFY_ROOT);
  }
  if (env.RUSTOK_VERIFY_REPO_ROOT) {
    return path.resolve(env.RUSTOK_VERIFY_REPO_ROOT);
  }
  return path.resolve(scriptDir, "../..");
}

const cli = parseCliArgs(process.argv.slice(2));
if (cli.showHelp) {
  printUsage();
  process.exit(0);
}
if (cli.unknownArgs.length > 0) {
  console.error("FFA UI boundary sweep failed:");
  console.error(`✗ Unknown arguments: ${cli.unknownArgs.join(" ")}`);
  printUsage();
  process.exit(1);
}

const repoRoot = resolveRepoRoot(cli.cliRoot, process.env);

const registryPath = "docs/modules/registry.md";
const packageJsonPath = "package.json";

function repoPath(relativePath) {
  return path.join(repoRoot, relativePath);
}

function readRepo(relativePath) {
  return readFileSync(repoPath(relativePath), "utf8");
}

function fail(message) {
  failures.push(message);
}

function normalizePath(filePath) {
  return path.relative(repoRoot, filePath).replace(/\\/g, "/");
}

function listFiles(directory, predicate = () => true) {
  const result = [];
  if (!existsSync(directory)) {
    return result;
  }

  function walk(current) {
    for (const entry of readdirSync(current, { withFileTypes: true })) {
      const fullPath = path.join(current, entry.name);
      if (entry.isDirectory()) {
        walk(fullPath);
      } else if (entry.isFile() && predicate(fullPath)) {
        result.push(fullPath);
      }
    }
  }

  if (statSync(directory).isDirectory()) {
    walk(directory);
  }
  return result;
}

function parseRegistryRows() {
  const registry = readRepo(registryPath);
  return registry
    .split(/\r?\n/)
    .filter((line) => line.startsWith("| `") && line.includes("docs/implementation-plan.md"))
    .map((line) => {
      const columns = line.split("|").map((column) => column.trim());
      const sourcePlanPath = (columns[6] ?? "").match(/(crates\/[^`) ]+\/docs\/implementation-plan\.md)/)?.[1];
      return {
        moduleSlug: columns[1]?.replace(/`/g, "") ?? "<unknown>",
        uiSurfaces: columns[2] ?? "",
        ffaStatus: columns[3]?.replace(/`/g, ""),
        fbaStatus: columns[4]?.replace(/`/g, ""),
        structuralShape: columns[5]?.replace(/`/g, ""),
        sourcePlanPath,
      };
    })
    .filter((row) => row.sourcePlanPath);
}

function parseLocalStatus(planPath) {
  const plan = readRepo(planPath);
  return {
    ffaStatus: plan.match(/- FFA status: `([^`]+)`/)?.[1],
    fbaStatus: plan.match(/- FBA status: `([^`]+)`/)?.[1],
    structuralShape: plan.match(/- Structural shape: `([^`]+)`/)?.[1],
  };
}

function hasAny(paths) {
  return paths.some((candidate) => existsSync(candidate));
}

function surfaceSrcPaths(moduleRoot, surface, candidates) {
  return candidates.map((candidate) => path.join(moduleRoot, surface, "src", candidate));
}

function expectedSurfaces(row, moduleRoot) {
  const fromBoard = [];
  if (/\badmin\b/.test(row.uiSurfaces) && existsSync(path.join(moduleRoot, "admin", "src"))) {
    fromBoard.push("admin");
  }
  if (/\bstorefront\b/.test(row.uiSurfaces) && existsSync(path.join(moduleRoot, "storefront", "src"))) {
    fromBoard.push("storefront");
  }
  if (fromBoard.length > 0) {
    return fromBoard;
  }
  return ["admin", "storefront"].filter((surface) => existsSync(path.join(moduleRoot, surface, "src")));
}

function checkStructure(row) {
  if (row.structuralShape !== "core_transport_ui") {
    return;
  }

  const moduleRoot = path.dirname(path.dirname(repoPath(row.sourcePlanPath)));
  const surfaces = expectedSurfaces(row, moduleRoot);
  if (surfaces.length === 0) {
    fail(`${row.moduleSlug}: core_transport_ui row has no admin/storefront src surface`);
    return;
  }

  for (const surface of surfaces) {
    const hasCore = hasAny(surfaceSrcPaths(moduleRoot, surface, ["core.rs", "core"]));
    const hasTransport = hasAny(surfaceSrcPaths(moduleRoot, surface, ["transport.rs", "transport", "native.rs"]));
    const hasUi = hasAny(surfaceSrcPaths(moduleRoot, surface, ["ui/leptos.rs", "ui/leptos"]));
    if (!hasCore || !hasTransport || !hasUi) {
      fail(`${row.moduleSlug}/${surface}: expected core_transport_ui structure, got core=${hasCore} transport=${hasTransport} ui=${hasUi}`);
    }
  }
}

function checkRustPatterns(row) {
  if (row.structuralShape !== "core_transport_ui") {
    return;
  }

  const moduleRoot = path.dirname(path.dirname(repoPath(row.sourcePlanPath)));
  for (const surface of expectedSurfaces(row, moduleRoot)) {
    const srcRoot = path.join(moduleRoot, surface, "src");
    for (const filePath of listFiles(srcRoot, (candidate) => candidate.endsWith(".rs"))) {
      const relative = normalizePath(filePath);
      const source = readFileSync(filePath, "utf8");
      const isCore = relative.endsWith("/core.rs") || relative.includes("/core/");
      const isUi = relative.includes("/ui/") || relative.endsWith("/ui.rs");

      if (
        isCore &&
        /use .*leptos|leptos::|leptos_router|leptos_ui_routing|#\[component|#\[server|IntoView|ReadSignal|WriteSignal|Resource</.test(source)
      ) {
        fail(`${relative}: core must stay Leptos/server-function free`);
      }

      if (isUi && /(?:crate|super|self)::api\b|\bapi::/.test(source)) {
        fail(`${relative}: UI adapter must not call raw api::* directly`);
      }
    }
  }
}

function checkRegistryLocalSync(rows) {
  for (const row of rows) {
    const local = parseLocalStatus(row.sourcePlanPath);
    for (const field of ["ffaStatus", "fbaStatus", "structuralShape"]) {
      if (row[field] !== local[field]) {
        fail(`${row.moduleSlug}: central ${field}=${row[field]} must match ${row.sourcePlanPath} ${field}=${local[field]}`);
      }
    }
  }
}

function checkAggregateCoverage() {
  const packageJson = JSON.parse(readRepo(packageJsonPath));
  const scripts = packageJson.scripts ?? {};
  const aggregate = scripts["verify:ffa:ui:migration"] ?? "";

  for (const required of [
    "npm run verify:ffa:ui:migration:boundary-sweep",
    "npm run verify:inventory:admin-boundary",
    "npm run verify:workflow:admin-boundary",
  ]) {
    if (!aggregate.includes(required)) {
      fail(`package.json verify:ffa:ui:migration must include ${required}`);
    }
  }
}

const rows = parseRegistryRows();
checkRegistryLocalSync(rows);
for (const row of rows) {
  checkStructure(row);
  checkRustPatterns(row);
}
checkAggregateCoverage();

if (failures.length > 0) {
  console.error("FFA UI boundary sweep failed:");
  for (const failure of failures.sort()) {
    console.error(`✗ ${failure}`);
  }
  process.exit(Math.min(failures.length, 255));
}

console.log("FFA UI boundary sweep passed");
