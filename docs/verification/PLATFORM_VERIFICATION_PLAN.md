# RusToK — Главный план верификации платформы

- **Дата актуализации структуры:** 2026-03-24
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

- [План foundation-верификации](./platform-foundation-verification-plan.md) — фазы 0-5: сборка, архитектура, ядро, auth, RBAC, tenancy.
- [План верификации событий, доменов и интеграций](./platform-domain-events-integrations-verification-plan.md) — фазы 6, 7, 13.
- [План верификации API-поверхностей](./platform-api-surfaces-verification-plan.md) — фазы 8-9.
- [План верификации frontend-поверхностей](./platform-frontend-surfaces-verification-plan.md) — фазы 10-12.
- [План верификации качества и эксплуатационной готовности](./platform-quality-operations-verification-plan.md) — фазы 14-20.
- [Реестр проблем платформенной верификации](./platform-verification-issues-registry.md) — выделенный реестр проблем и follow-up notes по исправлениям.

### Rolling / специализированные companion-планы

- [План rolling-верификации RBAC для server и runtime-модулей](./rbac-server-modules-verification-plan.md) — периодический прицельный проход по RBAC-контрактам.
- [План верификации Leptos-библиотек](./leptos-libraries-verification-plan.md) — rolling-план для библиотечного UI-контура.
- [План rolling-верификации целостности ядра платформы](./platform-core-integrity-verification-plan.md) — верификация server + обе admin-панели + core crates как самодостаточного целого, включая i18n и UI core модулей.

---

## Правила периодического прогона

- Этот master-план хранит только чистый чеклист текущего/следующего запуска.
- Исторические `[x]`, `[!]` и детальные описания исправлений не накапливаются здесь.
- Подробности проверок ведутся в специализированных планах, а история проблем — в отдельном реестре.
- Если в ходе нового прогона найдена новая проблема, её нужно отразить в профильном детальном плане и при необходимости добавить в [реестр проблем](./platform-verification-issues-registry.md).
- После изменения архитектуры, API, модулей, UI-контрактов, observability или процесса верификации нужно синхронизировать [docs/index.md](../index.md) и [README каталога verification](./README.md).

## Порядок прохождения

1. Сначала пройти foundation-блок.
2. Затем проверить события, доменные модули и интеграции.
3. После этого проверить API и frontend surfaces.
4. Завершить прогон quality/operations/release-readiness блоком.
5. Отдельно сверить targeted rolling-планы по RBAC и Leptos libraries, если задеты соответствующие контуры.

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
- [ ] При изменениях server/runtime modules дополнительно пройти [План rolling-верификации RBAC для server и runtime-модулей](./rbac-server-modules-verification-plan.md).

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
- [ ] Для rolling-проверки Leptos library contracts использовать [План верификации Leptos-библиотек](./leptos-libraries-verification-plan.md).

### Фаза 13. Интеграционные связи

- [ ] Пройти E2E integration checks из [Плана верификации событий, доменов и интеграций](./platform-domain-events-integrations-verification-plan.md).

### Фаза 14. Тестовое покрытие

- [ ] Пройти test coverage checks из [Плана верификации качества и эксплуатационной готовности](./platform-quality-operations-verification-plan.md).

### Фаза 15. Observability и операционная готовность

- [ ] Пройти observability/ops checks из [Плана верификации качества и эксплуатационной готовности](./platform-quality-operations-verification-plan.md).

### Фаза 16. Синхронизация документации с кодом

- [ ] Пройти документационный блок из [Плана верификации качества и эксплуатационной готовности](./platform-quality-operations-verification-plan.md).

### Фаза 17. CI/CD и DevOps

- [ ] Пройти CI/CD checks из [Плана верификации качества и эксплуатационной готовности](./platform-quality-operations-verification-plan.md).

### Фаза 18. Безопасность

- [ ] Пройти security checks из [Плана верификации качества и эксплуатационной готовности](./platform-quality-operations-verification-plan.md).

### Фаза 19. Антипаттерны и качество кода

- [ ] Пройти quality/antipattern checks из [Плана верификации качества и эксплуатационной готовности](./platform-quality-operations-verification-plan.md).

### Фаза 20. Правильность написания кода

- [ ] Пройти code correctness checks из [Плана верификации качества и эксплуатационной готовности](./platform-quality-operations-verification-plan.md).

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
| Targeted RBAC rolling plan | ⬜ | |
| Targeted Leptos libraries rolling plan | ⬜ | |
| **ИТОГО** | ⬜ | |

---

## Связанные документы

- [Реестр проблем платформенной верификации](./platform-verification-issues-registry.md)
- [README каталога verification](./README.md)
- [Карта документации](../index.md)
- [Verification scripts README](../../scripts/verify/README.md)
- [Паттерны vs Антипаттерны](../standards/patterns-vs-antipatterns.md)
- [Запрещённые действия](../standards/forbidden-actions.md)
- [Known Pitfalls](../ai/KNOWN_PITFALLS.md)
