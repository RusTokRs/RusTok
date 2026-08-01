# Implementation plan for `rustok-customer`

## Current state

`rustok-customer` owns tenant-scoped customer records, the optional user/profile bridge, customer read projections, and the customer admin package. It is not a replacement for the platform user or public-profile domain. Commerce may compose customer data through published contracts but must not reintroduce a customer service facade.

The admin package uses a framework-agnostic core, native transport facade, and explicit Leptos adapter. Its native server functions use `HostRuntimeContext`; the old runtime dependency and legacy API facade are removed. Local documentation is synchronized with the current customer boundary.

The admin surface is an accepted single-adapter owner fragment: it is an authenticated operator workflow with no public/headless customer-admin contract, so its native `#[server]` adapter is intentional and no package-local GraphQL fallback is added.

Customer detail/create/update enrichment now constructs `ProfilePresentationService` with the authenticated request audience before calling the existing customer/profile bridge. Human operators use their real profile actor id; service principals use trusted-service audience without claiming profile ownership. The bridge therefore applies Profiles `public` / `authenticated` / `followers_only` / `private` policy before returning a localized summary and no longer passes raw `ProfileService` from the custom host.

Canonical root `CustomerReadPort` construction uses `InProcessCustomerReadPort`. The owner read-policy helper and root wrapper retain correlation, exact owner/local operations, bounded delegated context shape, bounded operation-specific request shape, stable code, retryability, a closed error-kind label, and error-message presence/length. Exact stable local classifications return the same public-safe `PortError` unchanged. The persistent implementation in `ports.rs` remains unchanged and available as an explicit compatibility path. Complete `PortError` values, raw tenant/actor/channel/locale/causation/trace/idempotency values, raw customer/user UUIDs, exact pagination and enrichment counts, raw search, email, customer rows, profile names, preferred locales, and profile payloads are not logged.

The mounted customer-admin native bootstrap, list, detail, create, and update endpoints use static public context and typed owner envelopes. Original framework, customer storage, and profile causes remain in bounded server diagnostics with per-call correlation, tenant, actor, optional customer, channel, locale, stable code, and boundary context. `RequestContext` remains diagnostic-only, so this source slice does not change operation admission or profile audience policy.

The public customer-admin transport facade applies a second fail-closed client boundary across the same five operations. It no longer re-exports the private native `ApiError::ServerFn(String)` type; the public `ApiError::ServerFn` has no payload and always displays `Customer admin request could not be completed`. The final private native error is retained only in structured client diagnostics with operation, correlation id, and bounded request-shape facts.

## FFA/FBA boundary

- FFA status: `in_progress`
- FBA status: `boundary_ready`
- Structural shape: `core_transport_ui`
- FBA provider contract: `CustomerReadPort` / `customer.read_projection.v1` in `crates/rustok-customer/contracts/customer-fba-registry.json`.
- `read_customer_projection_by_user` is the owner boundary for storefront authenticated-customer lookup; commerce must not construct `CustomerService`.
- Canonical root construction is `InProcessCustomerReadPort` / `in_process_customer_read_port`; `rustok_customer::ports` remains a compatibility path rather than the covered root entrypoint.
- Customer read diagnostic safety: `source_ready_unvalidated`; evidence is `crates/rustok-customer/contracts/evidence/customer-read-diagnostic-safety-source.json` with reviewed source handoff in `crates/rustok-customer/contracts/evidence/customer-read-diagnostic-safety-source-review.json`.
- Static and source-locked runtime evidence: `crates/rustok-customer/contracts/evidence/customer-contract-test-static-matrix.json`, `crates/rustok-customer/contracts/evidence/customer-runtime-contract-smoke.json`, and `crates/rustok-customer/contracts/evidence/customer-read-projection-runtime-smoke.json`.
- Customer-admin native error-safety source evidence is `crates/rustok-customer/contracts/evidence/admin-native-error-safety-source.json`; it remains explicitly unvalidated.
- Admin client transport error safety: `source_ready_unvalidated`; evidence is `crates/rustok-customer/contracts/evidence/admin-client-transport-error-safety-source.json` with reviewed source handoff in `crates/rustok-customer/contracts/evidence/admin-client-transport-error-safety-source-review.json`.
- `scripts/verify/verify-customer-admin-boundary.mjs` locks the admin boundary; `node scripts/verify/verify-customer-admin-native-error-safety.mjs` locks the mounted static error envelopes; `node scripts/verify/verify-customer-admin-client-transport-error-safety.mjs` locks the final payload-free client envelope; `node scripts/verify/verify-customer-fba-no-compile.mjs` locks no-compile provider metadata and promotion blockers; `node scripts/verify/verify-customer-read-local-context.mjs` and `node scripts/verify/verify-customer-read-policy-context.mjs` lock bounded canonical read and owner-policy diagnostics without claiming execution evidence.

## Open results

0. **Keep profile provisioning enrichment owner-owned.**
   `CustomerReadPort::list_profile_enrichment` exposes only linked user id, name components and preferred locale for profile backfill. It must remain a narrow projection rather than exposing customer entities or persistence to Profiles or its future CLI adapter.

1. **Keep customer-admin profile presentation audience-bound.**
   **Status:** source-complete. Customer Admin detail/create/update construct the Profiles owner `ProfilePresentationService`; customer permission does not become profile ownership, and restricted/missing summaries remain absent.
   **Remaining evidence:** run customer admin detail flows as public-profile viewer, authenticated viewer, follower, profile owner, unrelated operator, and service principal; retain absence/error screenshots or transport traces.
   **Done when:** native customer admin runtime evidence proves the same matrix as public Profiles GraphQL without a raw reader bypass.

2. **Run compiled customer-port evidence.** Execute normalized identity guards, tenant-scoped read/list projections, and `PortCallPolicy::read()` deadline semantics before considering FBA promotion.
   **Depends on:** a build environment and a runtime-composed consumer.
   **Done when:** targeted customer service/port tests produce runtime evidence for every `CustomerReadPort` operation and fallback profile.

3. **Expand settings or profile flows only as customer-owned capabilities.**
   Keep customer settings distinct from auth and public-profile ownership, with explicit tenant and optional user/profile bridge rules.
   **Depends on:** a product requirement and the public auth/profile contracts.
   **Done when:** new flows have a module-owned API, tenant-isolation tests, and no duplicate policy in auth, Profiles, or Commerce.

4. **Retain actionable customer read diagnostics without payload leakage.**
   **Status:** source-ready / unvalidated for owner policy admission and canonical root reads. The owner helper records bounded context/error shape, and the root wrapper records bounded context plus UUID presence/non-nil, pagination zero/non-zero, search length, and enrichment duplicate/empty facts. Exact stable `operation + code + message` classification, technical-versus-ordinary severity, and unchanged error returns are retained without complete errors, raw context, identifiers, or exact pagination/count values.
   **Remaining evidence:** execute both focused guards, compile the owner and mounted consumers, and retain runtime traces for policy rejection, not-found, invalid-context, storage, profile-unavailable, and list-validation outcomes. Audit direct compatibility-path callers separately.
   **Done when:** runtime evidence proves canonical root consumers and owner admission emit actionable diagnostics without raw customer/profile payloads or complete error/context envelopes, and compatibility bypasses are either removed or explicitly accepted.

5. **Validate customer-admin native public error safety.**
   **Status:** source-ready / unvalidated. Mounted bootstrap, list, detail, create, and update endpoints no longer directly serialize framework or typed customer/profile causes. Domain validation, not-found, and duplicate outcomes retain stable public meaning; technical profile and storage causes are internal-only.
   **Remaining evidence:** run `verify-customer-admin-native-error-safety.mjs`, compile `rustok-customer-admin` with `ssr`, exercise all five endpoints against validation, not-found, duplicate, profile, and database failures, and retain server logs proving correlation without customer/profile payload leakage.
   **Done when:** source guard, compile, runtime envelope, tenant isolation, profile audience, restart, and remote-profile evidence are retained without promoting FFA/FBA from source inspection alone.

6. **Validate the customer-admin final client transport envelope.**
   **Status:** source-ready / unvalidated. The public facade maps every private native error through a per-call context and exposes only a payload-free `ApiError` with one static message.
   **Remaining evidence:** run `verify-customer-admin-client-transport-error-safety.mjs`, compile default/hydrate/ssr profiles, and retain browser plus mounted failures proving the raw private native string never reaches the rendered admin error.
   **Done when:** all five operations preserve their successful DTOs and server-side domain policy while framework and unexpected transport text remains private across serialization and display.

## Verification

- `npm run verify:customer:admin-boundary`
- `node scripts/verify/verify-customer-admin-native-error-safety.mjs`
- `node scripts/verify/verify-customer-admin-client-transport-error-safety.mjs`
- `node scripts/verify/verify-customer-read-local-context.mjs`
- `node scripts/verify/verify-customer-read-policy-context.mjs`
- `node scripts/verify/verify-customer-fba-no-compile.mjs`
- `npm run verify:ecommerce:fba`
- `cargo check -p rustok-customer-admin`
- `cargo check -p rustok-customer-admin --features hydrate`
- `cargo check -p rustok-customer-admin --features ssr`
- `cargo xtask module validate customer`
- `cargo xtask module test customer`
- targeted customer CRUD, identity, ownership, profile-bridge, audience-matrix, and local-outcome tests

## Change rules

1. Keep customer records and their policy in this module.
2. Customer/profile presentation must use the Profiles owner audience-bound service; customer permissions must not bypass profile visibility or claim profile ownership.
3. Update local documentation, `rustok-module.toml`, and related auth/profile docs when the customer contract changes.
4. Update this status block and `docs/modules/registry.md` with an FFA/FBA boundary change.
5. Keep raw customer/profile payloads, complete `PortError` values, raw delegated context, request UUIDs, and exact pagination/count values out of owner diagnostics; retain only bounded shape facts needed for recovery.
