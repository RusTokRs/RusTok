# Page Builder Parity Actualization — Authenticated Inline Adapter

Date: 2026-08-06  
Status: historical adapter overlay / adapter source-ready / execution-pending

## Purpose

This current-source overlay records the reusable Page Builder authenticated real-DOM adapter introduced before the Pages consumer existed. The broader Page Builder plan remains authoritative for the complete programme.

The adapter boundary remains independently valid and is now consumed by the later Pages authenticated inline consumer packet. This document does not retroactively claim that the earlier adapter slice added Pages authentication, grant issuance or persistence.

## Adapter source

`fly-leptos` owns a reusable browser adapter that:

- receives an opaque trusted grant binding session, page, revision, exact Fly project hash and expiry;
- marks only an explicit allow-list of instrumented noninteractive static leaf text nodes;
- treats the real DOM as a temporary plain-text buffer;
- emits one bounded commit request on `focusout`;
- redacts the authorization proof from debugging and never writes it into DOM;
- restores pre-existing attributes and unregisters its listener on teardown.

`rustok-page-builder-storefront` exposes a feature-gated authenticated inline surface and canonical session that:

- is absent unless `inline-edit` is enabled;
- leaves the existing read-only renderer uninstrumented;
- rejects provider-owned, composite, templated, interactive and runtime-owned subtrees;
- validates grant identity, expiry, sequence, selected page and exact project hash;
- rejects unchanged focusout requests without changing history or hash;
- invokes a consumer `InlineEditAuthorizationPort` immediately before mutation;
- converts a changed request into one canonical Fly `EditorCommand::Patch` for `content`;
- returns the complete current project plus prior/new hashes and command sequence.

Because the canonical project hash changes, a successful changed edit requires a new grant.

## Ownership retained

- Fly remains the sole document and command authority.
- DOM content is not imported as a component tree.
- Page Builder does not authenticate Pages users or persist Pages bodies.
- rich text remains a separate capability.
- anonymous storefront profiles do not enable the adapter.

## Later Pages consumer

The earlier statement that Pages consumer grant issuance and save transport remain open was correct for PR #3039. It is superseded by the later Pages authenticated inline consumer packet:

`docs/modules/pages-page-builder-authenticated-inline-consumer-packet-2026-08-06.md`

That later slice adds Pages-owned signed grants, direct authenticated-session binding, document-only server functions and persistence through `PageService::save_document`. The authenticated route mount still remains open.

This adapter evidence continues to report these historical source booleans as false:

- Pages consumer grant issuance added by the adapter slice;
- Pages consumer save transport added by the adapter slice;
- anonymous storefront inline mount added by the adapter slice.

## Execution status

No tests, verifiers, formatting, Cargo checks, WASM builds, browser runs, dependency graph checks, workflows or CI were executed by the implementation agent. Source-ready does not imply execution-ready or rollout-ready.
