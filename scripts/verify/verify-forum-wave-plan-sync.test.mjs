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
const pagesGateAcceptancePath =
  "crates/rustok-pages/contracts/evidence/pages-reference-consumer-gate-acceptance-source.json";
const waveAdmissionPath =
  "crates/rustok-forum/contracts/evidence/forum-page-builder-wave-admission-source.json";
const waveAdmissionVerifierPath =
  "scripts/verify/verify-forum-page-builder-wave-admission.mjs";

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

function pagesGateAcceptanceSource(overrides = {}) {
  return {
    format: "pages_reference_consumer_gate_acceptance_source_v1",
    status: "source_ready_maintainer_execution_pending",
    output: {
      format: "pages_reference_consumer_gate_acceptance_v1",
      accepted_status: "owner_accepted_pages_reference_consumer_gate",
    },
    ...overrides,
  };
}

function waveAdmissionSource(overrides = {}) {
  const base = {
    format: "forum_page_builder_wave_admission_source_v1",
    status: "source_ready_maintainer_execution_pending",
    pages_gate_input: {
      format: "pages_reference_consumer_gate_acceptance_v1",
      required_status: "owner_accepted_pages_reference_consumer_gate",
    },
    lineage: {
      same_exact_source_commit_required_across_all_packets: true,
      same_immutable_repo_digest_required_across_pages_gate_browser_and_serverfn: true,
      accepted_pages_gate_is_a_precondition_not_a_forum_wave_acceptance: true,
    },
    output: {
      format: "forum_page_builder_wave_admission_v1",
      status: "forum_wave_inputs_admitted_observed_control_plane_pending",
    },
  };
  return {
    ...base,
    ...overrides,
    pages_gate_input: {
      ...base.pages_gate_input,
      ...(overrides.pages_gate_input ?? {}),
    },
    lineage: {
      ...base.lineage,
      ...(overrides.lineage ?? {}),
    },
    output: {
      ...base.output,
      ...(overrides.output ?? {}),
    },
  };
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
      source_contracts: [
        pagesGateAcceptancePath,
        waveAdmissionPath,
        waveAdmissionVerifierPath,
        verifierContractPath,
      ],
    },
    observed_run: {
      required: true,
      status: "not_run",
      blocked_by: "pages_reference_consumer_gate",
      accepted_gate_evidence: {
        required: true,
        format: "pages_reference_consumer_gate_acceptance_v1",
        status: "owner_accepted_pages_reference_consumer_gate",
      },
      wave_admission: {
        required: true,
        source_status: "source_ready_maintainer_execution_pending",
        format: "forum_page_builder_wave_admission_v1",
        status: "forum_wave_inputs_admitted_observed_control_plane_pending",
        execution_status: "maintainer_execution_pending",
      },
      required_correlation_path: "builder_write -> forum_publish -> storefront_read",
    },
    verification: {
      no_compile_gates: [
        `node ${waveAdmissionVerifierPath}`,
        `node ${verifierContractPath}`,
        `node ${verifierTestContractPath}`,
      ],
    },
    deferred: [
      "observed tenant control-plane run after an accepted pages reference-consumer gate and admitted exact-source Forum browser/runtime/server-function evidence",
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

function run(planContent, evidencePacket, overrides = {}) {
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
    writeFixture(
      root,
      pagesGateAcceptancePath,
      JSON.stringify(pagesGateAcceptanceSource(overrides.pagesGateAcceptance), null, 2),
    );
    writeFixture(
      root,
      waveAdmissionPath,
      JSON.stringify(waveAdmissionSource(overrides.waveAdmission), null, 2),
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

test("Forum Wave plan sync accepts canonical source-ready admitted-input cursor", () => {
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

test("Forum Wave plan sync rejects missing accepted Pages gate packet contract", () => {
  const result = run(
    plan(),
    evidence({ observed_run: { accepted_gate_evidence: { required: false } } }),
  );
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /explicit accepted Pages gate packet/);
});

test("Forum Wave plan sync rejects Wave admission source drift", () => {
  const result = run(plan(), evidence(), {
    waveAdmission: {
      output: { status: "unexpected" },
    },
  });
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /Wave admission source identity\/cursor drifted/);
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

test("Forum Wave plan sync rejects missing Wave admission source registration", () => {
  const result = run(
    plan(),
    evidence({ static_readiness: { source_contracts: [verifierContractPath] } }),
  );
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /source contracts must register/);
});

test("Forum Wave plan sync rejects missing Wave admission verification registration", () => {
  const result = run(
    plan(),
    evidence({
      verification: {
        no_compile_gates: [
          `node ${verifierContractPath}`,
          `node ${verifierTestContractPath}`,
        ],
      },
    }),
  );
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /no-compile verification set is missing/);
});
