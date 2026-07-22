---
id: doc://docs/standards/rt-json-v1.md
kind: technical_reference
language: en
status: deprecated
---

# Legacy `rt_json_v1` Implementation Snapshot

> Deprecated: this document describes the currently implemented legacy
> contract only. New architecture and implementation work must follow the
> [Richtext implementation plan](../modules/rich-text-implementation-plan.md)
> and the
> [proposed capability-boundary ADR](../../DECISIONS/2026-07-22-richtext-capability-boundary.md).

`rt_json_v1` is not the target RusToK contract. It remains documented while the
current code still reads/writes it so that the atomic cutover can inventory and
remove every dependency without pretending that the target is already live.

## Implemented envelope

The current backend expects an outer envelope:

```json
{
  "version": "rt_json_v1",
  "locale": "en",
  "doc": {
    "type": "doc",
    "content": []
  }
}
```

The implementation is primarily in `crates/rustok-core/src/rt_json.rs` and
`crates/rustok-core/src/content_format.rs`. Blog and Forum serialize the JSON
into string body columns and reconstruct a separate `content_json` transport
field. Some callers also accept `rt_json` as an alias.

## Implemented node and mark names

The legacy allowlist contains:

- nodes: `doc`, `paragraph`, `heading`, `bullet_list`, `ordered_list`,
  `list_item`, `blockquote`, `code_block`, `horizontal_rule`, `hard_break`,
  `text`, `image`, and `embed`;
- marks: `bold`, `italic`, `strike`, `code`, and `link`.

The configured baseline limits are depth 8, 2,000 nodes, 100,000 text
characters, and 8 marks on one text node. Link schemes are limited; image and
embed URLs have separate legacy checks.

## Known implementation limitations

This snapshot must not be used as a design baseline:

- locale is duplicated inside the document instead of remaining solely in the
  owner row/request;
- the locale parser is narrower than the platform locale contract;
- validation does not enforce the complete ProseMirror tree grammar;
- unknown nodes/marks can be silently dropped, which can lose author data;
- unrecognised object fields can survive normalization;
- different callers enforce different levels of validation, and direct
  Comments writes can bypass document parsing;
- the manual browser mapping between legacy snake-case node names and Tiptap
  names can drift and lose content;
- the same source is transported as both a serialized body and `content_json`;
- the legacy migration binary does not cover current owner-local Blog, Forum,
  and Comments tables and is not a valid target migration path.

## Superseded compatibility strategy

The previous dual-read/dual-write and indefinite Markdown fallback strategy is
superseded by the repository's initial-implementation rule. The target cutover
will migrate repository-owned data and callers atomically, then delete aliases,
fallbacks, legacy editor modes, and this snapshot.

Historical Markdown, if material rows exist, is an offline migration input
only. It is not a supported target storage or authoring format.

## Removal criteria

Delete this document with the legacy implementation when all of the following
are true:

1. owner-local Blog, Forum, and Comments rows/revisions are migrated;
2. all transports use the typed `RichTextDocument` contract;
3. `rustok-core::rt_json`, generic legacy content-format helpers, aliases, and
   the old migration binary are removed;
4. Search/Index consume the canonical plain-text projection;
5. Next and Leptos editor/read paths pass the target verification matrix.
