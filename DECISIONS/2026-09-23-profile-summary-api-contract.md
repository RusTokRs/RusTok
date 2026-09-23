# Profile Summary API Contract

## Status

Accepted and implemented on 2026-09-23.

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

## Invariants

1. Blog has no build-time dependency on `rustok-profiles`.
2. Profiles persistence entities and services remain private to Profiles.
3. Profile visibility is evaluated by the Profiles owner before summaries are returned.
4. The current tenant id is supplied by the Blog request context and passed unchanged to the owner provider.
5. The profile reader is bounded and batch-oriented; no per-author GraphQL provider query is introduced.
6. Provider absence and provider failure are fail-closed for the optional enrichment only; posts continue to resolve.
7. The public GraphQL type remains `ProfileSummary`; this is a contract-preserving extraction, not a schema rename.

## Consequences

New consumers can use the same reader contract without importing Profiles implementation details. Hosts that do not compose Profiles simply omit the shared provider and Blog continues to serve posts without author enrichment.

The API crate intentionally contains the GraphQL summary type because that type is a shared public contract between the profile owner and presentation consumers. Business persistence state remains in `rustok-profiles`.

## Verification

Source-level review performed against the canonical Blog/Profiles/GraphQL composition. Maintainer runtime evidence, gatekeeper verification, build, and automated tests remain unrun by the agent; the repository owner will run the test suite.
