# Forum Page Builder FFA/FBA promotion review actualization — 2026-08-12

## Base rechecked

`main@5451b4ebb00d79923288588bc0c2622f1b2341e7`.

## Gap rechecked

PR #3465 made the retrospective observed-Wave owner packet explicit and deliberately stopped before FFA/FBA promotion. Its accepted output, `forum_page_builder_wave_observed_acceptance_v1` with status `owner_accepted_observed_control_plane_wave_promotion_review_pending`, is eligible input for a separate explicit promotion review, but repository source did not yet define that promotion-review artifact or a fail-closed production runner.

That distinction matters: evidence acceptance and permission to execute a later control-plane change are different decisions, and neither decision should itself mutate rollout state.

## Source slice

Status: `forum-ffa-fba-promotion-review-source-ready / maintainer-execution-pending`.

This slice adds:

- `crates/rustok-forum/contracts/evidence/forum-page-builder-ffa-fba-promotion-review-source.json`;
- `scripts/evidence/review-forum-page-builder-ffa-fba-promotion.mjs`;
- `scripts/evidence/review-forum-page-builder-ffa-fba-promotion.test.mjs`;
- `scripts/verify/verify-forum-page-builder-ffa-fba-promotion-review.mjs`.

The promotion runner accepts only an **accepted observed-Wave owner packet** and fails closed unless:

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

## Executable synthetic coverage

`review-forum-page-builder-ffa-fba-promotion.test.mjs` invokes the production promotion-review runner and covers 13 cases:

1. valid approved review with `ffa_promoted=false` and `fba_promoted=false`;
2. valid explicit reject;
3. non-accepted predecessor status rejected;
4. predecessor source-commit drift rejected;
5. stale observed Wave rejected at promotion-review time;
6. prior owner-decision drift rejected;
7. missing retained freshness success rejected;
8. missing retained admission-lineage success rejected;
9. prior rollout-mutation overclaim rejected;
10. prior FFA-promotion overclaim rejected;
11. retained privacy overclaim rejected;
12. invalid promotion owner identifier rejected;
13. unsupported promotion-review decision rejected.

These fixtures exercise only the fail-closed promotion-review decision source. They are not live Pages/Forum/control-plane evidence and cannot change tenant rollout settings.

## Anti-drift and CI boundary

The focused `Pages Page Builder Provider Health` workflow now source-locks the promotion-review contract, production runner, synthetic cases and actualization together with the existing provider-health, Pages gate, Forum admission/lineage and observed-Wave owner layers.

The verifier also requires the canonical shared plan to retain the rule that a separate explicit FFA/FBA promotion review happens only after accepted observed-Wave owner evidence.

## Non-claims / next cursor

This source slice does not execute provider-health deployment evidence, the Pages reference candidate/gate, Forum browser/runtime/server-function evidence, Forum admission, the observed control-plane Wave, observed-Wave owner acceptance or the promotion review itself. `maintainer execution remains pending`.

It also does not execute the final control-plane change and does not promote FFA/FBA.

The live execution cursor remains:

1. execute and accept the exact Pages/provider-health gate chain;
2. execute Forum browser/runtime/server-function evidence and retain Forum admission on the same source/deployment boundary;
3. execute and retain the observed control-plane Wave;
4. pass freshness + exact retained-admission lineage and retain accepted observed-Wave owner evidence;
5. run the new FFA/FBA promotion-review packet while the retained Wave is still fresh;
6. only after `owner_approved_ffa_fba_promotion_review_execution_pending`, perform the separate maintainer-owned FFA/FBA control-plane promotion/change execution.
