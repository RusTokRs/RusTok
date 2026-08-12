#!/usr/bin/env node

import { execFileSync, spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import {
  mkdtempSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..");
const runner = path.join(
  repoRoot,
  "scripts/evidence/verify-page-builder-accessibility-browser-packet.mjs",
);
const executionContractPath =
  "crates/rustok-page-builder/contracts/evidence/page-builder-generic-accessibility-browser-execution-contract.json";
const executionContract = JSON.parse(
  readFileSync(path.join(repoRoot, executionContractPath), "utf8"),
);
const sourceCommit = execFileSync("git", ["rev-parse", "HEAD"], {
  cwd: repoRoot,
  encoding: "utf8",
}).trim();
const deploymentDigest = `ghcr.io/rustok/page-builder@sha256:${"a".repeat(64)}`;
const temporaryRoot = mkdtempSync(path.join(os.tmpdir(), "rustok-page-builder-a11y-packet-"));

function sha256(value) {
  return createHash("sha256").update(value).digest("hex");
}

function sourceHashes() {
  return Object.fromEntries(
    executionContract.required_source_files.map((relativePath) => [
      relativePath,
      sha256(readFileSync(path.join(repoRoot, relativePath))),
    ]),
  );
}

function validPacket() {
  return {
    format: executionContract.output.format,
    status: executionContract.output.status,
    source_commit: sourceCommit,
    deployment_digest: deploymentDigest,
    node_version: process.version,
    playwright_version: "1.58.2",
    source_files: sourceHashes(),
    input_records: {
      editor_storage_state: {
        bytes: 512,
        sha256: "b".repeat(64),
      },
      profile_url_sha256: {
        full: "c".repeat(64),
        read_only: "d".repeat(64),
      },
    },
    observations: {
      full: {
        passed: true,
        criticalFailures: 0,
        facts: {
          pageCount: 2,
          tabFocusBetweenAdjacentPages: true,
          keyboardActivationUpdatedPressedState: true,
          addPageSequentialFocusOrder: true,
          ariaTreePressedStateObserved: true,
          ariaTreeAddPageNameObserved: true,
          pageMetadataAccessibleNamesResolved: true,
        },
      },
      read_only: {
        passed: true,
        criticalFailures: 0,
        facts: {
          pageCount: 2,
          editFieldsetBrowserDisabled: true,
          editFieldsetAriaDisabled: true,
          propertiesFieldsetBrowserDisabled: true,
          propertiesFieldsetAriaDisabled: true,
          mutationControlsBrowserDisabled: true,
          pageNavigationKeyboardAvailable: true,
        },
      },
    },
    retained_secrets: false,
    raw_dom_retained: false,
    aria_snapshot_text_retained: false,
    screen_reader_execution_pending: true,
    wcag_conformance_not_claimed: true,
    executed_at: "2026-08-12T15:45:00.000Z",
  };
}

function execute(name, packet, expectedDigest = deploymentDigest) {
  const packetPath = path.join(temporaryRoot, `${name}.json`);
  const outputPath = path.join(repoRoot, "target", `page-builder-accessibility-${name}-verification.json`);
  writeFileSync(packetPath, `${JSON.stringify(packet, null, 2)}\n`, "utf8");
  rmSync(outputPath, { force: true });
  const result = spawnSync(
    process.execPath,
    [
      runner,
      "--packet",
      packetPath,
      "--expected-source",
      sourceCommit,
      "--expected-deployment-digest",
      expectedDigest,
      "--output",
      outputPath,
    ],
    { cwd: repoRoot, encoding: "utf8" },
  );
  return { result, outputPath };
}

function requireSuccess(name, packet) {
  const { result, outputPath } = execute(name, packet);
  if (result.status !== 0) {
    throw new Error(`${name}: expected success\n${result.stdout}\n${result.stderr}`);
  }
  const output = JSON.parse(readFileSync(outputPath, "utf8"));
  if (
    output.format !== "page_builder_generic_accessibility_browser_packet_verification_v1" ||
    output.status !== "browser_packet_verified_owner_review_ready_screen_reader_pending" ||
    output.source_commit !== sourceCommit ||
    output.deployment_digest !== deploymentDigest ||
    output.owner_review_required !== true ||
    output.deployment_provenance_verified_by_this_packet !== false ||
    output.cryptographic_origin_to_repo_digest_binding_claimed !== false ||
    output.screen_reader_execution_pending !== true ||
    output.wcag_conformance_not_claimed !== true ||
    output.profiles?.full?.passed !== true ||
    output.profiles?.read_only?.passed !== true
  ) {
    throw new Error(`${name}: verification output drifted`);
  }
}

function requireFailure(name, packet, expectedMessage, expectedDigest = deploymentDigest) {
  const { result } = execute(name, packet, expectedDigest);
  if (result.status === 0) throw new Error(`${name}: expected failure`);
  const combined = `${result.stdout}\n${result.stderr}`;
  if (!combined.includes(expectedMessage)) {
    throw new Error(`${name}: expected '${expectedMessage}'\n${combined}`);
  }
}

try {
  requireSuccess("valid", validPacket());

  const sourceTamper = validPacket();
  sourceTamper.source_files[executionContract.required_source_files[0]] = "0".repeat(64);
  requireFailure("source-tamper", sourceTamper, "retained source hash does not match checkout");

  const screenReaderOverclaim = validPacket();
  screenReaderOverclaim.screen_reader_execution_pending = false;
  requireFailure("screen-reader-overclaim", screenReaderOverclaim, "screen_reader_execution_pending must remain true");

  const wcagOverclaim = validPacket();
  wcagOverclaim.wcag_conformance_not_claimed = false;
  requireFailure("wcag-overclaim", wcagOverclaim, "wcag_conformance_not_claimed must remain true");

  const missingFact = validPacket();
  missingFact.observations.full.facts.ariaTreePressedStateObserved = false;
  requireFailure("missing-fact", missingFact, "full required fact ariaTreePressedStateObserved is not true");

  const retainedDataDrift = validPacket();
  retainedDataDrift.raw_profile_url = "https://example.invalid/private";
  requireFailure("retained-data-drift", retainedDataDrift, "browser packet keys drifted");

  requireFailure(
    "digest-mismatch",
    validPacket(),
    "browser packet deployment_digest does not match the separately supplied expected RepoDigest",
    `ghcr.io/rustok/page-builder@sha256:${"e".repeat(64)}`,
  );

  console.log("[verify-page-builder-accessibility-browser-packet.test] PASS cases=7");
} finally {
  rmSync(temporaryRoot, { recursive: true, force: true });
  for (const name of [
    "valid",
    "source-tamper",
    "screen-reader-overclaim",
    "wcag-overclaim",
    "missing-fact",
    "retained-data-drift",
    "digest-mismatch",
  ]) {
    rmSync(
      path.join(repoRoot, "target", `page-builder-accessibility-${name}-verification.json`),
      { force: true },
    );
  }
}
