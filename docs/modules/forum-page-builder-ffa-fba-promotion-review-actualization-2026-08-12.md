# Forum Page Builder FFA/FBA promotion review actualization — 2026-08-12

## Base rechecked

Promotion review source was introduced after `main@5451b4ebb00d79923288588bc0c2622f1b2341e7`. The promotion-execution continuation is rechecked from fresh `main@85c5f608882a523c8583c832c679d59b91e6ba98`, including the promotion-safe storage/lifecycle/service/GraphQL CAS chain merged through PR #3477.

## Gap rechecked

PR #3465 made the retrospective observed-Wave owner packet explicit and deliberately stopped before FFA/FBA promotion. Its accepted output, `forum_page_builder_wave_observed_acceptance_v1` with status `owner_accepted_observed_control_plane_wave_promotion_review_pending`, is eligible input for a separate explicit promotion review.

The promotion-review source then established the separate decision packet, but intentionally stopped at `owner_approved_ffa_fba_promotion_review_execution_pending`. Subsequent module-control-plane work added an atomic settings compare-and-swap from storage through lifecycle ownership, a server-reviewed rollout service and the `compareAndSwapModuleSettings` GraphQL transport.

That leaves one source gap before maintainers can execute the approved change safely: a fail-closed execution harness that binds the approved review packet to the exact checkout/deployment, reads the current Pages settings snapshot, performs only the reviewed Page Builder transition through CAS, verifies the authoritative postcondition and retains bounded rollback evidence without conflating the tenant rollout with readiness-board promotion.

## Promotion-review source slice

Status: `forum-ffa-fba-promotion-review-source-ready / maintainer-execution-pending`.

The review layer consists of:

- `crates/rustok-forum/contracts/evidence/forum-page-builder-ffa-fba-promotion-review-source.json`;
- `scripts/evidence/review-forum-page-builder-ffa-fba-promotion.mjs`;
- `scripts/evidence/review-forum-page-builder-ffa-fba-promotion.test.mjs`;
- `scripts/verify/verify-forum-page-builder-ffa-fba-promotion-review.mjs`.

The promotion-review runner accepts only an **accepted observed-Wave owner packet** and fails closed unless:

- input format/status is exactly `forum_page_builder_wave_observed_acceptance_v1` / `owner_accepted_observed_control_plane_wave_promotion_review_pending`;
- retained `source_commit` equals checkout `HEAD`;
- retained deployment identity is a canonical `REPOSITORY@sha256:<64 lowercase hex>` RepoDigest;
- the prior owner decision remains exactly `accept_observed_wave_evidence`;
- the prior packet retained successful freshness and exact admission-lineage verification;
- the observed Wave `next_due_at` is still in the future at promotion-review time;
- the prior acceptance still records no control-plane/rollout mutation, no FFA/FBA promotion and no current-provider-health or cryptographic-origin overclaim;
- privacy non-claims remain fail closed.

The only promotion-review decisions are `approve_ffa_fba_promotion_review` and `reject`. Owner identity is a bounded operator assertion, not a cryptographic signature.

An approved result means only that the explicit FFA/FBA control-plane change is approved for a **separate execution step**. It **does not mutate rollout**, does not modify Pages or Forum persistence, and **does not promote FFA/FBA**. Output status is `owner_approved_ffa_fba_promotion_review_execution_pending`; actual control-plane execution remains maintainer-owned.

A rejected result is retained as `owner_rejected_ffa_fba_promotion_review` and likewise performs no rollout mutation.

## Promotion-execution source slice

Status: `forum-ffa-fba-promotion-execution-source-ready / maintainer-live-execution-pending / readiness-governance-pending`.

The execution layer consists of:

- `crates/rustok-forum/contracts/evidence/forum-page-builder-ffa-fba-promotion-execution-source.json`;
- `scripts/evidence/execute-forum-page-builder-ffa-fba-promotion.mjs`;
- `scripts/evidence/execute-forum-page-builder-ffa-fba-promotion.test.mjs`;
- `scripts/verify/verify-forum-page-builder-ffa-fba-promotion-execution.mjs`.

The production runner requires an exact approved `forum_page_builder_ffa_fba_promotion_review_v1` packet with status `owner_approved_ffa_fba_promotion_review_execution_pending`. It rechecks checkout `HEAD`, the immutable deployment RepoDigest, the approved review decision/targets, retained freshness/admission-lineage facts and the observed Wave lease at **execution time**. A review whose retained Wave lease has expired is not reusable.

Target credentials and routing identity come only from bounded environment values. The operator must have both `modules:manage` for the CAS write and `pages:read` for the authoritative `pageBuilderRolloutSnapshot` postcondition. The runner accepts no approval boolean or reviewer identity as a transport credential.

The reviewed tenant change is deliberately narrow:

1. read the enabled `pages` row and its static lifecycle revision through `tenantModules`;
2. retain only semantic hashes of the full current settings and target origin/tenant identity;
3. deep-clone the complete settings document and change only `builder.enabled`, `builder.preview.enabled`, `builder.properties.enabled` and `builder.publish.enabled` to the reviewed `all_on` profile;
4. preserve every non-Builder and unknown-to-the-runner setting exactly;
5. call only `compareAndSwapModuleSettings` with `expectedEnabled=true`, the exact full pre-write settings snapshot, its current lifecycle revision, and a fresh idempotency key;
6. verify the returned applied settings through a fresh `tenantModules` read and verify the four authoritative flags through `pageBuilderRolloutSnapshot`.

The ordinary unconditional `updateModuleSettings` command is not an execution fallback. A `MODULE_SETTINGS_SNAPSHOT_CONFLICT` with `requires_rereview=true` produces `control_plane_change_snapshot_conflict_rereview_required`, performs no rollback and requires a fresh read plus a fresh promotion review. This prevents a stale approval from overwriting a newer settings or enablement decision.

Rollback is also CAS-only. It is attempted only when the promotion mutation is **confirmed** but the authoritative postcondition fails. Rollback expects the exact confirmed applied settings and lifecycle revision, then restores the exact original snapshot with a fresh idempotency key. A confirmed restore produces `control_plane_change_postcondition_failed_rolled_back` and the execution still fails because the promoted state did not remain active. A rollback conflict or ambiguous rollback outcome produces `control_plane_change_requires_manual_reconciliation`.

An ambiguous initial mutation outcome — transport failure, non-200 response, malformed response or non-conflict GraphQL error — is never followed by an automatic rollback because the runner cannot safely know whether the write committed. It emits a bounded manual-reconciliation receipt instead.

A target that is already semantically `all_on` is not accepted as new execution evidence. The runner must observe and perform the reviewed state transition itself.

Successful output status is `control_plane_change_executed_readiness_promotion_pending`. The receipt retains source hashes, review hash, source commit, RepoDigest, API-origin hash, tenant-slug hash, settings semantic hashes and bounded GraphQL response hashes/byte lengths. It does not retain raw settings, raw GraphQL bodies, credentials, cookies, tenant identity, Forum content, metrics or traces.

## Readiness boundary

A successful tenant/control-plane write is **not** the FFA/FBA readiness-board promotion.

The execution runner never changes `docs/modules/registry.md`, a module-local FFA/FBA status block, or any other canonical source. Its success receipt keeps `ffa_promoted=false` and `fba_promoted=false` and records that a separate evidence-backed governance change is required.

FFA `parity_verified` and FBA `transport_verified` therefore remain blocked until maintainers have the complete accepted Pages/provider-health/Forum/Wave chain, an approved promotion review, a successful control-plane execution receipt and the explicit owner/platform governance review required by the readiness policy.

## Executable synthetic coverage

The promotion-review synthetic suite covers the review-only decision boundary. The promotion-execution suite exercises the production execution runner against a stateful localhost GraphQL fixture and covers:

1. approved `all_on` CAS execution while preserving unrelated settings and retaining no raw secrets/settings;
2. non-approved review rejected before any target request;
3. review source-commit drift rejected before any target request;
4. execution RepoDigest drift rejected before any target request;
5. stale observed-Wave lease rejected before any target request;
6. already-`all_on` state rejected as non-evidence without a mutation;
7. CAS snapshot conflict retained as re-review-required with no rollback;
8. confirmed mutation plus failed postcondition restored through CAS rollback with a rolled-back receipt;
9. rollback CAS conflict retained as manual reconciliation;
10. ambiguous initial mutation retained as manual reconciliation with no automatic rollback.

These fixtures use synthetic localhost transport only. They are not live Pages/Forum/control-plane evidence and cannot promote readiness.

## Anti-drift boundary

`verify-forum-page-builder-ffa-fba-promotion-review.mjs` continues to lock the review-only contract. `verify-forum-page-builder-ffa-fba-promotion-execution.mjs` separately locks the execution contract, CAS GraphQL/service ownership, permission requirements, exact reviewed profile, conflict/re-review semantics, rollback rules, privacy restrictions and the readiness-governance separation.

No workflow file is changed by this continuation slice.

## Non-claims / next cursor

This source work does not execute provider-health deployment evidence, the Pages reference candidate/gate, Forum browser/runtime/server-function evidence, Forum admission, the observed control-plane Wave, observed-Wave owner acceptance, the promotion review or the final control-plane mutation. `maintainer live execution remains pending`.

It also does not update FFA/FBA readiness statuses.

The live execution cursor is now:

1. execute and accept the exact Pages/provider-health gate chain;
2. execute Forum browser/runtime/server-function evidence and retain Forum admission on the same source/deployment boundary;
3. execute and retain the observed control-plane Wave;
4. pass freshness + exact retained-admission lineage and retain accepted observed-Wave owner evidence;
5. run the FFA/FBA promotion-review packet while the retained Wave is still fresh;
6. only after `owner_approved_ffa_fba_promotion_review_execution_pending`, run `execute-forum-page-builder-ffa-fba-promotion.mjs` against the exact reviewed deployment and retain a successful `forum_page_builder_ffa_fba_promotion_execution_v1` receipt;
7. only after that successful receipt, perform the separate evidence-backed owner/platform readiness governance review before changing FFA/FBA status to any higher readiness level.

No tests, Node verifiers, GraphQL/HTTP calls, live mutations, browser runs, workflows or CI were executed by this implementation slice.
