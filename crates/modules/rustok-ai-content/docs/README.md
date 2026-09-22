# rustok-ai-content documentation

`rustok-ai-content` is a domain-owned support crate for content AI verticals:
moderation and blog draft generation contracts.

## Purpose

- Isolate content moderation from the core `rustok-ai` runtime.
- Own the generated payload contract and validation for `blog_draft`.
- Provide the policy seam for blog, forum, and comment moderation scenarios.

## Scope

- Registration for `content_moderation` and `blog_draft`.
- Typed moderation contracts and approval integration hooks.
- `GeneratedBlogDraft` validation for all optional text fields: `title`,
  `slug`, `body`, `excerpt`, `seo_title`, and `seo_description`.

## Verification

- `node scripts/verify/verify-ai-content-contract.mjs` — static validation of
  domain-owned descriptors, policy matrix, and blog-draft contract tests.
- `cargo test -p rustok-ai-content --lib`

## Related documents

- [Crate README](../README.md)
- [Implementation plan](./implementation-plan.md)
