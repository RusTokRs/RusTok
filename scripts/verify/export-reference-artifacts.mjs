#!/usr/bin/env node
import { spawnSync } from "node:child_process";
import fs from "node:fs";
import path from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";

function fail(message) {
  console.error(`[reference] ${message}`);
  process.exit(1);
}

function run(command, args, options = {}) {
  const { capture = false, ...spawnOptions } = options;
  const result = spawnSync(command, args, {
    encoding: "utf8",
    stdio: capture ? "pipe" : "inherit",
    ...spawnOptions,
  });
  if (result.error) {
    fail(`${command}: ${result.error.message}`);
  }
  if (result.status !== 0) {
    const stderr = capture && result.stderr?.trim() ? `: ${result.stderr.trim()}` : "";
    fail(`${command} exited with status ${result.status}${stderr}`);
  }
  return capture ? result.stdout.trim() : "";
}

async function download(url, destination, init) {
  let response;
  try {
    response = await fetch(url, init);
  } catch (error) {
    fail(`${url}: request failed: ${error.message}`);
  }
  if (!response.ok) {
    fail(`${url}: HTTP ${response.status} ${response.statusText}`);
  }
  fs.writeFileSync(destination, Buffer.from(await response.arrayBuffer()));
}

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(scriptDir, "..", "..");
const baseUrl = (process.env.RUSTOK_BASE_URL ?? "http://127.0.0.1:5150").replace(/\/$/, "");
const outDir = path.resolve(process.argv[2] ?? path.join(repoRoot, "artifacts", "reference"));
const timestamp = new Date().toISOString().replace(/[-:]/g, "").replace(/\.\d{3}Z$/, "Z");
const targetDir = path.join(outDir, timestamp);
const openapiDir = path.join(targetDir, "openapi");
const graphqlDir = path.join(targetDir, "graphql");
const rustdocSkipped = process.env.SKIP_RUSTDOC ?? "0";

fs.mkdirSync(openapiDir, { recursive: true });
fs.mkdirSync(graphqlDir, { recursive: true });

if (rustdocSkipped !== "1") {
  console.log("[reference] generating rustdoc artifacts");
  run("cargo", ["doc", "--no-deps", "-p", "rustok-server", "-p", "rustok-workflow"], {
    cwd: repoRoot,
  });
}

console.log(`[reference] exporting OpenAPI JSON/YAML from ${baseUrl}`);
await download(`${baseUrl}/api/openapi.json`, path.join(openapiDir, "openapi.json"));
await download(`${baseUrl}/api/openapi.yaml`, path.join(openapiDir, "openapi.yaml"));

console.log("[reference] exporting GraphQL schema SDL");
await download(`${baseUrl}/api/graphql/schema.graphql`, path.join(graphqlDir, "schema.graphql"));

console.log("[reference] exporting full GraphQL schema introspection");
const introspectionQuery = fs.readFileSync(
  path.join(scriptDir, "graphql-introspection-query.graphql"),
  "utf8",
);
await download(`${baseUrl}/api/graphql`, path.join(graphqlDir, "introspection.json"), {
  method: "POST",
  headers: { "content-type": "application/json" },
  body: JSON.stringify({ query: introspectionQuery }),
});

console.log("[reference] writing manifest");
const gitCommit = run("git", ["rev-parse", "HEAD"], { cwd: repoRoot, capture: true });
const files = [
  "openapi/openapi.json",
  "openapi/openapi.yaml",
  "graphql/introspection.json",
  "graphql/schema.graphql",
];
const manifest = {
  schema: "rustok.reference_artifacts.v1",
  created_at_utc: timestamp,
  base_url: baseUrl,
  git_commit: gitCommit,
  rustdoc_skipped: rustdocSkipped,
  files,
};
fs.writeFileSync(
  path.join(targetDir, "manifest.json"),
  `${JSON.stringify(manifest, null, 2)}\n`,
);
fs.writeFileSync(
  path.join(targetDir, "manifest.txt"),
  [
    `created_at_utc=${timestamp}`,
    `base_url=${baseUrl}`,
    `git_commit=${gitCommit}`,
    `rustdoc_skipped=${rustdocSkipped}`,
    `files=${[...files, "manifest.json"].join(",")}`,
    "",
  ].join("\n"),
);

console.log("[reference] verifying artifact completeness");
run(process.execPath, [path.join(scriptDir, "verify-reference-artifacts.mjs"), targetDir], {
  cwd: repoRoot,
});
console.log(`[reference] done: ${targetDir}`);
