# `rustok-pages` does not get default integration with `rustok-comments`

- Date: 2026-03-29
- Status: Accepted

## Context

After the storage split of `rustok-blog`, `rustok-pages` and `rustok-comments`, the storage boundaries were already
separated, but the product boundary between `pages` and `comments` remained unfixed.

The actual state of the code and runtime at this point is as follows:

- `rustok-blog` already uses `rustok-comments` as a live comment backend;
- `rustok-pages` is developing page-builder, blocks, menus, and channel-aware publication surface;
- `pages` has no built-in comment transport/UI/runtime path;
- local descriptions of `rustok-comments` still sounded as if integration with `pages`
  was mandatory or already active.

If every page is considered commentable by default, this creates an implicit product contract:

- any page read-path begins to imply discussion lifecycle and moderation policy;
- static/landing/help-center pages receive an unnecessary domain responsibility;
- future integration becomes more difficult to make targeted and opt-in.

This tail of the plan needs to be explicitly closed so that `pages` and `comments` do not remain in a state
of architectural ambiguity.

## Decision

### 1. `pages` has no default comments surface

In the current product, `rustok-pages` does not integrate with `rustok-comments` by default.

This means:

- no automatic creation of `comment_threads` for each page;
- no mandatory comments UI in the `pages` admin/storefront surface;
- no implicit dependency `rustok-pages -> rustok-comments`.

### 2. `rustok-comments` remains a generic backend for opt-in non-forum discussions

`rustok-comments` remains the canonical storage-owner for blog comments and for future
opt-in non-forum discussion surfaces, but `pages` is not automatically considered such a surface.

### 3. Future page-level comments integration is only possible as explicit opt-in

If the product later requires comments on a page-like surface, this must be structured as a
separate opt-in integration:

- for specific page templates / page kinds / dedicated surfaces;
- with a separate specification for moderation, publication, SEO, and storefront rendering;
- without turning all `pages` into default discussion targets.

## Consequences

- The `rustok-comments` plan closes the `pages <-> comments` item with a "not by default" decision.
- Local docs and metadata of `rustok-comments` must no longer assert that `pages`
  is already a live integration target.
- `rustok-pages` continues to develop as a page/content presentation module without a built-in
  discussion lifecycle.
- If a commentable knowledge-base/article surface appears later on top of `pages`, it will require
  a separate ADR/spec instead of an implicit extension of the current contract.
