# RusToK — Главный план верификации платформы

- **Дата актуализации структуры:** 2026-04-08
- **Статус:** Готов к новому периодическому прогону
- **Режим:** Master-plan для повторяемых verification-сессий
- **Цель:** Запускать регулярную верификацию платформы по укрупнённым фазам без накопления исторического шума в одном документе

---

## Как теперь устроен набор verification-планов

Главный документ больше не хранит весь детальный чеклист и историю исправлений в одном файле.
Он используется как orchestration-слой для периодических запусков, а подробные проверки вынесены в специализированные документы внутри `docs/verification/`.

### Master / orchestration

- [Главный план верификации платформы](./PLATFORM_VERIFICATION_PLAN.md) — этот файл, reset-friendly master-checklist для нового прогона.

### Детальные платформенные планы

- [План foundation-верификации](./platform-foundation-verification-plan.md) — workspace baseline, module composition, foundation crates, auth/RBAC/tenant foundation.
- [План верификации событий, доменов и интеграций](./platform-domain-events-integrations-verification-plan.md) — event runtime, доменные модули, integration boundaries.
- [План верификации API-поверхностей](./platform-api-surfaces-verification-plan.md) — GraphQL, REST, `#[server]`, operational endpoints.
- [План верификации frontend-поверхностей](./platform-frontend-surfaces-verification-plan.md) — host apps, module-owned UI, shared libraries, i18n/routes.
- [План верификации качества и эксплуатационной готовности](./platform-quality-operations-verification-plan.md) — локальные проверки качества, observability, security/dependency hygiene, documentation sync и release-readiness.

### Специализированные companion-планы

- [План верификации RBAC, сервера и runtime-модулей](./rbac-server-modules-verification-plan.md) — прицельный проход по live authorization contract и capability boundaries.
- [План верификации Leptos-библиотек](./leptos-libraries-verification-plan.md) — companion-план для shared Leptos/UI library layer.
- [План верификации целостности ядра платформы](./platform-core-integrity-verification-plan.md) — server + admin surfaces + core crates как единый runtime baseline.

---

## Правила периодического прогона

- Этот master-план хранит только чистый чеклист текущего/следующего запуска.
- Исторические `[x]`, `[!]` и детальные описания исправлений не накапливаются здесь.
- Подробности проверок ведутся в специализированных планах, а история проблем — в отдельном реестре.
- Если в ходе нового прогона найдена новая проблема, её нужно отразить прямо в профильном детальном плане и закрыть в том же verification-cycle.
- После изменения архитектуры, API, модулей, UI-контрактов, observability или процесса верификации нужно синхронизировать [docs/index.md](../index.md) и [README каталога verification](./README.md).

## Порядок прохождения

1. Сначала пройти foundation-блок.
2. Затем проверить события, доменные модули и интеграции.
3. После этого проверить API и frontend surfaces.
4. Завершить прогон quality/operations/release-readiness блоком.
5. Отдельно сверить targeted companion-планы по RBAC и Leptos-библиотекам, если задеты соответствующие контуры.

---

## Master-checklist нового прогона

### Фаза 0. Компиляция и сборка

- [ ] Пройти build baseline из [Плана foundation-верификации](./platform-foundation-verification-plan.md).
- [ ] Зафиксировать блокеры окружения отдельно от продуктовых дефектов.

### Фаза 1. Соответствие архитектуре

- [ ] Сверить registry, taxonomy и dependency graph через [План foundation-верификации](./platform-foundation-verification-plan.md).

### Фаза 2. Ядро платформы

- [ ] Проверить `rustok-core`, `rustok-outbox`, `rustok-events`, `rustok-telemetry` по [Плану foundation-верификации](./platform-foundation-verification-plan.md).

### Фаза 3. Авторизация и аутентификация

- [ ] Пройти auth surface по [Плану foundation-верификации](./platform-foundation-verification-plan.md).

### Фаза 4. RBAC

- [ ] Выполнить platform-level RBAC checks из [Плана foundation-верификации](./platform-foundation-verification-plan.md).
- [ ] При изменениях server/runtime modules дополнительно пройти [План верификации RBAC, сервера и runtime-модулей](./rbac-server-modules-verification-plan.md).

### Фаза 5. Multi-Tenancy

- [ ] Пройти tenancy checks из [Плана foundation-верификации](./platform-foundation-verification-plan.md).

### Фаза 6. Событийная система

- [ ] Пройти event/outbox checks из [Плана верификации событий, доменов и интеграций](./platform-domain-events-integrations-verification-plan.md).

### Фаза 7. Доменные модули

- [ ] Пройти модульные проверки из [Плана верификации событий, доменов и интеграций](./platform-domain-events-integrations-verification-plan.md).

### Фаза 8. API GraphQL

- [ ] Пройти GraphQL contract checks из [Плана верификации API-поверхностей](./platform-api-surfaces-verification-plan.md).

### Фаза 9. API REST

- [ ] Пройти REST contract checks из [Плана верификации API-поверхностей](./platform-api-surfaces-verification-plan.md).

### Фаза 10. Фронтенды Leptos

- [ ] Пройти Leptos app checks из [Плана верификации frontend-поверхностей](./platform-frontend-surfaces-verification-plan.md).

### Фаза 11. Фронтенды Next.js

- [ ] Пройти Next.js app checks из [Плана верификации frontend-поверхностей](./platform-frontend-surfaces-verification-plan.md).

### Фаза 12. Фронтенд-библиотеки

- [ ] Пройти platform-level library/package checks из [Плана верификации frontend-поверхностей](./platform-frontend-surfaces-verification-plan.md).
- [ ] Для targeted-проверки library contracts использовать [План верификации Leptos-библиотек](./leptos-libraries-verification-plan.md).

### Фаза 13. Интеграционные связи

- [ ] Пройти E2E integration checks из [Плана верификации событий, доменов и интеграций](./platform-domain-events-integrations-verification-plan.md).

### Фаза 14. Локальный quality baseline

- [ ] Пройти локальные проверки качества из [Плана верификации качества и эксплуатационной готовности](./platform-quality-operations-verification-plan.md).

### Фаза 15. Observability и operational readiness

- [ ] Пройти observability/ops checks из [Плана верификации качества и эксплуатационной готовности](./platform-quality-operations-verification-plan.md).

### Фаза 16. Documentation sync и release-readiness

- [ ] Пройти documentation sync и release-readiness checks из [Плана верификации качества и эксплуатационной готовности](./platform-quality-operations-verification-plan.md).

### Фаза 17. Security и dependency hygiene

- [ ] Пройти security/dependency hygiene checks из [Плана верификации качества и эксплуатационной готовности](./platform-quality-operations-verification-plan.md).

### Фаза 18. Quality anti-patterns и correctness

- [ ] Пройти остаточные quality/correctness checks из [Плана верификации качества и эксплуатационной готовности](./platform-quality-operations-verification-plan.md).

---

## Итоговый отчёт прогона

Заполняется по завершении текущего цикла верификации:

| Блок | Статус | Комментарий |
|------|--------|-------------|
| Foundation | ⬜ | |
| Events / Domains / Integrations | ⬜ | |
| API Surfaces | ⬜ | |
| Frontend Surfaces | ⬜ | |
| Quality / Operations / Release Readiness | ⬜ | |
| Targeted RBAC/server companion plan | ⬜ | |
| Targeted Leptos libraries companion plan | ⬜ | |
| **ИТОГО** | ⬜ | |

---

## Связанные документы

- [README каталога verification](./README.md)
- [Карта документации](../index.md)
- [Verification scripts README](../../scripts/verify/README.md)
- [Паттерны vs Антипаттерны](../standards/patterns-vs-antipatterns.md)
- [Запрещённые действия](../standards/forbidden-actions.md)
- [Known Pitfalls](../ai/KNOWN_PITFALLS.md)
