#!/usr/bin/env node

import { test } from "node:test";
import assert from "node:assert/strict";
import { mkdtempSync, mkdirSync, writeFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { spawnSync } from "node:child_process";

const scriptPath = path.resolve("scripts/verify/verify-forum-wave-evidence-freshness.mjs");

function evidence(overrides = {}) {
  const base = {
    module_slug: "forum",
    wave: "1",
    mode: "live",
    created_at: "2026-06-01T00:00:00Z",
    control_plane: { audit_trail: "control_plane_builder_wave_audit" },
    fallback: { profiles: [{ name: "all_on" }] },
    observability: { metrics: { preview_p95_ms: "live_wave1_actual:120" }, traces: { builder_write_to_forum_publish: "trace" } },
    rollback: { decision: "keep" },
    approvals: { platform_on_call: "approved" },
    waivers: [],
    refresh_history: {
      latest_refresh: {
        refreshed_at: "2026-06-01T00:00:00Z",
        verified_by: "rustok-forum module team",
        no_compile_gates: [
          "npm run verify:page-builder:consumer:forum",
          "npm run verify:forum:wave-evidence-freshness",
          "npm run test:verify:forum:wave-evidence-freshness",
        ],
        sections_refreshed: [
          "control_plane.audit_trail",
          "fallback.profiles",
          "observability.metrics",
          "observability.traces",
          "rollback.decision",
          "approvals",
          "waivers",
          "refresh_history.latest_refresh",
        ],
      },
    },
    refresh_policy: {
      cadence: "monthly",
      max_age_days: 45,
      next_due_at: "2026-07-01T00:00:00Z",
      required_gate: "npm run verify:page-builder:consumer:forum",
      stale_evidence_action: "block_builder_consumer_rollout_until_refreshed",
      owner: "rustok-forum module team",
      required_sections: [
        "control_plane.audit_trail",
        "fallback.profiles",
        "observability.metrics",
        "observability.traces",
        "rollback.decision",
        "approvals",
        "waivers",
        "refresh_history.latest_refresh",
      ],
    },
  };
  return {
    ...base,
    ...overrides,
    refresh_policy: {
      ...base.refresh_policy,
      ...(overrides.refresh_policy ?? {}),
    },
  };
}

function withEvidence(packet, assertion, now = "2026-06-20T00:00:00Z") {
  const root = mkdtempSync(path.join(tmpdir(), "rustok-forum-wave-freshness-"));
  const evidencePath = path.join(root, "forum-wave1-rollout-evidence.json");
  try {
    writeFileSync(evidencePath, JSON.stringify(packet, null, 2));
    const result = spawnSync("node", [scriptPath], {
      cwd: path.resolve("."),
      env: {
        ...process.env,
        RUSTOK_FORUM_WAVE_EVIDENCE_PATH: evidencePath,
        RUSTOK_VERIFY_NOW: now,
      },
      encoding: "utf8",
    });
    assertion(result);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
}

test("forum wave evidence freshness verifier passes fresh fixture", () => {
  withEvidence(evidence(), (result) => {
    assert.equal(result.status, 0, result.stderr || result.stdout);
    assert.match(result.stdout, /PASS/);
  });
});

test("forum wave evidence freshness verifier rejects stale next_due_at", () => {
  withEvidence(evidence(), (result) => {
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /past refresh_policy\.next_due_at/);
  }, "2026-07-02T00:00:00Z");
});

test("forum wave evidence freshness verifier rejects too-wide max-age window", () => {
  withEvidence(evidence({ refresh_policy: { next_due_at: "2026-08-01T00:00:00Z" } }), (result) => {
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /must not exceed max_age_days/);
  });
});

test("forum wave evidence freshness verifier rejects missing required refresh section", () => {
  withEvidence(
    evidence({ refresh_policy: { required_sections: ["control_plane.audit_trail"] } }),
    (result) => {
      assert.notEqual(result.status, 0);
      assert.match(result.stderr, /required_sections missing fallback\.profiles/);
    },
  );
});


test("forum wave evidence freshness verifier rejects missing actual refresh section", () => {
  const packet = evidence({ fallback: null });
  withEvidence(packet, (result) => {
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /evidence packet missing required refresh section fallback\.profiles/);
  });
});

test("forum wave evidence freshness verifier rejects empty materialized refresh sections", () => {
  const packet = evidence({ observability: { metrics: {}, traces: { builder_write_to_forum_publish: "trace" } } });
  withEvidence(packet, (result) => {
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /observability\.metrics must be a non-empty object/);
  });
});

test("forum wave evidence freshness verifier rejects refresh history gate drift", () => {
  const packet = evidence({
    refresh_history: {
      latest_refresh: {
        refreshed_at: "2026-06-01T00:00:00Z",
        verified_by: "rustok-forum module team",
        no_compile_gates: ["npm run verify:page-builder:consumer:forum"],
        sections_refreshed: [
          "control_plane.audit_trail",
          "fallback.profiles",
          "observability.metrics",
          "observability.traces",
          "rollback.decision",
          "approvals",
          "waivers",
          "refresh_history.latest_refresh",
        ],
      },
    },
  });
  withEvidence(packet, (result) => {
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /no_compile_gates missing npm run verify:forum:wave-evidence-freshness/);
  });
});
