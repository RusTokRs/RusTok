# Page Builder Parity Actualization — Authenticated Inline Adapter

Date: 2026-08-06  
Status: current-source overlay / adapter source-ready / consumer mount open / execution-pending

## Purpose

This current-source overlay updates only the authenticated storefront inline-edit boundary in `docs/modules/page-builder-implementation-plan.md` and the shared Pages/Page Builder continuation cursor.

The broader Page Builder plan remains authoritative for the complete programme. This overlay records that the reusable feature-gated authenticated real-DOM adapter now exists in source, while Pages consumer authentication, grant issuance, persistence and public-host mounting remain open.

## Current source

`fly-leptos` now owns a reusable browser adapter that:

- receives an opaque trusted grant binding session, page, revision, exact Fly project hash and expiry;
- marks only an explicit allow-list of instrumented static leaf text nodes;
- treats the real DOM as a temporary plain-text buffer;
- emits one bounded commit request on `focusout`;
- redacts the authorization proof from debugging and never writes it into DOM;
- restores pre-existing attributes and unregisters its listener on teardown.

`rustok-page-builder-storefront` now exposes a feature-gated authenticated inline surface and canonical session that:

- is absent unless `inline-edit` is enabled;
- leaves the existing read-only renderer uninstrumented;
- rejects provider-owned nodes, composite nodes with children, templated nodes and every node inside runtime-owned binding, condition or repeater subtrees;
- treats those binding, condition and repeater targets plus descendants as runtime-owned subtrees;
- permits a stable static leaf nested inside an ordinary unowned layout;
- validates grant identity, expiry, sequence, selected page and exact project hash;
- invokes a consumer `InlineEditAuthorizationPort` immediately before mutation;
- converts the request into one canonical Fly `EditorCommand::Patch` for `content`;
- returns the complete current project plus prior/new hashes and command sequence.

Because the canonical project hash changes, the grant is intentionally one-commit. A successful consumer must persist the current project and issue a replacement grant.

## Ownership retained

- Fly remains the sole document and command authority.
- DOM content is not imported as a component tree.
- Page Builder does not authenticate Pages users or persist Pages bodies.
- Pages remains responsible for document revisions, update permission and transport.
- anonymous storefront profiles do not enable the `inline-edit` feature.
- rich text remains a separate capability and is not edited through this plain-text adapter.

## Remaining open consumer boundary

Pages consumer grant issuance and save transport remain open. The next source slice must:

1. authenticate the storefront user and tenant/page/channel context;
2. require Pages update permission and the Page Builder inline-edit capability;
3. issue a short-lived grant for the exact body revision and Fly project hash;
4. verify/reauthorize each request server-side;
5. persist the returned project through the existing document-only body revision owner;
6. return the new revision/hash and replacement grant;
7. remain excluded from anonymous dependency and bundle profiles.

## Execution status

No tests, verifiers, formatting, Cargo checks, WASM builds, browser runs, dependency graph checks, workflows or CI were executed by the implementation agent. Source-ready does not imply execution-ready or rollout-ready.
