# Admin App (Leptos) — Implementation Plan

## Фокус

Довести `apps/admin` до устойчивого production-уровня как Rust/Leptos админку с сильными UI/API контрактами и наблюдаемостью клиентских сценариев.

## Улучшения

### Архитектурные долги

- Удалить остаточные compatibility paths после верификации сборки (`src/components/`, `src/api/`, `src/providers/`, `src/i18n.rs`, `src/modules/`, `src/app.rs`), чтобы live API совпадал с текущей FSD-структурой.
- Завершить консолидацию FSD-структуры с минимизацией cross-layer импортов.
- Устранить дубли бизнес-логики между widgets/features и shared-integration слоем.
- Сформировать единый набор UI primitives и policy повторного использования.
- Добавить недостающий агрегатный `widgets/user_table` вместо локальных таблиц/обвязки по страницам.

### API/UI контракты

- Зафиксировать контракт GraphQL-операций и типизацию ошибок в пользовательских формах.
- Синхронизировать UI-поведение с `apps/next-admin` (loading/error/empty states).
- Стандартизировать контракт локализации для всех новых экранов и системных сообщений.

### Observability

- Добавить клиентские метрики UX-потоков (critical actions, failures, latency).
- Пробрасывать correlation id в запросах для связки с backend traces.
- Описать frontend incident checklist для деградации API и auth flows.

### Security

- Ввести централизованную проверку permission guards на route и action уровне.
- Защитить клиентские формы от небезопасных payload-мутаций и XSS-вставок в rich fields.
- Расширить контроль за токенами с явным policy хранения/обновления сессии.

### Test coverage

- Увеличить покрытие unit/component тестов для shared UI и критичных форм.
- Добавить e2e smoke-сценарии для core admin workflows.
- Ввести contract checks для i18n ключей и API-type drift.
- Довести `cargo build -p rustok-admin` и `cargo-udeps --package rustok-admin` до зелёного baseline без `cargo-udeps.ignore` для устаревших UI/FSD хвостов.

## Паритет стеков (Leptos/Next.js)

- Любая feature для админки/витрины планируется, декомпозируется и трекается сразу для обеих реализаций (Leptos и Next.js) в одном цикле поставки.

### Phase 1 update (2026-05-23): capability-first parity для page builder

`rustok-pages-admin` в `apps/admin` теперь имеет минимальные page-builder surfaces поверх существующего backend-контракта `grapesjs_v1`:

- `preview` — contract-safe preview документа из `body.contentJson`;
- `tree` — дерево projectData + snapshot legacy blocks;
- `properties` — host-owned metadata (`locale`, `channels`, `template`, `body format`);
- `publish` — публикация через тот же backend flow, что и таблица страниц.

### Must-have parity (обязательно между `apps/next-admin` и `apps/admin`)

- Канонический write-path: `body.format = grapesjs_v1`, payload в `body.contentJson`.
- Совместимость с legacy pages: запись `body` не удаляет `blocks` автоматически.
- Единая capability-модель `preview/tree/properties/publish`.
- Единый паттерн ошибок write-path для rich/page-builder форм: `validation/sanitize/runtime`.

### Host-specific UX (допускается)

- Разный layout и визуальные компоненты (React/Leptos/Flutter host-specific).
- Разный уровень «визуальности» preview/tree при сохранении одинакового capability-контракта.
- Разная навигационная упаковка экрана, если backend payload и RBAC semantics не меняются.

### Checklist готовности фичи

- [x] Реализовано в Leptos-варианте.
- [x] Реализовано в Next.js-варианте.
- [x] Контракты API/UI совпадают на capability-уровне.
- [x] Навигация и RBAC-поведение эквивалентны для `pages` write/publish surfaces.

## FSD/UI follow-up backlog

- Закрыть все cross-layer импорты, которые нарушают правило `pages -> widgets -> features -> entities -> shared`.
- Удалить compatibility aliases и старые import paths после подтверждённой сборки и smoke-верификации.
- Выровнять shared UI/state contracts с `apps/next-admin` для loading, empty, error и permission-gated сценариев.
- Зафиксировать repeatable verification-runbook для FSD-слоёв и UI-контрактов вместе с RBAC/navigation checks.

### Текущий статус rich-text (blog/forum/pages)

- **Админка (Leptos, `apps/admin`)**: [~] Частично реализовано (module-owned `pages` получил capability surfaces `preview/tree/properties/publish`, выровнен UX ошибок `validation/sanitize/runtime` для rich/page builder write-path).
- **Админка (Next.js, `apps/next-admin`)**: [~] Частично реализовано (blog/forum используют реальный Tiptap, pages переведены на GrapesJS + `grapesjs_v1`, требуется поддерживать parity discipline с Leptos и storefront rendering slice).
- **Витрина (Leptos SSR, `apps/storefront`)**: [ ] Не начато (rich-text rendering parity для blog/forum/pages запланирован).
- **Витрина (Next.js, `apps/next-frontend`)**: [ ] Не начато (rich-text rendering parity для blog/forum/pages запланирован).
