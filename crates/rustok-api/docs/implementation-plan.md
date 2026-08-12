# Implementation plan for `rustok-api`

## Current state

`rustok-api` owns neutral shared host/API contracts: request, auth, tenant,
channel, GraphQL, route, locale, permission, and transport-agnostic port
primitives. `PortContext`, `PortError`, and `PortCallPolicy` provide shared
read/write/replay/best-effort policy without module business logic.

The SeaORM-backed `HostRuntimeContext` is gated behind the neutral `runtime`
feature. The `server` feature includes `runtime` and adds Axum/Async-GraphQL;
runtime helpers therefore do not pull web frameworks into standalone module
owners.

The crate has no dependency on `rustok-core`; core owns runtime RBAC/security
and consumes API contracts. `apps/server` is the composition root, not a second
shared API framework. Module resolvers, controllers, and domain ports remain
with their owners.

The first neutral richtext contract is now implemented in `src/richtext.rs`.
`RichTextDocument` is the canonical ProseMirror/Tiptap root JSON shape,
`RichTextProfileId` is an unversioned owner-selected identifier, and
`RichTextView` carries server-derived HTML for reads. Serde rejects the removed
version/locale envelope and unknown structural fields. The optional `server`
feature exposes the document as the semantic `RichText` GraphQL scalar. This
crate intentionally does not validate profiles, render HTML, or select locale.

`AuthPrincipalKind` is now a host-neutral shared authorization contract available
without the `server` feature. It distinguishes direct users, OAuth delegated
users, and service principals. Server-enabled builds expose the separate
`AuthPrincipalContext` request carrier; module owners can consume the enum
without taking an Axum or Async-GraphQL dependency.

`MarketplaceRegistryFreshness` is the neutral cross-transport observation for
one configured logical module registry. It carries typed status, last-success
Unix milliseconds, and consecutive failures while deliberately excluding
endpoint URLs and provider error text. `rustok-modules` owns the catalog port;
`rustok-api` owns only the DTO reused by Rust and headless transports.

## FFA/FBA boundary

- FFA status: `not_started`
- FBA status: `not_started`
- Structural shape: `no_ui_boundary`
- This shared-contract crate has no module-owned UI or FBA provider port.

## Open results

1. **Keep shared contract extraction evidence-based.** Move a helper into this
   crate only when it is framework-neutral and needed by independent consumers;
   keep module resolvers, controllers, and domain policy with their owners.
   **Depends on:** demonstrated multi-module use and owner approval.
   **Done when:** the shared API is dependency-neutral, consumers remove local
   duplicates, and no domain behavior enters the crate.

2. **Preserve port-policy consistency across consumers.** Evolve `PortContext`,
   `PortError`, and `PortCallPolicy` atomically for read, write, replay, and
   best-effort semantics.
   **Depends on:** all registered port consumers and their public contracts.
   **Done when:** targeted migration tests prove identical deadline, idempotency,
   actor, and typed-error behavior without local policy forks.

3. **Maintain composition and documentation boundaries.** Update API docs,
   server composition docs, and module transport docs with a changed shared
   contract, and run the focused surface verification.
   **Depends on:** the changed public contract.
   **Done when:** the documentation and `verify:api:surface-contract` describe
   the same dependency direction and owner responsibilities.

4. **Keep the adopted neutral richtext transport canonical during closeout.**
   Blog, Forum, and Comments owner transports use the structural contract. This
   crate continues to own
   `RichTextDocument`, neutral `RichTextProfileId`, the read-only
   `RichTextView`, generated schema, and optional transport adapters without
   importing Tiptap or executable content policy. Validation, rendering,
   profile definitions, and plain-text extraction remain in
   `rustok-content::richtext`.
   **Depends on:** deletion of the old core/generic format paths under the
   [central Richtext plan](../../../docs/modules/rich-text-implementation-plan.md)
   and synchronized Blog/Forum/Comments transports.
   **Current evidence:** `cargo test -p rustok-api` and
   `cargo test -p rustok-api --features server` cover the structural contract,
   removed envelope, unknown-field rejection, generated schema shape, and
   GraphQL feature compilation.
   **Done when:** repository-owned richtext transports use one typed document,
   the separate `rustok-core::rt_json` implementation is absent, and this crate
   remains dependency-neutral.

## Delivered result: explicit authenticated principal kind

- `AuthPrincipalKind::{DirectUser, DelegatedUser, Service}` is compiled without
  the `server` feature and contains no host/runtime dependency.
- `from_authenticated_facts` accepts only the three validated grant/client/session
  shapes and returns `None` for unknown or ambiguous combinations.
- `AuthPrincipalContext` and its Axum extractor are server-only request carriers;
  they are separate from legacy `AuthContext` transport metadata.
- RBAC is the first owner consumer. Its control-plane policy receives only the
  typed kind and tenant id, with no fallback to string inference.
- Source and same-revision compile/test evidence remain required before the
  current RBAC cycle item can be completed.

## Verification

- `npm run verify:api:surface-contract`
- `node scripts/verify/verify-rbac-explicit-principal-kind.mjs`
- `cargo check -p rustok-api`
- `cargo check -p rustok-api --features server`
- Targeted compile/tests when changing shared request, auth, tenant, channel,
  GraphQL, route, locale, permission, or port contracts.
- Documentation synchronization for `apps/server` and module-owned transports.
- Generated richtext schema drift and Serde/GraphQL adapter tests when the
  Richtext cutover is implemented.

## Change rules

1. Keep this crate neutral and dependency-light; do not add module business
   logic, resolver ownership, or runtime composition.
2. Update the root README and local docs with a public contract change.
3. Update host and consumer-module documentation with changed shared semantics.
