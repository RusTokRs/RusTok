# План реализации `rustok-test-utils`

## Текущий статус

- Статус: **active baseline**.
- Кратко: crate уже покрывает базовые сценарии (db setup, mock event bus, fixtures, helpers),
  но отсутствует формализация coverage-матрицы по модулям и единые quality-gates для тестового инструментария.

## Gap-анализ

### Что уже сделано

- Есть рабочие модули `db`, `events`, `fixtures`, `helpers`.
- Есть публичные re-export entry points для быстрого подключения в тестах.
- crate используется как общий слой повторного использования в тестах платформы.

### Что отсутствует

- Формальная карта соответствия: какие модульные тестовые сценарии покрываются какими утилитами.
- Набор golden/contract tests для самих test utilities (особенно для mock event behavior).
- Стандартизованные примеры для multi-tenant/RBAC/integration edge-cases.
- Политика versioning для тестовых API (изменения builders/fixtures без лишних поломок).

## Этапы работ

### Этап 1 — Инвентаризация и стандартизация

- Зафиксировать каталог утилит по типам тестов (unit/integration/contract).
- Уточнить рекомендованные entry points и anti-patterns использования.
- Синхронизировать документацию с центральным testing-гайдом.

### Этап 2 — Расширение покрытия

- Добавить недостающие fixtures для key-domain сценариев.
- Укрепить mock event utilities проверками порядка/идемпотентности публикаций.
- Ввести готовые helper-паттерны для tenancy/RBAC тестовых контекстов.

### Этап 3 — Устойчивость и release-gates

- Добавить self-tests для публичных test-utils API.
- Ввести quality gates: smoke-набор тестов для проверки критичных helpers.
- Зафиксировать deprecation policy для изменения test-fixtures API.

## Критерии готовности

- Есть документированная матрица «сценарий теста → рекомендованный helper/fixture».
- Публичные API `rustok-test-utils` покрыты собственными regression tests.
- Для tenancy/RBAC/event-потоков есть стандартизованные reusable fixtures.
- Изменения test-utils API сопровождаются migration notes для потребителей.

## Метрики верификации

- **Scenario coverage:** доля приоритетных тестовых сценариев, имеющих рекомендованный helper (целевое значение: ≥ 90%).
- **Utility stability:** процент зелёных self-tests `rustok-test-utils` в CI (целевое значение: 100%).
- **Adoption consistency:** доля новых тестов, использующих общие helpers вместо локального дублирования (целевое значение: рост MoM).
- **Migration safety:** число регрессий у потребителей после изменения test-utils API (целевое значение: 0 критичных регрессий).
