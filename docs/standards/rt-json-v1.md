---
id: doc://docs/standards/rt-json-v1.md
kind: project_overview
language: markdown
last_verified_snapshot: snap_jsonl_00000021
source_language: markdown
status: verified
---

# `rt_json_v1` Specification

`rt_json_v1` is the canonical JSON rich-text format for RusToK (blog/forum).

## Payload format

```json
{
  "version": "rt_json_v1",
  "locale": "ru",
  "doc": {
    "type": "doc",
    "content": []
  }
}
```

- `version` is required and must be `rt_json_v1`.
- `locale` is required, valid in `ll` or `ll-RR` format and must match the request locale.
- `doc` is required and contains the node tree.

## Allowed nodes

Supported node types:

- `doc`
- `paragraph`
- `heading` (`attrs.level` from 1 to 6)
- `bullet_list`
- `ordered_list`
- `list_item`
- `blockquote`
- `code_block`
- `horizontal_rule`
- `hard_break`
- `text`
- `image` (`attrs.src`)
- `embed` (`attrs.provider`, `attrs.url`)

## Allowed marks

Supported marks:

- `bold`
- `italic`
- `strike`
- `code`
- `link` (`attrs.href`)

A single `text` node may have at most 8 marks.

## Depth and size limits

- Maximum tree depth: `8`.
- Maximum node count: `2000`.
- Maximum total text content size: `100000` characters.

## URL / embed policy

- For `link.attrs.href`: only `http`, `https`, `mailto` are allowed.
- For `image.attrs.src`: only `http`, `https` are allowed.
- For `embed`, only the following providers are allowed:
  - `youtube` (`youtube.com`, `www.youtube.com`, `youtu.be`)
  - `vimeo` (`vimeo.com`, `player.vimeo.com`)
- For `embed.attrs.url`, `https` is required.

## Unknown node/mark handling

- Unknown nodes and marks are not persisted (dropped during sanitize).
- If after sanitize the document becomes empty/invalid — the request is rejected as a validation error.

## Format versions

- `rt_json_v1` — the currently supported version for writing and rendering in the backend.
- `rt_json_v2` — reserved (known-but-unsupported): backend v1 recognizes it but rejects with an explicit incompatibility error.
- Any other version is considered unknown and rejected.

## Versioning and compatibility

- **Backward compatibility (legacy -> v1)**: if `version` is absent, the backend attempts to transform the payload into `rt_json_v1`:
  - if payload already looks like `doc`, it is wrapped in `{"version":"rt_json_v1","locale":"<request-locale>","doc":...}`;
  - if payload is an object with `doc`, missing `version/locale` fields are added.
- **Forward compatibility (v2+ -> v1 backend)**: unknown versions (`version != rt_json_v1`) are rejected.

## Backend enforcement

In blog/forum, client-side validation is considered **advisory only**.

Every incoming rich-text JSON on the backend goes through:

1. schema validation (`version/locale/doc`, allowed nodes/marks, limits, URL/embed policy),
2. sanitize (drop unknown nodes/marks, normalize attrs),
3. storage of only sanitized JSON.

## Migration plan `markdown -> rt_json_v1` (without breaking release)

The migration is performed in stages, preserving backward compatibility:

1. **Dual-write-ready API (current stage)**
   - DTO create/update accept `body_format`/`content_format` and `content_json`.
   - Backend accepts both formats: `markdown` and `rt_json_v1`.
   - For rich-content, sanitized `rt_json_v1` is stored.
2. **Dual-read + legacy fallback**
   - When reading old records without a format or with historical markdown, a `markdown` fallback is applied.
   - No database migration is required at this step.
3. **Background conversion of historical data**
   - A batch-job converts markdown records to `rt_json_v1` in the background (tenant-by-tenant / module-by-module).
   - For each record, both audit trail and safe retry capability are preserved.
4. **Gradual rollout by write channels**
   - UI/API clients gradually switch to sending `rt_json_v1`.
   - Metrics: share of `rt_json_v1` writes, validation errors, sanitize-drop rate.
5. **Legacy write restriction (after saturation)**
   - After reaching the target threshold (>95% rich writes), a soft-warning is introduced for new markdown writes.
   - Hard markdown disable is only permitted by a separate ADR and release note.

Key principle: **read compatibility is preserved at all stages**, so no "big bang" migration is necessary.
