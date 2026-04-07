# Исследование: единый стандарт модулей RusTok

> Источник: пользовательский `deep-research-modules.md`, импортирован в репозиторий и нормализован в UTF-8 2026-04-07.

## Назначение

Этот документ фиксирует выводы исследования, на основании которого выравнивается модульная платформа RusTok. Нормативные требования и исполнимый roadmap живут в [`docs/modules/module-system-plan.md`](../modules/module-system-plan.md) и [`docs/modules/manifest.md`](../modules/manifest.md); этот файл остаётся аналитической базой.

## Ключевые выводы

- В платформе уже есть основа единого стандарта: `modules.toml` как build-time manifest, `rustok-module.toml` как модульный контракт, manifest-driven wiring для `apps/server`, `apps/admin` и `apps/storefront`.
- Главный источник дрейфа не в отсутствии механики, а в неполном применении стандарта: часть path-модулей долго жила без `rustok-module.toml`, с неполным docs minimum или с несогласованными зависимостями между `modules.toml`, runtime-кодом и локальным manifest-слоем.
- Для no-UI и capability-модулей нужен не implicit fallback по отсутствию UI crate, а явная классификация в manifest-контракте. Для UI-модулей требуется такая же явная фиксация wiring через `[provides.*_ui]`.
- Единый стандарт должен охватывать не только код и wiring, но и документацию, quality gates, i18n, verification path и правила публикации.
- Windows-окружение должно быть first-class локальным путём аудита: обязательные проверки не могут зависеть от наличия Git Bash, если их можно встроить в `cargo xtask`, Node или PowerShell.

## Целевой стандарт модуля

Для каждого path-модуля из `modules.toml` обязателен единый минимальный набор:

- `Cargo.toml`
- `rustok-module.toml`
- `README.md`
- `docs/README.md`
- `docs/implementation-plan.md`

Дополнительно, если модуль публикует UI surfaces:

- `admin/Cargo.toml` и запись в `[provides.admin_ui]`
- `storefront/Cargo.toml` и запись в `[provides.storefront_ui]`
- manifest-level i18n contract через `[provides.*_ui.i18n]`

## Инварианты стандарта

- `modules.toml.depends_on`, `RusToKModule::dependencies()` и `[dependencies]` в `rustok-module.toml` должны совпадать по составу.
- `module.slug`, `crate` и `entry_type` должны быть согласованы между `modules.toml`, `Cargo.toml`, `rustok-module.toml` и runtime registry.
- UI-проводка валидируется по фактическому наличию subcrate и manifest wiring, а не по договорённости в prose.
- Scoped docs minimum является частью acceptance contract, а не optional приложением к коду.
- Полный модульный аудит должен запускаться на Windows через `cargo xtask`, Node и PowerShell; Bash-only perimeter checks допустимы только как дополнительный контур.

## Подтверждённые разрывы, которые нужно было закрыть

На старте работ исследование зафиксировало типовые дефекты:

- отсутствие `rustok-module.toml` у части path-модулей;
- отсутствие `docs/README.md` и `docs/implementation-plan.md` у scoped modules;
- дрейф навигации в `docs/modules/_index.md`;
- частичный skip-path в `cargo xtask module validate`, скрывающий дефекты вместо fail-fast поведения;
- отсутствие repo-wide policy по кодировкам и line endings;
- зависимость части verification flow от Bash при локальной работе на Windows.

## Использование исследования

Это исследование используется как база для трёх нормативных направлений:

1. [`docs/modules/module-system-plan.md`](../modules/module-system-plan.md) фиксирует roadmap и волны remediation.
2. [`docs/modules/manifest.md`](../modules/manifest.md) описывает обязательный контракт модуля.
3. [`docs/verification/README.md`](../verification/README.md) описывает исполнимый verification path, включая Windows-hybrid сценарий.
