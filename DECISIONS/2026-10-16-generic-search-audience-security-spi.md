# Generic Search Audience Security SPI and Multi-Module Post-Authorization

- Date: 2026-10-16
- Decision status: Accepted
- Implementation status: In progress
- Owners: platform / search team
- Extends: `2026-07-31-forum-search-versioned-invalidation-rollout.md`
- Supersedes: None
- Superseded by: None

## Context

Prior to this decision, `rustok-search` owned specialized modules (`forum_storefront_execution.rs`, `storefront_result_eligibility.rs`) specifically crafted to enforce real-time audience security for Forum entities (topics, categories, replies). 

While dependency boundaries were respected at the Cargo level via ports and host adapters (`apps/server`), the physical presence of domain-specific filenames and domain-specific trait methods (`public_forum_discovery_allowed`, `filter_forum_result_candidates`, `StorefrontSearchResultCandidateKind::ForumTopic`, etc.) inside `rustok-search` created an Open-Closed Principle (OCP) violation:
1. Any new domain module requiring real-time audience or access control filtering (e.g. Events, LMS, Courses, Private Documents) would require adding another specialized `*_storefront_execution.rs` and domain-specific port into `rustok-search`.
2. Generic storefront search (`storefrontSearch`) was forced to hardcode an exclusion of `forum` documents at the SQL level, preventing true cross-domain unified search.

A platform-level, module-agnostic Service Provider Interface (SPI) is required so that:
- Any module can declare audience and real-time security rules without modifying `rustok-search`.
- Real-time preflight and post-authorization security invariants are 100% preserved.
- Existing Forum contracts and verification scripts (`FORUM-23B2D`, `LINK-FORUM-03`) remain unbroken during evolutionary migration.

## Decision

1. **Platform SPI Definition in `rustok-core`:**
   `rustok-core::search_projection` (or sibling `search_audience`) defines:
   - `SearchCandidateDescriptor`: neutral identifier carrying `document_id`, `source_module`, and `entity_type`.
   - `SearchAudienceRequest`: typed request carrying tenant, locale, candidates, auth context, and request context.
   - `SearchAudiencePolicyPort`: generic trait with two security phases:
     - `public_discovery_allowed(tenant_id, auth)`: preflight gate preventing facet/candidate oracle leaks before FTS execution.
     - `filter_eligible_candidates(request)`: real-time batch candidate authorization returning only allowed `Uuid`s.
   - `SearchAudiencePolicyRegistry`: registry managed via `ModuleRuntimeExtensions` where modules register their policy implementations at runtime bootstrap.

2. **Generic Execution in `rustok-search`:**
   `rustok-search` provides `execute_audience_eligible_search` which:
   - Evaluates preflight discovery across requested source modules.
   - Performs bounded candidate scans (anti-DoS limit of 100 candidates per audience-gated module).
   - Groups candidates by `source_module` and dispatches batch filtering to corresponding `SearchAudiencePolicyPort` implementations concurrently.
   - Re-aggregates facets, totals, and pagination offsets strictly over authorized documents.

3. **Backward-Compatible Bridge for Forum:**
   Existing `StorefrontSearchResultEligibilityPort` and `execute_forum_storefront_search` are retained as an adapter layer over the generic SPI, ensuring 100% binary, signature, and verification compatibility with existing contracts and test suites.

## Sources of truth and ownership

- **Platform contract:** `rustok-core` owns `SearchAudiencePolicyPort`, `SearchCandidateDescriptor`, and `SearchAudiencePolicyRegistry`.
- **Search execution:** `rustok-search` owns Postgres FTS scanning, bounded candidate retrieval, and post-authorization aggregation.
- **Audience authority:** The respective domain module (e.g. `rustok-forum`, `rustok-events`) remains the sole authority on visibility, roles, and real-time access rules.
- **Host composition:** `apps/server` registers domain providers into the runtime extensions at application startup.

## Invariants

### Allowed states
- A module without an audience policy is treated according to its static `is_public` projected document flag.
- A module with an audience policy runs through preflight gate and candidate batch-authorization.
- Multiple audience-gated modules can be queried simultaneously in a unified search request; candidate authorization runs concurrently.

### Forbidden states
- Searching against an audience-gated module when its preflight gate returns `false` is forbidden; FTS must not run and must return an empty result without leaking facets or totals.
- Candidate scan exceeding bounded limit (100) must fail closed or require query narrowing to prevent memory/CPU exhaustion.
- Calculation of facets, query rules, or totals over unauthorized candidates is strictly forbidden.
- Direct runtime dependency between `rustok-search` and domain modules remains strictly forbidden.

## Non-goals

- SQL-level row filtering inside PostgreSQL full-text search: real-time user-specific dynamic ACLs cannot be statically stored in FTS indexes without causing stale permission leaks.
- Replacement of `SearchProjectionSource`: ingestion and projection remain governed by the existing projection registry.

## Context dimensions

- **Tenant:** Strictly tenant-scoped; candidate verification rejects cross-tenant IDs.
- **Auth & Request Context:** Propagated through `SearchAudienceRequest` to enable actor, role, session, and channel policy resolution.
- **Locale:** Propagated for localized permission or status evaluation.
