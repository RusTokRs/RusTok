# FORUM-26D posting policy evaluator

Status: source-ready / unvalidated.

## Delivered

- `ForumPostingPolicyRules` is a public pure rules contract for trust, account age, reading, approved posts, active flags, reputation, moderation history, action-scoped usage windows, bump interval and candidate link/mention/attachment limits.
- `ForumPostingPolicyEvaluator::decide` normalizes both the rules and the FORUM-26C input before evaluating.
- The evaluator derives the exact required fact set from the normalized rules plus the requested action. A caller cannot omit a required fact, add an unrelated fact or select a more permissive fact surface.
- Every required fact is resolved before a partial policy decision is allowed. If one or more required facts are unavailable, the evaluator returns one deterministic `indeterminate` decision and preserves the selected fact's retryability.
- Multiple unavailable facts are resolved by stable safety-first fact precedence, independent of vector order.
- The rule precedence is public as `FORUM_POSTING_POLICY_PRECEDENCE`:
  1. required fact unavailable;
  2. active flags;
  3. moderation history;
  4. trust level;
  5. account age;
  6. reading activity;
  7. approved posts;
  8. reputation;
  9. topic-create window;
  10. reply-create window;
  11. edit window;
  12. bump interval;
  13. link limit;
  14. mention limit;
  15. attachment limit;
  16. allowed.
- Topic, reply and edit usage observations must use the exact configured window. An exhausted snapshot returns the configured full window as a conservative retry hint; this is not claimed as an exact distributed limiter result.
- Bump interval denial returns the exact remaining difference between the configured minimum and observed age.
- Empty normalized rules allow the action.
- `DuplicateContent` and `ExternalSpamScore` remain reserved decision reasons and are not present in the FORUM-26D precedence because their owner inputs do not exist.
- `body_bytes` remains part of the bounded FORUM-26C candidate metrics but FORUM-26D does not invent an uncontracted body-size rule.

## Ownership boundary

The evaluator is deterministic and side-effect free. It performs no database read or write, no port call, no clock read, no random operation and no transport work.

`forum_user_stats` is not imported or read. The evaluator consumes only the exact typed facts supplied through the normalized FORUM-26C contract; future composition must obtain those facts from named authoritative owners.

Usage-window evaluation is snapshot policy evaluation only. Shared rate limiting still owns distributed reservation, contention, exact retry timing and commit/release semantics. A future owner-enforcement slice must not treat the conservative full-window hint as proof of a limiter reservation.

## Excluded

- no account-age, reading, approved-post, flag, reputation or moderation-history fact adapter;
- no policy configuration persistence, tenant settings or administration;
- no topic/reply/edit/bump owner enforcement;
- no distributed rate-limit reservation or mutation;
- no duplicate-content hash or retained fingerprint;
- no external or AI scoring call;
- no trust-state write, promotion or demotion;
- no migration, event, worker, GraphQL, REST, OpenAPI, admin UI or storefront change.

The next bounded FORUM-26 slice should compose authoritative fact adapters for the subset needed by an initial posting profile, keeping every unavailable capability explicit. Owner enforcement and distributed limiter execution should remain later separate slices.

## Canonical plan debt

The canonical `crates/rustok-forum/docs/implementation-plan.md` is not replaced through the GitHub contents API. It exceeds two thousand lines and complete replacement risks unrelated roadmap loss. A safe repository-local edit still needs to mark FORUM-26 `in_progress`, record FORUM-26A-D, update the FORUM-20 trust dependency and retain fact composition, owner enforcement, duplicate hashing, shared rate limiting and optional external scoring as remaining work.

`CRATE_API.md` is likewise not completely replaced. The public evaluator and rules are exported from the crate root and recorded by the machine contract and verifier.

## Validation status

The following commands are source-ready but were not run by the implementation agent:

```text
cargo test -p rustok-forum --test posting_policy_contract -- --nocapture
cargo test -p rustok-forum --test posting_policy_evaluator -- --nocapture
node scripts/verify/verify-forum-posting-policy-contract.mjs
node scripts/verify/verify-forum-posting-policy-evaluator.mjs
cargo xtask module validate forum
```

Tests, Cargo commands, formatting, verifier execution, workflows and CI remain the maintainer's responsibility for this slice.
