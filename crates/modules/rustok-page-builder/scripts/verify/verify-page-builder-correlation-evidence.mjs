#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const repoRoot = path.resolve(__dirname, "..", "..", "..", "..");
const contractPath = path.join(
  repoRoot,
  "crates",
  "rustok-page-builder",
  "contracts",
  "page-builder-correlation-contract.json",
);

function fail(message) {
  console.error("[verify-page-builder-correlation-evidence] FAIL");
  console.error(`- ${message}`);
  process.exit(1);
}

function readText(relativePath) {
  const filePath = path.join(repoRoot, relativePath);
  if (!fs.existsSync(filePath)) {
    fail(`missing file: ${relativePath}`);
  }
  return fs.readFileSync(filePath, "utf8");
}

function readJson(relativePath) {
  try {
    return JSON.parse(readText(relativePath));
  } catch (error) {
    fail(`invalid JSON in ${relativePath}: ${error.message}`);
  }
}

function expectArray(value, label) {
  if (!Array.isArray(value)) {
    fail(`${label} must be an array`);
  }
  return value;
}

function includesAny(values, candidates) {
  return candidates.some((candidate) => values.includes(candidate));
}

const contract = readJson(path.relative(repoRoot, contractPath));
const requiredPath = expectArray(contract.required_path, "required_path");
const acceptedResults = new Set(
  expectArray(contract.accepted_results, "accepted_results"),
);
const minimumSamples = contract.required_trace_samples_per_packet ?? 2;
const requiredSpanGroups = contract.required_spans ?? {};

for (const source of expectArray(contract.source_markers, "source_markers")) {
  const text = readText(source.path);
  for (const marker of expectArray(source.markers, `${source.path}.markers`)) {
    if (!text.includes(marker)) {
      fail(`${source.path} missing source marker '${marker}'`);
    }
  }
}

for (const docPath of expectArray(contract.docs, "docs")) {
  const doc = readText(docPath);
  for (const term of ["correlation", "builder write", "storefront read"]) {
    if (!doc.toLowerCase().includes(term)) {
      fail(`${docPath} must document '${term}' for the correlation gate`);
    }
  }
}

for (const packetRef of expectArray(contract.evidence_packets, "evidence_packets")) {
  const packet = readJson(packetRef.path);
  if (packet.module_slug !== packetRef.module_slug) {
    fail(`${packetRef.path} module_slug mismatch`);
  }
  if (String(packet.wave) !== String(packetRef.wave)) {
    fail(`${packetRef.path} wave mismatch`);
  }
  if (packet.mode !== packetRef.mode) {
    fail(`${packetRef.path} mode mismatch`);
  }
  const traceSamples = expectArray(
    packet.observability?.trace_samples,
    `${packetRef.path}.observability.trace_samples`,
  );
  if (traceSamples.length < minimumSamples) {
    fail(`${packetRef.path} must contain at least ${minimumSamples} trace samples`);
  }
  const aggregateSpans = new Set();
  for (const [index, sample] of traceSamples.entries()) {
    const traceId = String(sample.trace_id ?? "");
    if (!traceId.startsWith(`trace-${packet.module_slug}-wave${packet.wave}-`)) {
      fail(`${packetRef.path}.trace_samples[${index}] has unstable trace_id '${traceId}'`);
    }
    const correlationPath = String(sample.correlation_path ?? "");
    for (const segment of requiredPath) {
      if (!correlationPath.includes(segment)) {
        fail(`${packetRef.path}.trace_samples[${index}] missing correlation segment '${segment}'`);
      }
    }
    if (!acceptedResults.has(sample.result)) {
      fail(`${packetRef.path}.trace_samples[${index}] has unexpected result '${sample.result}'`);
    }
    for (const span of expectArray(sample.spans, `${packetRef.path}.trace_samples[${index}].spans`)) {
      aggregateSpans.add(span);
    }
  }
  for (const [group, requiredSpans] of Object.entries(requiredSpanGroups)) {
    const spans = expectArray(requiredSpans, `required_spans.${group}`);
    if (!includesAny([...aggregateSpans], spans)) {
      fail(`${packetRef.path} missing at least one required ${group} span: ${spans.join(", ")}`);
    }
  }
}

console.log("[verify-page-builder-correlation-evidence] PASS");
