#!/usr/bin/env node

import assert from "node:assert/strict";
import { spawn, execFileSync } from "node:child_process";
import { createServer } from "node:http";
import {
  existsSync,
  mkdirSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";
import test from "node:test";

const repoRoot = fileURLToPath(new URL("../../", import.meta.url));
const runnerPath = path.join(
  repoRoot,
  "scripts/evidence/execute-forum-page-builder-ffa-fba-promotion.mjs",
);
const targetRoot = path.join(repoRoot, "target");
const head = execFileSync("git", ["rev-parse", "HEAD"], {
  cwd: repoRoot,
  encoding: "utf8",
}).trim();
const deploymentDigest = `ghcr.io/rustok/server@sha256:${"a".repeat(64)}`;
const tenantSlug = "promotion-test-tenant";
const authToken = "synthetic-promotion-token";

let sequence = 0;

function canonicalize(value) {
  if (Array.isArray(value)) return value.map(canonicalize);
  if (value !== null && typeof value === "object") {
    return Object.fromEntries(
      Object.entries(value)
        .sort(([left], [right]) => left.localeCompare(right))
        .map(([key, nested]) => [key, canonicalize(nested)]),
    );
  }
  return value;
}

function canonicalJson(value) {
  return JSON.stringify(canonicalize(value));
}

function initialSettings() {
  return {
    unrelated: {
      retained: "private-setting-value",
      count: 7,
    },
    builder: {
      enabled: false,
      preview: { enabled: false, retained: "preview-marker" },
      properties: { enabled: false },
      publish: { enabled: false },
      retained: "builder-marker",
    },
  };
}

function allOnSettings() {
  const value = initialSettings();
  value.builder.enabled = true;
  value.builder.preview.enabled = true;
  value.builder.properties.enabled = true;
  value.builder.publish.enabled = true;
  return value;
}

function approvedReview(overrides = {}) {
  const now = Date.now();
  const review = {
    format: "forum_page_builder_ffa_fba_promotion_review_v1",
    status: "owner_approved_ffa_fba_promotion_review_execution_pending",
    reviewed_at: new Date(now - 60_000).toISOString(),
    source_commit: head,
    deployment_image_digest: deploymentDigest,
    observed_acceptance: {
      bytes: 2048,
      sha256: "b".repeat(64),
      reviewed_at: new Date(now - 120_000).toISOString(),
      wave_created_at: new Date(now - 180_000).toISOString(),
      wave_next_due_at: new Date(now + 60 * 60 * 1000).toISOString(),
      prior_owner_decision: "accept_observed_wave_evidence",
      freshness_verifier_passed_at_prior_review: true,
      admission_lineage_verifier_passed_at_prior_review: true,
    },
    promotion_review: {
      owner_id: "synthetic-maintainer",
      decision: "approve_ffa_fba_promotion_review",
      targets: ["ffa", "fba"],
      identity_is_operator_assertion: true,
      cryptographic_signature_verified: false,
    },
    boundaries: {
      review_only: true,
      approval_is_not_control_plane_execution: true,
      control_plane_or_rollout_mutated: false,
      pages_or_forum_persistence_mutated: false,
      current_provider_health_asserted: false,
      cryptographic_origin_to_repo_digest_binding_claimed: false,
      forum_wave_promoted: false,
      ffa_promoted: false,
      fba_promoted: false,
      separate_control_plane_execution_required: true,
    },
    privacy: {
      raw_input_path_persisted: false,
      raw_metrics_or_trace_values_persisted: false,
      forum_content_persisted: false,
      tenant_or_actor_identifiers_persisted: false,
      free_text_reason_persisted: false,
    },
  };
  return deepMerge(review, overrides);
}

function deepMerge(base, overrides) {
  if (
    base === null ||
    typeof base !== "object" ||
    Array.isArray(base) ||
    overrides === null ||
    typeof overrides !== "object" ||
    Array.isArray(overrides)
  ) {
    return overrides;
  }
  const merged = { ...base };
  for (const [key, value] of Object.entries(overrides)) {
    merged[key] =
      key in base &&
      base[key] !== null &&
      typeof base[key] === "object" &&
      !Array.isArray(base[key]) &&
      value !== null &&
      typeof value === "object" &&
      !Array.isArray(value)
        ? deepMerge(base[key], value)
        : value;
  }
  return merged;
}

async function readRequestBody(request) {
  const chunks = [];
  let total = 0;
  for await (const chunk of request) {
    total += chunk.length;
    if (total > 1024 * 1024) throw new Error("synthetic request exceeded 1 MiB");
    chunks.push(chunk);
  }
  return Buffer.concat(chunks).toString("utf8");
}

function sendJson(response, document, status = 200) {
  const body = JSON.stringify(document);
  response.writeHead(status, {
    "content-type": "application/json",
    "content-length": Buffer.byteLength(body),
  });
  response.end(body);
}

function conflictEnvelope() {
  return {
    errors: [
      {
        message: "Module settings changed since the reviewed snapshot",
        extensions: {
          code: "MODULE_SETTINGS_SNAPSHOT_CONFLICT",
          retryable_issue: false,
          requires_rereview: true,
        },
      },
    ],
    data: null,
  };
}

async function startGraphqlServer(scenario, initial = initialSettings()) {
  const state = {
    settings: structuredClone(initial),
    original: structuredClone(initial),
    mutationCount: 0,
    requestCount: 0,
    authorizationSeen: false,
    tenantSeen: false,
  };

  const server = createServer(async (request, response) => {
    state.requestCount += 1;
    try {
      if (request.method !== "POST" || request.url !== "/api/graphql") {
        sendJson(response, { error: "not found" }, 404);
        return;
      }
      state.authorizationSeen = request.headers.authorization === `Bearer ${authToken}`;
      state.tenantSeen = request.headers["x-tenant-slug"] === tenantSlug;
      if (!state.authorizationSeen || !state.tenantSeen) {
        sendJson(response, { errors: [{ message: "unauthorized" }], data: null }, 401);
        return;
      }

      const envelope = JSON.parse(await readRequestBody(request));
      const query = String(envelope.query ?? "");
      const variables = envelope.variables ?? {};

      if (query.includes("tenantModules")) {
        sendJson(response, {
          data: {
            tenantModules: [
              {
                moduleSlug: "pages",
                enabled: true,
                settings: JSON.stringify(state.settings),
              },
            ],
          },
        });
        return;
      }

      if (query.includes("compareAndSwapModuleSettings")) {
        state.mutationCount += 1;
        assert.equal(variables.moduleSlug, "pages");
        assert.equal(variables.expectedEnabled, true);
        const expected = JSON.parse(variables.expectedSettings);
        const requested = JSON.parse(variables.settings);

        if (scenario === "conflict" && state.mutationCount === 1) {
          sendJson(response, conflictEnvelope());
          return;
        }
        if (scenario === "ambiguous" && state.mutationCount === 1) {
          sendJson(response, {
            errors: [
              {
                message: "synthetic unexpected mutation failure",
                extensions: { code: "INTERNAL_SERVER_ERROR" },
              },
            ],
            data: null,
          });
          return;
        }
        if (scenario === "rollback_conflict" && state.mutationCount === 2) {
          state.settings = { ...state.settings, concurrent_marker: true };
          sendJson(response, conflictEnvelope());
          return;
        }
        if (canonicalJson(expected) !== canonicalJson(state.settings)) {
          sendJson(response, conflictEnvelope());
          return;
        }

        state.settings = structuredClone(requested);
        sendJson(response, {
          data: {
            compareAndSwapModuleSettings: {
              moduleSlug: "pages",
              enabled: true,
              settings: JSON.stringify(state.settings),
            },
          },
        });
        return;
      }

      if (query.includes("pageBuilderRolloutSnapshot")) {
        const builder = state.settings.builder ?? {};
        const forceBad =
          scenario === "postcondition_rollback" || scenario === "rollback_conflict";
        sendJson(response, {
          data: {
            pageBuilderRolloutSnapshot: {
              tenantSlug,
              builderEnabled: forceBad ? false : builder.enabled === true,
              previewEnabled: builder.preview?.enabled === true,
              propertiesEnabled: builder.properties?.enabled === true,
              publishEnabled: builder.publish?.enabled === true,
              providerHealthObserved: false,
            },
          },
        });
        return;
      }

      sendJson(response, { errors: [{ message: "unknown operation" }], data: null });
    } catch (error) {
      sendJson(response, { errors: [{ message: error.message }], data: null }, 500);
    }
  });

  await new Promise((resolve, reject) => {
    server.once("error", reject);
    server.listen(0, "127.0.0.1", resolve);
  });
  const address = server.address();
  assert(address && typeof address === "object");
  return {
    origin: `http://127.0.0.1:${address.port}`,
    state,
    async close() {
      await new Promise((resolve, reject) =>
        server.close((error) => (error ? reject(error) : resolve())),
      );
    },
  };
}

function fixturePaths(label) {
  sequence += 1;
  mkdirSync(targetRoot, { recursive: true });
  const safe = label.replace(/[^a-z0-9]+/giu, "-").toLowerCase();
  const review = path.join(tmpdir(), `rustok-promotion-review-${process.pid}-${sequence}-${safe}.json`);
  const output = path.join(targetRoot, `promotion-execution-test-${process.pid}-${sequence}-${safe}.json`);
  rmSync(review, { force: true });
  rmSync(output, { force: true });
  return { review, output };
}

async function runRunner({ scenario, review = approvedReview(), initial = initialSettings() }) {
  const fixture = fixturePaths(scenario);
  writeFileSync(fixture.review, `${JSON.stringify(review, null, 2)}\n`, "utf8");
  const synthetic = await startGraphqlServer(scenario, initial);
  try {
    const child = spawn(
      process.execPath,
      [runnerPath, "--promotion-review", fixture.review, "--output", fixture.output],
      {
        cwd: repoRoot,
        env: {
          ...process.env,
          RUSTOK_FORUM_FFA_FBA_PROMOTION_API_ORIGIN: synthetic.origin,
          RUSTOK_FORUM_FFA_FBA_PROMOTION_TENANT_SLUG: tenantSlug,
          RUSTOK_FORUM_FFA_FBA_PROMOTION_AUTH_TOKEN: authToken,
          RUSTOK_FORUM_FFA_FBA_PROMOTION_DEPLOYMENT_IMAGE_DIGEST: deploymentDigest,
        },
        stdio: ["ignore", "pipe", "pipe"],
      },
    );
    let stdout = "";
    let stderr = "";
    child.stdout.setEncoding("utf8");
    child.stderr.setEncoding("utf8");
    child.stdout.on("data", (chunk) => {
      stdout += chunk;
    });
    child.stderr.on("data", (chunk) => {
      stderr += chunk;
    });
    const code = await new Promise((resolve, reject) => {
      child.once("error", reject);
      child.once("close", resolve);
    });
    const output = existsSync(fixture.output)
      ? JSON.parse(readFileSync(fixture.output, "utf8"))
      : null;
    return { code, stdout, stderr, output, fixture, synthetic };
  } finally {
    await synthetic.close();
    rmSync(fixture.review, { force: true });
  }
}

function cleanup(result) {
  rmSync(result.fixture.output, { force: true });
}

function assertPrivacy(receipt) {
  const serialized = JSON.stringify(receipt);
  assert.equal(serialized.includes(authToken), false);
  assert.equal(serialized.includes(tenantSlug), false);
  assert.equal(serialized.includes("private-setting-value"), false);
  assert.equal(serialized.includes("preview-marker"), false);
  assert.equal(receipt.readiness.ffa_promoted, false);
  assert.equal(receipt.readiness.fba_promoted, false);
  assert.equal(receipt.readiness.registry_or_local_plan_status_mutated, false);
}

test("executes approved all_on promotion through CAS and preserves unrelated settings", async () => {
  const result = await runRunner({ scenario: "success" });
  try {
    assert.equal(result.code, 0, result.stderr);
    assert(result.output);
    assert.equal(result.output.status, "control_plane_change_executed_readiness_promotion_pending");
    assert.equal(result.output.mutation.outcome, "confirmed");
    assert.equal(result.output.postcondition.passed, true);
    assert.equal(result.output.boundaries.control_plane_change_executed, true);
    assert.equal(result.output.boundaries.readiness_board_mutated, false);
    assert.equal(result.synthetic.state.settings.unrelated.retained, "private-setting-value");
    assert.equal(result.synthetic.state.settings.builder.retained, "builder-marker");
    assert.equal(result.synthetic.state.settings.builder.preview.retained, "preview-marker");
    assert.equal(result.synthetic.state.settings.builder.enabled, true);
    assert.equal(result.synthetic.state.settings.builder.publish.enabled, true);
    assert.equal(result.synthetic.state.mutationCount, 1);
    assert.equal(result.synthetic.state.authorizationSeen, true);
    assert.equal(result.synthetic.state.tenantSeen, true);
    assertPrivacy(result.output);
  } finally {
    cleanup(result);
  }
});

test("rejects non-approved promotion review before target requests", async () => {
  const result = await runRunner({
    scenario: "review_rejected",
    review: approvedReview({ status: "owner_rejected_ffa_fba_promotion_review" }),
  });
  try {
    assert.notEqual(result.code, 0);
    assert.match(result.stderr, /requires an approved FFA\/FBA promotion-review packet/u);
    assert.equal(result.synthetic.state.requestCount, 0);
    assert.equal(result.output, null);
  } finally {
    cleanup(result);
  }
});

test("rejects promotion review source-commit drift before target requests", async () => {
  const result = await runRunner({
    scenario: "source_drift",
    review: approvedReview({ source_commit: "c".repeat(40) }),
  });
  try {
    assert.notEqual(result.code, 0);
    assert.match(result.stderr, /source_commit does not equal checkout HEAD/u);
    assert.equal(result.synthetic.state.requestCount, 0);
  } finally {
    cleanup(result);
  }
});

test("rejects promotion review deployment RepoDigest drift before target requests", async () => {
  const result = await runRunner({
    scenario: "digest_drift",
    review: approvedReview({
      deployment_image_digest: `ghcr.io/rustok/server@sha256:${"d".repeat(64)}`,
    }),
  });
  try {
    assert.notEqual(result.code, 0);
    assert.match(result.stderr, /execution deployment RepoDigest does not equal approved promotion review/u);
    assert.equal(result.synthetic.state.requestCount, 0);
  } finally {
    cleanup(result);
  }
});

test("rejects stale observed Wave lease before target requests", async () => {
  const result = await runRunner({
    scenario: "stale_review",
    review: approvedReview({
      observed_acceptance: {
        wave_next_due_at: new Date(Date.now() - 60_000).toISOString(),
      },
    }),
  });
  try {
    assert.notEqual(result.code, 0);
    assert.match(result.stderr, /retained observed Wave lease expired/u);
    assert.equal(result.synthetic.state.requestCount, 0);
  } finally {
    cleanup(result);
  }
});

test("rejects already-all_on target as non-evidence without mutation", async () => {
  const result = await runRunner({ scenario: "already", initial: allOnSettings() });
  try {
    assert.notEqual(result.code, 0);
    assert.match(result.stderr, /already match all_on/u);
    assert.equal(result.synthetic.state.mutationCount, 0);
    assert.equal(result.output, null);
  } finally {
    cleanup(result);
  }
});

test("records CAS snapshot conflict and requires re-review without rollback", async () => {
  const result = await runRunner({ scenario: "conflict" });
  try {
    assert.notEqual(result.code, 0);
    assert(result.output);
    assert.equal(result.output.status, "control_plane_change_snapshot_conflict_rereview_required");
    assert.equal(result.output.mutation.outcome, "snapshot_conflict");
    assert.equal(result.output.mutation.requires_rereview, true);
    assert.equal(result.output.rollback.attempted, false);
    assert.equal(result.synthetic.state.mutationCount, 1);
    assert.deepEqual(result.synthetic.state.settings, result.synthetic.state.original);
    assertPrivacy(result.output);
  } finally {
    cleanup(result);
  }
});

test("rolls back confirmed mutation when postcondition fails and retains rolled-back receipt", async () => {
  const result = await runRunner({ scenario: "postcondition_rollback" });
  try {
    assert.notEqual(result.code, 0);
    assert(result.output);
    assert.equal(result.output.status, "control_plane_change_postcondition_failed_rolled_back");
    assert.equal(result.output.mutation.outcome, "confirmed");
    assert.equal(result.output.postcondition.passed, false);
    assert.equal(result.output.rollback.outcome, "confirmed_restored");
    assert.equal(result.output.rollback.net_target_state_retained, false);
    assert.equal(result.synthetic.state.mutationCount, 2);
    assert.deepEqual(result.synthetic.state.settings, result.synthetic.state.original);
    assertPrivacy(result.output);
  } finally {
    cleanup(result);
  }
});

test("records manual reconciliation when rollback CAS conflicts", async () => {
  const result = await runRunner({ scenario: "rollback_conflict" });
  try {
    assert.notEqual(result.code, 0);
    assert(result.output);
    assert.equal(result.output.status, "control_plane_change_requires_manual_reconciliation");
    assert.equal(result.output.mutation.outcome, "confirmed");
    assert.equal(result.output.rollback.outcome, "snapshot_conflict");
    assert.equal(result.output.manual_reconciliation_required, true);
    assert.equal(result.synthetic.state.mutationCount, 2);
    assert.equal(result.synthetic.state.settings.concurrent_marker, true);
    assertPrivacy(result.output);
  } finally {
    cleanup(result);
  }
});

test("records ambiguous mutation without automatic rollback", async () => {
  const result = await runRunner({ scenario: "ambiguous" });
  try {
    assert.notEqual(result.code, 0);
    assert(result.output);
    assert.equal(result.output.status, "control_plane_change_requires_manual_reconciliation");
    assert.equal(result.output.mutation.outcome, "ambiguous");
    assert.equal(result.output.rollback.attempted, false);
    assert.equal(result.output.rollback.reason, "ambiguous_mutation_outcome_must_not_auto_rollback");
    assert.equal(result.synthetic.state.mutationCount, 1);
    assert.deepEqual(result.synthetic.state.settings, result.synthetic.state.original);
    assertPrivacy(result.output);
  } finally {
    cleanup(result);
  }
});
