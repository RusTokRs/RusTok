# Forum Page Builder Wave observed owner acceptance actualization — 2026-08-12

## Base rechecked

`main@9edf72ce3d1bd8d3de742816aef022b07fa80ffe`.

## Gap rechecked

Forum Wave source now has three distinct post-input boundaries:

1. `forum_page_builder_wave_admission_v1` admits exact Pages/Forum execution inputs;
2. `verify-forum-wave-admission-lineage.mjs` binds a future observed live Wave packet to the exact retained admission packet;
3. `verify-forum-wave-evidence-freshness.mjs` validates the observed control-plane packet, freshness, audit/fallback/metrics/traces, rollback decision, approvals and waiver boundary.

Before this slice there was no separate retrospective owner-decision packet after those two live verifiers. A fresh live Wave packet could be structurally valid, but repository source had no explicit `accept/reject` artifact that remained distinct from FFA/FBA promotion.

## Source slice

Status: `forum-wave-observed-owner-acceptance-source-ready / maintainer-execution-pending`.

This slice adds `scripts/evidence/accept-forum-page-builder-wave.mjs` and the machine-readable `forum_page_builder_wave_observed_acceptance_source_v1` contract.

The runner accepts only a supplied live Forum Wave packet plus its retained admission packet and fails closed unless:

- the Wave is `mode=live`, `provenance=observed_control_plane` and `execution_status=maintainer_verified`;
- Wave `source_commit` equals checkout `HEAD`;
- the existing Forum Wave freshness verifier passes at owner-review time;
- the existing retained admission-lineage verifier passes against the supplied admission packet;
- the supplied admission packet hash, source commit and immutable RepoDigest remain equal to the reviewed Wave.

The only owner decisions are `accept_observed_wave_evidence` and `reject`. Owner identity is a bounded operator assertion, not a cryptographic signature.

An accepted packet means only: the retrospective observed Wave evidence is eligible for a separate explicit promotion review. It does not mutate control-plane state, does not promote FFA/FBA, does not assert current provider health, and does not claim cryptographic origin-to-RepoDigest provenance.

## Source-ready Wave cursor

`forum-wave1-rollout-evidence.json` now carries an explicit `observed_run.owner_acceptance` cursor:

- format `forum_page_builder_wave_observed_acceptance_v1`;
- accepted status `owner_accepted_observed_control_plane_wave_promotion_review_pending`;
- source status `source_ready_maintainer_execution_pending`;
- execution status `maintainer_execution_pending`.

The source anti-drift guard checks that cursor without fabricating a live packet.

## Executable synthetic coverage

`accept-forum-page-builder-wave.test.mjs` invokes the production owner-decision runner and covers 10 cases:

1. valid fresh lineage-verified accept;
2. valid explicit reject;
3. source-ready Wave rejected at owner review;
4. invalid owner identifier rejected;
5. unsupported decision rejected;
6. stale observed Wave rejected by the production freshness verifier;
7. retained admission hash drift rejected;
8. admission source-commit drift rejected;
9. admission privacy overclaim rejected;
10. live refresh record dropping the admission-lineage verifier gate rejected.

These fixtures test the owner-decision runner only. They are not live Pages/Forum/control-plane evidence.

## Non-claims / next cursor

This slice does not execute the Pages reference candidate/gate, provider-health live chain, Forum browser/runtime/server-function evidence, Forum admission, observed control-plane Wave, rollback action, approvals or owner acceptance. `maintainer execution remains pending`.

It also does not promote FFA/FBA.

Next execution cursor remains:

1. execute and accept the live Pages/provider-health gate chain;
2. execute Forum browser/runtime/server-function packets on the same source/deployment boundary;
3. retain Forum admission;
4. execute and retain the observed control-plane Wave;
5. pass freshness + exact retained admission lineage;
6. run this retrospective owner-decision packet;
7. only after an accepted owner packet, perform a separate explicit FFA/FBA promotion review.
