# Implementation plan for `rustok-comments`

## Current state

`rustok-comments` owns generic comment threads, comments, localized bodies,
thread status/moderation, and comment-domain observability. It is separate from
forum replies and shared content storage. Blog uses the module on its production
read/write path; page-like surfaces require explicit opt-in.

The admin moderation surface is an intentional native-only exception: it has a
module-owned core, native transport facade, and Leptos adapter, with
`HostRuntimeContext`. Thread and locale route/query policy is
core-owned, and UI does not call raw transport.

The native-only comments admin exception uses host-neutral native admin transport
and the shared `UiRouteQueryIntent` contract for prepared host route-query
writes.

The shared `rustok-api::richtext` document contract and
`rustok-content::richtext` `comment` profile are implemented and Comments is
the first owner cut over to them. `CreateCommentInput` and
`UpdateCommentInput` accept only `RichTextDocument`; `CommentRecord` returns
`RichTextView` plus the server-derived plain-text projection. Comment body rows
persist canonical ProseMirror/Tiptap JSON without a format selector, and public
previews use the plain-text projection. The cutover migration fails closed on
pre-existing non-canonical rows so an offline conversion can be completed
before the schema column is dropped.

## FFA/FBA boundary

- FFA status: `in_progress`
- FBA status: `boundary_ready`
- Structural shape: `core_transport_ui`
- FBA provider contract: `CommentsThreadPort` / `comments.thread.v1` in
  `crates/rustok-comments/contracts/comments-fba-registry.json`.
- Static and runtime-order evidence:
  `crates/rustok-comments/contracts/evidence/comments-contract-test-static-matrix.json`
  and `crates/rustok-comments/contracts/evidence/comments-provider-runtime-order-smoke.json`.
- `scripts/verify/verify-comments-admin-boundary.mjs` and
  `npm run verify:comments:fba` lock the native-only admin boundary and comment
  provider policy order.
- Public-port create/delete publish `comment.created` and `comment.deleted`
  through `TransactionalEventBus::publish_in_tx` when the provider is runtime
  composed with its owner event bus. Blog's idempotent reply-count projection is
  implemented statically under `DECISIONS/2026-07-16-comments-blog-event-projection.md`;
  runtime delivery, retry, and recovery evidence remain open.

## Open results

1. **Implement and execute the Blog reply-count event projection.** Consume
   `comment.created` and `comment.deleted` idempotently, publish the Blog-owned
   update event in the projection transaction, and prove retry/degraded behavior.
   **Depends on:** runtime event delivery and projection storage fixtures.

2. **Execute CommentsThreadPort runtime and consumer evidence.** Cover read and
   write paths, canonical read/write policy, idempotency, typed errors,
   fallback/degraded profiles, and blog embedded/native compatibility before
   FBA promotion.
   **Depends on:** a runtime-composed provider and blog consumer fixtures.
   **Done when:** evidence proves provider and consumer behavior without a
   direct comments-service bypass.

3. **Extend moderation and opt-in integrations through comment ownership.**
   Add a new commentable surface only with explicit target binding, moderation,
   rich-text, tenant, and observability contracts; do not reuse forum storage.
   **Depends on:** the consuming module's product requirement and public API.
   **Done when:** the new surface has owner-owned storage and transport tests,
   and its opt-in decision is documented.

4. **Keep operational guidance synchronized with thread semantics.** Update
   status alerts, moderation playbook, metrics, and local docs with a change to
   thread lifecycle or comment delivery.
   **Depends on:** the changed comments runtime contract.
   **Done when:** closed/spam/trash behavior and recovery are observable and
   documented for operators.

5. **Close the direct-write richtext bypass and join the atomic cutover.**
   **Implemented for Comments.** A direct `CommentsThreadPort` or service
   write accepts the typed `RichTextDocument`, selects the `comment` profile
   server-side, and passes the strict validator. `comment_bodies` no longer
   stores a format selector; reads use canonical HTML/plain-text projections.
   The remaining verification is runtime evidence for every consumer and the
   offline conversion procedure for existing Markdown rows.
   **Depends on:** the
   [central Richtext plan](../../../docs/modules/rich-text-implementation-plan.md)
   and synchronized Blog consumer contract.
   **Done when:** invalid/empty/oversized documents fail at every entry point,
   no direct port bypass exists, and Next/Leptos reads share the server renderer.

## Verification

- `npm run verify:comments:admin-boundary`
- `npm run verify:comments:fba`
- `cargo xtask module validate comments`
- `cargo xtask module test comments`
- Targeted moderation/status, blog integration, comment-port, and admin runtime
  tests.
- `cargo check -p rustok-comments`
- `cargo check -p rustok-blog`

## Change rules

1. Keep generic comment storage and moderation in this module.
2. Update local docs, `rustok-module.toml`, and consumer docs with a comment
   contract or opt-in integration change.
3. Update this status block and `docs/modules/registry.md` with an FFA/FBA
   boundary change.
