#!/usr/bin/env node

import { test } from "node:test";
import assert from "node:assert/strict";
import { mkdtempSync, mkdirSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { spawnSync } from "node:child_process";

const freshnessScriptPath = path.resolve("scripts/verify/verify-forum-wave-evidence-freshness.mjs");
const readinessScriptPath = path.resolve(
  "crates/rustok-page-builder/scripts/verify/verify-page-builder-consumer-readiness.mjs",
);
const waveAdmissionVerifier = "node scripts/verify/verify-forum-page-builder-wave-admission.mjs";
const requiredGates = [
  "npm run verify:page-builder:consumer:forum",
  waveAdmissionVerifier,
  "npm run verify:forum:wave-evidence-freshness",
  "npm run test:verify:forum:wave-evidence-freshness",
];
const requiredObservedSections = [
  "admission",
  "control_plane.audit_trail",
  "fallback.profiles",
  "observability.metrics",
  "observability.traces",
  "rollback.decision",
  "approvals",
  "waivers",
];
const sourceCommit = "0123456789abcdef0123456789abcdef01234567";
const deploymentImageDigest = `example.invalid/rustok-server@sha256:${"a".repeat(64)}`;
const admissionPacketSha256 = "b".repeat(64);

function expectedProfile(name, values) {
  return {
    name,
    expected_smoke: {
      list: "expected_pass",
      open: "expected_pass",
      preview: values.preview,
      save_draft: values.saveDraft,
      publish_dry: values.publish,
    },
    expected_read_guarantees: {
      admin_list_no_5xx: true,
      admin_read_no_5xx: true,
      storefront_read_no_5xx: true,
    },
  };
}

function sourceReadyEvidence(overrides = {}) {
  const base = {
    schema_version: 2,
    artifact: "page_builder_wave_evidence_packet",
    module_slug: "forum",
    wave: "1",
    mode: "source_ready",
    provenance: "synthetic_fixture",
    execution_status: "not_run_by_implementation_agent",
    prepared_on: "2026-07-24",
    static_readiness: {
      fallback_profiles: [
        expectedProfile("all_on", {
          preview: "expected_pass",
          saveDraft: "expected_pass",
          publish: "expected_pass",
        }),
        expectedProfile("publish_off", {
          preview: "expected_pass",
          saveDraft: "expected_pass",
          publish: "expected_typed_feature_disabled",
        }),
        expectedProfile("preview_off", {
          preview: "expected_typed_feature_disabled",
          saveDraft: "expected_pass",
          publish: "expected_typed_feature_disabled",
        }),
        expectedProfile("builder_off", {
          preview: "expected_typed_feature_disabled",
          saveDraft: "expected_readonly_fallback",
          publish: "expected_typed_feature_disabled",
        }),
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
      required_profiles: ["all_on", "publish_off", "preview_off", "builder_off"],
      required_evidence: [...requiredObservedSections],
    },
    verification: {
      no_compile_gates: [...requiredGates],
      execution_status: "not_run_by_implementation_agent",
    },
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

function observedProfile(name, values) {
  return {
    name,
    smoke: {
      list: "pass",
      open: "pass",
      preview: values.preview,
      save_draft: values.saveDraft,
      publish_dry: values.publish,
    },
    read_guarantees: {
      admin_list_no_5xx: true,
      admin_read_no_5xx: true,
      storefront_read_no_5xx: true,
    },
    decision: "keep",
  };
}

function liveEvidence(overrides = {}) {
  const requiredSections = [...requiredObservedSections, "refresh_history.latest_refresh"];
  const base = {
    artifact: "page_builder_wave_evidence_packet",
    module_slug: "forum",
    wave: "1",
    mode: "live",
    provenance: "observed_control_plane",
    execution_status: "maintainer_verified",
    source_commit: sourceCommit,
    deployment_image_digest: deploymentImageDigest,
    created_at: "2026-06-01T00:00:00Z",
    admission: {
      format: "forum_page_builder_wave_admission_v1",
      status: "forum_wave_inputs_admitted_observed_control_plane_pending",
      source_commit: sourceCommit,
      deployment_image_digest: deploymentImageDigest,
      packet_sha256: admissionPacketSha256,
      pages_reference_consumer_gate_accepted: true,
      exact_source_commit_bound: true,
      exact_deployment_digest_bound: true,
    },
    control_plane: { audit_trail: "control_plane_builder_wave_audit" },
    fallback: {
      profiles: [
        observedProfile("all_on", { preview: "pass", saveDraft: "pass", publish: "pass" }),
        observedProfile("publish_off", {
          preview: "pass",
          saveDraft: "pass",
          publish: "typed_feature_disabled_error",
        }),
        observedProfile("preview_off", {
          preview: "typed_feature_disabled_error",
          saveDraft: "pass",
          publish: "typed_feature_disabled_error",
        }),
        observedProfile("builder_off", {
          preview: "typed_feature_disabled_error",
          saveDraft: "readonly_fallback",
          publish: "typed_feature_disabled_error",
        }),
      ],
    },
    observability: {
      metrics: {
        preview_p95_ms: "live_wave1_actual:120",
        publish_p95_ms: "live_wave1_actual:230",
        sanitize_failure_rate: "live_wave1_actual:0.001",
        runtime_error_rate: "live_wave1_actual:0.000",
      },
      traces: {
        builder_write_to_forum_publish: "observed-trace-a",
        forum_publish_to_storefront_read: "observed-trace-b",
      },
    },
    rollback: { decision: "keep" },
    approvals: {
      platform_on_call: "approved",
      forum_owner: "approved",
      builder_owner: "approved",
      runtime_owner: "approved",
    },
    waivers: [],
    refresh_history: {
      latest_refresh: {
        refreshed_at: "2026-06-01T00:00:00Z",
        verified_by: "rustok-forum module team",
        no_compile_gates: [...requiredGates],
        sections_refreshed: [...requiredSections],
      },
    },
    refresh_policy: {
      cadence: "monthly",
      max_age_days: 45,
      next_due_at: "2026-07-01T00:00:00Z",
      required_gate: "npm run verify:page-builder:consumer:forum",
      stale_evidence_action: "block_builder_consumer_rollout_until_refreshed",
      owner: "rustok-forum module team",
      required_sections: [...requiredSections],
    },
  };
  return {
    ...base,
    ...overrides,
    admission: overrides.admission === null
      ? null
      : {
          ...base.admission,
          ...(overrides.admission ?? {}),
        },
    refresh_policy: {
      ...base.refresh_policy,
      ...(overrides.refresh_policy ?? {}),
    },
  };
}

function withEvidence(packet, assertion, now = "2026-06-20T00:00:00Z") {
  const root = mkdtempSync(path.join(tmpdir(), "rustok-forum-wave-provenance-"));
  const evidencePath = path.join(root, "forum-wave1-rollout-evidence.json");
  try {
    writeFileSync(evidencePath, JSON.stringify(packet, null, 2));
    const result = spawnSync("node", [freshnessScriptPath], {
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

function writeFixture(root, relativePath, content) {
  const filePath = path.join(root, relativePath);
  mkdirSync(path.dirname(filePath), { recursive: true });
  writeFileSync(filePath, content);
}

function readinessFixture(packet) {
  const root = mkdtempSync(path.join(tmpdir(), "rustok-forum-builder-readiness-"));
  writeFixture(
    root,
    "crates/rustok-forum/rustok-module.toml",
    `
page_builder = true
contract_version = "1.0"
builder_contract_version = "1.0"
[fba.builder_consumer.degraded_modes]
builder_disabled = "forum_widgets_readonly_keep_forum_routes"
preview_disabled = "forum_widget_preview_hidden_keep_forum_routes"
publish_disabled = "forum_widget_publish_feature_disabled_keep_forum_routes"
fallback_mode = "readonly"
# fallback_mode = "degraded"
# fallback_mode = "hidden"
builder_off = ["builder.enabled=false", "builder.publish.enabled=false"]
publish_off = ["builder.publish.enabled=false"]
[fba.builder_consumer.rollout_policy]
audit_trail = "control_plane_builder_wave_audit"
before_snapshot_required = true
after_snapshot_required = true
decision_required = true
owner_signoff_required = true
rollback_without_redeploy_target_minutes = 10
pilot_smoke = "list -> open -> preview -> save_draft -> publish_dry"
rollback_triggers = [
  "runtime_error_rate_above_alert_threshold",
  "publish_latency_p95_above_slo_for_10m",
  "sanitize_failures_above_alert_threshold",
  "storefront_published_read_regression",
]
read_surfaces_guarantee = "forum_owned_list_read_topic_paths_stay_available_when_builder_capabilities_are_disabled"
`,
  );
  writeFixture(
    root,
    "crates/rustok-forum/docs/implementation-plan.md",
    "## Current state\n## FFA/FBA status\nPage Builder readiness\n## Open results\n",
  );
  writeFixture(
    root,
    "crates/rustok-forum/contracts/evidence/fw2-fallback-static-matrix.json",
    JSON.stringify(
      {
        schema: "rustok.forum.fw2_fallback_static_matrix.v1",
        status: "design_static_ready",
        profiles: ["builder_off", "publish_off"],
        assertions: [
          {
            id: "forum-read-routes-survive-builder-off",
            expected_http_class: "non_5xx",
            source_markers: ["forum-read-source-marker"],
          },
          {
            id: "forum-moderation-routes-survive-publish-off",
            expected_http_class: "non_5xx",
            source_markers: ["forum-moderation-route-marker"],
          },
          {
            id: "forum-service-moderation-policy-stays-domain-owned",
            expected_http_class: "non_5xx",
            source_markers: ["forum-moderation-service-marker"],
          },
        ],
        deferred: ["deferred"],
      },
      null,
      2,
    ),
  );
  writeFixture(
    root,
    "crates/rustok-forum/src/controllers/mod.rs",
    "forum-read-source-marker\nforum-moderation-route-marker\n",
  );
  writeFixture(
    root,
    "crates/rustok-forum/src/services/moderation.rs",
    "forum-moderation-service-marker\n",
  );
  writeFixture(
    root,
    "crates/rustok-forum/contracts/evidence/forum-wave1-rollout-evidence.json",
    JSON.stringify(packet, null, 2),
  );
  return root;
}

function withReadiness(packet, assertion) {
  const root = readinessFixture(packet);
  try {
    const result = spawnSync("node", [readinessScriptPath, "forum"], {
      cwd: path.resolve("."),
      env: { ...process.env, RUSTOK_VERIFY_REPO_ROOT: root },
      encoding: "utf8",
    });
    assertion(result);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
}

test("forum Wave verifier accepts source-ready evidence with admitted-input cursor but no runtime claims", () => {
  withEvidence(sourceReadyEvidence(), (result) => {
    assert.equal(result.status, 0, result.stderr || result.stdout);
    assert.match(result.stdout, /mode=source_ready/);
  });
});

test("forum Wave verifier rejects missing source-ready accepted Pages gate evidence", () => {
  withEvidence(
    sourceReadyEvidence({ observed_run: { accepted_gate_evidence: { required: false } } }),
    (result) => {
      assert.notEqual(result.status, 0);
      assert.match(result.stderr, /explicit accepted Pages gate evidence/);
    },
  );
});

test("forum Wave verifier rejects live-only sections in source-ready evidence", () => {
  withEvidence(sourceReadyEvidence({ observability: { metrics: {} } }), (result) => {
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /must not materialize live-only section observability/);
  });
});

test("forum Wave verifier rejects false source-ready execution claims", () => {
  withEvidence(sourceReadyEvidence({ execution_status: "maintainer_verified" }), (result) => {
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /must not claim maintainer execution/);
  });
});

test("forum Wave verifier accepts fresh observed live evidence bound to admitted exact-source inputs", () => {
  withEvidence(liveEvidence(), (result) => {
    assert.equal(result.status, 0, result.stderr || result.stdout);
    assert.match(result.stdout, /mode=live/);
  });
});

test("forum Wave verifier rejects live evidence without admitted lineage", () => {
  withEvidence(liveEvidence({ admission: null }), (result) => {
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /admission must bind the exact admitted Pages\/Forum source and deployment lineage/);
  });
});

test("forum Wave verifier rejects synthetic provenance on a live packet", () => {
  withEvidence(liveEvidence({ provenance: "synthetic_fixture" }), (result) => {
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /provenance must be observed_control_plane/);
  });
});

test("forum Wave verifier rejects stale observed evidence", () => {
  withEvidence(liveEvidence(), (result) => {
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /past refresh_policy\.next_due_at/);
  }, "2026-07-02T00:00:00Z");
});

test("forum Wave verifier rejects missing observed evidence sections", () => {
  withEvidence(
    liveEvidence({ refresh_policy: { required_sections: ["admission", "control_plane.audit_trail"] } }),
    (result) => {
      assert.notEqual(result.status, 0);
      assert.match(result.stderr, /required_sections missing fallback\.profiles/);
    },
  );
});

test("Page Builder Forum readiness accepts source-ready provenance", () => {
  withReadiness(sourceReadyEvidence(), (result) => {
    assert.equal(result.status, 0, result.stderr || result.stdout);
    assert.match(result.stdout, /PASS/);
  });
});

test("Page Builder Forum readiness rejects source-ready live claims", () => {
  withReadiness(sourceReadyEvidence({ approvals: { forum_owner: "approved" } }), (result) => {
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /must not materialize live-only section 'approvals'/);
  });
});

test("Page Builder Forum readiness rejects fake live provenance", () => {
  withReadiness(liveEvidence({ provenance: "synthetic_fixture" }), (result) => {
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /provenance must be observed_control_plane/);
  });
});
