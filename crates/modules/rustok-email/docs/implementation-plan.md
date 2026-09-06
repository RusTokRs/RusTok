# Implementation plan for `rustok-email`

## Current state

`rustok-email` is a capability-only core module. It owns SMTP delivery,
template rendering, typed delivery requests and receipts, and the
`EmailDeliveryPort`; authorization remains with the calling module or host.
The disabled-provider path returns a typed noop receipt, and the published port
requires deadline and write-idempotency semantics.

Cycle-001 inspection found that these published semantics are not yet the
runtime delivery contract. The port previously validated the presence of a
deadline and idempotency key but did not apply either value. Built-in auth email
continues to use a server-owned sender and detached tasks instead of the owner
port or durable delivery state. Tenant email settings are persisted separately
from the bootstrap configuration actually used by the SMTP sender.

Draft PR #2490 stages bounded process-local idempotency, real delivery deadlines,
input bounds, HTML autoescaping, recipient-log redaction, and secret-safe email
settings projections. It also closes a confirmed exposure where callers with
`settings:read` could receive the runtime SMTP password and where the native
admin path returned historical email settings JSON without redaction. These
changes are not in `main` and have no successful Cargo Check/test/Clippy evidence
on one SHA, so they do not count as a completed release fix.

## FFA/FBA status block

- FFA status: `not_started`
- FBA status: `transport_verified`
- Structural shape: `no_ui_boundary`
- FBA provider contract: `EmailDeliveryPort` / `email.delivery.v1` in
  `crates/rustok-email/contracts/email-fba-registry.json`.
- Runtime and fallback evidence:
  `crates/rustok-email/contracts/evidence/email-contract-test-static-matrix.json`
  and `crates/rustok-email/contracts/evidence/email-runtime-fallback-smoke.json`.
- `npm run verify:email:fba` and `npm run verify:foundation:fba-runtime-smoke`
  lock provider metadata, policy semantics, typed validation, and fallback
  behavior, but do not prove durable SMTP delivery or host adoption.

## Open results

1. **Adopt one durable email-delivery execution path.** Persist delivery intent
   and idempotency identity in durable owner state or a transactionally coupled
   outbox, supervise retries, and store a terminal receipt before acknowledging
   completion. Process-local reservations may remain an optimization only.
   **Depends on:** email migration ownership and the platform outbox contract.
   **Done when:** duplicate, timeout, cancellation, process restart, provider
   failure and replay tests prove at-most-one accepted delivery per identity or
   a documented provider-safe retry contract.

2. **Remove the server-owned auth delivery bypass.** Password-reset and email
   verification flows currently render and send through `apps/server` and use
   detached `tokio::spawn`; restart or cancellation can lose delivery. Move the
   flow to the published owner port and durable execution path without keeping
   a parallel compatibility sender.
   **Depends on:** durable delivery receipts and auth-owner request identity.
   **Done when:** auth no longer owns SMTP/rendering policy, background work is
   supervised, and targeted restart/cancellation tests preserve delivery intent.

3. **Make the settings control plane authoritative or remove it.** GraphQL and
   admin write tenant email JSON, while the actual sender reads bootstrap
   `ctx.settings()`. A successful save therefore does not change runtime SMTP
   behavior. Define which non-secret tenant fields are supported and resolve
   them in the owner sender, or remove the misleading write surface.
   **Depends on:** provider/configuration ownership and reload semantics.
   **Done when:** reads, writes, runtime delivery, health and operator docs use
   one configuration source with explicit restart or live-reload behavior.

4. **Scrub historical email secrets with an owned migration.** Existing
   `platform_settings.email` rows may contain SMTP passwords or other secret
   fields. Draft PR #2490 prevents further reads/writes of these fields, but it
   does not remove data already at rest. `EmailModule` is not currently present
   in the global migration-source registry, so migration ownership must be made
   explicit before cleanup.
   **Depends on:** platform migration aggregation and secret-rotation procedure.
   **Done when:** historical rows are scrubbed, affected credentials are rotated,
   migration replay is tested, and no API or native surface returns the secret.

5. **Merge and verify the staged security/port hardening.** Draft PR #2490
   applies deadlines, bounded request identity, HTML escaping, PII-safe logs and
   email-settings redaction. Fix every email-specific compile, test or Clippy
   failure before merging; queued or pre-compilation failures are not evidence.
   **Depends on:** a working locked dependency graph and Rust runners.
   **Done when:** targeted owner, server and admin tests pass on one PR SHA and
   the reviewed changes are merged without weakening the explicit blockers.

6. **Probe SMTP readiness rather than configuration shape only.** Current health
   validates configured fields but does not prove DNS/connectivity/TLS/auth.
   Add a bounded provider probe or document a deliberately weaker readiness
   contract with separate delivery-failure alerting.
   **Depends on:** provider operational policy and safe probe semantics.
   **Done when:** outage/authentication failure is observable without leaking
   credentials or blocking unrelated traffic indefinitely.

7. **Extend typed delivery payloads only with an owned contract.** Add a new
   template, delivery field, or receipt behavior together with module docs and
   host integration tests.
   **Depends on:** the consuming module's public delivery requirement.
   **Done when:** request validation, idempotency, template-error retry policy,
   and disabled-provider behavior are covered by targeted tests.

## Verification

- `npm run verify:email:fba`
- `npm run verify:foundation:fba-runtime-smoke`
- `cargo xtask module validate email`
- `cargo xtask module test email`
- `cargo check -p rustok-email --lib`
- `cargo test -p rustok-email ports::tests`
- `cargo test -p rustok-email template::tests service::tests`
- Targeted server settings, auth delivery and admin native-adapter tests.
- Restart/replay/provider-failure evidence after durable delivery is implemented.

## Change rules

1. Keep delivery policy, rendering, provider behavior and receipts in this module.
2. Do not acknowledge durable delivery from process-local state or a detached task.
3. Keep credentials in a secret-bearing runtime source; tenant settings and public
   projections may contain only explicitly supported non-secret fields.
4. Update the root README, local docs and `rustok-module.toml` with a public
   delivery contract change.
5. Update this status block and `docs/modules/registry.md` with an FBA boundary
   change.

## Periodic release verification handoff

- Cycle: `cycle-001`
- Status: `blocked`
- Last verified at (UTC): `2026-07-30`
- Scope inspected: `EmailDeliveryPort; SMTP and disabled-provider behavior; templates and auth mailers; GraphQL/native/admin settings surfaces; password-reset and verification callers; deadlines, idempotency, retries, cancellation, restart behavior, health, secrets and PII`
- Findings: `P0=0, P1=5, P2=1, P3=1`
- Fixed in this pass: `staged draft PR #2490 with real delivery deadlines, bounded process-local request identity, input bounds, HTML autoescaping, PII-safe logs, SMTP-password redaction, secret-field rejection and safe email settings projections; added the missing cycle handoff`
- Remaining risks or blockers: `draft changes are not in main or compiled verified; durable delivery/outbox is absent; auth uses a server-owned detached delivery path; saved tenant settings do not drive runtime SMTP; historical secret-bearing rows lack an owned scrub migration; SMTP readiness does not probe the provider`
- Evidence: `PR #2490 at SHA 53f84914b37d61b7b5078a3cba42caf65c96a65a; owner port/service/template sources; server auth/email/settings and GraphQL callers; admin native adapter; platform settings migration and migration-source registry; CI run 30519187116 had not started jobs; earlier smoke stopped before compilation on an unlocked Athanor dependency and advisory validation found two expired exceptions`
- Next action: `resume PR #2490 when targeted runners are available, fix email-specific failures, then implement one durable owner delivery path and remove the auth/settings bypasses before closing-gate completion`
- Resume command: `cargo xtask module validate email && cargo xtask module test email && cargo test -p rustok-email ports::tests`
