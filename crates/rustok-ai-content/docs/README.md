# `rustok-ai-content` Documentation

`rustok-ai-content` is a domain-owned support crate for content AI verticals: moderation and blog draft generation contracts.

## Purpose

- isolate the content moderation vertical from the core `rustok-ai` runtime;
- own the generated payload contract and validation for `blog_draft`;
- prepare a unified policy seam for blog/forum/comment moderation scenarios.

## Area of Responsibility

- registration seam for `content_moderation` and `blog_draft`;
- typed moderation contracts and approval integration hooks;
- typed blog draft contract (`GeneratedBlogDraft`) and validation for all optional text fields: `title`, `slug`, `body`, `excerpt`, `seo_title`, `seo_description`.

## Verification

- `node scripts/verify/verify-ai-content-contract.mjs` — compile-free static gate for domain-owned descriptors, policy matrix and blog draft contract tests.
- `cargo check -p rustok-ai-content`

## Related Documents

- [README crate](../README.md)
- [Implementation Plan](./implementation-plan.md)
