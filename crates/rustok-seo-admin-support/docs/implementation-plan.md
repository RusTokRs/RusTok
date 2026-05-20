# План реализации `rustok-seo-admin-support`

Статус: базовый support crate уже введён и используется owner-module admin пакетами.

## Execution checkpoint

- Current phase: plan_sync
- Last checkpoint: Initial bootstrap by registry workflow.
- Next step: Синхронизировать план с текущим кодом и выбрать первый незавершённый пункт.
- Open blockers: None.
- Hand-off notes for next agent: После каждого инкремента обновлять этот блок.
- Last updated at (UTC): 2026-05-20T00:00:00Z

## Цель

- не дублировать SEO panel logic в `pages`, `product`, `blog` и будущих content-модулях;
- не превращать `rustok-seo-admin` в universal entity editor;
- держать reusable UI/tooling слой отдельно от SEO runtime и от owner-module screen ownership.

## Выполнено

- [x] создан support crate с root README и local docs;
- [x] вынесены shared GraphQL helper-ы для `seoMeta`, `upsertSeoMeta`, `publishSeoRevision`;
- [x] реализован `SeoEntityPanel` для owner-side entity editors;
- [x] реализован `SeoCapabilityNotice` для модулей, где capability slot нужен раньше, чем runtime target support;
- [x] встроены owner-side SEO panels в `rustok-pages/admin`, `rustok-product/admin`, `rustok-blog/admin`, `rustok-forum/admin`;
- [x] убран package-local locale override: support crate читает host effective locale, canonicalizes его и не держит editable locale field в panel UI.

## Осталось

- [x] вынести typed snippet preview и diagnostics cards в отдельные reusable subcomponents;
- [x] добавить targeted tests для panel transport, scoring logic и host-locale wiring.
- [x] заменить raw `structured_data` textarea на typed schema input contract (`schema type` + JSON object payload) с сохранением текущего GraphQL write contract.

## Проверка

- `cargo check -p rustok-seo-admin-support`
- `cargo check -p rustok-pages-admin`
- `cargo check -p rustok-blog-admin`
- `cargo check -p rustok-product-admin`
- `cargo check -p rustok-forum-admin`


## Quality backlog

- [ ] Актуализировать покрытие тестами по ключевым сценариям модуля.
- [ ] Проверить полноту и актуальность `README.md` и локальных docs.
- [ ] Зафиксировать/обновить verification gates для текущего состояния модуля.
