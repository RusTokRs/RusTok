# FORUM-20BA audience-plan synchronization

Status: source-ready / unvalidated documentation synchronization.

This slice repairs the canonical roadmap and owner-note drift accumulated after
`FORUM-20AU`. It changes no runtime behavior, persistence, transport contract,
dependency, or public request/response shape.

## Synchronized delivered chain

- `FORUM-20AV` enforces inherited category reply-create audience policy in every
  public reply owner path before preparation or writes.
- `FORUM-20AW` composes exact authenticated GraphQL and REST request context and
  the host-published audience-facts capability into both reply-create transports.
- `FORUM-20AX` adds optional normalized topic-local reply-create narrowing that
  composes after category layers and cannot broaden them.
- `FORUM-20AY` adds inherited category moderation audience persistence and owner
  enforcement before topic/reply moderation transactions while preserving the
  exact topic-author solution path.
- `FORUM-20AZ` composes the existing GraphQL and REST solution mutations through
  exact authenticated moderation context and the same host-published facts port.

The canonical Forum ledger now records the implementation chain through
`FORUM-20AZ`; this documentation synchronization is `FORUM-20BA`.

## Trust correction

The historical `FORUM-20AV`, `FORUM-20AX`, `FORUM-20AY`, and `FORUM-20AZ`
notes described authoritative Forum trust as unavailable or blocked. That was
true at their original source bases but is no longer current. `FORUM-26A/B/G`
now provide Forum-owned trust state and facts, and the server publishes them
through `ForumUserTrustAudienceFactsPort` as part of the shared exact audience
facts composition.

The updated notes preserve the historical boundary that those individual
slices did not create trust state, while no longer presenting delivered trust
composition as open work.

## Preserved remaining scope

- migrate remaining Forum reads and search/index/SEO/deep-link consumers to the
  same exact richer audience decision;
- add visibility-scoped category/all-read commands over an exact bounded owner
  scope;
- route any future public moderation transports through the delivered
  context-aware owner methods instead of adding transport-local policy;
- add scheduled reconciliation, payload redaction, channel delivery and
  delivery-time authorization;
- capture PostgreSQL concurrency, inheritance, lease/contention and
  cross-consumer runtime evidence.

No new moderation route is claimed. `FORUM-20AZ` covers the existing solution
routes; approve/reject/hide and pin/lock/status remain owner methods without a
current public Forum transport.

## Verification status

Tests, Cargo commands, formatting, verifiers, workflows, and CI were not run by
the implementation agent. The source-ready commands are recorded in
`forum-audience-plan-sync.json` and the canonical plan.
