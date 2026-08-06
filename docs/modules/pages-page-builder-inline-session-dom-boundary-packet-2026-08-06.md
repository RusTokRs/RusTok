# Pages / Page Builder Inline Session DOM Boundary Packet

Date: 2026-08-06  
Status: `source-fixed / maintainer-validation-pending`

## Finding

The authenticated Page Builder inline storefront retained the grant session identifier in two browser-visible places:

```text
id="fly-inline-<grant session>"
data-inline-session="<grant session>"
```

That contradicted the established Pages/Page Builder boundary that bearer material, sessions, grants and proofs must not enter authoring URLs or DOM attributes.

## Fix

`PageBuilderAuthenticatedInlineStorefront` now derives its deterministic hydration root from non-secret document identity:

```text
Fly page id + expected project hash
```

The `data-inline-session` attribute is removed entirely.

The grant still binds the authenticated session internally. The session remains available only inside the trusted Rust/WASM grant and request objects used for authorization. It is no longer rendered into SSR HTML or hydrated DOM identity.

## Preserved behavior

This change does not alter:

- grant signing or verification;
- direct-user or authenticated-session admission;
- page, locale, revision, project-hash or expiry binding;
- monotonic request sequence validation;
- editable component eligibility;
- canonical `EditorCommand::Patch` mutation;
- Pages `save_document(expected_revision)` persistence;
- replacement grant issuance;
- public or anonymous storefront rendering;
- database, GraphQL, REST, event, publish or rollback schemas.

## Regression boundary

The existing authenticated inline adapter source guard now requires:

```text
let root_id = inline_root_id(&grant)
dom_id(grant.page_id())
grant.expected_project_hash().hex()
```

It rejects:

```text
data-inline-session
dom_id(grant.session_id())
data-inline-proof
```

A focused Rust regression assertion verifies that the generated root id contains neither the grant session nor authorization proof.

## Validation status

No tests, static verifiers, formatting, Cargo commands, WASM builds, server builds, browser scenarios, workflows or CI were run by the implementation agent.

Suggested checks, intentionally not run:

```bash
node crates/rustok-page-builder/scripts/verify/verify-page-builder-authenticated-inline-edit-adapter.mjs
cargo test -p rustok-page-builder-storefront inline_dom_identity_excludes_grant_session_and_authorization_proof
cargo check -p rustok-page-builder-storefront --features inline-edit,ssr
cargo check -p rustok-page-builder-storefront --target wasm32-unknown-unknown --features inline-edit,hydrate
```

Browser evidence remains pending and must additionally inspect the SSR response and hydrated DOM for session/proof absence.
