# Next Storefront App — Implementation Plan

## Фокус

Развивать `apps/next-frontend` как основную Next.js витрину с четкими API/UI контрактами, наблюдаемой производительностью и безопасной обработкой клиентских сценариев.

## Улучшения

### Архитектурные долги

- Довести модульную структуру `src/modules`/`src/shared` до строгих границ ответственности.
- Устранить дубли transport/auth логики между маршрутами за счет shared gateways.
- Оптимизировать стратегию SSR/ISR и cache invalidation для витринного контента.

### API/UI контракты

- Зафиксировать контракт storefront GraphQL запросов и ошибок для UI-компонентов.
- Выровнять UX-состояния с `apps/storefront` (loading, empty, partial, failure).
- Стандартизировать контракты i18n и URL-based locale routing.

### Observability

- Ввести web-vitals + бизнесовые метрики по ключевым воронкам storefront.
- Добавить распределенную трассировку frontend -> server запросов.
- Настроить алерты по росту frontend ошибок и просадкам Core Web Vitals.

### Security

- Усилить валидацию и sanitization query/input параметров storefront страниц.
- Зафиксировать политику безопасной работы с cookies/session и third-party scripts.
- Добавить защиту от abuse-traffic на публичные фильтры/поиск (rate/throttle hints).

### Test coverage

- Расширить e2e сценарии по каталогу, поиску, корзине и checkout pre-steps.
- Добавить contract-тесты i18n маршрутизации и API response mapping.
- Ввести визуальные/регрессионные проверки для ключевых пользовательских экранов.
