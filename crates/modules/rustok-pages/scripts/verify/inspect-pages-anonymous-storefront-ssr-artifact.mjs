#!/usr/bin/env node

import { createHash } from "node:crypto";
import { existsSync, mkdirSync, readFileSync, statSync, writeFileSync } from "node:fs";
import path from "node:path";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = process.env.RUSTOK_VERIFY_REPO_ROOT
  ? path.resolve(process.env.RUSTOK_VERIFY_REPO_ROOT)
  : path.resolve(scriptDir, "..", "..", "..", "..");
const graphVerifier = path.join(
  repoRoot,
  "crates/rustok-pages/scripts/verify/verify-pages-anonymous-storefront-graph.mjs",
);

function usage(message) {
  if (message) console.error(message);
  console.error(
    "usage: node inspect-pages-anonymous-storefront-ssr-artifact.mjs " +
      "--artifact <file> [--artifact <file> ...] --output <packet.json> " +
      "[--profile <id>]",
  );
  process.exit(2);
}

const args = process.argv.slice(2);
const artifacts = [];
let outputPath = null;
let profile = "host-storefront-ssr";
for (let index = 0; index < args.length; index += 1) {
  const argument = args[index];
  if (argument === "--artifact") {
    const value = args[++index];
    if (!value) usage("--artifact requires a file path");
    artifacts.push(path.resolve(value));
  } else if (argument === "--output") {
    const value = args[++index];
    if (!value) usage("--output requires a packet path");
    outputPath = path.resolve(value);
  } else if (argument === "--profile") {
    const value = args[++index];
    if (!value) usage("--profile requires an id");
    profile = value;
  } else if (argument === "--help" || argument === "-h") {
    usage();
  } else {
    usage(`unknown argument: ${argument}`);
  }
}
if (artifacts.length === 0) usage("at least one --artifact is required");
if (!outputPath) usage("--output is required");

const graph = spawnSync(process.execPath, [graphVerifier], {
  cwd: repoRoot,
  encoding: "utf8",
  maxBuffer: 64 * 1024 * 1024,
  env: { ...process.env, CARGO_TERM_COLOR: "never" },
});
if (graph.error) {
  console.error(`failed to start dependency graph verifier: ${graph.error.message}`);
  process.exit(1);
}
if (graph.status !== 0) {
  process.stderr.write(graph.stderr ?? "");
  process.stdout.write(graph.stdout ?? "");
  console.error(`dependency graph verifier exited ${graph.status}`);
  process.exit(1);
}

const forbiddenMarkers = [
  "rustok-pages-admin",
  "rustok_pages_admin",
  "rustok-page-builder-admin",
  "rustok_page_builder_admin",
  "fly-browser",
  "fly_browser",
  "fly-ui",
  "fly_ui",
  "fly-leptos",
  "fly_leptos",
  "PagesFlyBuilder",
  "PageBuilderAdminHostContext",
  "PageBuilderAdmin",
  "ConsumerPropertiesPanel",
];

const inspected = [];
const findings = [];
for (const artifactPath of artifacts) {
  if (!existsSync(artifactPath)) {
    findings.push({ artifact: artifactPath, marker: null, error: "artifact does not exist" });
    continue;
  }
  const stat = statSync(artifactPath);
  if (!stat.isFile()) {
    findings.push({ artifact: artifactPath, marker: null, error: "artifact is not a regular file" });
    continue;
  }
  if (stat.size === 0) {
    findings.push({ artifact: artifactPath, marker: null, error: "artifact is empty" });
    continue;
  }

  const bytes = readFileSync(artifactPath);
  const sha256 = createHash("sha256").update(bytes).digest("hex");
  const relative = path.relative(repoRoot, artifactPath);
  const artifactLabel = relative.startsWith("..") ? artifactPath : relative;
  const matchedMarkers = [];
  for (const marker of forbiddenMarkers) {
    if (bytes.indexOf(Buffer.from(marker, "utf8")) >= 0) {
      matchedMarkers.push(marker);
      findings.push({ artifact: artifactLabel, marker, error: "forbidden authoring marker" });
    }
  }
  inspected.push({
    path: artifactLabel,
    bytes: stat.size,
    sha256,
    forbidden_markers_found: matchedMarkers,
  });
}

const packet = {
  format: "pages_anonymous_storefront_ssr_artifact_execution_v1",
  status: findings.length === 0 ? "passed" : "failed",
  profile,
  generated_at: new Date().toISOString(),
  source_commit: process.env.GITHUB_SHA || process.env.RUSTOK_SOURCE_COMMIT || null,
  graph_verifier: {
    command: `node ${path.relative(repoRoot, graphVerifier)}`,
    status: "passed",
    stdout: `${graph.stdout ?? ""}`.trim(),
  },
  artifact_contract: {
    explicit_artifact_paths_required: true,
    byte_scan_is_combined_with_feature_resolved_dependency_graph: true,
    absence_of_a_client_bundle_is_not_reported_as_a_passing_client_bundle: true,
  },
  forbidden_markers: forbiddenMarkers,
  artifacts: inspected,
  findings,
};

mkdirSync(path.dirname(outputPath), { recursive: true });
writeFileSync(outputPath, `${JSON.stringify(packet, null, 2)}\n`, "utf8");

if (findings.length > 0) {
  console.error(
    `[inspect-pages-anonymous-storefront-ssr-artifact] FAIL findings=${findings.length} packet=${outputPath}`,
  );
  for (const finding of findings) {
    console.error(`- ${finding.artifact}: ${finding.error}${finding.marker ? ` (${finding.marker})` : ""}`);
  }
  process.exit(1);
}

console.log(
  `[inspect-pages-anonymous-storefront-ssr-artifact] PASS artifacts=${inspected.length} packet=${outputPath}`,
);
