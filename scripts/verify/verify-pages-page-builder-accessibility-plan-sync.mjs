#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";

const repoRoot = process.env.RUSTOK_VERIFY_REPO_ROOT
  ? path.resolve(process.env.RUSTOK_VERIFY_REPO_ROOT)
  : process.cwd();

const paths = {
  shared: "docs/modules/pages-page-builder-parity-continuation-plan.md",
  central: "docs/modules/page-builder-implementation-plan.md",
  local: "crates/rustok-page-builder/docs/implementation-plan.md",
  actualization: "docs/modules/pages-page-builder-parity-accessibility-actualization-2026-08-12.md",
  accessibilityActualization: "docs/modules/page-builder-admin-accessibility-actualization-2026-08-10.md",
  accessibilityGuard: "scripts/verify/verify-page-builder-admin-accessibility.mjs",
  accessibilityBrowserGuard:
    "scripts/verify/verify-page-builder-accessibility-browser-evidence-harness.mjs",
  accessibilityBrowserContract:
    "crates/rustok-page-builder/contracts/evidence/page-builder-generic-accessibility-browser-execution-contract.json",
  accessibilityPacketVerifierContract:
    "crates/rustok-page-builder/contracts/evidence/page-builder-generic-accessibility-browser-packet-verifier-source.json",
  accessibilityPacketVerifierRunner:
    "scripts/evidence/verify-page-builder-accessibility-browser-packet.mjs",
  accessibilityPacketVerifierTest:
    "scripts/evidence/verify-page-builder-accessibility-browser-packet.test.mjs",
  accessibilityPacketVerifierGuard:
    "scripts/verify/verify-page-builder-accessibility-browser-packet-verifier.mjs",
};

const failures = [];

function read(relativePath) {
  const absolutePath = path.join(repoRoot, relativePath);
  if (!fs.existsSync(absolutePath)) {
    failures.push(`${relativePath}: required file is missing`);
    return "";
  }
  const stat = fs.lstatSync(absolutePath);
  if (!stat.isFile() || stat.isSymbolicLink()) {
    failures.push(`${relativePath}: must be a regular non-symlink file`);
    return "";
  }
  return fs.readFileSync(absolutePath, "utf8");
}

function requireText(source, marker, label) {
  if (!source.includes(marker)) failures.push(`${label}: missing '${marker}'`);
}

function forbidText(source, marker, label) {
  if (source.includes(marker)) failures.push(`${label}: stale marker remains '${marker}'`);
}

const shared = read(paths.shared);
const central = read(paths.central);
const local = read(paths.local);
const actualization = read(paths.actualization);
const accessibilityActualization = read(paths.accessibilityActualization);
const accessibilityGuard = read(paths.accessibilityGuard);
const accessibilityBrowserGuard = read(paths.accessibilityBrowserGuard);
const accessibilityBrowserContract = read(paths.accessibilityBrowserContract);
const accessibilityPacketVerifierContract = read(paths.accessibilityPacketVerifierContract);
const accessibilityPacketVerifierRunner = read(paths.accessibilityPacketVerifierRunner);
const accessibilityPacketVerifierTest = read(paths.accessibilityPacketVerifierTest);
const accessibilityPacketVerifierGuard = read(paths.accessibilityPacketVerifierGuard);

for (const marker of [
  "Date: 2026-08-12",
  "generic-editor-accessibility-source-ready",
  "PR #3444",
  "Generic Page Builder editor accessibility: source-ready / execution-open",
  "Generic editor control accessibility | Source-ready | Keyboard/focus/browser/screen-reader evidence pending",
  paths.accessibilityActualization,
  paths.actualization,
  "generic editor accessibility semantics is complete at this cursor",
  "verify-page-builder-admin-accessibility.mjs",
]) requireText(shared, marker, paths.shared);

for (const marker of [
  "through PR #3444 on 2026-08-12",
  "Generic editor control accessibility semantics: source-ready",
  "Generic editor executable accessibility evidence: pending",
  "Generic editor programmatic accessibility semantics and static anti-drift guard are source-ready",
  "Complete generic typed asset/control surfaces and programmatic accessibility semantics at source level",
  "Retain executable keyboard/focus/accessibility-tree/browser/screen-reader evidence for the built editor",
  paths.actualization,
  "verify-page-builder-admin-accessibility.mjs",
  "verify-pages-page-builder-accessibility-plan-sync.mjs",
]) requireText(central, marker, paths.central);
forbidText(
  central,
  "Complete remaining generic typed asset/control surfaces and accessibility\n  evidence.",
  paths.central,
);

for (const marker of [
  "2026-08-12 parity/accessibility reconciliation",
  "generic-editor-accessibility-source-ready",
  "browser-accessibility-evidence-pending",
  "Generic editor control semantics are also source-ready after PR #3444",
  "Retain generic editor keyboard/focus/accessible-name/state/browser/screen-reader evidence",
  paths.actualization,
  "verify-page-builder-admin-accessibility.mjs",
  "verify-pages-page-builder-accessibility-plan-sync.mjs",
]) requireText(local, marker, paths.local);

for (const marker of [
  "source-parity-rechecked",
  "generic-editor-accessibility-source-ready",
  "focused-ci-gate-ready",
  "generic-accessibility-browser-harness-source-ready",
  "generic-accessibility-browser-packet-verifier-source-ready",
  "browser-accessibility-evidence-pending",
  "main@21004ce4d5fe9d63e804319eae5dc3b0e8f9c5b5",
  "PR #3444",
  "PR #3453",
  "PR #3456",
  ".github/workflows/pages-page-builder-parity.yml",
  paths.accessibilityBrowserGuard,
  paths.accessibilityBrowserContract,
  paths.accessibilityPacketVerifierContract,
  paths.accessibilityPacketVerifierRunner,
  paths.accessibilityPacketVerifierTest,
  paths.accessibilityPacketVerifierGuard,
  "browser_packet_verified_owner_review_ready_screen_reader_pending",
  "maintainer browser execution pending",
  "owner_review_required = true",
  "deployment_provenance_verified_by_this_packet = false",
  "screen_reader_execution_pending = true",
  "wcag_conformance_not_claimed = true",
  "synthetic test creates no deployment claim",
]) requireText(actualization, marker, paths.actualization);

for (const marker of [
  "generic-editor-control-accessibility-source-ready",
  "source-guard-ready",
  "browser-accessibility-evidence-pending",
  "scripts/verify/verify-page-builder-admin-accessibility.mjs",
]) requireText(accessibilityActualization, marker, paths.accessibilityActualization);

for (const marker of [
  "aria-pressed=active.to_string()",
  "Page Builder admin accessibility source verified.",
]) requireText(accessibilityGuard, marker, paths.accessibilityGuard);

for (const marker of [
  paths.accessibilityBrowserContract,
  "Page Builder generic accessibility browser evidence harness source: ok",
  "screen_reader_execution_pending: true",
  "wcag_conformance_not_claimed: true",
]) requireText(accessibilityBrowserGuard, marker, paths.accessibilityBrowserGuard);

for (const marker of [
  '"status": "source_ready_maintainer_execution_pending"',
  '"format": "page_builder_generic_accessibility_browser_execution_v1"',
  '"profiles": [',
  '"full"',
  '"read_only"',
  '"screen-reader execution"',
  '"WCAG conformance"',
]) requireText(accessibilityBrowserContract, marker, paths.accessibilityBrowserContract);

for (const marker of [
  '"format": "page_builder_generic_accessibility_browser_packet_verifier_source_v1"',
  '"status": "source_ready_maintainer_execution_pending"',
  '"required_format": "page_builder_generic_accessibility_browser_execution_v1"',
  '"source_commit_must_equal_checkout_head": true',
  '"packet_deployment_digest_must_equal_expected": true',
  '"retained_source_hashes_must_match_contract_and_checkout": true',
  '"privacy_non_claim_flags_must_remain_fail_closed": true',
  '"format": "page_builder_generic_accessibility_browser_packet_verification_v1"',
  '"status": "browser_packet_verified_owner_review_ready_screen_reader_pending"',
  '"screen-reader execution"',
  '"WCAG conformance"',
]) requireText(
  accessibilityPacketVerifierContract,
  marker,
  paths.accessibilityPacketVerifierContract,
);

for (const marker of [
  'execFileSync("git", ["rev-parse", "HEAD"]',
  '"--expected-source"',
  '"--expected-deployment-digest"',
  "retained source hash does not match checkout",
  "owner_review_required: true",
  "deployment_provenance_verified_by_this_packet: false",
  "screen_reader_execution_pending: true",
  "wcag_conformance_not_claimed: true",
]) requireText(
  accessibilityPacketVerifierRunner,
  marker,
  paths.accessibilityPacketVerifierRunner,
);

for (const marker of [
  'requireSuccess("valid"',
  'requireFailure("source-tamper"',
  'requireFailure("screen-reader-overclaim"',
  'requireFailure("wcag-overclaim"',
  'requireFailure("missing-fact"',
  'requireFailure("retained-data-drift"',
  '"digest-mismatch"',
  "PASS cases=7",
]) requireText(
  accessibilityPacketVerifierTest,
  marker,
  paths.accessibilityPacketVerifierTest,
);

for (const marker of [
  paths.accessibilityPacketVerifierContract,
  paths.accessibilityPacketVerifierRunner,
  paths.accessibilityPacketVerifierTest,
  "source_ready=true execution=pending owner_review=pending screen_reader=pending",
]) requireText(
  accessibilityPacketVerifierGuard,
  marker,
  paths.accessibilityPacketVerifierGuard,
);

if (failures.length > 0) {
  console.error("Pages/Page Builder accessibility plan sync verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("Pages/Page Builder accessibility plan sync verified.");
