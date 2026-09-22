# SEO operations runbook

This runbook captures the D9 baseline for the production SEO Suite. It supplements `replay-repair-runbook.md` and covers three common operational scenarios without changing API contracts.

## When to use

- backlog in `seo_event_deliveries` or `seo_index_deliveries` stops shrinking;
- sitemap/robots or storefront metadata lag behind owner-module changes;
- the operator needs to safely run repair/replay without republishing all SEO entities.

## 1. SEO event backlog stuck

1. Verify that the tenant module is enabled and rollout flags are not turned off for the tenant.
2. Capture delivery summary via GraphQL/REST control-plane (`seoIndexDeliveryStatus` or `/api/seo/index/tracking`).
3. Group failures by `last_error`, `target_kind`, `status` and retry counter.
4. If there are transient transport errors — restart the consumer/worker and wait for bounded retry.
5. If there are deterministic validation/config errors — stop replay, fix the root cause and only then run repair.

### Stop criteria

- `dead_letter` grows faster than `retry` transitions to `sent`;
- one idempotency key creates more than one actual state transition;
- tenant/module gating gives `PERMISSION_DENIED` or `NOT_FOUND` for the operator without an expected reason.

## 2. Partial indexing failures

1. Filter `seo_index_deliveries` by tenant, `target_kind` and failed/dead-letter status.
2. Check cursor: the high-water mark must not roll back.
3. Run `repair_only` for a limited target scope with a limit of `1..500`.
4. After repair, verify that the failed count decreases and the cursor remains forward-only.
5. For recurring dead-letter items, open an owner-module data issue instead of force replay.

### Rollback / containment

- Do not delete delivery rows manually.
- Do not reset the cursor backward.
- To stop blast radius, disable the tenant rollout flag instead of changing the transport contract.

## 3. Replay / reindex procedure

1. Start with `repair_only`.
2. Use `repair+historical_replay` only after confirming that repair does not close the gap.
3. Keep replay mode forward-only: `not_started -> repair_only -> replay_requested -> replaying -> replay_completed`.
4. For each run, record: tenant, target scope, limit, operator, command/surface and final counters.
5. After replay, perform storefront parity smoke: runtime page context, robots/sitemap source and non-home metadata routes.

## Evidence checklist

- The command/surface run is recorded in an issue/PR.
- There are before/after counters by delivery statuses.
- There is a sample `last_error` for remaining failed/dead-letter rows.
- It is confirmed that GraphQL and REST return compatible semantic error codes.
- For the Next host, the fallback reason is confirmed (`module_disabled`, `not_found`, `permission_denied`, `transport_failure`) instead of a blanket failure.

## Live artifact schema (D8/D9 closeout)

Each live artifact for D8/D9 closeout must contain the same minimal set of fields, so that owner sign-off cannot be moved to `signed` based on incomplete screenshots or logs:

- `captured_at`, `surface`, `command_or_ci_job` and redacted environment (`git_sha`, SEO flags, backend/host base URL without secrets);
- before/after snapshots for `pending`, `sent`, `retry`, `failed`, `dead_letter`, `replay_mode`;
- semantic error sample for `BAD_USER_INPUT`, `PERMISSION_DENIED`, `NOT_FOUND` or transport failure, if the surface covers error parity;
- storefront sample for route, `target_kind`, fallback reason/source and JSON-LD/metadata assertions;
- `redactions_applied` with confirmation of removal of auth tokens, cookies, tenant secrets and user/customer identifiers;
- `result.passed`, `result.high_severity_defects` and `result.owner_review_required`.

Owner sign-off moves from `pending_live_runtime_evidence` to `signed` only after all required artifacts from `apps/next-frontend/contracts/seo/runtime-parity-fixtures.json` are attached, each artifact satisfies its `liveEvidenceArtifactTemplates[].mustCapture`, high-severity defects are absent or have a remediation owner, and the owner has reviewed redacted samples.

## Live incident evidence template (D9)

Use this block when D8 live backend/host evidence is available. Do not mark D9 live evidence complete from static review alone.

- Incident / drill id:
- Tenant and enabled SEO flags:
- Surface used: GraphQL / REST / Next Admin / runbook CLI
- Command, CI job, or operator action:
- Before counters: `pending`, `sent`, `retry`, `failed`, `dead_letter`
- After counters: `pending`, `sent`, `retry`, `failed`, `dead_letter`
- Sample `last_error` for remaining failed/dead-letter rows:
- Cursor transition observed:
- Semantic error parity sample (`BAD_USER_INPUT`, `PERMISSION_DENIED`, `NOT_FOUND`, transport):
- Required artifact template file matched:
- Storefront smoke evidence: runtime page context, robots/sitemap source, product/blog metadata route
- Stop criteria triggered: yes/no; if yes, remediation owner and rollback/containment action
