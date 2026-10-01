# Profile Summary API Contract

- Date: 2026-09-23
- Decision status: Accepted
- Implementation status: Implemented
- Owners: Profiles and Blog module owners
- Extends: None
- Supersedes: None
- Superseded by: None

## Context

Blog presents an optional author profile summary on post reads. The capability is owned by Profiles, but Blog must remain usable when Profiles is absent or its presentation provider is unavailable.

The previous implementation compiled Blog directly against `rustok-profiles` and imported the Profiles-owned GraphQL summary type and request DataLoader. That made an explicitly optional presentation capability a build-time implementation dependency and forced the consumer to know owner-specific privacy/loading infrastructure.

## Decision

Introduce `rustok-profiles-api` as the neutral cross-module contract for public profile presentation.

The API owns:

- the transport-neutral `ProfileSummary` value used by downstream presentation consumers;
- the `ProfileSummaryAudience` request audience contract;
- the `ProfileSummaryReader` async batch-read port;
- the GraphQL `GqlProfileSummary` / `GqlProfileVisibility` types so Profiles and consumers preserve one schema identity.

`rustok-profiles` remains the sole implementation owner. `ProfilePresentationService` implements the API reader and is composed by the host through `HostRuntimeContext` shared values.

Blog depends only on `rustok-profiles-api`. It reads the optional provider from its manifest-attached GraphQL runtime data. Missing provider or provider failure degrades author enrichment to `author_profile = null` without changing post availability.

The host translates the existing request-scoped profile audience into the API audience without exposing Profiles persistence entities or private owner services to Blog.

## Sources of truth and ownership

- Canonical owner of user profiles: `rustok-profiles`.
- Neutral public presentation contract: `rustok-profiles-api`.
- Consumer: `rustok-blog` depends only on `rustok-profiles-api`.

## Invariants

1. Blog has no build-time dependency on `rustok-profiles`.
2. Profiles persistence entities and services remain private to Profiles.
3. Profile visibility is evaluated by the Profiles owner before summaries are returned.
4. The current tenant id is supplied by the Blog request context and passed unchanged to the owner provider.
5. The profile reader is bounded and batch-oriented; no per-author GraphQL provider query is introduced.
6. Provider absence and provider failure are fail-closed for the optional enrichment only; posts continue to resolve.
7. The public GraphQL type remains `ProfileSummary`; this is a contract-preserving extraction, not a schema rename.

## Non-goals

- Refactoring profile data persistence or authentication credentials.
- Inverting profile management into the Blog domain.

## Data, transaction, and concurrency boundary

Not applicable. This decision specifies a read-only batch projection API and port contract without cross-module transactions.

## Context dimensions

Tenant, channel, locale, and request-scoped audience participate explicitly in profile visibility evaluation.

## Events and projections

Profile update events trigger read-cache invalidations in `rustok-profiles`; consumers receive fresh summaries on subsequent batch queries.

## Failure semantics

Provider absence or execution failure degrades author profile enrichment to `null`, ensuring post resolution fails open without error.

## Migration and cutover

Direct dependency from `rustok-blog` to `rustok-profiles` replaced with dependency on `rustok-profiles-api`.

## Alternatives considered

- Direct dependency on `rustok-profiles`: rejected because optional presentation should not force full module implementation into builds.
- Inline summary DTO duplication: rejected to avoid schema drifting between Blog and Profiles.

## Verification

Source-level checks verify that `rustok-blog` depends only on `rustok-profiles-api` and that `GqlProfileSummary` preserves schema parity.

## Consequences

New consumers can use the same reader contract without importing Profiles implementation details. Hosts that do not compose Profiles simply omit the shared provider and Blog continues to serve posts without author enrichment.
