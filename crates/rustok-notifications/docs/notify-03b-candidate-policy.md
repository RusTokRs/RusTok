# NOTIFY-03B/07A — candidate policy and in-app finalization

## Status

Source-ready. Maintainer verification has not been executed.

## Owner flow

`NotificationCandidateService::process_candidate` claims one durable fan-out
candidate and evaluates, in order:

1. exact/wildcard in-app preference resolution;
2. the mandatory injected `NotificationRecipientPolicy`;
3. recipient-specific source `authorize_target_open`;
4. a final transactional preference recheck;
5. idempotent notification insert or semantic replay validation;
6. candidate completion under the same unexpired lease CAS.

Suppressed candidates are terminal `skipped` rows. Retryable policy/provider or
database failures retain retry timing. Permanent semantic failures become
`failed`. No channel delivery attempt is created.

## Boundary decisions

- Notifications does not read Profiles-owned profile or block tables.
- No allow-all recipient policy implementation exists in the owner crate.
- Provider and privacy calls run outside the final notification transaction.
- The preference is rechecked inside that transaction to close a concurrent
  preference-disable race.
- Privacy and source authorization must run again on inbox open and before any
  delayed delivery because cross-owner state can change after creation.
- The public 03A fan-out item entity remains source-compatible; candidate-only
  lease columns are encapsulated by the owner service.

## Deferred

- production Profiles/block/mute/tenant policy adapter and runtime composition;
- production outbox consumer and candidate worker;
- grouping and moderator audience expansion;
- channel delivery enqueue and providers;
- inbox open/read APIs and repeated privacy checks;
- PostgreSQL concurrency evidence;
- retention, redaction, and reconciliation commands.

## Verification set

```bash
cargo fmt --all -- --check
RUSTFLAGS="-Dwarnings" cargo check -p rustok-notifications --all-targets
cargo test -p rustok-notifications --test persistence_sqlite -- --nocapture
cargo test -p rustok-notifications --test fanout_sqlite -- --nocapture
cargo test -p rustok-notifications --test candidate_sqlite -- --nocapture
node scripts/verify/verify-notifications-persistence.mjs
node scripts/verify/verify-notifications-source-fanout.mjs
node scripts/verify/verify-notifications-candidate-policy.mjs
cargo xtask module validate notifications
```

These commands were not run by the implementation agent.
