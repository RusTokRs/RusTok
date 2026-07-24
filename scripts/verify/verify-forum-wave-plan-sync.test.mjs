#!/usr/bin/env node

import { test } from "node:test";
import assert from "node:assert/strict";
import { mkdtempSync, mkdirSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { spawnSync } from "node:child_process";

const scriptPath = path.resolve("scripts/verify/verify-forum-wave-plan-sync.mjs");
const verifierContractPath = "scripts/verify/verify-forum-wave-plan-sync.mjs";
const verifierTestContractPath = "scripts/verify/verify-forum-wave-plan-sync.test.mjs";

function plan(overrides = {}) {
  const status = overrides.status ?? "in_progress";
  const ledgerResult =
    overrides.ledgerResult ??
    "Widget contract exists; richer widgets and observed Page Builder evidence remain.";
  const observedRequirement =
    overrides.observedRequirement ??
    "Replace the synthetic Wave packet with an observed tenant control-plane run";
  const pagesBlocker =
    overrides.pagesBlocker ?? "after the `pages` reference-consumer gate.";
  const degradedGuarantee =
    overrides.degradedGuarantee ??
    "Page Builder stays optional; forum routes must not depend on provider availability.";
  const verification =
    overrides.verification ??
    `npm run verify:page-builder:consumer:forum\nnpm run verify:forum:wave-evidence-freshness`;

  return `
## Program ledger

| Task | Status | Current result or nearest deliverable |
| --- | --- | --- |
| \`FORUM-32\` | \`${status}\` | ${ledgerResult} |

## \`FORUM-32\` — Page Builder and widget evolution

**Status:** \`${status}\`  
**Priority:** P2  
**Dependencies:** stable bounded read ports; Page Builder/pages provider readiness

### Remaining scope

Add richer widgets while preserving bounded public Forum read ports.

${observedRequirement}
that correlates builder write, forum publication and storefront read ${pagesBlocker}
${degradedGuarantee}

### Verification

\`\`\`bash
${verification}
\`\`\`

## \`FORUM-33\` — analytics, observability and reconciliation
`;
}

function evidence(overrides = {}) {
  const base = {
    schema_version: 2,
    artifact: "page_builder_wave_evidence_packet",
    module_slug: "forum",
    wave: "1",
    mode: "source_ready",
    provenance: "synthetic_fixture",
    execution_status: "not_run_by_implementation_agent",
    static_readiness: {
      source_contracts: [verifierContractPath],
    },
    observed_run: {
      required: true,
      status: "not_run",
      blocked_by: "pages_reference_consumer_gate",
      required_correlation_path: "builder_write -> forum_publish -> storefront_read",
    },
    verification: {
      no_compile_gates: [
        `node ${verifierContractPath}`,
        `node ${verifierTestContractPath}`,
      ],
    },
    deferred: [
      "observed tenant control-plane run after the pages reference-consumer gate",
    ],
  };
  return {
    ...base,
    ...overrides,
    static_readiness: {
      ...base.static_readiness,
      ...(overrides.static_readiness ?? {}),
    },
    observed_run: {
      ...base.observed_run,
      ...(overrides.observed_run ?? {}),
    },
    verification: {
      ...base.verification,
      ...(overrides.verification ?? {}),
    },
  };
}

function writeFixture(root, relativePath, content) {
  const filePath = path.join(root, relativePath);
  mkdirSync(path.dirname(filePath), { recursive: true });
  writeFileSync(filePath, content);
}

function run(planContent, evidencePacket) {
  const root = mkdtempSync(path.join(tmpdir(), "rustok-forum-wave-plan-sync-"));
  try {
    writeFixture(
      root,
      "crates/rustok-forum/docs/implementation-plan.md",
      planContent,
    );
    writeFixture(
      root,
      "crates/rustok-forum/contracts/evidence/forum-wave1-rollout-evidence.json",
      JSON.stringify(evidencePacket, null, 2),
    );
    return spawnSync("node", [scriptPath], {
      cwd: path.resolve("."),
      env: { ...process.env, RUSTOK_VERIFY_REPO_ROOT: root },
      encoding: "utf8",
    });
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
}

test("Forum Wave plan sync accepts canonical source-ready state", () => {
  const result = run(plan(), evidence());
  assert.equal(result.status, 0, result.stderr || result.stdout);
  assert.match(result.stdout, /verification passed/);
});

test("Forum Wave plan sync rejects premature done status", () => {
  const result = run(plan({ status: "done" }), evidence());
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /must remain in_progress/);
});

test("Forum Wave plan sync rejects missing observed-run requirement", () => {
  const result = run(
    plan({ observedRequirement: "Keep the current packet without an observed run." }),
    evidence(),
  );
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /must require replacement of the synthetic packet/);
});

test("Forum Wave plan sync rejects premature live evidence", () => {
  const result = run(
    plan(),
    evidence({
      mode: "live",
      provenance: "observed_control_plane",
      execution_status: "maintainer_verified",
    }),
  );
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /evidence mode must remain source_ready/);
});

test("Forum Wave plan sync rejects blocker drift", () => {
  const result = run(
    plan(),
    evidence({ observed_run: { blocked_by: "unbounded_runtime_dependency" } }),
  );
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /blocker must remain pages_reference_consumer_gate/);
});

test("Forum Wave plan sync rejects removed optional-provider guarantee", () => {
  const result = run(
    plan({ degradedGuarantee: "Forum routes require Page Builder availability." }),
    evidence(),
  );
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /optional-provider degraded-mode guarantee/);
});

test("Forum Wave plan sync rejects live-only sections in source-ready evidence", () => {
  const result = run(
    plan(),
    evidence({ observability: { metrics: { preview_p95_ms: "live_wave1_actual:120" } } }),
  );
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /must not materialize live-only key observability/);
});

test("Forum Wave plan sync rejects missing plan verification command", () => {
  const result = run(
    plan({ verification: "npm run verify:page-builder:consumer:forum" }),
    evidence(),
  );
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /missing npm run verify:forum:wave-evidence-freshness/);
});

test("Forum Wave plan sync rejects missing source-contract registration", () => {
  const result = run(
    plan(),
    evidence({ static_readiness: { source_contracts: [] } }),
  );
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /source contracts must register/);
});

test("Forum Wave plan sync rejects missing mutation-test registration", () => {
  const result = run(
    plan(),
    evidence({
      verification: {
        no_compile_gates: [`node ${verifierContractPath}`],
      },
    }),
  );
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /no-compile verification set is missing/);
});
