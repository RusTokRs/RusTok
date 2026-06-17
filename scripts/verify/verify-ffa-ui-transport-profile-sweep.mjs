#!/usr/bin/env node
// Repository-wide FFA transport profile sweep.
//
// This gate checks that every core_transport_ui surface has an explicit transport
// profile: multi-adapter by code markers, a documented accepted single-adapter/
// owner-fragment state, or an in-progress transport parity gap.

import { existsSync, readdirSync, readFileSync, statSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));

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
  console.log("Usage: node scripts/verify/verify-ffa-ui-transport-profile-sweep.mjs [--root <path>|--root=<path>] [-h|--help]");
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
  console.error("FFA UI transport profile sweep failed:");
  console.error(`✗ Unknown arguments: ${cli.unknownArgs.join(" ")}`);
  printUsage();
  process.exit(1);
}

const repoRoot = resolveRepoRoot(cli.cliRoot, process.env);
const failures = [];

function repoPath(relativePath) {
  return path.join(repoRoot, relativePath);
}

function readRepo(relativePath) {
  return readFileSync(repoPath(relativePath), "utf8");
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
  return readRepo("docs/modules/registry.md")
    .split(/\r?\n/)
    .filter((line) => line.startsWith("| `") && line.includes("docs/implementation-plan.md"))
    .map((line) => {
      const columns = line.split("|").map((column) => column.trim());
      return {
        moduleSlug: columns[1]?.replace(/`/g, "") ?? "<unknown>",
        uiSurfaces: columns[2] ?? "",
        ffaStatus: columns[3]?.replace(/`/g, "") ?? "",
        structuralShape: columns[5]?.replace(/`/g, ""),
        sourcePlanPath: (columns[6] ?? "").match(/(crates\/[^`) ]+\/docs\/implementation-plan\.md)/)?.[1],
        registryText: line,
      };
    })
    .filter((row) => row.sourcePlanPath && row.structuralShape === "core_transport_ui");
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

function classifySurface(moduleRoot, surface) {
  const srcRoot = path.join(moduleRoot, surface, "src");
  const files = listFiles(srcRoot, (filePath) => filePath.endsWith(".rs"));
  const relativeFiles = files.map(normalizePath);
  const source = files.map((filePath) => readFileSync(filePath, "utf8")).join("\n");

  const native = relativeFiles.some((filePath) => filePath.includes("/native_server_adapter.rs") || filePath.endsWith("/native.rs")) || /#\[server|\/api\/fn|ServerFn/.test(source);
  const graphql = relativeFiles.some((filePath) => filePath.includes("/graphql_adapter.rs")) || /leptos_graphql|GraphqlRequest|GraphQL|graphql|\/api\/graphql|RUSTOK_GRAPHQL_URL/.test(source);
  const rest = relativeFiles.some((filePath) => filePath.includes("/rest_adapter.rs")) || /reqwest::|Method::(GET|POST|PUT|DELETE|PATCH)|RUSTOK_API_URL/.test(source);
  const ownerFragment = !native && !graphql && !rest;

  return {
    native,
    graphql,
    rest,
    ownerFragment,
    adapterCount: [native, graphql, rest].filter(Boolean).length,
  };
}

function hasDocumentedException(text) {
  return /single-adapter|native-only|GraphQL-only|no legacy GraphQL|no GraphQL\/REST|GraphQL\/REST fallback is intentionally not added|temporary native-only|transport still reaches|owner transport cutover|consumed by commerce checkout orchestration|handoff slice|handoff card|owner-module UI fragments/.test(text);
}

function hasTrackedParityGap(text) {
  return /transport parity gap|not a target state|GraphQL admin parity|GraphQL parity work|native\/GraphQL admin parity/.test(text);
}

for (const row of parseRegistryRows()) {
  const moduleRoot = path.dirname(path.dirname(repoPath(row.sourcePlanPath)));
  const planText = readRepo(row.sourcePlanPath);
  const docsText = `${planText}\n${row.registryText}`;

  for (const surface of expectedSurfaces(row, moduleRoot)) {
    const profile = classifySurface(moduleRoot, surface);
    if (profile.adapterCount >= 2) {
      continue;
    }

    if (hasDocumentedException(docsText)) {
      continue;
    }

    if (row.ffaStatus === "in_progress" && hasTrackedParityGap(docsText)) {
      continue;
    }

    const profileLabel = profile.ownerFragment
      ? "owner_fragment"
      : [
          profile.native ? "native" : null,
          profile.graphql ? "graphql" : null,
          profile.rest ? "rest" : null,
        ]
          .filter(Boolean)
          .join("+");
    failures.push(
      `${row.moduleSlug}/${surface}: transport profile ${profileLabel || "none"} has less than two adapters and lacks accepted single-adapter/owner-fragment evidence or an in-progress transport parity gap in ${row.sourcePlanPath}`,
    );
  }
}

if (failures.length > 0) {
  console.error("FFA UI transport profile sweep failed:");
  for (const failure of failures.sort()) {
    console.error(`✗ ${failure}`);
  }
  process.exit(Math.min(failures.length, 255));
}

console.log("FFA UI transport profile sweep passed");
