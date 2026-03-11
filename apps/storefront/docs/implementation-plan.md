# Storefront App (Leptos SSR) — Implementation Plan

## Фокус

Развивать `apps/storefront` как стабильную SSR-витрину с предсказуемой производительностью, безопасной обработкой пользовательского ввода и едиными контрактами с backend.

## Улучшения

### Архитектурные долги

- Формализовать границы между SSR orchestration, shared integrations и feature-модулями.
- Снизить дублирование UI/business сценариев с `apps/next-frontend` через общие контракты.
- Оптимизировать стратегию data fetching и кеширования для SSR страниц.

### API/UI контракты

- Зафиксировать витринные API-контракты (каталог, контентные блоки, фильтры, пагинация).
- Стандартизировать UI-состояния для ошибок/пустых данных/частичных ответов.
- Синхронизировать i18n и маршрутизацию локалей с backend ожиданиями.

### Observability

- Добавить web-vitals и SSR latency метрики для ключевых страниц.
- Ввести трассировку запросов storefront -> server по correlation id.
- Определить алерты на рост TTFB/ошибок рендеринга.

### Security

- Усилить sanitization пользовательского/контентного HTML перед SSR.
- Добавить защиту от злоупотребления публичными фильтрами и поисковыми параметрами.
- Зафиксировать policy для cookie/session взаимодействия с backend auth.

### Test coverage

- Добавить integration/e2e сценарии для каталога, карточки товара и поиска.
- Расширить тесты SSR hydration consistency и i18n fallback.
- Ввести регрессионные тесты для critical storefront маршрутов.

## Паритет стеков (Leptos/Next.js)

- Любая feature для админки/витрины планируется, декомпозируется и трекается сразу для обеих реализаций (Leptos и Next.js) в одном цикле поставки.

### Checklist готовности фичи

- [ ] Реализовано в Leptos-варианте.
- [ ] Реализовано в Next.js-варианте.
- [ ] Контракты API/UI совпадают.
- [ ] Навигация и RBAC-поведение эквивалентны.

### Текущий статус rich-text (blog/forum/pages)

- **Админка (Leptos, `apps/admin`)**: [ ] Не начато / в процессе синхронизации с Next.js-реализацией.
- **Админка (Next.js, `apps/next-admin`)**: [~] Частично реализовано (подключены Tiptap/Page Builder маршруты, требуется завершить работу с реальными entity ID и parity-check с Leptos).
- **Витрина (Leptos SSR, `apps/storefront`)**: [ ] Не начато (rich-text rendering parity для blog/forum/pages запланирован).
- **Витрина (Next.js, `apps/next-frontend`)**: [ ] Не начато (rich-text rendering parity для blog/forum/pages запланирован).

