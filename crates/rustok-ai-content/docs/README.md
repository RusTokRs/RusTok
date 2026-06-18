# Документация `rustok-ai-content`

`rustok-ai-content` — domain-owned support crate для content AI verticals: moderation и blog draft generation contracts.

## Назначение

- изолировать content moderation vertical от core-runtime `rustok-ai`;
- владеть generated payload contract и validation для `blog_draft`;
- подготовить единый policy seam для blog/forum/comment moderation сценариев.

## Зона ответственности

- registration seam для `content_moderation` и `blog_draft`;
- typed moderation contracts и approval integration hooks;
- typed blog draft contract (`GeneratedBlogDraft`) и minimal non-empty title/body validation.

## Проверка

- `cargo check -p rustok-ai-content`

## Связанные документы

- [README crate](../README.md)
- [План реализации](./implementation-plan.md)
