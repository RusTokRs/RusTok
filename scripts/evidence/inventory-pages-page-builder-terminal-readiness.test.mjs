#!/usr/bin/env node

import assert from "node:assert/strict";
import {
  collectPendingEvidence,
  evaluateInventory,
  registryHasRow,
} from "./inventory-pages-page-builder-terminal-readiness.mjs";

function caseRun(name, callback) {
  callback();
  console.log(`ok - ${name}`);
}

caseRun("collects every nested executed-evidence blocker as a stable JSON Pointer", () => {
  const fixture = {
    provider: {
      first: { executed_evidence: "pending" },
      second: { executed_evidence: "verified" },
    },
    consumers: [
      {
        module_slug: "pages",
        repair: {
          nested: { executed_evidence: "pending" },
        },
      },
    ],
  };
  assert.deepEqual(collectPendingEvidence(fixture).sort(), [
    "/consumers/0/repair/nested/executed_evidence",
    "/provider/first/executed_evidence",
  ]);
});

caseRun("escapes JSON Pointer tokens", () => {
  const fixture = {
    "a/b": {
      "x~y": {
        executed_evidence: "pending",
      },
    },
  };
  assert.deepEqual(collectPendingEvidence(fixture), [
    "/a~1b/x~0y/executed_evidence",
  ]);
});

caseRun("ignores unrelated pending values", () => {
  const fixture = {
    status: "pending",
    nested: {
      evidence: "pending",
      executed_evidence: "verified",
    },
  };
  assert.deepEqual(collectPendingEvidence(fixture), []);
});

caseRun("remains incomplete while Page Builder FBA blockers exist", () => {
  assert.deepEqual(
    evaluateInventory({
      prerequisiteValid: true,
      pendingEvidencePaths: ["/provider/x/executed_evidence"],
      pagesRolloutPending: false,
    }),
    {
      complete: false,
      owner_platform_review_ready: false,
      page_builder_fba_complete: false,
      pages_ffa_complete: true,
    },
  );
});

caseRun("remains incomplete while Pages execution rollout marker exists", () => {
  assert.deepEqual(
    evaluateInventory({
      prerequisiteValid: true,
      pendingEvidencePaths: [],
      pagesRolloutPending: true,
    }),
    {
      complete: false,
      owner_platform_review_ready: false,
      page_builder_fba_complete: true,
      pages_ffa_complete: false,
    },
  );
});

caseRun("requires a valid predecessor even when canonical blockers are clear", () => {
  assert.deepEqual(
    evaluateInventory({
      prerequisiteValid: false,
      pendingEvidencePaths: [],
      pagesRolloutPending: false,
    }),
    {
      complete: false,
      owner_platform_review_ready: false,
      page_builder_fba_complete: true,
      pages_ffa_complete: true,
    },
  );
});

caseRun("becomes review-ready only when predecessor and both canonical blocker sets are clear", () => {
  assert.deepEqual(
    evaluateInventory({
      prerequisiteValid: true,
      pendingEvidencePaths: [],
      pagesRolloutPending: false,
    }),
    {
      complete: true,
      owner_platform_review_ready: true,
      page_builder_fba_complete: true,
      pages_ffa_complete: true,
    },
  );
});

caseRun("does not infer readiness from a reduced but nonzero blocker set", () => {
  const paths = [
    "/provider/consumer_properties_contract/executed_evidence",
    "/consumers/0/cache_consumer/executed_evidence",
  ];
  const result = evaluateInventory({
    prerequisiteValid: true,
    pendingEvidencePaths: paths,
    pagesRolloutPending: false,
  });
  assert.equal(result.complete, false);
  assert.equal(result.owner_platform_review_ready, false);
});

caseRun("matches a required registry row by cells, not by column padding", () => {
  const required =
    "| `page_builder` | admin + storefront | `in_progress` | `boundary_ready` | `core_transport_ui` |";
  const padded = [
    "| Module slug      | UI surfaces        | FFA status    | FBA status       | Structural shape    | Source plan |",
    "| ---------------- | ------------------ | ------------- | ---------------- | ------------------- | ----------- |",
    "| `page_builder`   | admin + storefront | `in_progress` | `boundary_ready` | `core_transport_ui` | [Live plan] |",
  ].join("\n");
  assert.equal(registryHasRow(padded, required), true);
});

caseRun("rejects a registry row whose status or shape differs", () => {
  const required =
    "| `page_builder` | admin + storefront | `in_progress` | `boundary_ready` | `core_transport_ui` |";
  const stale =
    "| `page_builder` | no module-owned UI | `not_started` | `boundary_ready` | `no_ui_boundary` | [Live plan] |";
  assert.equal(registryHasRow(stale, required), false);
  // A row of another module with the same statuses must not satisfy the precondition.
  const otherModule =
    "| `pages` | admin + storefront | `in_progress` | `boundary_ready` | `core_transport_ui` | [Live plan] |";
  assert.equal(registryHasRow(otherModule, required), false);
});

caseRun("rejects a malformed required row instead of matching anything", () => {
  const registry =
    "| `page_builder` | admin + storefront | `in_progress` | `boundary_ready` | `core_transport_ui` | [Live plan] |";
  assert.equal(registryHasRow(registry, undefined), false);
  assert.equal(registryHasRow(registry, ""), false);
  assert.equal(registryHasRow(registry, "| `page_builder` |"), false);
});
