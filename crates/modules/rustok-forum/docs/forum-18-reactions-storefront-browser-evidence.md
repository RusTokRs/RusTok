# FORUM-18 Reactions storefront browser evidence

Status: **executable browser source / maintainer execution pending**

## Scope

This slice adds browser-evidence source for the already-merged bounded Forum/Reactions storefront host composition. It changes no Forum or Reactions production runtime behavior.

The Rust Playwright harness is:

```text
tests/e2e-rust/tests/leptos_storefront_forum_reactions.rs
```

The machine-readable evidence contract is:

```text
crates/rustok-forum/contracts/forum-reactions-storefront-browser-evidence.json
```

The source guard is:

```text
scripts/verify/verify-forum-reactions-storefront-browser-evidence.mjs
```

## Fixture boundary

The harness intentionally does not seed Forum or Reactions state and does not bypass production authorization. Maintainer execution supplies two fully prepared storefront URLs through environment variables:

```text
RUSTOK_FORUM_TOPIC_REACTIONS_E2E_URL
RUSTOK_FORUM_REPLY_REACTIONS_E2E_URL
```

The target tenant must already have Forum and Reactions enabled. The topic and reply must be visible under the real Forum storefront visibility/lifecycle rules, and the reply URL must be the same canonical topic route with one valid `?reply=<uuid>` selection.

`PLAYWRIGHT_CHROMIUM_EXECUTABLE` remains the existing optional browser override used by the Rust E2E crate.

## Browser cases

### Topic composition

The topic URL must navigate with an HTTP status below 400. The rendered document must contain:

```text
data-storefront-composition="forum-topic-reactions"
```

and must not contain:

```text
data-storefront-composition="forum-reply-reactions"
```

This retains the selected-topic host composition as the one active Reactions presentation target.

### Selected-reply composition

The reply URL must carry an explicit `reply=` query selection and navigate with an HTTP status below 400. The rendered document must contain:

```text
data-storefront-composition="forum-reply-reactions"
```

and must not contain the topic composition marker.

This retains the bounded replacement rule introduced by the selected-reply slice: one valid selected reply replaces the topic ReactionBar instead of mounting a second ambiguous Reactions control.

## Preserved ownership

The harness observes only mounted browser output. It does not call `reactionSnapshot`, `applyReaction`, raw Forum revision GraphQL fields or producer-private storage. Forum owner/storefront packages remain free of Reactions owner/presentation dependencies, while `rustok-reactions-storefront::ReactionBar` remains the reusable presentation owner.

This evidence source does not claim network-call cardinality for every visible reply. The no-fan-out invariant remains enforced by the host source/contract guards; this browser slice proves only the observable one-target composition result.

## Verification handoff

Maintainers can run:

```bash
node scripts/verify/verify-forum-reactions-storefront-browser-evidence.mjs
cargo test -p rustok-e2e-rust --test leptos_storefront_forum_reactions -- --nocapture
```

No tests, Node verifiers, Cargo commands, formatting, browser launch, HTTP navigation, workflows, CI, event-digest generation or database evidence were executed while preparing this slice.

## Remaining FORUM-18 evidence

After this source is merged, FORUM-18 remains `in_progress`. Maintainer execution still needs to retain the browser run plus the pending event-digest, release lockfile, owner/event/reconciliation, Forum+Blog provider, GraphQL schema/runtime, native/GraphQL revision transport and broader runtime evidence recorded in the canonical plan.

`crates/rustok-forum/docs/implementation-plan.md` remains the only authoritative Forum roadmap.
