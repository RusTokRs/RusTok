# FORUM-26C posting policy contract

Status: source-ready / unvalidated.

## Delivered

- `ForumPostingPolicyEvaluationInput` is a public Forum-owned contract for one exact tenant, user and posting action.
- The action is typed as topic creation, reply creation, topic edit, reply edit or topic bump.
- Candidate input contains bounded numeric metrics only: body byte count, link count, mention count and attachment count. Raw body text, rendered HTML, hashes, arbitrary JSON and external-scoring payloads are not part of this contract.
- `ForumPostingPolicyFacts` declares the exact facts required by a future owner evaluator. Every declared fact must be represented exactly once as either an available typed value or an explicit `ForumPostingPolicyUnavailableFact`.
- Supported fact identities cover authoritative trust, account age, reading activity, approved posts, active flags, reputation, recent moderation actions, bounded topic/reply/edit usage windows and time since the last bump.
- Unavailable facts carry one bounded stable lowercase reason code plus retryability. Missing, duplicated, undeclared or simultaneously available/unavailable facts fail validation instead of being converted into zero or a deny.
- `ForumPostingPolicyDecision` has three disjoint outcomes:
  - `allowed` carries no denial or retry metadata;
  - `denied` carries one typed policy reason, reason-matched owner fact when applicable, typed numeric evidence and a positive retry delay only for temporal limits;
  - `indeterminate` identifies one unavailable required fact and preserves its retryability.
- Trust evidence reuses the existing Forum trust bound `0..100`.
- Public exports are available from `rustok_forum` without adding a transport DTO, endpoint or runtime provider.

## Ownership boundary

This slice defines data shape and validation only. It does not choose required facts, thresholds or rule order and does not expose an evaluator, service or owner port.

`forum_user_stats` is not read and is not treated as trust, approved-content truth, reputation, moderation history or a rate-limit clock. Future facts must come from named authoritative owners with explicit degraded behavior.

The contract deliberately separates `indeterminate` from `denied`. A retryable owner outage cannot become a policy rejection, and a terminally unavailable required fact remains visible as an incomplete decision rather than a fabricated value.

## Excluded

- no automatic trust promotion or demotion;
- no posting-policy evaluator or rule configuration persistence;
- no account-age, reading, approved-post, flag, reputation or moderation-history fact provider;
- no topic/day, reply/minute, edit or bump enforcement in topic/reply owners;
- no shared distributed rate-limit call;
- no duplicate-content hashing or retained body fingerprint;
- no external or AI spam-scoring call;
- no migration, background worker, event, GraphQL, REST, OpenAPI, admin UI or storefront change.

The next bounded FORUM-26 slice should define the deterministic Forum-owned evaluator and rule precedence over this contract. It must consume only explicitly available facts, return `indeterminate` for required unavailable facts, and remain separate from distributed rate-limit execution and automatic trust-state writes.

## Canonical plan debt

The canonical `crates/rustok-forum/docs/implementation-plan.md` is not replaced through the GitHub contents API. It exceeds two thousand lines and complete replacement risks unrelated roadmap loss. A safe repository-local edit still needs to mark FORUM-26 `in_progress`, record FORUM-26A-C, update the FORUM-20 trust dependency and retain fact owners, deterministic evaluation, posting-owner enforcement, duplicate hashing, shared rate limiting and optional external scoring as remaining work.

`CRATE_API.md` is likewise not completely replaced in this slice. The public contract is source-exported from the crate root and recorded by the machine contract and verifier.

## Validation status

The following commands are source-ready but were not run by the implementation agent:

```text
cargo test -p rustok-forum --test posting_policy_contract -- --nocapture
node scripts/verify/verify-forum-posting-policy-contract.mjs
node scripts/verify/verify-forum-user-trust-audience-facts.mjs
cargo xtask module validate forum
```

Tests, Cargo commands, formatting, verifier execution, workflows and CI remain the maintainer's responsibility for this slice.
