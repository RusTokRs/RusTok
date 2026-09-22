# Separating `content`-storage, introducing `rustok-comments` and the new role of `rustok-content`

- Date: 2026-03-28
- Status: Accepted

## Context

The current model where `rustok-blog`, `rustok-forum` and part of `rustok-pages` rely on a shared storage
layer `rustok-content` (`nodes + metadata`) is already creating systemic problems:

- domain states are mixed with generic `kind` and `metadata`;
- read-paths perform unnecessary re-reads and lose filters/ordering;
- `blog`, `forum` and `pages` evolve as different bounded contexts but continue to share a single
  storage-owner;
- the generic `content` REST/GraphQL transport has begun to solidify as a product API, although the target model
  no longer assumes `rustok-content` as the owner of domain CRUD surfaces;
- an explicit conversion tool `blog post + comments <-> forum topic + replies` is needed, but it must
  not impose a common table for all discussion entities.

The team consciously chooses to break the current architectural boundary now, while the volume of code and data
still allows this to be done in a controlled manner.

## Decision

### 1. `rustok-content` ceases to be shared product storage

`rustok-content` remains in the platform, but its target role changes:

- shared content library for rich-text/Tiptap contract, locale fallback, slug/canonical helpers;
- orchestration layer for cross-domain operations;
- place for idempotency/audit/conversion records;
- not the canonical CRUD/storage backend for `blog`, `forum` and `pages`.

The generic `content` REST/GraphQL/API is considered a transitional layer and is subject to removal after the split.

### 2. `rustok-comments` is introduced as a separate optional module

A new domain module `rustok-comments` is created with its own storage boundary for classical
comments outside the forum:

- blog post comments;
- page comments and comments on other non-forum content-like entities;
- its own contracts for thread/comment/moderation within the comment-domain.

At the current step, the module is introduced as a scaffold and a point for fixing the new boundary.

### 3. `forum replies` and `comments` are not merged

A strict boundary rule is adopted:

- `forum replies != comments`;
- `rustok-forum` remains an independent discussion domain with its own categories, topics,
  replies, moderation, counters, and read-model;
- `rustok-comments` does not become the storage base for the forum.

### 4. `rustok-pages` participates in the same split

`rustok-pages` is not considered an eternal specialization of shared `content`.
Pages, blocks, and menus in the target architecture should transition to page-owned persistence model.

### 5. Conversion between blog and forum is done through orchestration

Supported target operations:

- `blog post + comments -> forum topic + replies`
- `forum topic + replies -> blog post + comments`

These are explicit orchestration/conversion flows, not live sync and not an argument for a common table.
After conversion, there must be one canonical source for further editing.

### 6. Legacy model is subject to removal, not endless adaptation

When performing the split:

- do not extend the generic `content` API with new product scenarios;
- do not develop a central string-based `kind` registry as a long-term domain model;
- do not increase the coupling of product modules to `NodeService`;
- remove legacy abstractions after replacement, rather than keeping them indefinitely for convenience.

## Consequences

- A new `rustok-comments` module must be created as an actual module in the workspace, server wiring, and documentation.
- A staged split of `blog`, `forum`, `pages` from shared `rustok-content` storage is needed.
- `rustok-content` documentation must be rewritten for its role as orchestration/shared library.
- `ContentOrchestrationService` cannot be carried over as-is: after the split it must work through
  domain services, not through transferring shared `node` records.
- The generic `content` transport will remain transitional only for the duration of the migration and will then be removed.
