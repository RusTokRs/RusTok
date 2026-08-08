# Forum / Page Builder browser evidence harness actualization — 2026-08-08

Status: `source-ready / maintainer-browser-execution-pending / runtime-evidence-pending`

## Rechecked cursor

PR #3254 completed the FORUM-32 source composition for canonical contribution metadata, Fly component/block identity, owner preview and owner-backed property editing. The next canonical cursor is executable evidence, not another Forum schema/data implementation inside Page Builder.

This slice retains the browser-execution source needed for that next step. It does not execute Chromium, does not promote runtime evidence, and does not claim an observed Page Builder Wave.

## Retained browser contract

The canonical browser contract is:

```text
crates/rustok-forum/contracts/evidence/forum-page-builder-browser-execution-contract.json
```

A successful maintainer execution writes only:

```text
format: forum_page_builder_browser_execution_v1
status: browser_execution_passed_runtime_evidence_pending
```

The packet is bound to the exact source commit and immutable deployment RepoDigest. Required production source files are hashed at execution time so browser evidence cannot be replayed against a different Forum/Page Builder implementation.

## Harness owner boundary

The runner lives with the repository's existing pinned Playwright dependency:

```text
apps/next-admin/playwright.forum-page-builder.config.ts
apps/next-admin/tests/forum-page-builder/global-setup.ts
apps/next-admin/tests/forum-page-builder/browser-evidence.spec.ts
```

`apps/next-admin` is only the evidence runner package. The browser target is the maintainer-supplied real Leptos Pages admin route. The harness does not add a Next-admin Forum/Page Builder serving path.

The configuration fixes Chromium to one worker, disables retries, trace, screenshots and video, and writes no Playwright artifact containing cookies, owner payloads or rendered Forum data. Global setup removes only the bounded evidence output inside repository `target/` before execution so a failed run cannot leave a stale prior success packet behind.

## Required maintainer fixtures

Maintainers provide reviewed external storage-state files and five real Pages admin URLs:

- `full`: Forum enabled, Page Builder authoring/preview/properties available, effective `forum_topics:read`;
- `preview_off`: Forum enabled, `tree + properties` available, preview capability disabled;
- `properties_off`: Forum enabled but the authoring/property contribution is filtered;
- `Forum-disabled`: Pages remains available but Forum is not tenant-enabled;
- `no_read`: Forum is enabled but the authenticated user lacks effective `forum_topics:read`.

The full profile must point to a reviewed disposable unpublished draft because the evidence deliberately inserts and saves one Forum block. The remaining profile URLs must also be dedicated evidence fixtures so capability/module state is not inferred from production editorial content.

The harness never seeds tenants, sessions, pages or provider profiles. It observes the reviewed environment only.

## Full-profile proof

The full profile uses the existing `forum.topic_list` contract because it can prove property normalization without requiring a pre-existing topic/reply fixture.

The browser must observe:

1. `forum.topic_list` is admitted in the real Fly palette and can be inserted;
2. the selected component can load `forum.topic_list.v1` from the owner property transport;
3. `per_page=101` is rejected by owner validation;
4. a valid `per_page=10` plus whitespace-padded UUID is accepted, with sanitize feedback and owner-normalized UUID retained in Fly `props`;
5. Fly undo restores the pre-property state and schema defaults;
6. Fly redo restores the owner-normalized state;
7. owner preview resolves successfully through the admitted Forum preview renderer/transport;
8. the ordinary Pages facade save completes.

The harness reads owner diagnostics and preview readiness only for assertions. Raw owner preview payloads, DOM/HTML and Forum content are never written to retained evidence.

## Capability-profile proof

`preview_off` must keep `forum.topic_list` and owner property schema loading available while the selected component has no actionable Forum preview renderer.

`properties_off` must expose no `forum.topic_list` palette block and no actionable Forum property editor.

The Forum-disabled route must expose neither Forum palette blocks nor the owner property/preview panels created by the Forum host extension.

The `no_read` session must not receive Forum contribution admission. This proves the browser-facing contribution registry follows the effective permission handshake; direct transport authorization execution remains a separate runtime-evidence gate.

## Retention/privacy boundary

The output retains only:

- exact source commit;
- immutable deployment RepoDigest;
- SHA-256 hashes of required source files;
- SHA-256 hashes and byte sizes of external storage-state files;
- SHA-256 hashes of the five profile URLs;
- bounded boolean/count observations;
- Node and Playwright versions.

It does not retain raw URLs, cookies, Authorization headers, storage-state contents, owner preview payloads, raw DOM/HTML, tenant/actor identifiers or Forum topic/reply content.

The previous output is removed before execution. If any profile fails, a new success evidence packet is not written.

## Source guard

The source-only guard is:

```bash
node scripts/verify/verify-forum-page-builder-browser-evidence-harness.mjs
```

It verifies the evidence contract, profile matrix, retention boundary, stale-output cleanup, stable production browser selectors, existing Forum host composition and the no-execution status.

## Maintainer browser command

After deploying the exact source commit and preparing the reviewed fixture URLs/storage states:

```bash
cd apps/next-admin
npx --no-install playwright test \
  --config playwright.forum-page-builder.config.ts
```

Required environment values are declared by the JSON contract and include the exact source commit, immutable deployment digest, two external storage-state files and five profile URLs.

## Promotion boundary

A passing browser packet may close only the retained browser portion of FORUM-32 for that exact source/image/environment.

It does not by itself prove:

- direct Forum property/preview transport tenant/module/RBAC rejection paths;
- server/runtime visibility semantics outside the browser flow;
- provider SLO health;
- observed tenant Wave readiness.

Those runtime authorization checks remain the next evidence slice. Observed Forum Wave evidence remains blocked on the existing Pages reference-consumer gate plus accepted Forum browser/runtime packets.

No browser execution is claimed by this source slice. No tests, Node verifiers, Cargo commands, formatters, builds, workflows, CI, database or runtime evidence were executed while preparing it.
